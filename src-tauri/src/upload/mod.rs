use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUploadParams {
  pub key: String,
  pub source_path: String,
  pub part_size: u64,
  pub content_type: Option<String>,
  pub content_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatus {
  pub transfer_id: String,
  pub key: String,
  pub bytes_total: u64,
  pub bytes_done: u64,
  pub parts_completed: u32,
  pub rate_bps: u64,
  pub eta_ms: Option<u64>,
  pub last_error: Option<SpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UploadEvent {
  Started { transfer_id: String },
  PartProgress { transfer_id: String, progress: UploadPartProgress },
  PartDone { transfer_id: String, part_number: u32, etag: String },
  Paused { transfer_id: String },
  Resumed { transfer_id: String },
  Completed { transfer_id: String },
  Failed { transfer_id: String, error: SpError },
}

pub struct UploadEngine;

impl UploadEngine {
  pub fn new(_params: NewUploadParams) -> SpResult<Self> { Err(err_not_implemented("upload.new")) }
  pub fn start(&self) -> SpResult<()> { Err(err_not_implemented("upload.start")) }
  pub fn pause(&self) -> SpResult<()> { Err(err_not_implemented("upload.pause")) }
  pub fn resume(&self) -> SpResult<()> { Err(err_not_implemented("upload.resume")) }
  pub fn cancel(&self) -> SpResult<()> { Err(err_not_implemented("upload.cancel")) }
  pub fn status(&self) -> SpResult<UploadStatus> { Err(err_not_implemented("upload.status")) }
}

