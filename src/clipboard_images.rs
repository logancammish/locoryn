use crate::{
    ChatImage, MAX_MARKDOWN_IMAGE_BYTES, MAX_MARKDOWN_IMAGE_PIXELS, file_usage, load_chat_image,
};

const PREVIEW_MAX_EDGE: u32 = 512;

pub async fn read() -> Result<Vec<ChatImage>, String> {
    // Clipboard transfers and PNG encoding block; keep them off the async workers.
    tokio::task::spawn_blocking(read_images)
        .await
        .map_err(|error| format!("Could not read clipboard: {error}"))?
}

fn read_images() -> Result<Vec<ChatImage>, String> {
    // Linux clipboard images already arrive as PNG. Preserve that payload instead
    // of arboard's PNG -> RGBA -> PNG round trip (then another decode for preview).
    #[cfg(target_os = "linux")]
    if let Some(result) = read_wayland_png() {
        return result.map(|image| vec![image]);
    }
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("Could not open clipboard: {error}"))?;
    let image_error = match clipboard.get_image() {
        Ok(image) => return image_from_rgba(image).map(|image| vec![image]),
        Err(error) => error,
    };
    // File managers copy image files as a URI list, rather than raw pixels.
    if let Ok(paths) = clipboard.get().file_list()
        && let Some(images) = images_from_paths(paths)?
    {
        return Ok(images);
    }
    Err(match image_error {
        arboard::Error::ContentNotAvailable => "The clipboard does not contain an image.".into(),
        error => format!("Could not read clipboard image: {error}"),
    })
}

#[cfg(target_os = "linux")]
fn read_wayland_png() -> Option<Result<ChatImage, String>> {
    use std::io::Read;
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

    std::env::var_os("WAYLAND_DISPLAY")?;
    // Retain arboard's X11 fallback on compositors without data-control, and its
    // file-list handling when the offer contains copied files rather than pixels.
    let (pipe, _) = get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        MimeType::Specific("image/png"),
    )
    .ok()?;
    Some((|| {
        let mut bytes = Vec::new();
        pipe.take(MAX_MARKDOWN_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read clipboard image: {error}"))?;
        image_from_png(bytes)
    })())
}

#[cfg(any(target_os = "linux", test))]
fn image_from_png(bytes: Vec<u8>) -> Result<ChatImage, String> {
    check_encoded_size(&bytes)?;
    crate::validate_image_dimensions(&bytes)?;
    let rgba = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|error| format!("Could not decode clipboard image: {error}"))?
        .into_rgba8();
    Ok(ChatImage {
        name: "Pasted image.png".into(),
        mime_type: "image/png".into(),
        bytes,
        preview_handle: preview_from_rgba(rgba),
    })
}

fn check_encoded_size(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_MARKDOWN_IMAGE_BYTES {
        Err("Image is larger than the 12 MB display limit.".into())
    } else {
        Ok(())
    }
}

fn preview_from_rgba(rgba: image::RgbaImage) -> iced::widget::image::Handle {
    let rgba = if rgba.width() > PREVIEW_MAX_EDGE || rgba.height() > PREVIEW_MAX_EDGE {
        let longest = u64::from(rgba.width().max(rgba.height()));
        let width = (u64::from(rgba.width()) * u64::from(PREVIEW_MAX_EDGE) / longest).max(1) as u32;
        let height =
            (u64::from(rgba.height()) * u64::from(PREVIEW_MAX_EDGE) / longest).max(1) as u32;
        // Sample only the small preview's pixels. The generic image resizer
        // allocates a source-width floating-point intermediate even for Nearest.
        image::RgbaImage::from_fn(width, height, |x, y| {
            let source_x =
                ((2 * u64::from(x) + 1) * u64::from(rgba.width()) / (2 * u64::from(width))) as u32;
            let source_y = ((2 * u64::from(y) + 1) * u64::from(rgba.height())
                / (2 * u64::from(height))) as u32;
            *rgba.get_pixel(source_x, source_y)
        })
    } else {
        rgba
    };
    iced::widget::image::Handle::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
}

