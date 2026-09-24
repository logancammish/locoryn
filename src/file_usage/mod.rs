//! Conversation-owned document snapshots. Model tools never open arbitrary paths.
pub mod context;
mod ocr;
mod pdf;
mod text;
pub mod tools;

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ATTACHMENTS: usize = 8;
pub const MAX_PDF_PAGES: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Text,
    Pdf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePage {
    pub text: String,
    #[serde(default)]
    pub ocr: bool,
    #[serde(default)]
    pub notice: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachedFile {
    pub id: String,
    pub name: String,
    pub kind: FileKind,
    pub size_bytes: usize,
    pub pages: Vec<FilePage>,
}

impl AttachedFile {
    pub fn summary(&self) -> String {
        match self.kind {
            FileKind::Text => format!("{} · text", self.name),
            FileKind::Pdf => format!("{} · {} pages", self.name, self.pages.len()),
        }
    }
    pub fn has_notices(&self) -> bool {
        self.pages.iter().any(|page| page.notice.is_some())
    }
}

pub fn new_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "file-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("File reading cancelled.".into())
    } else {
        Ok(())
    }
}

pub fn load(path: &Path, cancel: &AtomicBool) -> Result<Arc<AttachedFile>, String> {
    check_cancelled(cancel)?;
    let name = path
        .file_name()
        .ok_or("Select a file, not a folder.")?
        .to_string_lossy()
        .into_owned();
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("Cannot read {name}: {error}"))?;
    if !metadata.is_file() {
        return Err("Only regular files can be attached.".into());
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(format!("{name} exceeds the 16 MiB attachment limit."));
    }
    let file = File::open(path).map_err(|error| format!("Cannot read {name}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read {name}: {error}"))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("{name} exceeds the 16 MiB attachment limit."));
    }
    check_cancelled(cancel)?;
    let is_pdf = bytes.starts_with(b"%PDF-")
        || path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    let (kind, pages) = if is_pdf {
        (FileKind::Pdf, pdf::extract(&bytes, cancel)?)
    } else {
        (
            FileKind::Text,
            vec![FilePage {
                text: text::decode(&bytes)?,
                ocr: false,
                notice: None,
            }],
        )
    };
    Ok(Arc::new(AttachedFile {
        id: new_id(),
        name,
        kind,
        size_bytes: bytes.len(),
        pages,
    }))
}

/// Character offsets keep tool pagination valid for Unicode, including OCR text.
pub fn excerpt(text: &str, offset: usize, length: usize) -> (String, Option<usize>) {
    let mut chars = text.chars().skip(offset);
    let value: String = chars.by_ref().take(length).collect();
    let next = chars.next().map(|_| offset + value.chars().count());
    (value, next)
}

#[cfg(test)]
pub fn fixture(name: &str, pages: &[&str]) -> Arc<AttachedFile> {
    Arc::new(AttachedFile {
        id: name.into(),
        name: name.into(),
        kind: if name.ends_with(".pdf") {
            FileKind::Pdf
        } else {
            FileKind::Text
        },
        size_bytes: 0,
        pages: pages
            .iter()
            .map(|text| FilePage {
                text: (*text).into(),
                ocr: false,
                notice: None,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_snapshot_survives_source_deletion_and_serialization() {
        let directory = std::env::temp_dir().join(new_id());
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("Dockerfile");
        std::fs::write(&path, "FROM scratch\n# 🦀 source text\n").unwrap();
        let file = load(&path, &AtomicBool::new(false)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let restored: AttachedFile =
            serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
        assert_eq!(restored.name, "Dockerfile");
        assert_eq!(restored.pages[0].text, "FROM scratch\n# 🦀 source text\n");
        assert!(
            !serde_json::to_string(&file)
                .unwrap()
                .contains(directory.to_str().unwrap())
        );
        assert!(
            load(&directory, &AtomicBool::new(false))
                .unwrap_err()
                .contains("regular files")
        );
        let oversized = File::create(&path).unwrap();
        oversized.set_len(MAX_FILE_BYTES as u64 + 1).unwrap();
        assert!(
            load(&path, &AtomicBool::new(false))
                .unwrap_err()
                .contains("16 MiB")
        );
        drop(oversized);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
