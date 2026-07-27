//! Public facade and application-layer orchestration for downloads.
//!
//! This module owns the API called by the Tauri bridge, task spawning, event
//! emission, and coordination between the engine, runtime, and platform
//! adapters. It must not contain range I/O, target-path rules, or persistence
//! implementation details; those belong to the dedicated child modules.

use crate::transfer_db::{self, TransferKind, TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::TransferStateEvent;
use crate::types::*;
use crate::usage::UsageSync;
use crate::{r2_client, sp_backend::SpBackend};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Emitter;

mod engine;
mod platform;
mod policy;
mod runtime;
mod target;

use engine::*;
use platform::*;
use policy::*;
use runtime::*;
use target::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDownloadParams {
    pub key: String,
    pub dest_path: Option<String>,
    pub chunk_size: u64,
    pub expected_etag: Option<String>,
    pub android_tree_uri: Option<String>,
    pub android_relative_path: Option<String>,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatus {
    pub transfer_id: String,
    pub key: String,
    pub lifecycle_state: TransferLifecycle,
    pub phase: Option<TransferPhase>,
    pub bytes_total: Option<u64>,
    pub bytes_done: u64,
    pub rate_bps: u64,
    pub expected_etag: Option<String>,
    pub observed_etag: Option<String>,
    pub temp_path: Option<String>,
    pub last_error: Option<SpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DownloadEvent {
    Started {
        transfer_id: String,
    },
    ChunkProgress {
        transfer_id: String,
        progress: DownloadChunkProgress,
    },
    ChunkDone {
        transfer_id: String,
        range_start: u64,
        len: u64,
    },
    Paused {
        transfer_id: String,
    },
    Resumed {
        transfer_id: String,
    },
    Cancelling {
        transfer_id: String,
    },
    Completed {
        transfer_id: String,
    },
    Failed {
        transfer_id: String,
        error: SpError,
    },
    Cancelled {
        transfer_id: String,
    },
    SourceChanged {
        transfer_id: String,
    },
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn emit_download(app: &tauri::AppHandle, ev: &DownloadEvent) {
    let _ = app.emit("sp://download_event", ev);
}

async fn cleanup_download_artifacts(temp_path: &Path) {
    let _ = tokio::fs::remove_file(part_path_for(temp_path)).await;
    let _ = tokio::fs::remove_file(temp_path).await;
}

fn cleanup_download_artifacts_sync(temp_path: &Path) {
    let _ = std::fs::remove_file(part_path_for(temp_path));
    let _ = std::fs::remove_file(temp_path);
}

fn download_status_from_snapshot(snapshot: TransferSnapshot) -> DownloadStatus {
    DownloadStatus {
        transfer_id: snapshot.transfer_id,
        key: snapshot.key,
        lifecycle_state: snapshot.lifecycle_state,
        phase: snapshot.phase,
        bytes_total: snapshot.bytes_total,
        bytes_done: snapshot.bytes_done,
        rate_bps: snapshot.rate_bps,
        expected_etag: snapshot.expected_etag,
        observed_etag: snapshot.observed_etag,
        temp_path: snapshot.temp_path,
        last_error: snapshot.last_error,
    }
}

fn spawn_download_task(app: tauri::AppHandle, transfer_id: String, recovered: bool) {
    let _ = mutate_transfer(&transfer_id, |t| {
        t.worker_active = true;
    });
    tokio::spawn(async move {
        let res = run_download(&app, &transfer_id, recovered).await;
        if let Err(e) = res {
            let cleanup_reason = e.kind.clone();
            let _ = mutate_transfer(&transfer_id, |t| {
                t.worker_active = false;
                t.last_error = Some(e.clone());
            });
            match e.kind {
                ErrorKind::Cancelled => {}
                _ => {
                    if !should_keep_failed_artifacts(Some(&cleanup_reason)) {
                        if let Ok((_, _, temp_path, _, _, _, _, _)) =
                            load_runtime_fields(&transfer_id)
                        {
                            cleanup_download_artifacts(&temp_path).await;
                        }
                    }
                    let _ = transition_transfer(&transfer_id, TransferStateEvent::Fail);
                    emit_download(
                        &app,
                        &DownloadEvent::Failed {
                            transfer_id: transfer_id.clone(),
                            error: e,
                        },
                    )
                }
            }
        } else {
            let _ = mutate_transfer(&transfer_id, |t| {
                t.worker_active = false;
            });
        }
    });
}

pub fn init(app: &tauri::AppHandle) -> SpResult<()> {
    transfer_db::init(app)?;
    for snapshot in transfer_db::list_all_snapshots()? {
        if snapshot.kind != TransferKind::Download {
            continue;
        }
        if !matches!(snapshot.lifecycle_state, TransferLifecycle::Failed) {
            continue;
        }
        if should_keep_failed_artifacts(snapshot.last_fail_reason.as_ref()) {
            continue;
        }
        if let Some(temp_path) = snapshot.temp_path.as_deref() {
            cleanup_download_artifacts_sync(Path::new(temp_path));
        }
    }
    let snapshots = transfer_db::list_active_snapshots()?;
    for snapshot in snapshots {
        if snapshot.kind != TransferKind::Download {
            continue;
        }
        let target = DownloadTarget::from_snapshot(&snapshot)?;
        let temp_path = snapshot
            .temp_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or(target.temp_path_for(&snapshot.transfer_id, &snapshot.key)?);
        let pause_on_recover = matches!(
            snapshot.lifecycle_state,
            TransferLifecycle::Queued | TransferLifecycle::Running
        );
        let paused = Arc::new(AtomicBool::new(
            matches!(snapshot.lifecycle_state, TransferLifecycle::Paused) || pause_on_recover,
        ));
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut g = DL.lock().map_err(|_| SpError {
                kind: ErrorKind::NotRetriable,
                message: "download state lock poisoned".into(),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
            if g.contains_key(&snapshot.transfer_id) {
                continue;
            }
            g.insert(
                snapshot.transfer_id.clone(),
                Transfer {
                    key: snapshot.key.clone(),
                    target,
                    temp_path,
                    chunk: 4 * 1024 * 1024,
                    expected_etag: snapshot.expected_etag.clone(),
                    observed_etag: snapshot.observed_etag.clone(),
                    bytes_total: snapshot.bytes_total,
                    bytes_done: snapshot.bytes_done,
                    last_error: snapshot.last_error.clone(),
                    paused,
                    cancelled,
                    worker_active: false,
                    lifecycle_state: lifecycle_after_restart(&snapshot.lifecycle_state),
                    phase: snapshot.phase.clone(),
                    created_at_ms: snapshot.created_at_ms,
                    updated_at_ms: now_ms(),
                },
            );
        }
        if matches!(snapshot.lifecycle_state, TransferLifecycle::Paused) {
            continue;
        }
        if pause_on_recover {
            crate::logger::warn(
                "download",
                &format!(
                    "recovered interrupted download {} as paused; explicit resume required",
                    snapshot.transfer_id
                ),
            );
            continue;
        }
        if matches!(snapshot.lifecycle_state, TransferLifecycle::Cancelling) {
            continue;
        }
        if !snapshot.lifecycle_state.is_terminal() {
            spawn_download_task(app.clone(), snapshot.transfer_id.clone(), true);
        }
    }
    Ok(())
}

pub async fn start_download(app: tauri::AppHandle, params: NewDownloadParams) -> SpResult<String> {
    let target = DownloadTarget::from_params(&params)?;
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let temp_path = target.temp_path_for(&id, &params.key)?;
    {
        let mut g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let duplicate = g
            .values()
            .any(|t| t.key == params.key && !t.lifecycle_state.is_terminal());
        if duplicate {
            return Err(SpError {
                kind: ErrorKind::TaskExists,
                message: "download with same key already exists".into(),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            });
        }
        g.insert(
            id.clone(),
            Transfer {
                key: params.key.clone(),
                target,
                temp_path,
                chunk: params.chunk_size.max(1024 * 1024),
                expected_etag: params.expected_etag.clone(),
                observed_etag: None,
                bytes_total: None,
                bytes_done: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
                worker_active: false,
                lifecycle_state: TransferLifecycle::Queued,
                phase: Some(TransferPhase::PreparingTarget),
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            },
        );
    }
    persist_transfer(&id)?;
    spawn_download_task(app, id.clone(), false);
    Ok(id)
}

async fn run_download(app: &tauri::AppHandle, id: &str, recovered: bool) -> SpResult<()> {
    let (key, target, temp_path, chunk, expected_etag, bytes_done, paused, cancelled) =
        load_runtime_fields(id)?;
    let entry_phase = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let t = g.get(id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        if recovered {
            t.phase.unwrap_or(TransferPhase::PreparingTarget)
        } else {
            TransferPhase::PreparingTarget
        }
    };
    let start_event = if recovered {
        DownloadEvent::Resumed {
            transfer_id: id.to_string(),
        }
    } else {
        DownloadEvent::Started {
            transfer_id: id.to_string(),
        }
    };
    let _ = transition_transfer(id, TransferStateEvent::Run(entry_phase));
    let _ = mutate_transfer(id, |t| {
        t.last_error = None;
    });
    emit_download(app, &start_event);

    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let mut observer = RuntimeDownloadObserver { app, id };
    let output = download_to_stage(
        &client.op,
        DownloadEngineRequest {
            key,
            temp_path: temp_path.clone(),
            chunk_size: chunk,
            expected_etag,
            recorded_bytes_done: bytes_done,
        },
        DownloadControl { paused, cancelled },
        &mut observer,
    )
    .await?;

    materialize_target(app, id, &target, &temp_path).await?;
    transition_transfer(id, TransferStateEvent::Complete)?;
    mutate_transfer(id, |t| {
        t.bytes_done = output.total;
    })?;
    emit_download(
        app,
        &DownloadEvent::Completed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

struct RuntimeDownloadObserver<'a> {
    app: &'a tauri::AppHandle,
    id: &'a str,
}

impl DownloadEngineObserver for RuntimeDownloadObserver<'_> {
    fn remote_metadata(&mut self, total: u64, observed_etag: Option<&str>) -> SpResult<()> {
        let mut class_b = std::collections::HashMap::new();
        class_b.insert("HeadObject".into(), 1u64);
        let _ = UsageSync::record_local_delta(UsageDelta {
            class_a: Default::default(),
            class_b,
            ingress_bytes: 0,
            egress_bytes: 0,
            added_storage_bytes: 0,
            deleted_storage_bytes: 0,
        });
        mutate_transfer(self.id, |transfer| {
            transfer.bytes_total = Some(total);
            transfer.observed_etag = observed_etag.map(str::to_string);
            if total > 0 && transfer.bytes_done > total {
                transfer.bytes_done = 0;
            }
        })
    }

    fn source_changed(&mut self) -> SpResult<()> {
        emit_download(
            self.app,
            &DownloadEvent::SourceChanged {
                transfer_id: self.id.to_string(),
            },
        );
        Ok(())
    }

    fn download_started(&mut self, offset: u64) -> SpResult<()> {
        transition_transfer(
            self.id,
            TransferStateEvent::Run(TransferPhase::DownloadingRemote),
        )?;
        mutate_transfer(self.id, |transfer| {
            transfer.bytes_done = offset;
        })
    }

    fn paused(&mut self) -> SpResult<()> {
        transition_transfer(self.id, TransferStateEvent::Pause)?;
        emit_download(
            self.app,
            &DownloadEvent::Paused {
                transfer_id: self.id.to_string(),
            },
        );
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        transition_transfer(
            self.id,
            TransferStateEvent::Run(TransferPhase::DownloadingRemote),
        )?;
        emit_download(
            self.app,
            &DownloadEvent::Resumed {
                transfer_id: self.id.to_string(),
            },
        );
        Ok(())
    }

    fn chunk_done(&mut self, range_start: u64, len: u64, offset: u64) -> SpResult<()> {
        let mut class_b = std::collections::HashMap::new();
        class_b.insert("GetObject".into(), 1u64);
        let _ = UsageSync::record_local_delta(UsageDelta {
            class_a: Default::default(),
            class_b,
            ingress_bytes: 0,
            egress_bytes: len,
            added_storage_bytes: 0,
            deleted_storage_bytes: 0,
        });
        mutate_transfer(self.id, |transfer| {
            transfer.bytes_done = offset;
        })?;
        emit_download(
            self.app,
            &DownloadEvent::ChunkDone {
                transfer_id: self.id.to_string(),
                range_start,
                len,
            },
        );
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        let _ = transition_transfer(self.id, TransferStateEvent::CancelConfirm);
        mutate_transfer(self.id, |transfer| {
            transfer.last_error = Some(cancelled_error());
        })?;
        emit_download(
            self.app,
            &DownloadEvent::Cancelled {
                transfer_id: self.id.to_string(),
            },
        );
        Ok(())
    }
}

pub fn pause(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    if let Some(t) = g.get(transfer_id) {
        t.paused.store(true, Ordering::Relaxed);
    } else {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    drop(g);
    transition_transfer(transfer_id, TransferStateEvent::Pause)?;
    emit_download(
        app,
        &DownloadEvent::Paused {
            transfer_id: transfer_id.to_string(),
        },
    );
    Ok(())
}

pub fn resume(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let should_spawn = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let t = g.get(transfer_id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        t.paused.store(false, Ordering::Relaxed);
        matches!(t.lifecycle_state, TransferLifecycle::Paused) && !t.worker_active
    };
    let phase = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let t = g.get(transfer_id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        t.phase
            .ok_or_else(|| err_invalid("paused download missing phase"))?
    };
    let target = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let t = g.get(transfer_id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        t.target.clone()
    };
    ensure_resume_target_access(app, &target)?;
    transition_transfer(transfer_id, TransferStateEvent::Run(phase))?;
    emit_download(
        app,
        &DownloadEvent::Resumed {
            transfer_id: transfer_id.to_string(),
        },
    );
    if should_spawn {
        spawn_download_task(app.clone(), transfer_id.to_string(), true);
    }
    Ok(())
}

pub fn cancel(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    if let Some(t) = g.get(transfer_id) {
        t.cancelled.store(true, Ordering::Relaxed);
    } else {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    drop(g);
    transition_transfer(transfer_id, TransferStateEvent::CancelRequest)?;
    emit_download(
        app,
        &DownloadEvent::Cancelling {
            transfer_id: transfer_id.to_string(),
        },
    );
    Ok(())
}

pub fn status(transfer_id: &str) -> SpResult<DownloadStatus> {
    if let Ok(g) = DL.lock() {
        if let Some(t) = g.get(transfer_id) {
            return Ok(download_status_from_snapshot(snapshot_from_transfer(
                transfer_id,
                t,
            )));
        }
    }
    if let Some(snapshot) = transfer_db::get_snapshot(transfer_id)? {
        return Ok(download_status_from_snapshot(snapshot));
    }
    Err(SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })
}

pub fn list_active_snapshots() -> SpResult<Vec<TransferSnapshot>> {
    let persisted = transfer_db::list_active_snapshots()?;
    let runtime = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        g.iter()
            .filter_map(|(id, t)| {
                if t.lifecycle_state.is_terminal() {
                    return None;
                }
                Some(snapshot_from_transfer(id, t))
            })
            .collect::<Vec<_>>()
    };
    if runtime.is_empty() {
        return Ok(persisted);
    }
    let mut merged = std::collections::HashMap::new();
    for snapshot in persisted {
        merged.insert(snapshot.transfer_id.clone(), snapshot);
    }
    for snapshot in runtime {
        merged.insert(snapshot.transfer_id.clone(), snapshot);
    }
    let mut items = merged.into_values().collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(items)
}

pub fn list_snapshots() -> SpResult<Vec<TransferSnapshot>> {
    let persisted = transfer_db::list_all_snapshots()?;
    let runtime = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        g.iter()
            .map(|(id, t)| snapshot_from_transfer(id, t))
            .collect::<Vec<_>>()
    };
    let mut merged = std::collections::HashMap::new();
    for snapshot in persisted {
        merged.insert(snapshot.transfer_id.clone(), snapshot);
    }
    for snapshot in runtime {
        merged.insert(snapshot.transfer_id.clone(), snapshot);
    }
    let mut items = merged.into_values().collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(items)
}

pub fn remove(transfer_id: &str) -> SpResult<()> {
    {
        let mut g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        if let Some(t) = g.get(transfer_id) {
            if !t.lifecycle_state.is_terminal() {
                return Err(err_invalid("cannot remove active download"));
            }
        }
        g.remove(transfer_id);
    }
    transfer_db::delete_snapshot(transfer_id)
}

#[cfg(test)]
mod tests;