fn images_from_paths(paths: Vec<std::path::PathBuf>) -> Result<Option<Vec<ChatImage>>, String> {
    let paths: Vec<_> = paths
        .into_iter()
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "gif"
                    )
                })
        })
        .collect();
    if paths.len() > file_usage::MAX_ATTACHMENTS {
        return Err("Attach at most 8 files per message.".into());
    }
    if paths.is_empty() {
        return Ok(None);
    }
    paths
        .iter()
        .map(|path| load_chat_image(path))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn image_from_rgba(image: arboard::ImageData<'_>) -> Result<ChatImage, String> {
    let width = u32::try_from(image.width).map_err(|_| "Clipboard image is too wide.")?;
    let height = u32::try_from(image.height).map_err(|_| "Clipboard image is too tall.")?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_MARKDOWN_IMAGE_PIXELS
    {
        return Err("Clipboard image dimensions are invalid or too large.".into());
    }
    let rgba = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
        .ok_or_else(|| "Clipboard image data was invalid.".to_string())?;
    let mut bytes = Vec::new();
    // Platforms exposing raw pixels still need one encode, but the preview can
    // reuse those pixels directly instead of decoding the PNG we just produced.
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(&mut bytes),
        rgba.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|error| format!("Could not prepare clipboard image: {error}"))?;
    check_encoded_size(&bytes)?;
    let preview_handle = preview_from_rgba(rgba);
    Ok(ChatImage {
        name: "Pasted image.png".into(),
        mime_type: "image/png".into(),
        bytes,
        preview_handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screenshot(width: u32, height: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                (x / 8 % 256) as u8,
                (y / 8 % 256) as u8,
                ((x ^ y) % 256) as u8,
                255,
            ])
        })
    }

    fn png_bytes(rgba: &image::RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut bytes),
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn encoded_clipboard_image_keeps_original_bytes_with_a_small_preview() {
        let bytes = png_bytes(&screenshot(1600, 900));
        let image = image_from_png(bytes.clone()).unwrap();
        assert_eq!(image.bytes, bytes);
        let iced::widget::image::Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = image.preview_handle
        else {
            panic!("preview must be decoded before reaching the renderer");
        };
        assert_eq!((width, height), (512, 288));
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        assert_eq!(image::load_from_memory(&image.bytes).unwrap().width(), 1600);
    }

    #[test]
    fn encoded_clipboard_image_rejects_invalid_and_oversized_payloads() {
        assert!(image_from_png(b"not an image".to_vec()).is_err());
        assert!(image_from_png(vec![0; MAX_MARKDOWN_IMAGE_BYTES + 1]).is_err());
    }

    #[test]
    fn previews_handle_portrait_thin_and_small_images_without_upscaling() {
        for (width, height, expected) in [
            (900, 1600, (288, 512)),
            (1, 1024, (1, 512)),
            (1024, 1, (512, 1)),
            (2, 1, (2, 1)),
        ] {
            let rgba = image::RgbaImage::from_pixel(width, height, image::Rgba([12, 34, 56, 78]));
            let iced::widget::image::Handle::Rgba {
                width,
                height,
                pixels,
                ..
            } = preview_from_rgba(rgba)
            else {
                panic!("preview must be decoded");
            };
            assert_eq!((width, height), expected);
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|pixel| pixel == [12, 34, 56, 78])
            );
        }
    }

    #[test]
    #[ignore = "manual 4K paste performance comparison; run with --ignored --nocapture"]
    fn benchmark_clipboard_preparation() {
        use std::time::Instant;
        let bytes = png_bytes(&screenshot(3840, 2160));
        let start = Instant::now();
        // Previous Linux path: arboard decodes, Locoryn encodes again, and the
        // preview decodes that result and uploads every pixel to the renderer.
        let rgba = image::load_from_memory(&bytes).unwrap().into_rgba8();
        let reencoded = png_bytes(&rgba);
        let old_preview = crate::decoded_image_handle(&reencoded).unwrap();
        std::hint::black_box(old_preview);
        let previous = start.elapsed();

        let start = Instant::now();
        let pasted = image_from_png(bytes.clone()).unwrap();
        let optimized = start.elapsed();
        assert_eq!(pasted.bytes, bytes);
        eprintln!(
            "4K PNG preparation: previous={previous:?}, optimized={optimized:?}, speedup={:.2}x",
            previous.as_secs_f64() / optimized.as_secs_f64()
        );
    }

    #[test]
    fn copied_files_load_images_and_skip_non_images() {
        let paths = vec!["README.md".into(), "assets/icon.png".into()];
        let images = images_from_paths(paths).unwrap().unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].name, "icon.png");
        assert_eq!(images[0].bytes, std::fs::read("assets/icon.png").unwrap());
        assert!(
            images_from_paths(vec!["README.md".into()])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn copied_files_report_unreadable_images_and_attachment_limit() {
        assert!(images_from_paths(vec!["missing/image.PNG".into()]).is_err());
        assert!(
            images_from_paths(vec!["assets/icon.png".into(); 9])
                .unwrap_err()
                .contains("at most 8")
        );
    }

    #[test]
    fn clipboard_pixels_preserve_color_and_alpha_in_png() {
        let pixels = vec![255, 0, 0, 255, 0, 128, 255, 64];
        let image = image_from_rgba(arboard::ImageData {
            width: 2,
            height: 1,
            bytes: pixels.clone().into(),
        })
        .unwrap();
        assert_eq!(image.mime_type, "image/png");
        let decoded = image::load_from_memory(&image.bytes).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.into_raw(), pixels);
    }

    #[test]
    fn clipboard_rejects_invalid_or_oversized_pixels_before_encoding() {
        for (width, height, bytes) in [
            (0, 1, vec![]),
            (2, 2, vec![0; 3]),
            (usize::MAX, 1, vec![]),
            (8_000, 8_000, vec![]),
        ] {
            assert!(
                image_from_rgba(arboard::ImageData {
                    width,
                    height,
                    bytes: bytes.into(),
                })
                .is_err()
            );
        }
    }
}
