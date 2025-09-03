use crate::types::*;

pub struct UsageSync;

impl UsageSync {
  pub fn record_local_delta(_delta: UsageDelta) -> SpResult<()> {
    Err(err_not_implemented("usage.record_local_delta"))
  }

  pub async fn merge_and_write_day(_date: &str) -> SpResult<DailyLedger> {
    Err(err_not_implemented("usage.merge_and_write_day"))
  }

  pub async fn list_month(_prefix: &str) -> SpResult<Vec<DailyLedger>> {
    Err(err_not_implemented("usage.list_month"))
  }
}

