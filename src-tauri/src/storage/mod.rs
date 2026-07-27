//! OpenDAL storage-backend construction and lifecycle.
//!
//! This module turns the persisted backend configuration into a cached,
//! instrumented [`Operator`]. It owns endpoint normalization, credentials,
//! HTTP/TLS configuration, cache invalidation, and connectivity checks. It
//! must not contain SwiftPan object browsing, transfer, thumbnail, sharing, or
//! usage-ledger business rules.

use crate::types::*;
use once_cell::sync::Lazy;
use opendal::services::S3;
use opendal::{layers::HttpClientLayer, raw::HttpClient, Operator};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
// use std::time::Duration; // not currently used directly

// Cache one configured operator per credential fingerprint.
static OPERATOR_CACHE: Lazy<RwLock<Option<(String, Operator)>>> = Lazy::new(|| RwLock::new(None));
// Serialize construction to avoid concurrent backend initialization races.
static OPERATOR_BUILD_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

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

pub async fn build_operator(cfg: &R2Config) -> SpResult<Operator> {
    // Serve from cache if config matches
    let fp = cfg_fingerprint(cfg);
    if let Some((cached_fp, cached)) = OPERATOR_CACHE.read().await.as_ref() {
        if *cached_fp == fp {
            crate::logger::debug("storage", "build_operator using cached instance");
            return Ok(cached.clone());
        }
    }

    // Serialize construction to avoid concurrent builds which might hang on some platforms
    let _guard = OPERATOR_BUILD_LOCK.lock().await;
    // Double-check after acquiring the lock
    if let Some((cached_fp, cached)) = OPERATOR_CACHE.read().await.as_ref() {
        if *cached_fp == fp {
            crate::logger::debug(
                "storage",
                "build_operator using cached instance (post-lock)",
            );
            return Ok(cached.clone());
        }
    }
    crate::logger::debug(
        "storage",
        &format!(
            "build_operator endpoint={} bucket={} region={}",
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
    crate::logger::debug("storage", "build_operator conf ok");
    crate::logger::info("storage", "build_operator ok");
    {
        let mut w = OPERATOR_CACHE.write().await;
        *w = Some((fp, op.clone()));
    }
    Ok(op)
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

pub async fn sanity_check(operator: &Operator) -> SpResult<()> {
    crate::logger::debug("r2", "sanity_check(list 1) start");
    network_precheck().await?;
    let l = operator.list("").await.map_err(|e| SpError {
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

/// Invalidate the cached operator, forcing the next build to reconstruct it.
pub async fn invalidate_cached_operator() {
    let mut w = OPERATOR_CACHE.write().await;
    *w = None;
    crate::logger::info("storage", "storage operator cache invalidated");
}

#[cfg(test)]
mod tests;
