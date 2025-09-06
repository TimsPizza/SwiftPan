use crate::download::{DownloadStatus, NewDownloadParams};
use crate::r2_client;
use crate::share::{ShareLink, ShareParams};
use crate::sp_backend::{BackendState as BackendStatus, CredentialBundle, SpBackend};
use crate::types::*;
use crate::upload::{NewUploadParams, UploadStatus};
use tokio::io::AsyncWriteExt;

// Backend status (replaces vault_status)
#[tauri::command]
pub async fn backend_status() -> SpResult<BackendStatus> {
    let r = SpBackend::status();
    match &r {
        Ok(_) => crate::logger::info("bridge", "backend_status ok"),
        Err(e) => crate::logger::info("bridge", &format!("backend_status err: {}", e.message)),
    }
    r
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RedactedCredentials {
    pub endpoint: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub region: Option<String>,
}

fn redact_endpoint(ep: &str) -> String {
    // Protect the account segment (subdomain) while preserving provider host to disambiguate.
    // https://<account>.r2.cloudflarestorage.com → https://*****.r2.cloudflarestorage.com
    if let Some(rest) = ep.strip_prefix("https://") {
        if let Some(idx) = rest.find('.') {
            let host_tail = &rest[idx..];
            return format!("https://{}{}", "*****", host_tail);
        }
    }
    // Fallback: partial mask of any alnum run before first dot
    let mut parts = ep.splitn(2, '.');
    if let Some(first) = parts.next() {
        let tail = parts.next().unwrap_or("");
        let masked = if first.starts_with("http") {
            first.to_string()
        } else {
            "*****".into()
        };
        if tail.is_empty() {
            masked
        } else {
            format!("{}.{}", masked, tail)
        }
    } else {
        "*****".into()
    }
}

fn redact_key(s: &str) -> String {
    let n = s.len();
    if n <= 4 {
        return "****".into();
    }
    let keep = 4usize;
    let head = &s[..keep.min(n)];
    format!("{}{}", head, "*".repeat(n.saturating_sub(keep)))
}

#[tauri::command]
pub async fn backend_credentials_redacted() -> SpResult<RedactedCredentials> {
    crate::logger::debug("bridge", "backend_credentials_redacted");
    let b = SpBackend::get_decrypted_bundle_if_unlocked()?;
    Ok(RedactedCredentials {
        endpoint: redact_endpoint(&b.r2.endpoint),
        access_key_id: redact_key(&b.r2.access_key_id),
        secret_access_key: redact_key(&b.r2.secret_access_key),
        bucket: redact_key(&b.r2.bucket),
        region: b.r2.region,
    })
}

// Save encrypted credentials (replaces vault_set_manual)
#[tauri::command]
pub async fn backend_set_credentials(bundle: CredentialBundle) -> SpResult<()> {
    crate::logger::info("bridge", "backend_set_credentials called");
    let r = SpBackend::set_with_plaintext(bundle);
    match &r {
        Ok(_) => crate::logger::info("bridge", "backend_set_credentials ok"),
        Err(e) => crate::logger::info(
            "bridge",
            &format!("backend_set_credentials err: {}", e.message),
        ),
    }
    // Invalidate cached R2 client when credentials are updated
    if r.is_ok() {
        crate::r2_client::invalidate_cached_client().await;
    }
    r
}

// Legacy shims (no-ops / backwards compatibility)
#[tauri::command]
pub async fn vault_status() -> SpResult<BackendStatus> {
    backend_status().await
}
#[tauri::command]
pub async fn vault_set_manual(bundle: CredentialBundle) -> SpResult<()> {
    backend_set_credentials(bundle).await
}

#[tauri::command]
pub async fn r2_sanity_check() -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let res = r2_client::sanity_check(&client).await;
    if let Err(e) = &res {
        crate::logger::error("bridge", &format!("r2_sanity_check error: {}", e.message));
    }
    crate::logger::info("bridge", "r2_sanity_check returning");
    res
}

