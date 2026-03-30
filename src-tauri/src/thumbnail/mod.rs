use crate::types::*;

// Public entry: generate JPEG thumbnail bytes under given constraints.
// - max_px: bounding box size (e.g., 128)
// - max_bytes: soft cap (e.g., 16 KiB). We'll best-effort fit; if impossible, return Err.
pub async fn generate_thumbnail_bytes(
    src_path: &str,
    max_px: u32,
    max_bytes: usize,
) -> SpResult<Option<Vec<u8>>> {
    use std::path::Path;
    let p = Path::new(src_path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Image formats we handle natively in Rust
    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        match gen_image_jpeg_under(src_path, max_px, max_bytes).await {
            Ok(bytes) => return Ok(Some(bytes)),
            Err(e) => return Err(e),
        }
    }

    // Video formats: defer to platform-specific implementations (currently stubs)
    if matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "avi" | "webm") {
        #[cfg(target_os = "macos")]
        {
            return gen_video_thumb_macos(src_path, max_px, max_bytes)
                .await
                .map(Some);
        }
        #[cfg(target_os = "windows")]
        {
            return gen_video_thumb_windows(src_path, max_px, max_bytes)
                .await
                .map(Some);
        }
        #[cfg(target_os = "linux")]
        {
            return gen_video_thumb_linux(src_path, max_px, max_bytes)
                .await
                .map(Some);
        }
        // Other/unsupported platforms
        return Err(err_not_implemented(
            "video thumbnail not supported on this platform",
        ));
    }

    // Unknown types: skip silently
    Ok(None)
}

// -------- Image implementation (pure Rust) --------

async fn gen_image_jpeg_under(src_path: &str, max_px: u32, max_bytes: usize) -> SpResult<Vec<u8>> {
    use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView};
    use std::io::Cursor;

    let bytes = tokio::fs::read(src_path).await.map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read image: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;

    let mut img = image::load_from_memory(&bytes).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("decode image: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    // Without EXIF dependency, we skip auto-orientation for now.

    // Try progressive downscale and quality to fit under max_bytes
    let mut size_candidates = vec![max_px, 96, 80, 64, 48, 32];
    size_candidates.retain(|s| *s >= 16);
    let qualities = [80u8, 70, 60, 50, 40, 35, 30];

    for target in size_candidates {
        let (w, h) = img.dimensions();
        let scale = {
            let mw = target as f32 / w as f32;
            let mh = target as f32 / h as f32;
            mw.min(mh).min(1.0)
        };
        let resized: DynamicImage = if scale < 1.0 {
            let new_w = ((w as f32 * scale).round().max(1.0)) as u32;
            let new_h = ((h as f32 * scale).round().max(1.0)) as u32;
            img.resize_exact(new_w, new_h, FilterType::CatmullRom)
        } else {
            img.clone()
        };

        for &q in &qualities {
            let mut out: Vec<u8> = Vec::with_capacity(8 * 1024);
            {
                let mut enc = JpegEncoder::new_with_quality(&mut out, q);
                if let Err(e) = enc.encode_image(&resized) {
                    crate::logger::warn("thumbnail", &format!("jpeg encode q={q}: {e}"));
                    continue;
                }
            }
            if out.len() <= max_bytes {
                return Ok(out);
            }
        }
    }

    Err(SpError {
        kind: ErrorKind::NotRetriable,
        message: format!(
            "unable to fit thumbnail under {} bytes even after downscale/quality",
            max_bytes
        ),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

// -------- Video implementations (platform-specific stubs for now) --------

#[cfg(target_os = "macos")]
async fn gen_video_thumb_macos(
    _src_path: &str,
    _max_px: u32,
    _max_bytes: usize,
) -> SpResult<Vec<u8>> {
    Err(err_not_implemented("macOS video thumbnail not implemented"))
}

#[cfg(target_os = "windows")]
async fn gen_video_thumb_windows(
    _src_path: &str,
    _max_px: u32,
    _max_bytes: usize,
) -> SpResult<Vec<u8>> {
    Err(err_not_implemented(
        "Windows video thumbnail not implemented",
    ))
}

#[cfg(target_os = "linux")]
async fn gen_video_thumb_linux(
    _src_path: &str,
    _max_px: u32,
    _max_bytes: usize,
) -> SpResult<Vec<u8>> {
    Err(err_not_implemented("Linux video thumbnail not implemented"))
}
