//! Usage-accounting Tauri commands.
//!
//! This module owns bridge logging and dispatch for ledger synchronization,
//! monthly listing, and cost summaries. It must not implement accounting,
//! access credentials directly, or own unrelated command domains.

use crate::types::{DailyLedger, SpResult};

#[tauri::command]
pub async fn usage_merge_day(date: String) -> SpResult<DailyLedger> {
    crate::logger::info("bridge", &format!("usage_merge_day date={date}"));
    let result = crate::usage::UsageSync::merge_and_write_day(&date).await;
    if let Err(error) = &result {
        crate::logger::error("bridge", &format!("usage_merge_day err: {}", error.message));
    }
    result
}

#[tauri::command]
pub async fn usage_list_month(prefix: String) -> SpResult<Vec<DailyLedger>> {
    crate::logger::info("bridge", &format!("usage_list_month prefix={prefix}"));
    crate::usage::UsageSync::list_month(&prefix).await
}

#[tauri::command]
pub async fn usage_month_cost(prefix: String) -> SpResult<serde_json::Value> {
    crate::logger::info("bridge", &format!("usage_month_cost prefix={prefix}"));
    crate::usage::UsageSync::month_cost(&prefix).await
}
