use crate::types::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const STATIC_SHARE_PATH: &str = "analytics/static/share.json";

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

async fn load_ledger(client: &crate::r2_client::R2Client, force_refresh: bool) -> SpResult<ShareLedger> {
    // Try local cache if not forced and fresh within 24h
    if !force_refresh {
        if let Ok(p) = cache_path() {
            if p.exists() {
                if let Ok(bytes) = fs::read(&p) {
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
    let remote = match crate::r2_client::get_object_bytes(client, STATIC_SHARE_PATH).await {
        Ok((bytes, _)) => serde_json::from_slice::<ShareLedger>(&bytes).unwrap_or_default(),
        Err(_) => ShareLedger::default(),
    };
    // Update cache timestamp and persist locally
    let mut v = remote;
    v.updated_at_ms = now_ms();
    if let Ok(p) = cache_path() {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&p, serde_json::to_vec(&v).unwrap_or_default());
    }
    Ok(v)
}

async fn save_ledger(client: &crate::r2_client::R2Client, ledger: &ShareLedger) -> SpResult<()> {
    let mut v = ledger.clone();
    v.updated_at_ms = now_ms();
    let bytes = serde_json::to_vec(&v).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize share ledger: {e}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    crate::r2_client::put_object_bytes(client, STATIC_SHARE_PATH, bytes, None, false).await?;
    // save cache
    if let Ok(p) = cache_path() {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&p, serde_json::to_vec(&v).unwrap_or_default());
    }
    Ok(())
}

pub async fn generate_share_link(params: ShareParams) -> SpResult<ShareLink> {
    // Build client and presign
    let bundle = crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = crate::r2_client::build_client(&bundle.r2).await?;
    let (url, expires_at_ms) = crate::r2_client::presign_get_url(
        &client,
        &params.key,
        params.ttl_secs,
        params.download_filename.clone(),
    )
    .await?;
    // Update remote + cache ledger (force refresh to reduce conflicts)
    let mut ledger = load_ledger(&client, true).await?;
    let entry = ShareEntry {
        key: params.key.clone(),
        url: url.clone(),
        created_at_ms: now_ms(),
        expires_at_ms,
        ttl_secs: params.ttl_secs,
        download_filename: params.download_filename.clone(),
    };
    ledger.items.insert(0, entry);
    // Cap length to avoid unbounded growth
    if ledger.items.len() > 1000 {
        ledger.items.truncate(1000);
    }
    let _ = save_ledger(&client, &ledger).await;
    Ok(ShareLink { url, expires_at_ms })
}

pub async fn list_share_entries() -> SpResult<Vec<ShareEntry>> {
    let bundle = crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = crate::r2_client::build_client(&bundle.r2).await?;
    let v = load_ledger(&client, false).await?;
    Ok(v.items)
}
