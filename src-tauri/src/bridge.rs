use crate::download::{DownloadStatus, NewDownloadParams};
use crate::r2_client;
use crate::share::{ShareLink, ShareParams};
use crate::sp_backend::{BackendState as BackendStatus, CredentialBundle, SpBackend};
use crate::types::*;
use crate::upload::{NewUploadParams, NewUploadStreamParams, UploadStatus};
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFsExt as _, FileUri};
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

// Let user pick a directory (one-time), save the Tree-URI persistently
#[tauri::command]
pub async fn android_pick_download_dir(app: tauri::AppHandle) -> SpResult<String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::AndroidFsExt as _;

        let api = app.android_fs();
        let picker = api.file_picker();
        let picked = picker.pick_dir(None).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("pick_dir failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let Some(dir_uri) = picked else {
            return Err(SpError {
                kind: ErrorKind::Cancelled,
                message: "user cancelled dir selection".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        };
        // Persist URI permission for long-term use
        api.take_persistable_uri_permission(&dir_uri).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("take_persistable_uri_permission: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        // Serialize FileUri to string for storage
        let tree_uri = dir_uri.to_string().map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize FileUri: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        // Save to persistent settings
        let mut settings = crate::settings::get();
        settings.android_tree_uri = Some(tree_uri.clone());
        crate::settings::set(settings).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("save tree_uri to settings failed: {}", e.message),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        crate::logger::info("bridge", &format!("android tree_uri saved: {}", tree_uri));
        return Ok(tree_uri);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        Err(err_not_implemented("android_pick_download_dir"))
    }
}

// Return the stored Tree-URI if available
#[tauri::command]
pub async fn android_get_persisted_download_dir(
    _app: tauri::AppHandle,
) -> SpResult<Option<String>> {
    Ok(crate::settings::get().android_tree_uri.clone())
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AndroidCopyParams {
    pub src_path: String,      // absolute local path in sandbox
    pub tree_uri: String,      // content://.../tree/...
    pub relative_path: String, // e.g. "dir1/dir2/file.bin"
    pub mime: Option<String>,
}

// Copy one local file to the SAF tree location, creating parent dirs if needed
#[tauri::command]
pub async fn android_copy_from_path_to_tree(
    app: tauri::AppHandle,
    params: AndroidCopyParams,
) -> SpResult<()> {
    #[cfg(target_os = "android")]
    {
        use std::io::{BufReader, Read, Write};

        let api = app.android_fs();
        let base = FileUri::from_str(&params.tree_uri).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("tree uri parse: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        // ensure parent dirs
        if let Some(parent) = std::path::Path::new(&params.relative_path).parent() {
            let parent_rel = parent.to_string_lossy();
            api.create_dir_all(&base, parent_rel.as_ref())
                .map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("create_dir_all: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
        }

        // create file
        let file_uri = api
            .create_file(&base, &params.relative_path, params.mime.as_deref())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create_file: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        // open writable stream
        let mut ws = api.open_writable_stream(&file_uri).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open_writable_stream: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        // read src and copy
        let f = std::fs::File::open(&params.src_path).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open src {}: {e}", &params.src_path),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let mut br = BufReader::new(f);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = br.read(&mut buf).map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("read: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
            if n == 0 {
                break;
            }
            ws.write_all(&buf[..n]).map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("write: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        }
        // drop ws to close
        drop(ws);
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        let _ = params;
        Err(err_not_implemented("android_copy_from_path_to_tree"))
    }
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

#[tauri::command]
pub async fn backend_patch_credentials(patch: crate::sp_backend::R2ConfigPatch) -> SpResult<()> {
    crate::logger::info("bridge", "backend_patch_credentials called");
    let r = crate::sp_backend::SpBackend::patch_r2_config(patch);
    match &r {
        Ok(_) => crate::logger::info("bridge", "backend_patch_credentials ok"),
        Err(e) => crate::logger::info(
            "bridge",
            &format!("backend_patch_credentials err: {}", e.message),
        ),
    }
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
            "upload_new key={} part_size={} path={}",
            params.key, params.part_size, params.source_path
        ),
    );
    let r = crate::upload::start_upload(app, params).await;
    match &r {
        Ok(id) => crate::logger::info("bridge", &format!("upload_new ok id={}", id)),
        Err(e) => crate::logger::error("bridge", &format!("upload_new err: {}", e.message)),
    }
    r
}
#[tauri::command]
pub async fn upload_new_stream(
    app: tauri::AppHandle,
    params: NewUploadStreamParams,
) -> SpResult<String> {
    crate::logger::info(
        "bridge",
        &format!(
            "upload_new_stream key={} total={} part_size={}",
            params.key, params.bytes_total, params.part_size
        ),
    );
    let r = crate::upload::start_upload_stream(app, params).await;
    match &r {
        Ok(id) => crate::logger::info("bridge", &format!("upload_new_stream ok id={}", id)),
        Err(e) => crate::logger::error("bridge", &format!("upload_new_stream err: {}", e.message)),
    }
    r
}

#[tauri::command]
pub async fn upload_stream_write(
    _app: tauri::AppHandle,
    transfer_id: String,
    chunk: Vec<u8>,
) -> SpResult<()> {
    crate::upload::stream_write(&transfer_id, chunk)
}

#[tauri::command]
pub async fn upload_stream_finish(_app: tauri::AppHandle, transfer_id: String) -> SpResult<()> {
    crate::upload::stream_finish(&transfer_id)
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
            "download_new key={} chunk={} dest={}",
            params.key, params.chunk_size, params.dest_path
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

// Expose an app sandbox downloads directory for staged downloads
#[tauri::command]
pub async fn download_sandbox_dir() -> SpResult<String> {
    let mut p = crate::sp_backend::vault_dir()?;
    p.push("downloads");
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::create_dir_all(&p);
    Ok(p.to_string_lossy().to_string())
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
    crate::share::generate_share_link(params).await
}

#[tauri::command]
pub async fn share_list() -> SpResult<Vec<crate::share::ShareEntry>> {
    crate::share::list_share_entries().await
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
    // Normalize dest path (strip file://, reject other URIs) and ensure parent dir exists
    let path = {
        let raw = dest_path.trim();
        let raw = raw.strip_prefix("file://").unwrap_or(raw);
        if raw.contains("://") {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: "unsupported URI for download destination".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
        std::path::PathBuf::from(raw)
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create parent dir: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
    }
    let mut file = tokio::fs::File::create(&path).await.map_err(|e| SpError {
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
    // Usage accounted by HTTP layer.
    Ok(())
}

// (streaming upload commands are defined earlier with app handle)

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
pub async fn delete_object(key: String) -> SpResult<String> {
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

// UI helpers
#[tauri::command]
pub async fn ui_status_bar_height() -> SpResult<i32> {
    // -------------------------
    // ANDROID
    // -------------------------
    #[cfg(target_os = "android")]
    {
        use jni::objects::{JClass, JObject, JValue};
        use jni::JavaVM;

        unsafe {
            let ctx = ndk_context::android_context();
            let vm_ptr = ctx.vm();
            let ctx_obj = ctx.context();
            if vm_ptr.is_null() || ctx_obj.is_null() {
                return Ok(0);
            }

            // 附加当前线程（不要去销毁 VM；AttachGuard 退出时自动 detach）
            let vm = JavaVM::from_raw(vm_ptr as *mut _)
                .map_err(|_| ())
                .unwrap_or_else(|_| unreachable!());
            let mut env = match vm.attach_current_thread() {
                Ok(e) => e,
                Err(_) => return Ok(0),
            };

            // ndk_context 给的是全局引用；转本地引用避免乱删
            let context_glob = JObject::from_raw(ctx_obj as jni::sys::jobject);
            let context = match env.new_local_ref(&context_glob) {
                Ok(o) => JObject::from(o),
                Err(_) => return Ok(0),
            };

            // SDK_INT
            let version_cls: JClass = match env.find_class("android/os/Build$VERSION") {
                Ok(c) => c,
                Err(_) => return Ok(0),
            };
            let sdk_int = env
                .get_static_field(version_cls, "SDK_INT", "I")
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0);

            // Activity → Window → DecorView
            let is_activity = env
                .is_instance_of(&context, "android/app/Activity")
                .unwrap_or(false);
            if !is_activity {
                return Ok(status_bar_dimen_fallback(&mut env, &context)); // 兜底
            }
            let activity = context;

            let window = env
                .call_method(&activity, "getWindow", "()Landroid/view/Window;", &[])
                .ok()
                .and_then(|v| v.l().ok());
            let Some(window) = window else {
                return Ok(status_bar_dimen_fallback(&mut env, &activity));
            };

            let decor = env
                .call_method(&window, "getDecorView", "()Landroid/view/View;", &[])
                .ok()
                .and_then(|v| v.l().ok());
            let Some(decor) = decor else {
                return Ok(status_bar_dimen_fallback(&mut env, &activity));
            };

            let insets = env
                .call_method(
                    &decor,
                    "getRootWindowInsets",
                    "()Landroid/view/WindowInsets;",
                    &[],
                )
                .ok()
                .and_then(|v| v.l().ok());
            let Some(insets) = insets else {
                return Ok(status_bar_dimen_fallback(&mut env, &activity));
            };

            // API >= 30：WindowInsets.getInsets(Type.statusBars|displayCutout).top
            if sdk_int >= 30 {
                let type_cls: JClass = match env.find_class("android/view/WindowInsets$Type") {
                    Ok(c) => c,
                    Err(_) => return Ok(status_bar_dimen_fallback(&mut env, &activity)),
                };
                let sb = env
                    .call_static_method(type_cls, "statusBars", "()I", &[])
                    .ok()
                    .and_then(|v| v.i().ok())
                    .unwrap_or(0);
                // `JClass` is not Copy; re-find the class for the next call
                let type_cls: JClass = match env.find_class("android/view/WindowInsets$Type") {
                    Ok(c) => c,
                    Err(_) => return Ok(status_bar_dimen_fallback(&mut env, &activity)),
                };
                let dc = env
                    .call_static_method(type_cls, "displayCutout", "()I", &[])
                    .ok()
                    .and_then(|v| v.i().ok())
                    .unwrap_or(0);
                let mask = sb | dc;

                let insets_obj = env
                    .call_method(
                        &insets,
                        "getInsets",
                        "(I)Landroid/graphics/Insets;",
                        &[JValue::from(mask)],
                    )
                    .ok()
                    .and_then(|v| v.l().ok());
                if let Some(insets_obj) = insets_obj {
                    let top = env
                        .get_field(&insets_obj, "top", "I")
                        .ok()
                        .and_then(|v| v.i().ok())
                        .unwrap_or(0);
                    return Ok(top.max(0));
                }
                return Ok(status_bar_dimen_fallback(&mut env, &activity));
            }

            // 23..=29：getSystemWindowInsetTop()
            if sdk_int >= 23 {
                let top = env
                    .call_method(&insets, "getSystemWindowInsetTop", "()I", &[])
                    .ok()
                    .and_then(|v| v.i().ok())
                    .unwrap_or(0);
                return Ok(if top > 0 {
                    top
                } else {
                    status_bar_dimen_fallback(&mut env, &activity)
                });
            }

            // 老系统兜底
            return Ok(status_bar_dimen_fallback(&mut env, &activity));
        }

        // 兜底：读 "status_bar_height"
        fn status_bar_dimen_fallback(env: &mut jni::JNIEnv, context: &JObject) -> i32 {
            let resources = env
                .call_method(
                    context,
                    "getResources",
                    "()Landroid/content/res/Resources;",
                    &[],
                )
                .ok()
                .and_then(|v| v.l().ok());
            let Some(resources) = resources else {
                return 0;
            };

            let name = env.new_string("status_bar_height").ok();
            let dimen = env.new_string("dimen").ok();
            let pkg = env.new_string("android").ok();
            let (Some(name), Some(dimen), Some(pkg)) = (name, dimen, pkg) else {
                return 0;
            };

            let id = env
                .call_method(
                    &resources,
                    "getIdentifier",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
                    &[
                        JValue::Object(&JObject::from(name)),
                        JValue::Object(&JObject::from(dimen)),
                        JValue::Object(&JObject::from(pkg)),
                    ],
                )
                .ok()
                .and_then(|v| v.i().ok())
                .unwrap_or(0);

            if id <= 0 {
                return 0;
            }
            env.call_method(
                &resources,
                "getDimensionPixelSize",
                "(I)I",
                &[JValue::from(id)],
            )
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(0)
            .max(0)
        }
    }

    // -------------------------
    // iOS
    // -------------------------
    #[cfg(target_os = "ios")]
    {
        use objc::runtime::{Object, BOOL, YES};
        use objc::{class, msg_send, sel, sel_impl};

        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGRect {
            origin: (f64, f64),
            size: CGSize,
        }
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct UIEdgeInsets {
            top: f64,
            left: f64,
            bottom: f64,
            right: f64,
        }

        unsafe {
            // UIScreen.scale（把点转像素）
            let screen: *mut Object = msg_send![class!(UIScreen), mainScreen];
            let scale: f64 = if !screen.is_null() {
                let s: f64 = msg_send![screen, scale];
                if s > 0.0 {
                    s
                } else {
                    1.0
                }
            } else {
                1.0
            };

            // UIApplication.sharedApplication
            let app: *mut Object = msg_send![class!(UIApplication), sharedApplication];

            // iOS13+：优先从活跃 scene 的 keyWindow / windows 取 safeAreaInsets
            let windows: *mut Object = msg_send![app, windows];
            let has_windows: BOOL = msg_send![windows, count];
            if has_windows as i32 > 0 {
                let win: *mut Object = msg_send![windows, firstObject];
                if !win.is_null() {
                    // window.safeAreaInsets.top（单位：点）
                    let insets: UIEdgeInsets = msg_send![win, safeAreaInsets];
                    let px = (insets.top * scale).round() as i32;
                    if px >= 0 {
                        return Ok(px);
                    }
                }
            }

            // 退路：windowScene.statusBarManager.statusBarFrame.size.height（点）
            // 尝试拿第一个 window 的 scene
            if has_windows as i32 > 0 {
                let win: *mut Object = msg_send![windows, firstObject];
                if !win.is_null() {
                    let scene: *mut Object = msg_send![win, windowScene];
                    if !scene.is_null() {
                        let sbm: *mut Object = msg_send![scene, statusBarManager];
                        if !sbm.is_null() {
                            let frame: CGRect = msg_send![sbm, statusBarFrame];
                            let px = (frame.size.height * scale).round() as i32;
                            if px >= 0 {
                                return Ok(px);
                            }
                        }
                    }
                }
            }

            // 史前退路（可能为 0）：statusBarFrame（已废弃，但作为最后兜底）
            let frame: CGRect = msg_send![app, statusBarFrame];
            let px = (frame.size.height * scale).round() as i32;
            return Ok(px.max(0));
        }
    }

    // -------------------------
    // 其它（桌面端）
    // -------------------------
    #[allow(unreachable_code)]
    {
        Ok(0)
    }
}
