//! Thumbnail generation, upload, and cache Tauri commands.
//!
//! This module owns the bridge contract for thumbnail creation and cached data
//! lookup. It must not own general object browsing, credentials, transfers,
//! platform filesystem access, sharing, or usage commands.

use crate::sp_backend::SpBackend;
use crate::types::SpResult;
use base64::Engine;

#[tauri::command]
pub async fn generate_thumbnail_and_upload(
    _app: tauri::AppHandle,
    key: String,
    source_path: String,
) -> SpResult<Option<String>> {
    if !crate::settings::get().upload_thumbnail {
        return Ok(None);
    }
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = crate::storage::build_operator(&bundle.r2).await?;
    crate::thumbnail::generate_and_store(&operator, &key, &source_path, 128, 16 * 1024).await
}

#[tauri::command]
pub async fn thumbnail_get_cached_data(
    object_key: String,
    object_etag: Option<String>,
) -> SpResult<Option<String>> {
    if let Some(cached) = crate::transfer_db::get_thumbnail_cache(&object_key)? {
        if cached.object_etag == object_etag {
            return Ok(Some(cached.data_url));
        }
    }

    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = crate::storage::build_operator(&bundle.r2).await?;
    let Some(bytes) = crate::thumbnail::read_stored(&operator, &object_key).await? else {
        let _ = crate::transfer_db::delete_thumbnail_cache(&object_key);
        return Ok(None);
    };
    let thumbnail_key = crate::thumbnail::thumbnail_key_for(&object_key);
    let data_url = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    crate::transfer_db::upsert_thumbnail_cache(&crate::transfer_db::ThumbnailCacheEntry {
        object_key,
        object_etag,
        thumbnail_key,
        data_url: data_url.clone(),
        updated_at_ms: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(Some(data_url))
}
