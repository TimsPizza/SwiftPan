#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      crate::bridge::vault_status,
      crate::bridge::vault_set_manual,
      crate::bridge::vault_unlock,
      crate::bridge::vault_lock,
      crate::bridge::r2_sanity_check,
      crate::bridge::upload_new,
      crate::bridge::upload_ctrl,
      crate::bridge::upload_status,
      crate::bridge::download_new,
      crate::bridge::download_ctrl,
      crate::bridge::download_status,
      crate::bridge::share_generate,
      crate::bridge::usage_merge_day,
      crate::bridge::usage_list_month,
      crate::bridge::bg_set_limits,
      crate::bridge::bg_global,
      crate::bridge::download_now,
      crate::bridge::list_objects,
      crate::bridge::delete_object,
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
pub mod types;
pub mod bridge;
pub mod r2_client;
pub mod credential_vault;
pub mod upload;
pub mod download;
pub mod usage;
pub mod share;
pub mod background;
