//! Android SAF filesystem Tauri commands and adapters.
//!
//! This module owns download-directory selection and persistence plus generic
//! SAF-to-sandbox and sandbox-to-SAF copies, with explicit unsupported
//! fallbacks elsewhere. It must not own upload selection, UI measurements,
//! credentials, remote objects, or transfer engines.

use crate::types::*;
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFsExt as _, FileAccessMode, FileUri};

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
        api.take_persistable_uri_permission(&dir_uri)
            .map_err(|e| SpError {
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
pub struct AndroidFsCopyParams {
    pub direction: String, // "sandbox_to_tree", "tree_to_sandbox", "uri_to_sandbox"
    pub local_path: String,
    pub tree_uri: Option<String>,
    pub relative_path: Option<String>,
    pub mime: Option<String>,
    pub uri: Option<String>,
}

#[tauri::command]
pub async fn android_fs_copy(app: tauri::AppHandle, params: AndroidFsCopyParams) -> SpResult<()> {
    #[cfg(target_os = "android")]
    {
        use std::io::{BufReader, Read, Write};

        let api = app.android_fs();
        let result = match params.direction.as_str() {
            "sandbox_to_tree" => {
                let tree_uri = params
                    .tree_uri
                    .ok_or_else(|| err_invalid("tree_uri required"))?;
                let rel = params
                    .relative_path
                    .ok_or_else(|| err_invalid("relative_path required"))?;
                if rel.trim().is_empty() {
                    return Err(err_invalid("relative_path required"));
                }
                let base = FileUri::from_str(&tree_uri).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("tree uri parse: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                if let Some(parent) = std::path::Path::new(&rel).parent() {
                    let parent_rel = parent.to_string_lossy();
                    if !parent_rel.is_empty() && parent_rel != "." {
                        api.create_dir_all(&base, parent_rel.as_ref())
                            .map_err(|e| SpError {
                                kind: ErrorKind::NotRetriable,
                                message: format!("create_dir_all: {e}"),
                                retry_after_ms: None,
                                context: None,
                                at: chrono::Utc::now().timestamp_millis(),
                            })?;
                    }
                }
                let file_uri = api
                    .create_new_file(&base, &rel, params.mime.as_deref())
                    .map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("create_file: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                let mut ws = api.open_writable_stream(&file_uri).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("open_writable_stream: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                let f = std::fs::File::open(&params.local_path).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("open src {}: {e}", &params.local_path),
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
                drop(ws);
                Ok(())
            }
            "tree_to_sandbox" => {
                let tree_uri = params
                    .tree_uri
                    .ok_or_else(|| err_invalid("tree_uri required"))?;
                let rel = params
                    .relative_path
                    .ok_or_else(|| err_invalid("relative_path required"))?;
                let base = FileUri::from_str(&tree_uri).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("tree uri parse: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                let file_uri = api.try_resolve_file_uri(&base, &rel).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("resolve file uri: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                let rs = api
                    .open_file(&file_uri, FileAccessMode::Read)
                    .map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("open_file: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                let mut rs = BufReader::new(rs);
                if let Some(parent) = std::path::Path::new(&params.local_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| SpError {
                            kind: ErrorKind::NotRetriable,
                            message: format!("create_dir_all: {e}"),
                            retry_after_ms: None,
                            context: None,
                            at: chrono::Utc::now().timestamp_millis(),
                        })?;
                    }
                }
                let mut file = std::fs::File::create(&params.local_path).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("create dest {}: {e}", &params.local_path),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                let mut buf = [0u8; 64 * 1024];
                loop {
                    let n = rs.read(&mut buf).map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("read: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buf[..n]).map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("write: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                }
                file.flush().map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("flush: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                Ok(())
            }
            "uri_to_sandbox" => {
                let uri = params.uri.ok_or_else(|| err_invalid("uri required"))?;
                let file_uri = FileUri {
                    uri,
                    document_top_tree_uri: None,
                };
                let rs = api
                    .open_file(&file_uri, FileAccessMode::Read)
                    .map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("open_file: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                let mut rs = BufReader::new(rs);
                if let Some(parent) = std::path::Path::new(&params.local_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| SpError {
                            kind: ErrorKind::NotRetriable,
                            message: format!("create_dir_all: {e}"),
                            retry_after_ms: None,
                            context: None,
                            at: chrono::Utc::now().timestamp_millis(),
                        })?;
                    }
                }
                let mut file = std::fs::File::create(&params.local_path).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("create dest {}: {e}", &params.local_path),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                let mut buf = [0u8; 64 * 1024];
                loop {
                    let n = rs.read(&mut buf).map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("read: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buf[..n]).map_err(|e| SpError {
                        kind: ErrorKind::NotRetriable,
                        message: format!("write: {e}"),
                        retry_after_ms: None,
                        context: None,
                        at: chrono::Utc::now().timestamp_millis(),
                    })?;
                }
                file.flush().map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("flush: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                Ok(())
            }
            _ => Err(err_invalid("unsupported direction")),
        };
        return result;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        let _ = params;
        Err(err_not_implemented("android_fs_copy"))
    }
}
