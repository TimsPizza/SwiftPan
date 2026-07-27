//! Tauri-independent file upload execution engine.
//!
//! This module owns reading a local file, writing chunks through OpenDAL,
//! pause/cancel polling, remote finalization, and cancellation cleanup. Its
//! boundary is an injected [`Operator`] plus observer callbacks. It must not
//! construct credentials, access global runtime state, emit Tauri events,
//! inspect application settings, or generate thumbnails.

use super::{now_ms, open_upload_writer};
use crate::types::{ErrorKind, SpError, SpResult};
use opendal::Operator;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::io::AsyncReadExt;

pub(crate) struct UploadEngineRequest {
    pub(crate) key: String,
    pub(crate) source_path: PathBuf,
    pub(crate) part_size: u64,
    pub(crate) content_type: Option<String>,
    pub(crate) content_disposition: Option<String>,
}

pub(crate) struct UploadControl {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

pub(crate) trait UploadEngineObserver {
    fn uploading(&mut self) -> SpResult<()>;
    fn paused(&mut self) -> SpResult<()>;
    fn resumed(&mut self) -> SpResult<()>;
    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()>;
    fn finalizing(&mut self) -> SpResult<()>;
    fn cancelled(&mut self) -> SpResult<()>;
}

pub(crate) async fn upload_file(
    operator: &Operator,
    request: UploadEngineRequest,
    control: UploadControl,
    observer: &mut impl UploadEngineObserver,
) -> SpResult<()> {
    if request.part_size == 0 {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "upload part size must be greater than zero".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }

    let mut file = tokio::fs::File::open(&request.source_path)
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open src: {error}"),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
    file.metadata().await.map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("stat src: {error}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;

    let mut writer = open_upload_writer(
        operator,
        &request.key,
        request.content_type.as_deref(),
        request.content_disposition.as_deref(),
    )
    .await
    .map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("open writer: {error}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })?;
    observer.uploading()?;

    let mut part_number = 1;
    let mut was_paused = false;
    loop {
        if control.cancelled.load(Ordering::Relaxed) {
            break;
        }
        while control.paused.load(Ordering::Relaxed) {
            if !was_paused {
                observer.paused()?;
                was_paused = true;
            }
            if control.cancelled.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if control.cancelled.load(Ordering::Relaxed) {
            break;
        }
        if was_paused {
            observer.resumed()?;
            was_paused = false;
        }

        let mut buffer = vec![0; request.part_size as usize];
        let read = file.read(&mut buffer).await.map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("read src: {error}"),
            retry_after_ms: Some(200),
            context: None,
            at: now_ms(),
        })?;
        if read == 0 {
            break;
        }
        buffer.truncate(read);
        writer.write(buffer).await.map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("writer write: {error}"),
            retry_after_ms: Some(300),
            context: None,
            at: now_ms(),
        })?;
        observer.part_done(part_number, read as u64)?;
        part_number += 1;
    }

    if control.cancelled.load(Ordering::Relaxed) {
        let _ = writer.abort().await;
        observer.cancelled()?;
        return Err(cancelled_error());
    }

    observer.finalizing()?;
    writer.close().await.map(|_| ()).map_err(|error| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("writer close: {error}"),
        retry_after_ms: Some(300),
        context: None,
        at: now_ms(),
    })
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
