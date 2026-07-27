//! Download Tauri commands.
//!
//! This module owns bridge validation, logging, dispatch, sandbox-directory
//! lookup, and the legacy direct-download command. It must not implement the
//! resumable engine, runtime state, Android target materialization, credentials,
//! or unrelated command domains.

use crate::download::{DownloadStatus, NewDownloadParams};
use crate::sp_backend::SpBackend;
use crate::types::{err_not_implemented, ErrorKind, SpError, SpResult};
use tokio::io::AsyncWriteExt;

#[tauri::command]
pub async fn download_new(app: tauri::AppHandle, params: NewDownloadParams) -> SpResult<String> {
    let target = if let Some(dest_path) = params.dest_path.as_deref() {
        format!("dest={dest_path}")
    } else if let (Some(tree_uri), Some(relative_path)) = (
        params.android_tree_uri.as_deref(),
        params.android_relative_path.as_deref(),
    ) {
        format!("tree={tree_uri} rel={relative_path}")
    } else {
        "target=invalid".to_string()
    };
    crate::logger::info(
        "bridge",
        &format!(
            "download_new key={} chunk={} {}",
            params.key, params.chunk_size, target
        ),
    );
    let result = crate::download::start_download(app, params).await;
    match &result {
        Ok(id) => crate::logger::info("bridge", &format!("download_new ok id={id}")),
        Err(error) => {
            crate::logger::error("bridge", &format!("download_new err: {}", error.message))
        }
    }
    result
}

#[tauri::command]
pub async fn download_ctrl(
    app: tauri::AppHandle,
    transfer_id: String,
    action: String,
) -> SpResult<()> {
    crate::logger::info(
        "bridge",
        &format!("download_ctrl id={transfer_id} action={action}"),
    );
    let result = match action.as_str() {
        "pause" => crate::download::pause(&app, &transfer_id),
        "resume" => crate::download::resume(&app, &transfer_id),
        "cancel" => crate::download::cancel(&app, &transfer_id),
        _ => Err(err_not_implemented("download_ctrl action")),
    };
    if let Err(error) = &result {
        crate::logger::error("bridge", &format!("download_ctrl err: {}", error.message));
    }
    result
}

#[tauri::command]
pub async fn download_status(transfer_id: String) -> SpResult<DownloadStatus> {
    crate::download::status(&transfer_id)
}

#[tauri::command]
pub async fn download_sandbox_dir() -> SpResult<String> {
    let mut path = crate::sp_backend::vault_dir()?;
    path.push("downloads");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::create_dir_all(&path);
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn download_now(key: String, dest_path: String) -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = crate::storage::build_operator(&bundle.r2).await?;
    let bytes = operator
        .read(&key)
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("GetObject: {error}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let path = {
        let raw = dest_path.trim();
        let raw = raw.strip_prefix("file://").unwrap_or(raw);
        if raw.contains("://") {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: "unsupported URI for download destination".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
        std::path::PathBuf::from(raw)
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = tokio::fs::create_dir_all(parent).await {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create parent dir: {error}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
    }
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open dest failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    file.write_all(&bytes).await.map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("write file: {error}"),
        retry_after_ms: Some(300),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    file.flush().await.ok();
    Ok(())
}
