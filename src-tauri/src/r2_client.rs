use crate::types::*;
use crate::usage::UsageSync;
use once_cell::sync::Lazy;
use opendal::services::S3;
use opendal::{layers::HttpClientLayer, raw::HttpClient, Operator};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
// use std::time::Duration; // not currently used directly

#[derive(Clone)]
pub struct R2Client {
    pub op: Operator,
    pub bucket: String,
}

// Global cache to ensure a single S3 client instance per-credentials across the app
static R2_CLIENT_CACHE: Lazy<RwLock<Option<(String, R2Client)>>> = Lazy::new(|| RwLock::new(None));
// Build lock to serialize client construction and avoid concurrent `from_conf` races
static R2_BUILD_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn cfg_fingerprint(cfg: &R2Config) -> String {
    // Note: this is an in-memory identifier; we don't log it to avoid leaking secrets.
    format!(
        "{}|{}|{}|{}|{}",
        cfg.endpoint,
        cfg.access_key_id,
        cfg.secret_access_key,
        cfg.bucket,
        cfg.region.clone().unwrap_or_else(|| "auto".into())
    )
}

pub async fn build_client(cfg: &R2Config) -> SpResult<R2Client> {
    // Serve from cache if config matches
    let fp = cfg_fingerprint(cfg);
    if let Some((cached_fp, cached)) = R2_CLIENT_CACHE.read().await.as_ref() {
        if *cached_fp == fp {
            crate::logger::debug("r2", "build_client using cached instance");
            return Ok(cached.clone());
        }
    }

    // Serialize construction to avoid concurrent builds which might hang on some platforms
    let _guard = R2_BUILD_LOCK.lock().await;
    // Double-check after acquiring the lock
    if let Some((cached_fp, cached)) = R2_CLIENT_CACHE.read().await.as_ref() {
        if *cached_fp == fp {
            crate::logger::debug("r2", "build_client using cached instance (post-lock)");
            return Ok(cached.clone());
        }
    }
    crate::logger::debug(
        "r2",
        &format!(
            "build_client endpoint={} bucket={} region={}",
            cfg.endpoint,
            cfg.bucket,
            cfg.region.as_deref().unwrap_or("auto")
        ),
    );
    // Prevent IMDS probing on mobile which can stall silently
    std::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
    let region = cfg.region.clone().unwrap_or_else(|| "auto".to_string());
    // Sanitize endpoint to origin-only: scheme://host[:port]
    let mut endpoint = cfg.endpoint.clone();
    if let Some(pos) = endpoint.find('#') {
        endpoint.truncate(pos);
    }
    if let Some(pos) = endpoint.find('?') {
        endpoint.truncate(pos);
    }
    if let Some(scheme_pos) = endpoint.find("://") {
        let auth_start = scheme_pos + 3;
        if let Some(rel_pos) = endpoint[auth_start..].find('/') {
            endpoint.truncate(auth_start + rel_pos);
        }
    } else if let Some(rel_pos) = endpoint.find('/') {
        endpoint.truncate(rel_pos);
    }
    while endpoint.ends_with('/') {
        endpoint.pop();
    }
    // Build OpenDAL S3 operator
    let mut builder = S3::default();
    builder = builder.access_key_id(cfg.access_key_id.as_str());
    builder = builder.secret_access_key(cfg.secret_access_key.as_str());
    builder = builder.endpoint(endpoint.as_str());
    builder = builder.region(region.as_str());
    builder = builder.bucket(cfg.bucket.as_str());
    // Build reqwest client pinned to rustls + webpki roots for consistent TLS across desktop/mobile
    // and wrap with our HTTP instrumentation for precise S3 Class A/B accounting.
    let req_builder = reqwest::Client::builder().use_rustls_tls();
    let req_client = req_builder.build().map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("HttpClient build failed: {}", e),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    // Wrap with our InstrumentedReqwest, then construct OpenDAL HttpClient from it.
    let instr = crate::usage::http_instrument::InstrumentedReqwest::new(req_client);
    let http_client = HttpClient::with(instr);

    // Build operator and inject custom HTTP client via layer
    let op = Operator::new(builder)
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("Operator build failed: {}", e),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?
        .layer(HttpClientLayer::new(http_client))
        .finish();
    // Instrumentation is always-on; no toggle required.
    crate::logger::debug("r2", "build_client conf ok");
    crate::logger::info("r2", "build_client ok");
    let client = R2Client {
        op,
        bucket: cfg.bucket.clone(),
    };
    {
        let mut w = R2_CLIENT_CACHE.write().await;
        *w = Some((fp, client.clone()));
    }
    Ok(client)
}

