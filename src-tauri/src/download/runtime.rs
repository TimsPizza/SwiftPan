//! In-process download state and persistence adapter.
//!
//! This module owns the global transfer registry, mutation helpers, FSM
//! transitions, snapshot conversion, and SQLite persistence calls. It does not
//! perform remote or local file I/O, emit UI events, or decide platform target
//! behavior. Keeping those concerns out makes runtime state replaceable in
//! future process-recovery tests.

use super::{last_fail_reason_for, now_ms, DownloadTarget};
use crate::transfer_db::{self, TransferKind, TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::{apply_transfer_event, TransferState, TransferStateEvent};
use crate::types::{ErrorKind, SpError, SpResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

pub(super) struct Transfer {
    pub(super) key: String,
    pub(super) target: DownloadTarget,
    pub(super) temp_path: PathBuf,
    pub(super) chunk: u64,
    pub(super) expected_etag: Option<String>,
    pub(super) observed_etag: Option<String>,
    pub(super) bytes_total: Option<u64>,
    pub(super) bytes_done: u64,
    pub(super) last_error: Option<SpError>,
    pub(super) paused: Arc<AtomicBool>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) worker_active: bool,
    pub(super) lifecycle_state: TransferLifecycle,
    pub(super) phase: Option<TransferPhase>,
    pub(super) created_at_ms: i64,
    pub(super) updated_at_ms: i64,
}

pub(super) static DL: Lazy<Mutex<HashMap<String, Transfer>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn snapshot_from_transfer(id: &str, transfer: &Transfer) -> TransferSnapshot {
    let (dest_path, android_tree_uri, android_relative_path) = transfer.target.snapshot_fields();
    TransferSnapshot {
        transfer_id: id.to_string(),
        kind: TransferKind::Download,
        key: transfer.key.clone(),
        lifecycle_state: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
        bytes_total: transfer.bytes_total,
        bytes_done: transfer.bytes_done,
        rate_bps: 0,
        last_error: transfer.last_error.clone(),
        last_fail_reason: last_fail_reason_for(
            transfer.lifecycle_state.clone(),
            transfer.last_error.as_ref(),
        ),
        dest_path,
        android_tree_uri,
        android_relative_path,
        temp_path: Some(transfer.temp_path.to_string_lossy().to_string()),
        expected_etag: transfer.expected_etag.clone(),
        observed_etag: transfer.observed_etag.clone(),
        created_at_ms: transfer.created_at_ms,
        updated_at_ms: transfer.updated_at_ms,
    }
}

fn state_from_transfer(transfer: &Transfer) -> TransferState {
    TransferState {
        lifecycle: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
    }
}

pub(super) fn persist_transfer(id: &str) -> SpResult<()> {
    let snapshot = {
        let runtime = DL.lock().map_err(|_| runtime_lock_error())?;
        let transfer = runtime.get(id).ok_or_else(download_not_found)?;
        snapshot_from_transfer(id, transfer)
    };
    transfer_db::upsert_snapshot(&snapshot)
}

pub(super) fn mutate_transfer<F>(id: &str, mutate: F) -> SpResult<()>
where
    F: FnOnce(&mut Transfer),
{
    {
        let mut runtime = DL.lock().map_err(|_| runtime_lock_error())?;
        let transfer = runtime.get_mut(id).ok_or_else(download_not_found)?;
        mutate(transfer);
        transfer.updated_at_ms = now_ms();
    }
    persist_transfer(id)
}

pub(super) fn transition_transfer(id: &str, event: TransferStateEvent) -> SpResult<TransferState> {
    let next_state = {
        let mut runtime = DL.lock().map_err(|_| runtime_lock_error())?;
        let transfer = runtime.get_mut(id).ok_or_else(download_not_found)?;
        let next = apply_transfer_event(
            TransferKind::Download,
            &state_from_transfer(transfer),
            event,
        )?;
        transfer.lifecycle_state = next.lifecycle.clone();
        transfer.phase = next.phase;
        transfer.updated_at_ms = now_ms();
        next
    };
    persist_transfer(id)?;
    Ok(next_state)
}

pub(super) type RuntimeFields = (
    String,
    DownloadTarget,
    PathBuf,
    u64,
    Option<String>,
    u64,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
);

pub(super) fn load_runtime_fields(id: &str) -> SpResult<RuntimeFields> {
    let runtime = DL.lock().map_err(|_| runtime_lock_error())?;
    let transfer = runtime.get(id).ok_or_else(download_not_found)?;
    Ok((
        transfer.key.clone(),
        transfer.target.clone(),
        transfer.temp_path.clone(),
        transfer.chunk,
        transfer.expected_etag.clone(),
        transfer.bytes_done,
        transfer.paused.clone(),
        transfer.cancelled.clone(),
    ))
}

fn runtime_lock_error() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}

fn download_not_found() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "download not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    }
}
