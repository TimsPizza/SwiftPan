use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDownloadParams {
  pub key: String,
  pub dest_path: String,
  pub chunk_size: u64,
  pub expected_etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatus {
  pub transfer_id: String,
  pub key: String,
  pub bytes_total: Option<u64>,
  pub bytes_done: u64,
  pub rate_bps: u64,
  pub expected_etag: Option<String>,
  pub observed_etag: Option<String>,
  pub last_error: Option<SpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DownloadEvent {
  Started { transfer_id: String },
  ChunkProgress { transfer_id: String, progress: DownloadChunkProgress },
  ChunkDone { transfer_id: String, range_start: u64, len: u64 },
  Paused { transfer_id: String },
  Resumed { transfer_id: String },
  Completed { transfer_id: String },
  Failed { transfer_id: String, error: SpError },
  SourceChanged { transfer_id: String },
}

pub struct DownloadEngine;

impl DownloadEngine {
  pub fn new(_params: NewDownloadParams) -> SpResult<Self> { Err(err_not_implemented("download.new")) }
  pub fn start(&self) -> SpResult<()> { Err(err_not_implemented("download.start")) }
  pub fn pause(&self) -> SpResult<()> { Err(err_not_implemented("download.pause")) }
  pub fn resume(&self) -> SpResult<()> { Err(err_not_implemented("download.resume")) }
  pub fn cancel(&self) -> SpResult<()> { Err(err_not_implemented("download.cancel")) }
  pub fn status(&self) -> SpResult<DownloadStatus> { Err(err_not_implemented("download.status")) }
}

