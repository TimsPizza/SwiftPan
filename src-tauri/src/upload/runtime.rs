//! In-process upload state and stream-channel registry.
//!
//! This module owns the global transfer table, FSM transitions, snapshot
//! conversion, progress mutation, and streaming channel lookup. It must not
//! open local sources, write remote objects, construct credentials, generate
//! thumbnails, or emit Tauri events.

use super::{now_ms, UploadStatus};
use crate::transfer_db::{TransferKind, TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::{apply_transfer_event, TransferState, TransferStateEvent};
use crate::types::{err_invalid, ErrorKind, SpError, SpResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tokio::sync::mpsc;

pub(super) struct UploadTransfer {
    pub(super) key: String,
    pub(super) src: PathBuf,
    pub(super) part_size: u64,
    pub(super) bytes_total: u64,
    pub(super) bytes_done: u64,
    pub(super) parts_completed: u32,
    pub(super) last_error: Option<SpError>,
    pub(super) paused: Arc<AtomicBool>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) worker_active: bool,
    pub(super) lifecycle_state: TransferLifecycle,
    pub(super) phase: Option<TransferPhase>,
    pub(super) created_at_ms: i64,
    pub(super) updated_at_ms: i64,
}

pub(super) static UPLOADS: Lazy<Mutex<HashMap<String, UploadTransfer>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static STREAMS: Lazy<Mutex<HashMap<String, mpsc::Sender<Option<Vec<u8>>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn register_upload(
    id: &str,
    key: String,
    source_path: PathBuf,
    part_size: u64,
    bytes_total: u64,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> SpResult<()> {
    let queued = TransferState::queued(TransferKind::Upload);
    let timestamp = now_ms();
    let transfer = UploadTransfer {
        key,
        src: source_path,
        part_size,
        bytes_total,
        bytes_done: 0,
        parts_completed: 0,
        last_error: None,
        paused,
        cancelled,
        worker_active: false,
        lifecycle_state: queued.lifecycle,
        phase: queued.phase,
        created_at_ms: timestamp,
        updated_at_ms: timestamp,
    };
    UPLOADS
        .lock()
        .map_err(|_| upload_lock_error())?
        .insert(id.to_string(), transfer);
    Ok(())
}

fn state_from_transfer(transfer: &UploadTransfer) -> TransferState {
    TransferState {
        lifecycle: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
    }
}

pub(super) fn mutate_upload<F>(id: &str, mutate: F) -> SpResult<()>
where
    F: FnOnce(&mut UploadTransfer),
{
    let mut uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    let transfer = uploads.get_mut(id).ok_or_else(upload_not_found)?;
    mutate(transfer);
    transfer.updated_at_ms = now_ms();
    Ok(())
}

pub(super) fn transition_upload(id: &str, event: TransferStateEvent) -> SpResult<TransferState> {
    let mut uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    let transfer = uploads.get_mut(id).ok_or_else(upload_not_found)?;
    let next = apply_transfer_event(TransferKind::Upload, &state_from_transfer(transfer), event)?;
    transfer.lifecycle_state = next.lifecycle.clone();
    transfer.phase = next.phase;
    transfer.updated_at_ms = now_ms();
    Ok(next)
}

pub(super) fn snapshot_from_upload(id: &str, transfer: &UploadTransfer) -> TransferSnapshot {
    TransferSnapshot {
        transfer_id: id.to_string(),
        kind: TransferKind::Upload,
        key: transfer.key.clone(),
        lifecycle_state: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
        bytes_total: Some(transfer.bytes_total),
        bytes_done: transfer.bytes_done,
        rate_bps: 0,
        last_error: transfer.last_error.clone(),
        last_fail_reason: if matches!(transfer.lifecycle_state, TransferLifecycle::Failed) {
            transfer.last_error.as_ref().map(|error| error.kind.clone())
        } else {
            None
        },
        dest_path: None,
        android_tree_uri: None,
        android_relative_path: None,
        temp_path: None,
        expected_etag: None,
        observed_etag: None,
        created_at_ms: transfer.created_at_ms,
        updated_at_ms: transfer.updated_at_ms,
    }
}

pub(super) fn register_stream(id: String, sender: mpsc::Sender<Option<Vec<u8>>>) -> SpResult<()> {
    STREAMS
        .lock()
        .map_err(|_| stream_lock_error())?
        .insert(id, sender);
    Ok(())
}

pub(super) fn unregister_stream(id: &str) {
    STREAMS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(id);
}

pub(super) fn stream_write(id: &str, chunk: Vec<u8>) -> SpResult<()> {
    let streams = STREAMS.lock().map_err(|_| stream_lock_error())?;
    streams
        .get(id)
        .ok_or_else(upload_not_found)?
        .try_send(Some(chunk))
        .map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("stream write: {error}"),
            retry_after_ms: Some(100),
            context: None,
            at: now_ms(),
        })
}

pub(super) fn stream_finish(id: &str) -> SpResult<()> {
    let streams = STREAMS.lock().map_err(|_| stream_lock_error())?;
    streams
        .get(id)
        .ok_or_else(upload_not_found)?
        .try_send(None)
        .map_err(|error| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("stream finish: {error}"),
            retry_after_ms: Some(100),
            context: None,
            at: now_ms(),
        })
}

pub(super) fn pause_upload(id: &str) -> SpResult<()> {
    let uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    uploads
        .get(id)
        .ok_or_else(upload_not_found)?
        .paused
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

pub(super) fn resume_upload(id: &str) -> SpResult<TransferPhase> {
    let uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    let transfer = uploads.get(id).ok_or_else(upload_not_found)?;
    transfer
        .paused
        .store(false, std::sync::atomic::Ordering::Relaxed);
    transfer
        .phase
        .ok_or_else(|| err_invalid("paused upload missing phase"))
}

pub(super) fn cancel_upload(id: &str) -> SpResult<()> {
    let uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    uploads
        .get(id)
        .ok_or_else(upload_not_found)?
        .cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

pub(super) fn upload_status(id: &str) -> SpResult<UploadStatus> {
    let uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    let transfer = uploads.get(id).ok_or_else(upload_not_found)?;
    Ok(UploadStatus {
        transfer_id: id.into(),
        key: transfer.key.clone(),
        lifecycle_state: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
        bytes_total: transfer.bytes_total,
        bytes_done: transfer.bytes_done,
        parts_completed: transfer.parts_completed,
        rate_bps: 0,
        eta_ms: None,
        last_error: transfer.last_error.clone(),
    })
}

pub(super) fn list_active_snapshots() -> Vec<TransferSnapshot> {
    let uploads = match UPLOADS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    uploads
        .iter()
        .filter_map(|(id, transfer)| {
            (!transfer.lifecycle_state.is_terminal()).then(|| snapshot_from_upload(id, transfer))
        })
        .collect()
}

pub(super) fn remove_upload(id: &str) -> SpResult<()> {
    let mut uploads = UPLOADS.lock().map_err(|_| upload_lock_error())?;
    if let Some(transfer) = uploads.get(id) {
        if !transfer.lifecycle_state.is_terminal() {
            return Err(err_invalid("cannot remove active upload"));
        }
    }
    uploads.remove(id);
    Ok(())
}

fn upload_lock_error() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}

fn stream_lock_error() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload streams lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}

fn upload_not_found() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}
