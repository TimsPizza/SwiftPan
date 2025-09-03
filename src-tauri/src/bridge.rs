use crate::types::*;
use crate::credential_vault::{CredentialBundle, VaultExportPackage, VaultState as VaultStatus};
use crate::upload::{NewUploadParams, UploadStatus};
use crate::download::{NewDownloadParams, DownloadStatus};
use crate::share::{ShareLink, ShareParams};

#[tauri::command]
pub async fn vault_status() -> SpResult<VaultStatus> { Err(err_not_implemented("vault_status")) }

#[tauri::command]
pub async fn vault_set_manual(_bundle: CredentialBundle, _master_password: String) -> SpResult<()> {
  Err(err_not_implemented("vault_set_manual"))
}

#[tauri::command]
pub async fn vault_unlock(_master_password: String, _hold_ms: u64) -> SpResult<()> {
  Err(err_not_implemented("vault_unlock"))
}

#[tauri::command]
pub async fn vault_lock() -> SpResult<()> {
  Err(err_not_implemented("vault_lock"))
}

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
  Err(err_not_implemented("r2_sanity_check"))
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
pub async fn share_generate(_params: ShareParams) -> SpResult<ShareLink> { Err(err_not_implemented("share_generate")) }

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
