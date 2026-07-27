use crate::types::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) const STATIC_SHARE_PATH: &str = "analytics/static/share.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareParams {
    pub key: String,
    pub ttl_secs: u64, // 900, 3600, 86400
    pub download_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLink {
    pub url: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareEntry {
    pub key: String,
    pub url: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub ttl_secs: u64,
    pub download_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShareLedger {
    pub items: Vec<ShareEntry>,
    pub updated_at_ms: i64,
}

fn cache_path() -> SpResult<PathBuf> {
    Ok(crate::sp_backend::vault_dir()?.join("share_cache.json"))
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn load_ledger(operator: &opendal::Operator, force_refresh: bool) -> SpResult<ShareLedger> {
    let local_cache = cache_path().ok();
    load_ledger_with_cache(operator, force_refresh, local_cache.as_deref()).await
}

pub(crate) async fn load_ledger_with_cache(
    operator: &opendal::Operator,
    force_refresh: bool,
    local_cache: Option<&Path>,
) -> SpResult<ShareLedger> {
    // Try local cache if not forced and fresh within 24h
    if !force_refresh {
        if let Some(p) = local_cache {
            if p.exists() {
                if let Ok(bytes) = fs::read(p) {
                    if let Ok(v) = serde_json::from_slice::<ShareLedger>(&bytes) {
                        let age = now_ms().saturating_sub(v.updated_at_ms);
                        if age < 24 * 60 * 60 * 1000 {
                            return Ok(v);
                        }
                    }
                }
            }
        }
    }
    // Load from remote
    let remote = match operator.read(STATIC_SHARE_PATH).await {
        Ok(bytes) => serde_json::from_slice::<ShareLedger>(&bytes.to_vec()).unwrap_or_default(),
        Err(_) => ShareLedger::default(),
    };
    // Update cache timestamp and persist locally
    let mut v = remote;
    v.updated_at_ms = now_ms();
    if let Some(p) = local_cache {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(p, serde_json::to_vec(&v).unwrap_or_default());
    }
    Ok(v)
}

async fn save_ledger(operator: &opendal::Operator, ledger: &ShareLedger) -> SpResult<()> {
    let local_cache = cache_path().ok();
    save_ledger_with_cache(operator, ledger, local_cache.as_deref()).await
}

pub(crate) async fn save_ledger_with_cache(
    operator: &opendal::Operator,
    ledger: &ShareLedger,
    local_cache: Option<&Path>,
) -> SpResult<()> {
    let mut v = ledger.clone();
    v.updated_at_ms = now_ms();
    let bytes = serde_json::to_vec(&v).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize share ledger: {e}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    operator
        .write(STATIC_SHARE_PATH, bytes)
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("PutObject: {error}"),
            retry_after_ms: Some(500),
            context: None,
            at: now_ms(),
        })?;
    // save cache
    if let Some(p) = local_cache {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(p, serde_json::to_vec(&v).unwrap_or_default());
    }
    Ok(())
}

pub(crate) fn prepend_share_entry(ledger: &mut ShareLedger, entry: ShareEntry) {
    ledger.items.insert(0, entry);
    if ledger.items.len() > 1000 {
        ledger.items.truncate(1000);
    }
}

pub async fn generate_share_link(params: ShareParams) -> SpResult<ShareLink> {
    // Build the storage operator and presign.
    let bundle = crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = crate::storage::build_operator(&bundle.r2).await?;
    let url = operator
        .presign_read(&params.key, Duration::from_secs(params.ttl_secs))
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("Presign failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?
        .uri()
        .to_string();
    // Response-content-disposition must be part of the signature. Keep the
    // current behavior of ignoring this option until it is implemented safely.
    let _ = params.download_filename.as_deref();
    let expires_at_ms =
        (chrono::Utc::now() + chrono::Duration::seconds(params.ttl_secs as i64)).timestamp_millis();
    // Update remote + cache ledger (force refresh to reduce conflicts)
    let mut ledger = load_ledger(&operator, true).await?;
    let entry = ShareEntry {
        key: params.key.clone(),
        url: url.clone(),
        created_at_ms: now_ms(),
        expires_at_ms,
        ttl_secs: params.ttl_secs,
        download_filename: params.download_filename.clone(),
    };
    prepend_share_entry(&mut ledger, entry);
    let _ = save_ledger(&operator, &ledger).await;
    Ok(ShareLink { url, expires_at_ms })
}

pub async fn list_share_entries() -> SpResult<Vec<ShareEntry>> {
    let bundle = crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked()?;
    let operator = crate::storage::build_operator(&bundle.r2).await?;
    let v = load_ledger(&operator, false).await?;
    Ok(v.items)
}
