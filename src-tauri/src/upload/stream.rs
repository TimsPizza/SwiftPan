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
    pub(super) expected_bytes: u64,
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
    let mut bytes_received = 0u64;
    let mut explicitly_finished = false;
    while let Some(message) = receiver.recv().await {
        if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        while control.paused.load(std::sync::atomic::Ordering::Relaxed) {
            if !was_paused {
                observer.paused()?;
                was_paused = true;
            }
            if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if was_paused {
            observer.resumed()?;
            was_paused = false;
        }

        match message {
            Some(bytes) => {
                let len = bytes.len() as u64;
                if len == 0 {
                    continue;
                }
                let next_total = match bytes_received.checked_add(len) {
                    Some(total) if total <= request.expected_bytes => total,
                    _ => {
                        let _ = writer.abort().await;
                        return Err(stream_protocol_error(format!(
                            "stream exceeds declared length of {} bytes",
                            request.expected_bytes
                        )));
                    }
                };
                writer.write(bytes).await.map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("writer write: {error}"),
                    retry_after_ms: Some(300),
                    context: None,
                    at: now_ms(),
                })?;
                bytes_received = next_total;
                observer.part_done(part_number, len)?;
                part_number += 1;
            }
            None => {
                explicitly_finished = true;
                break;
            }
        }
    }

    if control.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = writer.abort().await;
        observer.cancelled()?;
        return Err(cancelled_error());
    }
    if !explicitly_finished {
        let _ = writer.abort().await;
        return Err(stream_protocol_error(
            "stream sender disconnected before explicit finish",
        ));
    }
    if bytes_received != request.expected_bytes {
        let _ = writer.abort().await;
        return Err(stream_protocol_error(format!(
            "stream ended after {bytes_received} bytes; expected {}",
            request.expected_bytes
        )));
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

fn stream_protocol_error(message: impl Into<String>) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: message.into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}
