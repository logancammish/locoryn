//! Optional local OCR: render one page at a time and discard temporary data.
use super::{MAX_TEXT_BYTES, check_cancelled, new_id};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};

pub(super) struct OcrSession {
    directory: PathBuf,
}
impl OcrSession {
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let directory = std::env::temp_dir().join(format!("locoryn-ocr-{}", new_id()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .map_err(|error| format!("Cannot prepare PDF OCR: {error}"))?;
        let session = Self { directory };
        fs::write(session.directory.join("input.pdf"), bytes).map_err(|error| error.to_string())?;
        Ok(session)
    }

    pub fn read_page(
        &self,
        page: usize,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<String, String> {
        let deadline = deadline.min(Instant::now() + Duration::from_secs(30));
        let mut render = Command::new("pdftoppm");
        render.current_dir(&self.directory).args([
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
            "-scale-to",
            "2400",
            "-gray",
            "-png",
            "input.pdf",
            "page",
        ]);
        run(render, "Poppler (pdftoppm)", cancel, deadline)?;
        let mut recognize = Command::new("tesseract");
        recognize
            .current_dir(&self.directory)
            .args(["page.png", "text", "--psm", "3"])
            .env("OMP_THREAD_LIMIT", "2");
        let result = run(recognize, "Tesseract", cancel, deadline).and_then(|()| {
            let mut text = String::new();
            fs::File::open(self.directory.join("text.txt"))
                .and_then(|file| {
                    file.take(MAX_TEXT_BYTES as u64 + 1)
                        .read_to_string(&mut text)
                })
                .map_err(|error| format!("Cannot read OCR output: {error}"))?;
            if text.len() > MAX_TEXT_BYTES {
                return Err("OCR text exceeds the 2 MiB limit.".into());
            }
            Ok(text)
        });
        let _ = fs::remove_file(self.directory.join("page.png"));
        let _ = fs::remove_file(self.directory.join("text.txt"));
        result
    }
}

impl Drop for OcrSession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn run(
    mut command: Command,
    name: &str,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if Instant::now() >= deadline {
        return Err("PDF OCR time limit reached. Attach a smaller page range.".into());
    }
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|error| format!("OCR requires Poppler (pdftoppm) and Tesseract on PATH. {name} could not start: {error}"))?;
    let result = loop {
        if let Err(error) = check_cancelled(cancel) {
            break Err(error);
        }
        if Instant::now() >= deadline {
            break Err("PDF OCR time limit reached. Attach a smaller page range.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(_)) => {
                break Err(format!(
                    "{name} could not read this page. Check the PDF and installed OCR language data."
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => break Err(format!("{name} failed: {error}")),
        }
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_tools_cancellation_and_expired_deadlines_are_actionable() {
        let missing = || Command::new("locoryn-nonexistent-ocr-test-command");
        let cancel = AtomicBool::new(false);
        assert!(
            run(
                missing(),
                "Tesseract",
                &cancel,
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap_err()
            .contains("on PATH")
        );
        assert!(
            run(missing(), "Tesseract", &cancel, Instant::now())
                .unwrap_err()
                .contains("time limit")
        );
        assert!(
            run(
                missing(),
                "Tesseract",
                &AtomicBool::new(true),
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap_err()
            .contains("cancelled")
        );
    }
    #[test]
    fn temporary_pdf_snapshot_is_removed_on_drop() {
        let session = OcrSession::new(b"test data").unwrap();
        let directory = session.directory.clone();
        assert!(directory.join("input.pdf").exists());
        drop(session);
        assert!(!directory.exists());
    }
}
