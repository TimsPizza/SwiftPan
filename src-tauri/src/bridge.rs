use crate::types::*;
use crate::credential_vault::{CredentialBundle, CredentialVault, VaultExportPackage, VaultState as VaultStatus};
use crate::upload::{NewUploadParams, UploadStatus};
use crate::download::{NewDownloadParams, DownloadStatus};
use crate::share::{ShareLink, ShareParams};
use crate::r2_client;

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
pub async fn upload_new(_params: NewUploadParams) -> SpResult<String> { Err(err_not_implemented("upload_new")) }

#[tauri::command]
pub async fn upload_ctrl(_transfer_id: String, _action: String) -> SpResult<()> {
  Err(err_not_implemented("upload_ctrl"))
}

#[tauri::command]
pub async fn upload_status(_transfer_id: String) -> SpResult<UploadStatus> { Err(err_not_implemented("upload_status")) }

#[tauri::command]
pub async fn download_new(_params: NewDownloadParams) -> SpResult<String> { Err(err_not_implemented("download_new")) }

#[tauri::command]
pub async fn download_ctrl(_transfer_id: String, _action: String) -> SpResult<()> {
  Err(err_not_implemented("download_ctrl"))
}

#[tauri::command]
pub async fn download_status(_transfer_id: String) -> SpResult<DownloadStatus> { Err(err_not_implemented("download_status")) }

#[tauri::command]
pub async fn share_generate(params: ShareParams) -> SpResult<ShareLink> {
  let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
  let client = r2_client::build_client(&bundle.r2).await?;
  let (url, expires_at_ms) = r2_client::presign_get_url(&client, &params.key, params.ttl_secs, params.download_filename).await?;
  Ok(ShareLink { url, expires_at_ms })
}

#[tauri::command]
pub async fn usage_merge_day(_date: String) -> SpResult<DailyLedger> { Err(err_not_implemented("usage_merge_day")) }

#[tauri::command]
pub async fn usage_list_month(_prefix: String) -> SpResult<Vec<DailyLedger>> { Err(err_not_implemented("usage_list_month")) }

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
