//! Background-manager Tauri commands.
//!
//! This module owns the current background command contract and development
//! event mock. It must not own transfer engines or pretend the unimplemented
//! manager actions are operational.

use crate::types::{err_not_implemented, SpResult};

#[tauri::command]
pub async fn bg_set_limits(_limits: serde_json::Value, _rate: serde_json::Value) -> SpResult<()> {
    Err(err_not_implemented("bg_set_limits"))
}

#[tauri::command]
pub async fn bg_global(_action: String) -> SpResult<()> {
    Err(err_not_implemented("bg_global"))
}

#[tauri::command]
pub async fn bg_mock_start(app: tauri::AppHandle) -> SpResult<()> {
    tauri::async_runtime::spawn(async move {
        use tauri::Emitter;
        let mut iteration = 0_u64;
        loop {
            let payload = serde_json::json!({
                "active_tasks": (iteration % 3) + 1,
                "moving_avg_bps": 5_000_000 + (iteration % 5) * 1_000_000,
                "cpu_hint": 0.2,
                "io_hint": 0.4,
            });
            let _ = app.emit("sp://background_stats", payload);
            iteration = iteration.wrapping_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}
