#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Android-specific FS via SAF (no-op on other platforms)
        .plugin(tauri_plugin_android_fs::init())
        // Persisted scope for Android SAF permissions
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            crate::settings::settings_get,
            crate::settings::settings_set,
            crate::logger::log_tail,
            crate::logger::log_clear,
            crate::logger::log_set_level,
            crate::logger::log_get_status,
            crate::bridge::backend_status,
            crate::bridge::backend_credentials_redacted,
            crate::bridge::backend_export_credentials_package,
            crate::bridge::backend_import_credentials_package,
            crate::bridge::backend_set_credentials,
            crate::bridge::backend_patch_credentials,
            crate::bridge::vault_status,     // legacy shim
            crate::bridge::vault_set_manual, // legacy shim
            crate::bridge::r2_sanity_check,
            crate::bridge::upload_new,
            crate::bridge::upload_new_stream,
            crate::bridge::upload_stream_write,
            crate::bridge::upload_stream_finish,
            crate::bridge::upload_ctrl,
            crate::bridge::upload_status,
            crate::bridge::download_new,
            crate::bridge::download_ctrl,
            crate::bridge::download_status,
            crate::bridge::download_sandbox_dir,
            crate::bridge::share_generate,
            crate::bridge::share_list,
            crate::bridge::usage_merge_day,
            crate::bridge::usage_list_month,
            crate::bridge::usage_month_cost,
            crate::bridge::bg_set_limits,
            crate::bridge::bg_global,
            crate::bridge::bg_mock_start,
            crate::bridge::download_now,
            crate::bridge::list_objects,
            crate::bridge::list_all_objects,
            crate::bridge::delete_object,
            crate::bridge::ui_status_bar_height,
            // Android SAF helpers
            crate::bridge::android_pick_download_dir,
            crate::bridge::android_get_persisted_download_dir,
            crate::bridge::android_fs_copy,
            crate::bridge::android_pick_upload_files,
            crate::bridge::android_upload_from_uri,
        ])
        .setup(|app| {
            // Initialize tracing-based file logger with simple rotation (4MB cap)
            crate::logger::init(app.handle().clone()).map_err(|e| {
                Box::<dyn std::error::Error>::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("logger init failed: {}", e.message),
                ))
            })?;
            // Load persisted app settings and apply log level
            if let Err(e) = crate::settings::init() {
                crate::logger::warn("app", &format!("settings init failed: {}", e.message));
            }
            // Pre-build a global R2 client if credentials are available, in background.
            tauri::async_runtime::spawn(async move {
                if let Ok(bundle) = crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked()
                {
                    crate::logger::info("app", "prebuilding R2 client at startup");
                    // Soft timeout so app doesn't stall if construction hangs
                    let build = crate::r2_client::build_client(&bundle.r2);
                    match tokio::time::timeout(std::time::Duration::from_secs(10), build).await {
                        Ok(Ok(_)) => crate::logger::info("app", "prebuild R2 client ok"),
                        Ok(Err(e)) => crate::logger::error(
                            "app",
                            &format!("prebuild R2 client error: {}", e.message),
                        ),
                        Err(_) => crate::logger::error("app", "prebuild R2 client timed out"),
                    }
                } else {
                    crate::logger::info("app", "no unlocked credentials at startup; skip prebuild");
                }
            });
            // Flush usage deltas on every app startup (per user request)
            tauri::async_runtime::spawn(async move {
                if crate::sp_backend::SpBackend::get_decrypted_bundle_if_unlocked().is_ok() {
                    match crate::usage::UsageSync::sync_all_local_deltas().await {
                        Ok(n) => crate::logger::info(
                            "app",
                            &format!("startup usage sync complete ({} day(s))", n),
                        ),
                        Err(e) => crate::logger::warn(
                            "app",
                            &format!("startup usage sync skipped: {}", e.message),
                        ),
                    }
                } else {
                    crate::logger::info(
                        "app",
                        "no unlocked credentials at startup; skip usage sync",
                    );
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("error while running tauri application: {}", e);
        });
}
pub mod background;
pub mod bridge;
pub mod download;
pub mod logger;
pub mod r2_client;
pub mod settings;
pub mod share;
pub mod sp_backend;
pub mod types;
pub mod upload;
pub mod usage;
