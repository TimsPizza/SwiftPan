//! Share-link Tauri commands.
//!
//! This module owns bridge logging and dispatch for share generation/history.
//! It must not implement signing, persistence, credentials, object browsing,
//! transfers, platform access, or usage accounting.

use crate::share::{ShareLink, ShareParams};
use crate::types::SpResult;

#[tauri::command]
pub async fn share_generate(params: ShareParams) -> SpResult<ShareLink> {
    crate::logger::debug(
        "bridge",
        &format!(
            "share_generate key={} ttl={} filename_present={}",
            params.key,
            params.ttl_secs,
            params.download_filename.is_some()
        ),
    );
    crate::share::generate_share_link(params).await
}

#[tauri::command]
pub async fn share_list() -> SpResult<Vec<crate::share::ShareEntry>> {
    crate::share::list_share_entries().await
}
