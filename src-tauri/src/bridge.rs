use crate::types::*;
use crate::credential_vault::{CredentialBundle, CredentialVault, VaultExportPackage, VaultState as VaultStatus};
use crate::upload::{NewUploadParams, UploadStatus};
use crate::download::{NewDownloadParams, DownloadStatus};
use crate::share::{ShareLink, ShareParams};
use crate::r2_client;
use tokio::io::AsyncWriteExt;

#[tauri::command]
pub async fn vault_status() -> SpResult<VaultStatus> { CredentialVault::status() }

#[tauri::command]
pub async fn vault_set_manual(bundle: CredentialBundle, master_password: String) -> SpResult<()> {
  CredentialVault::set_with_plaintext(bundle, &master_password)
}

#[tauri::command]
pub async fn vault_unlock(master_password: String, hold_ms: u64) -> SpResult<()> { CredentialVault::unlock(&master_password, hold_ms) }

#[tauri::command]
pub async fn vault_lock() -> SpResult<()> { CredentialVault::lock() }

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
  let client = r2_client::build_client(&bundle.r2).await?;
  r2_client::sanity_check(&client, "swiftpan-selftest/").await
}

#[tauri::command]
pub async fn upload_new(params: NewUploadParams) -> SpResult<String> { crate::upload::start_upload(params).await }
// Implemented below

#[tauri::command]
pub async fn upload_ctrl(_transfer_id: String, _action: String) -> SpResult<()> {
  match _action.as_str() {
    "pause" => crate::upload::pause(&_transfer_id),
    "resume" => crate::upload::resume(&_transfer_id),
    "cancel" => crate::upload::cancel(&_transfer_id),
    _ => Err(err_not_implemented("upload_ctrl action")),
  }
}

#[tauri::command]
pub async fn upload_status(transfer_id: String) -> SpResult<UploadStatus> { crate::upload::status(&transfer_id) }
// Implemented below

#[tauri::command]
pub async fn download_new(params: NewDownloadParams) -> SpResult<String> { crate::download::start_download(params).await }

#[tauri::command]
pub async fn download_ctrl(transfer_id: String, action: String) -> SpResult<()> {
  match action.as_str() {
    "pause" => crate::download::pause(&transfer_id),
    "resume" => crate::download::resume(&transfer_id),
    "cancel" => crate::download::cancel(&transfer_id),
    _ => Err(err_not_implemented("download_ctrl action")),
  }
}

#[tauri::command]
pub async fn download_status(transfer_id: String) -> SpResult<DownloadStatus> { crate::download::status(&transfer_id) }

#[tauri::command]
pub async fn share_generate(params: ShareParams) -> SpResult<ShareLink> {
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
  let client = r2_client::build_client(&bundle.r2).await?;
  let (url, expires_at_ms) = r2_client::presign_get_url(&client, &params.key, params.ttl_secs, params.download_filename).await?;
  Ok(ShareLink { url, expires_at_ms })
}

#[tauri::command]
pub async fn usage_merge_day(date: String) -> SpResult<DailyLedger> { crate::usage::UsageSync::merge_and_write_day(&date).await }

#[tauri::command]
pub async fn usage_list_month(prefix: String) -> SpResult<Vec<DailyLedger>> { crate::usage::UsageSync::list_month(&prefix).await }

#[tauri::command]
pub async fn bg_set_limits(
  _limits: serde_json::Value,
  _rate: serde_json::Value,
) -> SpResult<()> {
  Err(err_not_implemented("bg_set_limits"))
}

#[tauri::command]
pub async fn bg_global(_action: String) -> SpResult<()> {
  Err(err_not_implemented("bg_global"))
}

// Temporary utility for early testing: direct download to file (non-resumable)
#[tauri::command]
pub async fn download_now(key: String, dest_path: String) -> SpResult<()> {
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
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
  let mut file = tokio::fs::File::create(&dest_path).await.map_err(|e| SpError { kind: ErrorKind::NotRetriable, message: format!("open dest failed: {e}"), retry_after_ms: None, context: None, at: chrono::Utc::now().timestamp_millis() })?;
  tokio::io::copy(&mut body, &mut file).await.map_err(|e| SpError { kind: ErrorKind::RetryableNet, message: format!("stream copy failed: {e}"), retry_after_ms: Some(500), context: None, at: chrono::Utc::now().timestamp_millis() })?;
  file.flush().await.ok();
  Ok(())
}

#[tauri::command]
pub async fn list_objects(prefix: Option<String>, token: Option<String>, max_keys: Option<i32>) -> SpResult<crate::types::ListPage> {
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
  let client = r2_client::build_client(&bundle.r2).await?;
  let p = prefix.unwrap_or_else(|| "".into());
  r2_client::list_objects(&client, &p, token, max_keys.unwrap_or(1000)).await
}

#[tauri::command]
pub async fn delete_object(key: String) -> SpResult<()> {
  if key.starts_with(crate::types::ANALYTICS_PREFIX) {
    return Err(SpError { kind: ErrorKind::NotRetriable, message: "deleting analytics files is prohibited".into(), retry_after_ms: None, context: None, at: chrono::Utc::now().timestamp_millis() });
  }
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
  let client = r2_client::build_client(&bundle.r2).await?;
  r2_client::delete_object(&client, &key).await
}
