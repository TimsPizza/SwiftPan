//! Cross-kind transfer Tauri commands.
//!
//! This module owns aggregation and removal dispatch shared by upload and
//! download transfers. It must not implement either transfer engine or expose
//! feature-specific control/status commands.

use crate::transfer_db::TransferSnapshot;
use crate::types::{err_invalid, SpResult};

#[tauri::command]
pub async fn transfer_list_active() -> SpResult<Vec<TransferSnapshot>> {
    let mut items = crate::download::list_snapshots()?;
    items.extend(crate::upload::list_active_snapshots());
    items.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    Ok(items)
}

#[tauri::command]
pub async fn transfer_remove(transfer_id: String, kind: String) -> SpResult<()> {
    match kind.as_str() {
        "download" => crate::download::remove(&transfer_id),
        "upload" => crate::upload::remove(&transfer_id),
        _ => Err(err_invalid("invalid transfer kind")),
    }
}
