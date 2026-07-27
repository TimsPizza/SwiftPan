//! Upload object metadata and writer construction.
//!
//! This module owns content-type inference and the OpenDAL writer options that
//! turn upload metadata into the remote object contract. It must not read local
//! sources, access credentials, mutate transfer state, emit Tauri events, or
//! decide when an upload starts or completes.

use opendal::{Operator, Writer};

pub(super) fn inferred_content_type(key: &str, explicit: Option<&str>) -> String {
    if let Some(value) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return value.to_string();
    }

    let extension = std::path::Path::new(key)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("arw") => "image/x-sony-arw",
        Some("cr2") => "image/x-canon-cr2",
        Some("cr3") => "image/x-canon-cr3",
        Some("dng") => "image/x-adobe-dng",
        Some("nef") => "image/x-nikon-nef",
        Some("orf") => "image/x-olympus-orf",
        Some("raf") => "image/x-fuji-raf",
        Some("rw2") => "image/x-panasonic-rw2",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub(super) async fn open_upload_writer(
    operator: &Operator,
    key: &str,
    content_type: Option<&str>,
    content_disposition: Option<&str>,
) -> Result<Writer, opendal::Error> {
    let resolved_content_type = inferred_content_type(key, content_type);
    let mut writer = operator
        .writer_with(key)
        .content_type(&resolved_content_type);
    if let Some(value) = content_disposition
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        writer = writer.content_disposition(value);
    }
    writer.await
}
