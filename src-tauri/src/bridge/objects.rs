//! Remote-object Tauri commands.
//!
//! This module owns command argument/default handling, credential-to-storage
//! coordination, TLS retry dispatch, and bridge logging. Object projection,
//! deletion rules, thumbnail cleanup, and usage effects belong to the
//! application-level `objects` module.

use crate::sp_backend::SpBackend;
use crate::types::{FileEntry, ListPage, SpResult};
use crate::{objects, storage};

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = storage::build_operator(&bundle.r2).await?;
    let result = storage::sanity_check(&operator).await;
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
    let operator = storage::build_operator(&bundle.r2).await?;
    let prefix = prefix.unwrap_or_default();
    let mut result =
        objects::list_objects(&operator, &prefix, token.clone(), max_keys.unwrap_or(1000)).await;
    if let Err(error) = &result {
        let message = error.message.to_lowercase();
        if message.contains("unknownissuer") || message.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_objects TLS error; invalidating cached storage operator and retrying once",
            );
            storage::invalidate_cached_operator().await;
            let retry_operator = storage::build_operator(&bundle.r2).await?;
            result =
                objects::list_objects(&retry_operator, &prefix, token, max_keys.unwrap_or(1000))
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
    let operator = storage::build_operator(&bundle.r2).await?;
    let mut result = objects::list_all_objects(&operator, max_total.unwrap_or(10_000)).await;
    if let Err(error) = &result {
        let message = error.message.to_lowercase();
        if message.contains("unknownissuer") || message.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_all_objects TLS error; invalidating cached storage operator and retrying once",
            );
            storage::invalidate_cached_operator().await;
            let retry_operator = storage::build_operator(&bundle.r2).await?;
            result = objects::list_all_objects(&retry_operator, max_total.unwrap_or(10_000)).await;
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
    objects::validate_delete_key(&key)?;
    crate::logger::info("bridge", &format!("delete_object key={key}"));
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = storage::build_operator(&bundle.r2).await?;
    let result = objects::delete_object(&operator, &key).await;
    if let Err(error) = &result {
        crate::logger::error(
            "bridge",
            &format!("delete_object error: key={key} err={}", error.message),
        );
    }
    result
}
