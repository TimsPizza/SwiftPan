//! Upload Tauri commands.
//!
//! This module owns bridge validation, logging, and dispatch for file and
//! push-stream uploads. It must not implement upload I/O, runtime state,
//! Android SAF selection, credentials, or unrelated command domains.

use crate::types::{err_not_implemented, SpResult};
use crate::upload::{NewUploadParams, NewUploadStreamParams, UploadStatus};

#[tauri::command]
pub async fn upload_new(app: tauri::AppHandle, params: NewUploadParams) -> SpResult<String> {
    crate::logger::info(
        "bridge",
        &format!(
            "upload_new key={} part_size={} path={}",
            params.key, params.part_size, params.source_path
        ),
    );
    let result = crate::upload::start_upload(app, params).await;
    match &result {
        Ok(id) => crate::logger::info("bridge", &format!("upload_new ok id={id}")),
        Err(error) => crate::logger::error("bridge", &format!("upload_new err: {}", error.message)),
    }
    result
}

#[tauri::command]
pub async fn upload_new_stream(
    app: tauri::AppHandle,
    params: NewUploadStreamParams,
) -> SpResult<String> {
    crate::logger::info(
        "bridge",
        &format!(
            "upload_new_stream key={} total={} part_size={}",
            params.key, params.bytes_total, params.part_size
        ),
    );
    let result = crate::upload::start_upload_stream(app, params).await;
    match &result {
        Ok(id) => crate::logger::info("bridge", &format!("upload_new_stream ok id={id}")),
        Err(error) => crate::logger::error(
            "bridge",
            &format!("upload_new_stream err: {}", error.message),
        ),
    }
    result
}

#[tauri::command]
pub async fn upload_stream_write(
    _app: tauri::AppHandle,
    transfer_id: String,
    chunk: Vec<u8>,
) -> SpResult<()> {
    crate::upload::stream_write(&transfer_id, chunk)
}

#[tauri::command]
pub async fn upload_stream_finish(_app: tauri::AppHandle, transfer_id: String) -> SpResult<()> {
    crate::upload::stream_finish(&transfer_id)
}

#[tauri::command]
pub async fn upload_ctrl(
    app: tauri::AppHandle,
    transfer_id: String,
    action: String,
) -> SpResult<()> {
    match action.as_str() {
        "pause" => crate::upload::pause(&app, &transfer_id),
        "resume" => crate::upload::resume(&app, &transfer_id),
        "cancel" => crate::upload::cancel(&app, &transfer_id),
        _ => Err(err_not_implemented("upload_ctrl action")),
    }
}

#[tauri::command]
pub async fn upload_status(transfer_id: String) -> SpResult<UploadStatus> {
    crate::upload::status(&transfer_id)
}
