use super::MAX_TEXT_BYTES;

/// Preserve source code and Markdown exactly, normalizing only BOMs/newlines.
/// Unknown extensions are accepted when the contents are demonstrably text.
pub(super) fn decode(bytes: &[u8]) -> Result<String, String> {
    let text = if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        utf16(bytes, true)?
    } else if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        utf16(bytes, false)?
    } else {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        std::str::from_utf8(bytes).map_err(|_| "This file is not UTF-8 text. Save it as UTF-8 or UTF-16 with a BOM before attaching it.")?.to_owned()
    };
    if text
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t' | '\u{000c}'))
    {
        return Err("This is a binary or unsupported document. Attach a text/code file, a PDF, or export it as plain text.".into());
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(
            "Extracted text exceeds the 2 MiB limit. Split the file into smaller parts.".into(),
        );
    }
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("Invalid UTF-16 text: incomplete character.".into());
    }
    let words = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words).map_err(|_| "Invalid UTF-16 text.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_code_and_unicode_and_normalizes_bom_newlines() {
        assert_eq!(
            decode(b"\xef\xbb\xbf# Heading\r\n```rs\r\nfn main() {}\r\n```").unwrap(),
            "# Heading\n```rs\nfn main() {}\n```"
        );
        for little in [true, false] {
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for word in "hello 🦀\rnext".encode_utf16() {
                bytes.extend(if little {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            assert_eq!(decode(&bytes).unwrap(), "hello 🦀\nnext");
        }
    }
    #[test]
    fn rejects_binary_invalid_and_oversized_text() {
        assert!(decode(b"PK\x03\x04\x00zip").is_err());
        assert!(decode(&[0xff, 0xfe, 0x61]).is_err());
        assert!(decode(&vec![b'a'; MAX_TEXT_BYTES + 1]).is_err());
    }
}