#[tauri::command]
pub async fn upload_new(app: tauri::AppHandle, params: NewUploadParams) -> SpResult<String> {
    crate::logger::info(
        "bridge",
        &format!(
            "upload_new key={} part_size={}",
            params.key, params.part_size
        ),
    );
    let r = crate::upload::start_upload(app, params).await;
    match &r {
        Ok(id) => crate::logger::info("bridge", &format!("upload_new ok id={}", id)),
        Err(e) => crate::logger::info("bridge", &format!("upload_new err: {}", e.message)),
    }
    r
}
// Implemented below

#[tauri::command]
pub async fn upload_ctrl(
    app: tauri::AppHandle,
    _transfer_id: String,
    _action: String,
) -> SpResult<()> {
    match _action.as_str() {
        "pause" => crate::upload::pause(&app, &_transfer_id),
        "resume" => crate::upload::resume(&app, &_transfer_id),
        "cancel" => crate::upload::cancel(&app, &_transfer_id),
        _ => Err(err_not_implemented("upload_ctrl action")),
    }
}

#[tauri::command]
pub async fn upload_status(transfer_id: String) -> SpResult<UploadStatus> {
    crate::upload::status(&transfer_id)
}
// Implemented below

#[tauri::command]
pub async fn download_new(app: tauri::AppHandle, params: NewDownloadParams) -> SpResult<String> {
    crate::logger::info(
        "bridge",
        &format!(
            "download_new key={} chunk={} dest=*redacted*",
            params.key, params.chunk_size
        ),
    );
    let r = crate::download::start_download(app, params).await;
    match &r {
        Ok(id) => crate::logger::info("bridge", &format!("download_new ok id={}", id)),
        Err(e) => crate::logger::error("bridge", &format!("download_new err: {}", e.message)),
    }
    r
}

#[tauri::command]
pub async fn download_ctrl(
    app: tauri::AppHandle,
    transfer_id: String,
    action: String,
) -> SpResult<()> {
    crate::logger::info(
        "bridge",
        &format!("download_ctrl id={} action={}", transfer_id, action),
    );
    let r = match action.as_str() {
        "pause" => crate::download::pause(&app, &transfer_id),
        "resume" => crate::download::resume(&app, &transfer_id),
        "cancel" => crate::download::cancel(&app, &transfer_id),
        _ => Err(err_not_implemented("download_ctrl action")),
    };
    if let Err(e) = &r {
        crate::logger::error("bridge", &format!("download_ctrl err: {}", e.message));
    }
    r
}

#[tauri::command]
pub async fn download_status(transfer_id: String) -> SpResult<DownloadStatus> {
    crate::download::status(&transfer_id)
}

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
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let (url, expires_at_ms) = r2_client::presign_get_url(
        &client,
        &params.key,
        params.ttl_secs,
        params.download_filename,
    )
    .await?;
    Ok(ShareLink { url, expires_at_ms })
}

#[tauri::command]
pub async fn usage_merge_day(date: String) -> SpResult<DailyLedger> {
    crate::logger::info("bridge", &format!("usage_merge_day date={}", date));
    let r = crate::usage::UsageSync::merge_and_write_day(&date).await;
    if let Err(e) = &r {
        crate::logger::error("bridge", &format!("usage_merge_day err: {}", e.message));
    }
    r
}

#[tauri::command]
pub async fn usage_list_month(prefix: String) -> SpResult<Vec<DailyLedger>> {
    crate::logger::info("bridge", &format!("usage_list_month prefix={}", prefix));
    crate::usage::UsageSync::list_month(&prefix).await
}

#[tauri::command]
pub async fn usage_month_cost(prefix: String) -> SpResult<serde_json::Value> {
    crate::logger::info("bridge", &format!("usage_month_cost prefix={}", prefix));
    crate::usage::UsageSync::month_cost(&prefix).await
}

#[tauri::command]
pub async fn bg_set_limits(_limits: serde_json::Value, _rate: serde_json::Value) -> SpResult<()> {
    Err(err_not_implemented("bg_set_limits"))
}

#[tauri::command]
pub async fn bg_global(_action: String) -> SpResult<()> {
    Err(err_not_implemented("bg_global"))
}

