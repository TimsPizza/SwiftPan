//! Tauri-independent push-stream upload execution.
//!
//! This module owns consuming caller-provided byte chunks, writing them through
//! OpenDAL, and honoring pause/cancel controls. It does not own the global
//! channel registry, construct credentials, read application settings, emit
//! Tauri events, or claim process-restart recovery for an ephemeral stream.

use super::{cancelled_error, now_ms, open_upload_writer, UploadControl};
use crate::types::{ErrorKind, SpError, SpResult};
use opendal::Operator;
use tokio::sync::mpsc;

pub(super) struct StreamUploadRequest {
    pub(super) key: String,
    pub(super) content_type: Option<String>,
    pub(super) content_disposition: Option<String>,
}

pub(super) trait StreamUploadObserver {
    fn uploading(&mut self) -> SpResult<()>;
    fn paused(&mut self) -> SpResult<()>;
    fn resumed(&mut self) -> SpResult<()>;
    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()>;
    fn finalizing(&mut self) -> SpResult<()>;
    fn cancelled(&mut self) -> SpResult<()>;
}

pub(super) async fn upload_stream(
    operator: &Operator,
    request: StreamUploadRequest,
    mut receiver: mpsc::Receiver<Option<Vec<u8>>>,
    control: UploadControl,
    observer: &mut impl StreamUploadObserver,
) -> SpResult<()> {
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

    let mut was_paused = false;
    let mut part_number = 1;
    while let Some(message) = receiver.recv().await {
        if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        while control.paused.load(std::sync::atomic::Ordering::Relaxed) {
            if !was_paused {
                observer.paused()?;
                was_paused = true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if was_paused {
            observer.resumed()?;
            was_paused = false;
        }

        match message {
            Some(bytes) => {
                let len = bytes.len() as u64;
                writer.write(bytes).await.map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("writer write: {error}"),
                    retry_after_ms: Some(300),
                    context: None,
                    at: now_ms(),
                })?;
                observer.part_done(part_number, len)?;
                part_number += 1;
            }
            None => break,
        }
    }

    if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
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
