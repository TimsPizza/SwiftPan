//! Tauri-independent download execution engine.
//!
//! This module owns remote metadata reads, ranged object reads, staged-file
//! writes, pause/cancel polling, and final staged-file rename. Its boundary is
//! an OpenDAL [`Operator`] plus observer callbacks. It must not construct R2
//! credentials, access the global transfer registry, write SQLite snapshots,
//! emit Tauri events, or materialize Android SAF targets.

use super::{next_download_range, now_ms, part_path_for};
use crate::types::{err_invalid, ErrorKind, SpError, SpResult};
use opendal::Operator;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::io::AsyncWriteExt;

pub(crate) struct DownloadEngineRequest {
    pub(crate) key: String,
    pub(crate) temp_path: PathBuf,
    pub(crate) chunk_size: u64,
    pub(crate) expected_etag: Option<String>,
    pub(crate) recorded_bytes_done: u64,
}

pub(crate) struct DownloadControl {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

#[derive(Debug)]
pub(crate) struct DownloadEngineOutput {
    pub(crate) total: u64,
}

pub(crate) trait DownloadEngineObserver {
    fn remote_metadata(&mut self, total: u64, observed_etag: Option<&str>) -> SpResult<()>;
    fn source_changed(&mut self) -> SpResult<()>;
    fn download_started(&mut self, offset: u64) -> SpResult<()>;
    fn paused(&mut self) -> SpResult<()>;
    fn resumed(&mut self) -> SpResult<()>;
    fn chunk_done(&mut self, range_start: u64, len: u64, offset: u64) -> SpResult<()>;
    fn cancelled(&mut self) -> SpResult<()>;
}

pub(crate) async fn download_to_stage(
    operator: &Operator,
    request: DownloadEngineRequest,
    control: DownloadControl,
    observer: &mut impl DownloadEngineObserver,
) -> SpResult<DownloadEngineOutput> {
    let head = operator.stat(&request.key).await.map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("Stat: {error}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })?;
    let total = head.content_length();
    let observed_etag = head.etag().map(str::to_string);
    observer.remote_metadata(total, observed_etag.as_deref())?;

    if let Some(expected) = request.expected_etag.as_ref() {
        if observed_etag.as_ref() != Some(expected) {
            observer.source_changed()?;
            return Err(SpError {
                kind: ErrorKind::SourceChanged,
                message: match observed_etag {
                    Some(_) => "ETag mismatch".into(),
                    None => "remote source omitted the required ETag".into(),
                },
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            });
        }
    }

    if let Some(parent) = request.temp_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create parent dir: {error}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
    }

    let part_path = part_path_for(&request.temp_path);
    let finished_local = match tokio::fs::metadata(&request.temp_path).await {
        Ok(metadata) => {
            metadata.len() == total && request.recorded_bytes_done == total && total > 0
        }
        Err(_) => false,
    };

    if !finished_local {
        let mut offset = match tokio::fs::metadata(&part_path).await {
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        };
        if offset > total {
            let _ = tokio::fs::remove_file(&part_path).await;
            offset = 0;
        }
        // A full-length partial file is not proof that its bytes belong to the
        // current object. Without a committed final rename, restart the last
        // step instead of publishing unverified data.
        if total > 0 && offset == total {
            let _ = tokio::fs::remove_file(&part_path).await;
            offset = 0;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
            .await
            .map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("open temp: {error}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;

        observer.download_started(offset)?;

        let mut was_paused = false;
        while offset < total {
            if control.cancelled.load(Ordering::Relaxed) {
                return cancel_download(&part_path, observer).await;
            }
            while control.paused.load(Ordering::Relaxed) {
                if !was_paused {
                    observer.paused()?;
                    was_paused = true;
                }
                if control.cancelled.load(Ordering::Relaxed) {
                    return cancel_download(&part_path, observer).await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            if was_paused {
                observer.resumed()?;
                was_paused = false;
            }

            let range = next_download_range(offset, total, request.chunk_size)
                .ok_or_else(|| err_invalid("invalid download range"))?;
            let range_start = range.start;
            let data = operator
                .read_with(&request.key)
                .range(range)
                .await
                .map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("GetObject range: {error}"),
                    retry_after_ms: Some(500),
                    context: None,
                    at: now_ms(),
                })?;
            if data.is_empty() {
                return Err(SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("unexpected EOF at byte {offset} of {total}"),
                    retry_after_ms: Some(500),
                    context: None,
                    at: now_ms(),
                });
            }
            let chunk_bytes = data.to_bytes();
            file.write_all(&chunk_bytes)
                .await
                .map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("write: {error}"),
                    retry_after_ms: Some(300),
                    context: None,
                    at: now_ms(),
                })?;
            offset = offset.saturating_add(chunk_bytes.len() as u64);
            observer.chunk_done(range_start, chunk_bytes.len() as u64, offset)?;
        }

        file.flush().await.ok();
        if control.cancelled.load(Ordering::Relaxed) {
            return cancel_download(&part_path, observer).await;
        }
        tokio::fs::rename(&part_path, &request.temp_path)
            .await
            .map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("rename: {error}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
    }

    Ok(DownloadEngineOutput { total })
}

async fn cancel_download(
    part_path: &std::path::Path,
    observer: &mut impl DownloadEngineObserver,
) -> SpResult<DownloadEngineOutput> {
    let _ = tokio::fs::remove_file(part_path).await;
    observer.cancelled()?;
    Err(cancelled_error())
}

pub(super) fn cancelled_error() -> SpError {
    SpError {
        kind: ErrorKind::Cancelled,
        message: "cancelled".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}
