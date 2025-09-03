use crate::types::*;

#[tauri::command]
pub async fn vault_status() -> SpResult<serde_json::Value> {
  Err(err_not_implemented("vault_status")).map_err(|e| e)
}

#[tauri::command]
pub async fn vault_set_manual(
  _bundle: serde_json::Value,
  _master_password: String,
) -> SpResult<()> {
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
pub async fn upload_new(_params: serde_json::Value) -> SpResult<String> {
  Err(err_not_implemented("upload_new"))
}

#[tauri::command]
pub async fn upload_ctrl(_transfer_id: String, _action: String) -> SpResult<()> {
  Err(err_not_implemented("upload_ctrl"))
}

#[tauri::command]
pub async fn upload_status(_transfer_id: String) -> SpResult<serde_json::Value> {
  Err(err_not_implemented("upload_status"))
}

#[tauri::command]
pub async fn download_new(_params: serde_json::Value) -> SpResult<String> {
  Err(err_not_implemented("download_new"))
}

#[tauri::command]
pub async fn download_ctrl(_transfer_id: String, _action: String) -> SpResult<()> {
  Err(err_not_implemented("download_ctrl"))
}

#[tauri::command]
pub async fn download_status(_transfer_id: String) -> SpResult<serde_json::Value> {
  Err(err_not_implemented("download_status"))
}

#[tauri::command]
pub async fn share_generate(_params: serde_json::Value) -> SpResult<serde_json::Value> {
  Err(err_not_implemented("share_generate"))
}

#[tauri::command]
pub async fn usage_merge_day(_date: String) -> SpResult<serde_json::Value> {
  Err(err_not_implemented("usage_merge_day"))
}

#[tauri::command]
pub async fn usage_list_month(_prefix: String) -> SpResult<Vec<serde_json::Value>> {
  Err(err_not_implemented("usage_list_month"))
}

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