pub async fn network_precheck() -> SpResult<()> {
    crate::logger::info("r2", "Starting network precheck");

    // 1. 先测试基础网络连接
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("Failed to create HTTP client: {}", e),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

    // 测试基础网络连通性
    crate::logger::debug("r2", "Testing basic network connectivity");
    let response = tokio::time::timeout(
        Duration::from_secs(8),
        client.get("https://www.cloudflare.com").send(),
    )
    .await;

    match response {
        Ok(Ok(resp)) => {
            crate::logger::info("r2", &format!("Basic network test OK: {}", resp.status()));
        }
        Ok(Err(e)) => {
            crate::logger::error("r2", &format!("Basic network test failed: {}", e));
            return Err(SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("Network connectivity failed: {}", e),
                retry_after_ms: Some(3000),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
        Err(_) => {
            crate::logger::error("r2", "Basic network test timed out");
            return Err(SpError {
                kind: ErrorKind::RetryableNet,
                message: "Network connectivity timeout".to_string(),
                retry_after_ms: Some(3000),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    Ok(())
}

pub async fn sanity_check(client: &R2Client) -> SpResult<()> {
    crate::logger::debug("r2", "sanity_check(list 1) start");
    network_precheck().await?;
    let l = client.op.list("").await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list root: {}", e),
        retry_after_ms: Some(500),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let _ = l.first();
    crate::logger::info("r2", "sanity_check ok (list 1)");
    Ok(())
}

pub async fn presign_get_url(
    client: &R2Client,
    key: &str,
    ttl_secs: u64,
    download_filename: Option<String>,
) -> SpResult<(String, i64)> {
    let mut url = client
        .op
        .presign_read(key, Duration::from_secs(ttl_secs))
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("Presign failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?
        .uri()
        .to_string();
    // IMPORTANT: Do NOT append extra query params after presigning.
    // Adding parameters like response-content-disposition here will invalidate the signature.
    // If we need forced filename, we must include it in the signature process itself.
    let _ = download_filename; // currently ignored to preserve signature correctness
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_secs as i64);
    Ok((url, expires_at.timestamp_millis()))
}

pub async fn list_objects(
    client: &R2Client,
    prefix: &str,
    _continuation: Option<String>,
    max_keys: i32,
) -> SpResult<crate::types::ListPage> {
    use crate::types::{FileEntry, ANALYTICS_PREFIX};
    // Op counts handled at HTTP layer.

    // Emulate delimiter listing by grouping first-level prefixes
    use std::collections::BTreeSet;
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    // store (key, meta) so we can surface size/etag/last_modified
    let mut files: Vec<(String, opendal::Metadata)> = vec![];
    let l = client.op.list(prefix).await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list: {}", e),
        retry_after_ms: Some(500),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let mut count = 0;
    for e in l.into_iter() {
        if count >= max_keys {
            break;
        }
        let key = e.path().to_string();
        if key.ends_with('/') {
            continue;
        }
        // Top-level delimiter emulation
        let rel = key.strip_prefix(prefix).unwrap_or(&key);
        if let Some(pos) = rel.find('/') {
            let dir = format!("{}{}", prefix, &rel[..=pos]);
            dirs.insert(dir);
            continue;
        }
        let meta = e.metadata().clone();
        files.push((key, meta));
        count += 1;
    }
    let mut items: Vec<FileEntry> = vec![];
    for k in dirs.into_iter() {
        items.push(FileEntry {
            key: k.clone(),
            size: None,
            last_modified_ms: None,
            etag: None,
            is_prefix: true,
            protected: k.starts_with(ANALYTICS_PREFIX),
        });
    }
    for (key, meta) in files.into_iter() {
        let size = meta.content_length();
        let last_modified_ms = meta.last_modified().map(|dt| dt.timestamp_millis());
        let etag = meta.etag().map(|s| s.to_string());
        items.push(FileEntry {
            key: key.clone(),
            size: Some(size),
            last_modified_ms,
            etag,
            is_prefix: false,
            protected: key.starts_with(ANALYTICS_PREFIX),
        });
    }
    // Stable order: prefixes first then objects by name
    items.sort_by(|a, b| match (a.is_prefix, b.is_prefix) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.key.cmp(&b.key),
    });
    let page = crate::types::ListPage {
        prefix: prefix.to_string(),
        items,
        next_token: None,
    };
    crate::logger::info(
        "r2",
        &format!(
            "list_objects ok prefix={} items={} next_token_present={}",
            prefix,
            page.items.len(),
            page.next_token.is_some()
        ),
    );
    Ok(page)
}

pub async fn list_all_objects_flat(
    client: &R2Client,
    max_total: i32,
) -> SpResult<Vec<crate::types::FileEntry>> {
    use crate::types::{FileEntry, ANALYTICS_PREFIX};
    use futures::TryStreamExt;
    let mut items: Vec<FileEntry> = vec![];
    // Use OpenDAL recursive scan to traverse all entries under root.
    // We still stop client-side when reaching max_total.
    // Use lister_with().recursive(true) to stream all entries without manual BFS.
    let mut lister = client
        .op
        .lister_with("")
        .recursive(true)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("list recursive: {}", e),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    while let Some(entry_res) = lister.try_next().await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list entry: {}", e),
        retry_after_ms: Some(500),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })? {
        let key = entry_res.path().to_string();
        if key.ends_with('/') {
            continue;
        }
        let meta = entry_res.metadata().clone();
        let size = meta.content_length();
        let last_modified_ms = meta.last_modified().map(|dt| dt.timestamp_millis());
        let etag = meta.etag().map(|s| s.to_string());
        items.push(FileEntry {
            key: key.clone(),
            size: Some(size),
            last_modified_ms,
            etag,
            is_prefix: false,
            protected: key.starts_with(ANALYTICS_PREFIX),
        });
        if items.len() as i32 >= max_total {
            break;
        }
    }
    items.sort_by(|a, b| a.key.cmp(&b.key));
    crate::logger::info(
        "r2",
        &format!("list_all_objects_flat ok total_items={}", items.len()),
    );
    Ok(items)
}

pub async fn delete_object(client: &R2Client, key: &str) -> SpResult<String> {
    // Best-effort HEAD to capture size
    let size_opt = client.op.stat(key).await.ok().map(|m| m.content_length());
    client.op.delete(key).await.map_err(|e| {
        crate::logger::error("r2", &format!("DeleteObject error: {}", e));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("DeleteObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    })?;
    // Record deleted storage bytes locally; HTTP layer counts the operation itself.
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: Default::default(),
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: size_opt.unwrap_or(0),
    });
    Ok(key.to_string())
}

pub async fn get_object_bytes(client: &R2Client, key: &str) -> SpResult<(Vec<u8>, Option<String>)> {
    let data = client.op.read(key).await.map_err(|e| {
        crate::logger::error("r2", &format!("GetObject error: {}", e));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("GetObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    })?;
    let etag = client
        .op
        .stat(key)
        .await
        .ok()
        .and_then(|m| m.etag().map(|s| s.to_string()));
    // Op and egress bytes are tracked by HTTP layer.
    Ok((data.to_vec(), etag))
}

pub async fn put_object_bytes(
    client: &R2Client,
    key: &str,
    bytes: Vec<u8>,
    if_match: Option<String>,
    if_none_match: bool,
) -> SpResult<()> {
    let write_len = bytes.len() as u64;
    let _ = (if_match, if_none_match); // not supported with OpenDAL write
    client.op.write(key, bytes).await.map_err(|e| {
        crate::logger::error("r2", &format!("PutObject error: {}", e));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("PutObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    })?;
    // Op and ingress bytes tracked by HTTP layer.
    Ok(())
}

/// Invalidate the cached client, forcing the next `build_client` call to rebuild.
pub async fn invalidate_cached_client() {
    let mut w = R2_CLIENT_CACHE.write().await;
    *w = None;
    crate::logger::info("r2", "R2 client cache invalidated");
}
