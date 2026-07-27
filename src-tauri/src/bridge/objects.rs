//! Remote-object Tauri commands.
//!
//! This module owns bridge logging, credential-to-client coordination, list
//! retry policy, protected deletion checks, thumbnail cleanup, and R2 sanity
//! dispatch. It must not implement the R2 client itself or own transfers,
//! sharing, platform access, credentials, or usage commands.

use crate::r2_client;
use crate::sp_backend::SpBackend;
use crate::types::{ErrorKind, FileEntry, ListPage, SpError, SpResult, ANALYTICS_PREFIX};

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let result = r2_client::sanity_check(&client).await;
    if let Err(error) = &result {
        crate::logger::error(
            "bridge",
            &format!("r2_sanity_check error: {}", error.message),
        );
    }
    crate::logger::info("bridge", "r2_sanity_check returning");
    result
}

#[tauri::command]
pub async fn list_objects(
    prefix: Option<String>,
    token: Option<String>,
    max_keys: Option<i32>,
) -> SpResult<ListPage> {
    crate::logger::debug(
        "bridge",
        &format!(
            "list_objects prefix={prefix:?} token_present={} max_keys={max_keys:?}",
            token.is_some()
        ),
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let prefix = prefix.unwrap_or_default();
    let mut result =
        r2_client::list_objects(&client, &prefix, token.clone(), max_keys.unwrap_or(1000)).await;
    if let Err(error) = &result {
        let message = error.message.to_lowercase();
        if message.contains("unknownissuer") || message.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_objects TLS error; invalidating cached R2 client and retrying once",
            );
            r2_client::invalidate_cached_client().await;
            let retry_client = r2_client::build_client(&bundle.r2).await?;
            result =
                r2_client::list_objects(&retry_client, &prefix, token, max_keys.unwrap_or(1000))
                    .await;
        }
    }
    if let Err(error) = &result {
        crate::logger::error(
            "bridge",
            &format!(
                "list_objects error: prefix={} err={}",
                prefix, error.message
            ),
        );
    }
    result
}

#[tauri::command]
pub async fn list_all_objects(max_total: Option<i32>) -> SpResult<Vec<FileEntry>> {
    crate::logger::debug(
        "bridge",
        &format!("list_all_objects max_total={max_total:?}"),
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let mut result = r2_client::list_all_objects_flat(&client, max_total.unwrap_or(10_000)).await;
    if let Err(error) = &result {
        let message = error.message.to_lowercase();
        if message.contains("unknownissuer") || message.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_all_objects TLS error; invalidating cached R2 client and retrying once",
            );
            r2_client::invalidate_cached_client().await;
            let retry_client = r2_client::build_client(&bundle.r2).await?;
            result =
                r2_client::list_all_objects_flat(&retry_client, max_total.unwrap_or(10_000)).await;
        }
    }
    if let Err(error) = &result {
        crate::logger::error(
            "bridge",
            &format!("list_all_objects error: {}", error.message),
        );
    }
    result
}

#[tauri::command]
pub async fn delete_object(key: String) -> SpResult<String> {
    if key.starts_with(ANALYTICS_PREFIX) {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "deleting analytics files is prohibited".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    crate::logger::info("bridge", &format!("delete_object key={key}"));
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let result = r2_client::delete_object(&client, &key).await;
    if let Err(error) = &result {
        crate::logger::error(
            "bridge",
            &format!("delete_object error: key={key} err={}", error.message),
        );
    } else if !crate::thumbnail::is_thumbnail_key(&key) {
        let _ = r2_client::delete_object(&client, &crate::thumbnail::thumbnail_key_for(&key)).await;
        let _ = crate::transfer_db::delete_thumbnail_cache(&key);
    }
    result
}