// Start a mock background stats emitter to validate event pipeline
#[tauri::command]
pub async fn bg_mock_start(app: tauri::AppHandle) -> SpResult<()> {
    tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        let mut i: u64 = 0;
        loop {
            let payload = serde_json::json!({
                "active_tasks": (i % 3) + 1,
                "moving_avg_bps": 5_000_000 + (i % 5) * 1_000_000,
                "cpu_hint": 0.2,
                "io_hint": 0.4,
            });
            let _ = app.emit("sp://background_stats", payload);
            i = i.wrapping_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}

// Temporary utility for early testing: direct download to file (non-resumable)
#[tauri::command]
pub async fn download_now(key: String, dest_path: String) -> SpResult<()> {
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let bytes = r2_client::get_object_bytes(&client, &key)
        .await
        .map(|(b, _)| b)?;
    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open dest failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    file.write_all(&bytes).await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("write file: {e}"),
        retry_after_ms: Some(300),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    file.flush().await.ok();
    // Usage: B 类 GetObject +1；egress 计总拷贝字节
    let mut b = std::collections::HashMap::new();
    b.insert("GetObject".into(), 1u64);
    let _ = crate::usage::UsageSync::record_local_delta(crate::types::UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: bytes.len() as u64,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });
    Ok(())
}

#[tauri::command]
pub async fn list_objects(
    prefix: Option<String>,
    token: Option<String>,
    max_keys: Option<i32>,
) -> SpResult<crate::types::ListPage> {
    crate::logger::debug(
        "bridge",
        &format!(
            "list_objects prefix={:?} token_present={} max_keys={:?}",
            prefix,
            token.is_some(),
            max_keys
        ),
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let p = prefix.unwrap_or_else(|| "".into());
    let mut res =
        r2_client::list_objects(&client, &p, token.clone(), max_keys.unwrap_or(1000)).await;
    if let Err(e) = &res {
        let msg = e.message.to_lowercase();
        if msg.contains("unknownissuer") || msg.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_objects TLS error; invalidating cached R2 client and retrying once",
            );
            r2_client::invalidate_cached_client().await;
            let client2 = r2_client::build_client(&bundle.r2).await?;
            res = r2_client::list_objects(&client2, &p, token, max_keys.unwrap_or(1000)).await;
        }
    }
    if let Err(e) = &res {
        crate::logger::error(
            "bridge",
            &format!("list_objects error: prefix={} err={}", p, e.message),
        );
    }
    res
}

#[tauri::command]
pub async fn list_all_objects(max_total: Option<i32>) -> SpResult<Vec<crate::types::FileEntry>> {
    crate::logger::debug(
        "bridge",
        &format!("list_all_objects max_total={:?}", max_total),
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let mut res = r2_client::list_all_objects_flat(&client, max_total.unwrap_or(10_000)).await;
    if let Err(e) = &res {
        let msg = e.message.to_lowercase();
        if msg.contains("unknownissuer") || msg.contains("invalid peer certificate") {
            crate::logger::warn(
                "bridge",
                "list_all_objects TLS error; invalidating cached R2 client and retrying once",
            );
            r2_client::invalidate_cached_client().await;
            let client2 = r2_client::build_client(&bundle.r2).await?;
            res = r2_client::list_all_objects_flat(&client2, max_total.unwrap_or(10_000)).await;
        }
    }
    if let Err(e) = &res {
        crate::logger::error("bridge", &format!("list_all_objects error: {}", e.message));
    }
    res
}

#[tauri::command]
pub async fn delete_object(key: String) -> SpResult<()> {
    if key.starts_with(crate::types::ANALYTICS_PREFIX) {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "deleting analytics files is prohibited".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    crate::logger::info("bridge", &format!("delete_object key={}", key));
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let res = r2_client::delete_object(&client, &key).await;
    if let Err(e) = &res {
        crate::logger::error(
            "bridge",
            &format!("delete_object error: key={} err={}", key, e.message),
        );
    }
    res
}
