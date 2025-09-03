use crate::download::{DownloadStatus, NewDownloadParams};
use crate::r2_client;
use crate::share::{ShareLink, ShareParams};
use crate::sp_backend::{BackendState as BackendStatus, CredentialBundle, SpBackend};
use crate::types::*;
use crate::upload::{NewUploadParams, UploadStatus};
use tokio::io::AsyncWriteExt;

// Backend status (replaces vault_status)
#[tauri::command]
pub async fn backend_status() -> SpResult<BackendStatus> {
    SpBackend::status()
}

// Save encrypted credentials (replaces vault_set_manual)
#[tauri::command]
pub async fn backend_set_credentials(
    bundle: CredentialBundle,
    master_password: String,
) -> SpResult<()> {
    SpBackend::set_with_plaintext(bundle, &master_password)
}

// Legacy shims (no-ops / backwards compatibility)
#[tauri::command]
pub async fn vault_status() -> SpResult<BackendStatus> {
    backend_status().await
}
#[tauri::command]
pub async fn vault_set_manual(bundle: CredentialBundle, master_password: String) -> SpResult<()> {
    backend_set_credentials(bundle, master_password).await
}
#[tauri::command]
pub async fn vault_unlock(_master_password: String, _hold_ms: u64) -> SpResult<()> {
    Ok(())
}
#[tauri::command]
pub async fn vault_lock() -> SpResult<()> {
    Ok(())
}

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    r2_client::sanity_check(&client, "swiftpan-selftest/").await
}

#[tauri::command]
pub async fn upload_new(app: tauri::AppHandle, params: NewUploadParams) -> SpResult<String> {
    crate::upload::start_upload(app, params).await
}
// Implemented below

#[tauri::command]
pub async fn upload_ctrl(
    app: tauri::AppHandle,
    _transfer_id: String,
    _action: String,
) -> SpResult<()> {
    match _action.as_str() {
        "pause" => crate::upload::pause(&app, &_transfer_id),
        "resume" => crate::upload::resume(&app, &_transfer_id),
        "cancel" => crate::upload::cancel(&app, &_transfer_id),
        _ => Err(err_not_implemented("upload_ctrl action")),
    }
}

#[tauri::command]
pub async fn upload_status(transfer_id: String) -> SpResult<UploadStatus> {
    crate::upload::status(&transfer_id)
}
// Implemented below

#[tauri::command]
pub async fn download_new(app: tauri::AppHandle, params: NewDownloadParams) -> SpResult<String> {
    crate::download::start_download(app, params).await
}

#[tauri::command]
pub async fn download_ctrl(
    app: tauri::AppHandle,
    transfer_id: String,
    action: String,
) -> SpResult<()> {
    match action.as_str() {
        "pause" => crate::download::pause(&app, &transfer_id),
        "resume" => crate::download::resume(&app, &transfer_id),
        "cancel" => crate::download::cancel(&app, &transfer_id),
        _ => Err(err_not_implemented("download_ctrl action")),
    }
}

#[tauri::command]
pub async fn download_status(transfer_id: String) -> SpResult<DownloadStatus> {
    crate::download::status(&transfer_id)
}

#[tauri::command]
pub async fn share_generate(params: ShareParams) -> SpResult<ShareLink> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let (url, expires_at_ms) = r2_client::presign_get_url(
        &client,
        &params.key,
        params.ttl_secs,
        params.download_filename,
    )
    .await?;
    Ok(ShareLink { url, expires_at_ms })
}

#[tauri::command]
pub async fn usage_merge_day(date: String) -> SpResult<DailyLedger> {
    crate::usage::UsageSync::merge_and_write_day(&date).await
}

#[tauri::command]
pub async fn usage_list_month(prefix: String) -> SpResult<Vec<DailyLedger>> {
    crate::usage::UsageSync::list_month(&prefix).await
}

#[tauri::command]
pub async fn bg_set_limits(_limits: serde_json::Value, _rate: serde_json::Value) -> SpResult<()> {
    Err(err_not_implemented("bg_set_limits"))
}

#[tauri::command]
pub async fn bg_global(_action: String) -> SpResult<()> {
    Err(err_not_implemented("bg_global"))
}

// Start a mock background stats emitter to validate event pipeline
#[tauri::command]
pub async fn bg_mock_start(app: tauri::AppHandle) -> SpResult<()> {
    tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        let mut i: u64 = 0;
        loop {
            let payload = serde_json::json!({
                "active_tasks": (i % 3) + 1,
                "moving_avg_bps": 5_000_000 + (i % 5) * 1_000_000,
                "cpu_hint": 0.2,
                "io_hint": 0.4,
            });
            let _ = app.emit("sp://background_stats", payload);
            i = i.wrapping_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}

// Temporary utility for early testing: direct download to file (non-resumable)
#[tauri::command]
pub async fn download_now(key: String, dest_path: String) -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let resp = client
        .s3
        .get_object()
        .bucket(&client.bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("GetObject failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let mut body = resp.body.into_async_read();
    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open dest failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let copied = tokio::io::copy(&mut body, &mut file)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("stream copy failed: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    file.flush().await.ok();
    // Usage: B 类 GetObject +1；egress 计总拷贝字节
    let mut b = std::collections::HashMap::new();
    b.insert("GetObject".into(), 1u64);
    let _ = crate::usage::UsageSync::record_local_delta(crate::types::UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: copied as u64,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });
    Ok(())
}

#[tauri::command]
pub async fn list_objects(
    prefix: Option<String>,
    token: Option<String>,
    max_keys: Option<i32>,
) -> SpResult<crate::types::ListPage> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let p = prefix.unwrap_or_else(|| "".into());
    r2_client::list_objects(&client, &p, token, max_keys.unwrap_or(1000)).await
}

#[tauri::command]
pub async fn list_all_objects(max_total: Option<i32>) -> SpResult<Vec<crate::types::FileEntry>> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    r2_client::list_all_objects_flat(&client, max_total.unwrap_or(10_000)).await
}

#[tauri::command]
pub async fn delete_object(key: String) -> SpResult<()> {
    if key.starts_with(crate::types::ANALYTICS_PREFIX) {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "deleting analytics files is prohibited".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    r2_client::delete_object(&client, &key).await
}
