use super::{FilePage, MAX_PDF_PAGES, MAX_TEXT_BYTES, check_cancelled, ocr::OcrSession};
use std::{
    fmt::Write,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

// Bound extracted output even when a small PDF repeats a very large text stream.
struct LimitedText {
    text: String,
    limit: usize,
    limit_hit: bool,
}
impl pdf_extract::ConvertToFmt for &mut LimitedText {
    type Writer = Self;
    fn convert(self) -> Self {
        self
    }
}
impl Write for LimitedText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.text.len().saturating_add(value.len()) > self.limit {
            self.limit_hit = true;
            return Err(std::fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

pub(super) fn extract(bytes: &[u8], cancel: &AtomicBool) -> Result<Vec<FilePage>, String> {
    // Malformed font tables in third-party PDFs may panic in the extractor.
    std::panic::catch_unwind(|| extract_inner(bytes, cancel))
        .map_err(|_| "The PDF contains invalid data and could not be read.".to_string())?
}

fn extract_inner(bytes: &[u8], cancel: &AtomicBool) -> Result<Vec<FilePage>, String> {
    check_cancelled(cancel)?;
    let mut document = pdf_extract::Document::load_mem(bytes)
        .map_err(|error| format!("Cannot open PDF: {error}"))?;
    if document.is_encrypted() && document.decrypt("").is_err() {
        return Err("This PDF is password protected. Attach an unlocked copy.".into());
    }
    let page_count = document.get_pages().len();
    if page_count == 0 {
        return Err("The PDF contains no pages.".into());
    }
    if page_count > MAX_PDF_PAGES {
        return Err(format!(
            "The PDF contains {page_count} pages; attach a section of at most {MAX_PDF_PAGES} pages."
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut ocr: Option<Result<OcrSession, String>> = None;
    let mut pages = Vec::with_capacity(page_count);
    let mut remaining = MAX_TEXT_BYTES;
    for number in 1..=page_count {
        check_cancelled(cancel)?;
        let mut output = LimitedText {
            text: String::new(),
            limit: remaining,
            limit_hit: false,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::output_doc_page(
                &document,
                &mut pdf_extract::PlainTextOutput::new(&mut output),
                number as u32,
            )
        }));
        let extracted = matches!(result, Ok(Ok(())));
        if output.limit_hit {
            return Err("PDF text exceeds the 2 MiB limit. Attach a smaller section.".into());
        }
        let mut page = FilePage {
            text: output.text,
            ocr: false,
            notice: None,
        };
        // Empty pages and extraction failures get an OCR attempt. Text PDFs
        // never need Poppler or Tesseract, and mixed PDFs keep page numbering.
        if !extracted || page.text.trim().is_empty() {
            let session = ocr.get_or_insert_with(|| OcrSession::new(bytes));
            match session.as_ref().map_err(Clone::clone)
                .and_then(|session| session.read_page(number, cancel, deadline)) {
                Ok(text) if !text.trim().is_empty() => {
                    page.text = text;
                    page.ocr = true;
                }
                Ok(_) => page.notice = Some("No readable text was found on this page (it may be blank or contain only graphics).".into()),
                Err(error) => page.notice = Some(error),
            }
            if !extracted {
                // A failed extractor can leave partial output: never imply it is complete.
                if !page.ocr {
                    page.text.clear();
                }
                if page.notice.is_none() && !page.ocr {
                    page.notice = Some("Text extraction failed on this page.".into());
                }
            }
        }
        if page.text.len() > remaining {
            return Err("PDF text exceeds the 2 MiB limit. Attach a smaller section.".into());
        }
        remaining -= page.text.len();
        pages.push(page);
    }
    check_cancelled(cancel)?;
    if pages.iter().all(|page| page.text.trim().is_empty()) {
        return Err(format!(
            "No readable text was found in this PDF. {}",
            pages
                .iter()
                .find_map(|p| p.notice.as_deref())
                .unwrap_or("Try a searchable PDF.")
        ));
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_extract::{
        Document, Object, Stream,
        content::{Content, Operation},
        dictionary,
    };

    fn text_pdf(texts: &[&str]) -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let font = document.add_object(
            dictionary! { "Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica" },
        );
        let resources = document.add_object(dictionary! { "Font"=>dictionary! { "F1"=>font } });
        let kids: Vec<Object> = texts
            .iter()
            .map(|text| {
                let content = Content {
                    operations: vec![
                        Operation::new("BT", vec![]),
                        Operation::new("Tf", vec!["F1".into(), 18.into()]),
                        Operation::new("Td", vec![40.into(), 700.into()]),
                        Operation::new("Tj", vec![Object::string_literal(*text)]),
                        Operation::new("ET", vec![]),
                    ],
                };
                let stream =
                    document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
                document
                    .add_object(
                        dictionary! { "Type"=>"Page", "Parent"=>pages_id, "Contents"=>stream },
                    )
                    .into()
            })
            .collect();
        document.objects.insert(
            pages_id,
            dictionary! { "Type"=>"Pages", "Kids"=>kids, "Count"=>texts.len() as i64,
            "Resources"=>resources, "MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()] }
            .into(),
        );
        let catalog = document.add_object(dictionary! { "Type"=>"Catalog", "Pages"=>pages_id });
        document.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn searchable_pdf_preserves_page_order_and_text_without_ocr() {
        let bytes = text_pdf(&["First page", "Second page needle", "Third page"]);
        let pages = extract(&bytes, &AtomicBool::new(false)).unwrap();
        assert_eq!(pages.len(), 3);
        assert!(pages[0].text.contains("First page"));
        assert!(pages[1].text.contains("Second page needle"));
        assert!(pages[2].text.contains("Third page"));
        assert!(pages.iter().all(|page| !page.ocr && page.notice.is_none()));
    }

    #[test]
    fn rejects_invalid_oversized_and_cancelled_pdfs() {
        assert!(extract(b"%PDF-corrupt", &AtomicBool::new(false)).is_err());
        let too_many = text_pdf(&vec!["page"; MAX_PDF_PAGES + 1]);
        assert!(
            extract(&too_many, &AtomicBool::new(false))
                .unwrap_err()
                .contains("at most")
        );
        assert!(
            extract(&text_pdf(&["text"]), &AtomicBool::new(true))
                .unwrap_err()
                .contains("cancelled")
        );
    }

    #[test]
    #[ignore = "requires local Poppler (pdftoppm) and Tesseract with English language data"]
    fn scanned_pdf_uses_local_ocr() {
        let bytes = include_bytes!("fixtures/scanned.pdf");
        assert!(
            pdf_extract::extract_text_from_mem(bytes)
                .unwrap()
                .trim()
                .is_empty()
        );
        let pages = extract(bytes, &AtomicBool::new(false)).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].ocr);
        assert!(pages[0].text.contains("MAPLE 7392"), "{}", pages[0].text);
    }
}
