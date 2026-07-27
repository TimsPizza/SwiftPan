//! Android upload-source Tauri commands.
//!
//! This module owns SAF upload-file selection and dispatch from content URIs.
//! It must not own generic SAF copies, download-directory persistence, upload
//! engine behavior, remote objects, credentials, or non-Android UI concerns.

use crate::types::*;
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::AndroidFsExt as _;

#[tauri::command]
pub async fn android_pick_upload_files(app: tauri::AppHandle) -> SpResult<Vec<serde_json::Value>> {
    #[cfg(target_os = "android")]
    {
        let api = app.android_fs();
        let picker = api.file_picker();
        let mut output = Vec::new();

        fn extract_uri(serialized: &str) -> String {
            let trimmed = serialized.trim();
            if trimmed.starts_with('{') {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(uri) = value.get("uri").and_then(|value| value.as_str()) {
                        return uri.to_string();
                    }
                }
            }
            serialized.to_string()
        }

        if let Ok(files) = picker.pick_files(None, &[]) {
            for file in files {
                let serialized = match file.to_string() {
                    Ok(serialized) => serialized,
                    Err(_) => continue,
                };
                let uri = extract_uri(&serialized);
                let display_name = api
                    .get_name(&file)
                    .unwrap_or_else(|_| "upload.bin".to_string());
                let size = api.get_metadata(&file).ok().map(|metadata| metadata.len());
                let mime_type = api.get_mime_type(&file).ok();
                output.push(serde_json::json!({
                    "uri": uri,
                    "displayName": display_name,
                    "size": size,
                    "mimeType": mime_type
                }));
            }
            if !output.is_empty() {
                return Ok(output);
            }
        }

        let picked = picker.pick_file(None, &[]).map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("pick_file failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        if let Some(file) = picked {
            let serialized = file.to_string().map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("serialize FileUri: {error}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
            let uri = extract_uri(&serialized);
            let display_name = api
                .get_name(&file)
                .unwrap_or_else(|_| "upload.bin".to_string());
            let size = api.get_metadata(&file).ok().map(|metadata| metadata.len());
            let mime_type = api.get_mime_type(&file).ok();
            output.push(serde_json::json!({
                "uri": uri,
                "displayName": display_name,
                "size": size,
                "mimeType": mime_type
            }));
        }
        return Ok(output);
    }
    #[allow(unreachable_code)]
    {
        let _ = app;
        Err(err_not_implemented("android_pick_upload_files"))
    }
}

#[tauri::command]
pub async fn android_upload_from_uri(
    app: tauri::AppHandle,
    params: serde_json::Value,
) -> SpResult<String> {
    let key = params
        .get("key")
        .and_then(|value| value.as_str())
        .ok_or_else(|| err_invalid("key missing"))?
        .to_string();
    let uri = params
        .get("uri")
        .and_then(|value| value.as_str())
        .ok_or_else(|| err_invalid("uri missing"))?
        .to_string();
    let part_size = params
        .get("part_size")
        .and_then(|value| value.as_u64())
        .unwrap_or(8 * 1024 * 1024);
    let content_type = params
        .get("content_type")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    #[cfg(target_os = "android")]
    {
        crate::upload::start_upload_android_uri(app, key, uri, part_size, content_type).await
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, key, uri, part_size, content_type);
        Err(err_not_implemented("android_upload_from_uri"))
    }
}
