use serde::{Deserialize, Serialize};

pub type DeviceId = String; // non-sensitive

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorKind {
  Cancelled,
  RetryableNet,
  RetryableAuth,
  NotRetriable,
  SourceChanged,
  DiskFull,
  NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpError {
  pub kind: ErrorKind,
  pub message: String,
  pub retry_after_ms: Option<u64>,
  pub context: Option<serde_json::Value>,
  pub at: i64, // epoch ms
}

pub type SpResult<T> = Result<T, SpError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2Config {
  pub endpoint: String,
  pub access_key_id: String,
  pub secret_access_key: String,
  pub bucket: String,
  pub region: Option<String>, // default "auto"
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConcurrencyLimits {
  pub per_task_parts: u8,
  pub global_active_tasks: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
  pub max_bytes_per_sec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPartProgress {
  pub part_number: u32,
  pub bytes_transferred: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadChunkProgress {
  pub range_start: u64,
  pub bytes_transferred: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDelta {
  pub class_a: std::collections::HashMap<String, u64>,
  pub class_b: std::collections::HashMap<String, u64>,
  pub ingress_bytes: u64,
  pub egress_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLedger {
  pub date: String, // YYYY-MM-DD (UTC)
  pub class_a: std::collections::HashMap<String, u64>,
  pub class_b: std::collections::HashMap<String, u64>,
  pub ingress_bytes: u64,
  pub egress_bytes: u64,
  pub storage_bytes: u64,
  pub rev: u64,
  pub updated_at: String, // ISO UTC
}

// Constants
pub const ANALYTICS_PREFIX: &str = "analytics/daily/"; // hardcoded, protected from user operations

// Helper to create a standard NotImplemented error
pub fn err_not_implemented(msg: &str) -> SpError {
  SpError {
    kind: ErrorKind::NotImplemented,
    message: msg.to_string(),
    retry_after_ms: None,
    context: None,
    at: chrono::Utc::now().timestamp_millis(),
  }
}

