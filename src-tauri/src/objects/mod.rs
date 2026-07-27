//! SwiftPan object-browser business logic.
//!
//! This module translates raw object-store entries into the file model exposed
//! to the frontend. It owns prefix-as-directory projection, thumbnail hiding
//! and association, analytics deletion protection, related-thumbnail cleanup,
//! and deletion usage deltas. It must not construct credentials, configure an
//! OpenDAL backend, own transfer execution, or expose Tauri commands.

use crate::thumbnail;
use crate::types::{
    ErrorKind, FileEntry, ListPage, SpError, SpResult, UsageDelta, ANALYTICS_PREFIX,
};
use crate::usage::UsageSync;
use futures::TryStreamExt;
use opendal::Operator;
use std::collections::{BTreeSet, HashSet};

pub async fn list_objects(
    operator: &Operator,
    prefix: &str,
    _continuation: Option<String>,
    max_keys: i32,
) -> SpResult<ListPage> {
    let mut dirs = BTreeSet::new();
    let mut files: Vec<(String, opendal::Metadata)> = Vec::new();
    let entries = operator.list(prefix).await.map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list: {error}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })?;
    let mut count = 0;
    for entry in entries {
        if count >= max_keys {
            break;
        }
        let key = entry.path().to_string();
        if key.ends_with('/') || thumbnail::is_thumbnail_key(&key) {
            continue;
        }
        let relative = key.strip_prefix(prefix).unwrap_or(&key);
        if let Some(position) = relative.find('/') {
            dirs.insert(format!("{}{}", prefix, &relative[..=position]));
            continue;
        }
        files.push((key, entry.metadata().clone()));
        count += 1;
    }

    let mut items = dirs
        .into_iter()
        .map(|key| FileEntry {
            protected: key.starts_with(ANALYTICS_PREFIX),
            key,
            size: None,
            last_modified_ms: None,
            etag: None,
            thumbnail_key: None,
            is_prefix: true,
        })
        .collect::<Vec<_>>();
    items.extend(files.into_iter().map(|(key, metadata)| {
        FileEntry {
            size: Some(metadata.content_length()),
            last_modified_ms: metadata
                .last_modified()
                .map(|timestamp| timestamp.timestamp_millis()),
            etag: metadata.etag().map(str::to_string),
            protected: key.starts_with(ANALYTICS_PREFIX),
            key,
            thumbnail_key: None,
            is_prefix: false,
        }
    }));
    items.sort_by(|left, right| match (left.is_prefix, right.is_prefix) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.key.cmp(&right.key),
    });
    let page = ListPage {
        prefix: prefix.to_string(),
        items,
        next_token: None,
    };
    crate::logger::info(
        "objects",
        &format!(
            "list_objects ok prefix={} items={} next_token_present={}",
            prefix,
            page.items.len(),
            page.next_token.is_some()
        ),
    );
    Ok(page)
}

pub async fn list_all_objects(operator: &Operator, max_total: i32) -> SpResult<Vec<FileEntry>> {
    let mut files = Vec::new();
    let mut thumbnails = HashSet::new();
    let mut lister = operator
        .lister_with("")
        .recursive(true)
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("list recursive: {error}"),
            retry_after_ms: Some(500),
            context: None,
            at: now_ms(),
        })?;
    while let Some(entry) = lister.try_next().await.map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list entry: {error}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })? {
        let key = entry.path().to_string();
        if key.ends_with('/') {
            continue;
        }
        if thumbnail::is_thumbnail_key(&key) {
            thumbnails.insert(key);
            continue;
        }
        let metadata = entry.metadata();
        files.push(FileEntry {
            size: Some(metadata.content_length()),
            last_modified_ms: metadata
                .last_modified()
                .map(|timestamp| timestamp.timestamp_millis()),
            etag: metadata.etag().map(str::to_string),
            protected: key.starts_with(ANALYTICS_PREFIX),
            key,
            thumbnail_key: None,
            is_prefix: false,
        });
        if files.len() as i32 >= max_total {
            break;
        }
    }

    let mut items = files
        .into_iter()
        .map(|mut item| {
            let thumbnail_key = thumbnail::thumbnail_key_for(&item.key);
            if thumbnails.contains(&thumbnail_key) {
                item.thumbnail_key = Some(thumbnail_key);
            }
            item
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.key.cmp(&right.key));
    crate::logger::info(
        "objects",
        &format!("list_all_objects ok total_items={}", items.len()),
    );
    Ok(items)
}

pub async fn delete_object(operator: &Operator, key: &str) -> SpResult<String> {
    validate_delete_key(key)?;

    delete_one(operator, key).await?;
    if !thumbnail::is_thumbnail_key(key) {
        let _ = delete_one(operator, &thumbnail::thumbnail_key_for(key)).await;
        let _ = crate::transfer_db::delete_thumbnail_cache(key);
    }
    Ok(key.to_string())
}

pub fn validate_delete_key(key: &str) -> SpResult<()> {
    if key.starts_with(ANALYTICS_PREFIX) {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "deleting analytics files is prohibited".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    Ok(())
}

async fn delete_one(operator: &Operator, key: &str) -> SpResult<()> {
    let size = operator
        .stat(key)
        .await
        .ok()
        .map(|metadata| metadata.content_length());
    operator.delete(key).await.map_err(|error| {
        crate::logger::error("objects", &format!("DeleteObject error: {error}"));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("DeleteObject: {error}"),
            retry_after_ms: Some(500),
            context: None,
            at: now_ms(),
        }
    })?;
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: Default::default(),
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: size.unwrap_or(0),
    });
    Ok(())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests;
