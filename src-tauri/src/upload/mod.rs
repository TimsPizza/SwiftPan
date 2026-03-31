use crate::settings;
use crate::transfer_db::{TransferKind, TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::{apply_transfer_event, TransferState, TransferStateEvent};
use crate::types::*;
use crate::{r2_client, sp_backend::SpBackend};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUploadParams {
    pub key: String,
    pub source_path: String,
    pub part_size: u64,
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUploadStreamParams {
    pub key: String,
    pub bytes_total: u64,
    pub part_size: u64,
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatus {
    pub transfer_id: String,
    pub key: String,
    pub lifecycle_state: TransferLifecycle,
    pub phase: Option<TransferPhase>,
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
    Started {
        transfer_id: String,
    },
    PartProgress {
        transfer_id: String,
        progress: UploadPartProgress,
    },
    PartDone {
        transfer_id: String,
        part_number: u32,
        etag: String,
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
}

struct UTransfer {
    key: String,
    src: PathBuf,
    part_size: u64,
    bytes_total: u64,
    bytes_done: u64,
    parts_completed: u32,
    last_error: Option<SpError>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    worker_active: bool,
    lifecycle_state: TransferLifecycle,
    phase: Option<TransferPhase>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

static UL: Lazy<Mutex<HashMap<String, UTransfer>>> = Lazy::new(|| Mutex::new(HashMap::new()));
// Streaming upload channels: id -> sender
static USTREAMS: Lazy<Mutex<HashMap<String, mpsc::Sender<Option<Vec<u8>>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn emit_upload(app: &tauri::AppHandle, ev: &UploadEvent) {
    let _ = app.emit("sp://upload_event", ev);
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(target_os = "android")]
fn android_thumbnail_temp_path(transfer_id: &str, object_key: &str) -> SpResult<PathBuf> {
    let mut dir = crate::sp_backend::vault_dir()?;
    dir.push("uploads");
    dir.push("thumbnail_staging");
    std::fs::create_dir_all(&dir).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create thumbnail staging dir: {e}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let ext = std::path::Path::new(object_key)
        .extension()
        .and_then(|v| v.to_str())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("bin");
    dir.push(format!("{transfer_id}.{ext}"));
    Ok(dir)
}

fn state_from_transfer(transfer: &UTransfer) -> TransferState {
    TransferState {
        lifecycle: transfer.lifecycle_state.clone(),
        phase: transfer.phase,
    }
}

fn mutate_upload<F>(id: &str, f: F) -> SpResult<()>
where
    F: FnOnce(&mut UTransfer),
{
    let mut g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let transfer = g.get_mut(id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    f(transfer);
    transfer.updated_at_ms = now_ms();
    Ok(())
}

fn transition_upload(id: &str, event: TransferStateEvent) -> SpResult<TransferState> {
    let mut g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let transfer = g.get_mut(id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let next = apply_transfer_event(TransferKind::Upload, &state_from_transfer(transfer), event)?;
    transfer.lifecycle_state = next.lifecycle.clone();
    transfer.phase = next.phase;
    transfer.updated_at_ms = now_ms();
    Ok(next)
}

fn snapshot_from_upload(id: &str, transfer: &UTransfer) -> TransferSnapshot {
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

pub async fn start_upload(app: tauri::AppHandle, params: NewUploadParams) -> SpResult<String> {
    let meta = tokio::fs::metadata(&params.source_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("stat src: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let total = meta.len();
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut g = UL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "upload state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        g.insert(
            id.clone(),
            UTransfer {
                key: params.key.clone(),
                src: PathBuf::from(&params.source_path),
                part_size: params.part_size.max(8 * 1024 * 1024),
                bytes_total: total,
                bytes_done: 0,
                parts_completed: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
                worker_active: false,
                lifecycle_state: TransferState::queued(TransferKind::Upload).lifecycle,
                phase: TransferState::queued(TransferKind::Upload).phase,
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            },
        );
    }
    let id_spawn = id.clone();
    let app_spawn = app.clone();
    let _ = mutate_upload(&id, |t| {
        t.worker_active = true;
    });
    tokio::spawn(async move {
        let res = run_upload(
            &app_spawn,
            &id_spawn,
            params,
            paused.clone(),
            cancelled.clone(),
        )
        .await;
        if let Err(e) = res {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
                t.last_error = Some(e.clone());
            });
            match e.kind {
                ErrorKind::Cancelled => {}
                _ => {
                    let _ = transition_upload(&id_spawn, TransferStateEvent::Fail);
                    emit_upload(
                        &app_spawn,
                        &UploadEvent::Failed {
                            transfer_id: id_spawn.clone(),
                            error: e,
                        },
                    );
                }
            }
        } else {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
            });
        }
    });
    Ok(id)
}

pub async fn start_upload_stream(
    app: tauri::AppHandle,
    params: NewUploadStreamParams,
) -> SpResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut g = UL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "upload state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        g.insert(
            id.clone(),
            UTransfer {
                key: params.key.clone(),
                src: PathBuf::new(),
                part_size: params.part_size.max(512 * 1024),
                bytes_total: params.bytes_total,
                bytes_done: 0,
                parts_completed: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
                worker_active: false,
                lifecycle_state: TransferState::queued(TransferKind::Upload).lifecycle,
                phase: TransferState::queued(TransferKind::Upload).phase,
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            },
        );
    }
    let (tx, mut rx) = mpsc::channel::<Option<Vec<u8>>>(8);
    {
        let mut s = USTREAMS.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "upload streams lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        s.insert(id.clone(), tx);
    }

    let id_spawn = id.clone();
    let app_spawn = app.clone();
    let _ = mutate_upload(&id, |t| {
        t.worker_active = true;
    });
    tokio::spawn(async move {
        let res = async {
            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::PreparingSource),
            )?;
            emit_upload(
                &app_spawn,
                &UploadEvent::Started {
                    transfer_id: id_spawn.clone(),
                },
            );
            // Note: streaming uploads currently do not support auto-uploading local thumbnails.
            // If the setting is enabled, emit a warning so users/devs know why nothing happens.
            if settings::get().upload_thumbnail {
                crate::logger::warn(
                    "sp.backend",
                    "upload_thumbnail=true; streaming mode does not auto-upload thumbnails",
                );
            }
            let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
            let client = r2_client::build_client(&bundle.r2).await?;
            let mut writer = client.op.writer(&params.key).await.map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("open writer: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: now_ms(),
            })?;
            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::UploadingRemote),
            )?;
            let mut was_paused = false;
            let mut part_number: u32 = 1;
            while let Some(msg) = rx.recv().await {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                while paused.load(Ordering::Relaxed) {
                    if !was_paused {
                        transition_upload(&id_spawn, TransferStateEvent::Pause)?;
                        emit_upload(
                            &app_spawn,
                            &UploadEvent::Paused {
                                transfer_id: id_spawn.clone(),
                            },
                        );
                        was_paused = true;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                if was_paused {
                    transition_upload(
                        &id_spawn,
                        TransferStateEvent::Run(TransferPhase::UploadingRemote),
                    )?;
                    emit_upload(
                        &app_spawn,
                        &UploadEvent::Resumed {
                            transfer_id: id_spawn.clone(),
                        },
                    );
                    was_paused = false;
                }
                match msg {
                    Some(bytes) => {
                        let n = bytes.len() as u64;
                        writer.write(bytes).await.map_err(|e| SpError {
                            kind: ErrorKind::RetryableNet,
                            message: format!("writer write: {e}"),
                            retry_after_ms: Some(300),
                            context: None,
                            at: chrono::Utc::now().timestamp_millis(),
                        })?;
                        mutate_upload(&id_spawn, |t| {
                            t.bytes_done = t.bytes_done.saturating_add(n);
                            t.parts_completed += 1;
                        })?;
                        emit_upload(
                            &app_spawn,
                            &UploadEvent::PartProgress {
                                transfer_id: id_spawn.clone(),
                                progress: crate::types::UploadPartProgress {
                                    part_number,
                                    bytes_transferred: n,
                                },
                            },
                        );
                        emit_upload(
                            &app_spawn,
                            &UploadEvent::PartDone {
                                transfer_id: id_spawn.clone(),
                                part_number,
                                etag: String::new(),
                            },
                        );
                        part_number += 1;
                    }
                    None => break,
                }
            }
            if cancelled.load(Ordering::Relaxed) {
                transition_upload(&id_spawn, TransferStateEvent::CancelConfirm)?;
                emit_upload(
                    &app_spawn,
                    &UploadEvent::Cancelled {
                        transfer_id: id_spawn.clone(),
                    },
                );
                return Err(SpError {
                    kind: ErrorKind::Cancelled,
                    message: "cancelled".into(),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                });
            }
            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::FinalizingRemote),
            )?;
            writer.close().await.map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("writer close: {e}"),
                retry_after_ms: Some(300),
                context: None,
                at: now_ms(),
            })?;
            transition_upload(&id_spawn, TransferStateEvent::Complete)?;
            emit_upload(
                &app_spawn,
                &UploadEvent::Completed {
                    transfer_id: id_spawn.clone(),
                },
            );
            Ok::<(), SpError>(())
        }
        .await;
        if let Err(e) = res {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
                t.last_error = Some(e.clone());
            });
            match e.kind {
                ErrorKind::Cancelled => {}
                _ => {
                    let _ = transition_upload(&id_spawn, TransferStateEvent::Fail);
                    emit_upload(
                        &app_spawn,
                        &UploadEvent::Failed {
                            transfer_id: id_spawn.clone(),
                            error: e,
                        },
                    );
                }
            }
        } else {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
            });
        }
        // cleanup channel
        let mut s = USTREAMS.lock().unwrap_or_else(|p| p.into_inner());
        s.remove(&id_spawn);
    });
    Ok(id)
}

// Android-only: start an upload by reading from a SAF content URI directly on the backend.
#[cfg(target_os = "android")]
pub async fn start_upload_android_uri(
    app: tauri::AppHandle,
    key: String,
    uri: String,
    part_size: u64,
) -> SpResult<String> {
    use std::io::Read;
    use tauri_plugin_android_fs::AndroidFsExt as _;

    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));

    // Prepare state entry with tentative size 0; update after opening
    {
        let mut g = UL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "upload state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        g.insert(
            id.clone(),
            UTransfer {
                key: key.clone(),
                src: PathBuf::new(),
                part_size: part_size.max(512 * 1024),
                bytes_total: 0,
                bytes_done: 0,
                parts_completed: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
                worker_active: false,
                lifecycle_state: TransferState::queued(TransferKind::Upload).lifecycle,
                phase: TransferState::queued(TransferKind::Upload).phase,
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            },
        );
    }

    let id_spawn = id.clone();
    let app_spawn = app.clone();
    let _ = mutate_upload(&id, |t| {
        t.worker_active = true;
    });
    tokio::spawn(async move {
        let res = async {
            let should_upload_thumbnail = settings::get().upload_thumbnail;
            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::PreparingSource),
            )?;
            emit_upload(
                &app_spawn,
                &UploadEvent::Started {
                    transfer_id: id_spawn.clone(),
                },
            );

            let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
            let client = r2_client::build_client(&bundle.r2).await?;
            let mut writer = client.op.writer(&key).await.map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("open writer: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: now_ms(),
            })?;
            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::UploadingRemote),
            )?;

            // Open readable file from SAF URI
            let api = app_spawn.android_fs();
            // We receive a raw content:// URI string from the bridge. Construct FileUri directly.
            let file_uri = tauri_plugin_android_fs::FileUri {
                uri: uri.clone(),
                document_top_tree_uri: None,
            };
            let mut file = api.open_file_readable(&file_uri).map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("open_file_readable: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

            // Update total size if available
            if let Ok(meta) = file.metadata() {
                let _ = mutate_upload(&id_spawn, |t| {
                    t.bytes_total = meta.len();
                });
            }

            let mut part_number: u32 = 1;
            let mut buf = vec![0u8; part_size.max(256 * 1024) as usize];
            let mut was_paused = false;
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                while paused.load(Ordering::Relaxed) {
                    if !was_paused {
                        transition_upload(&id_spawn, TransferStateEvent::Pause)?;
                        emit_upload(
                            &app_spawn,
                            &UploadEvent::Paused {
                                transfer_id: id_spawn.clone(),
                            },
                        );
                        was_paused = true;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                if was_paused {
                    transition_upload(
                        &id_spawn,
                        TransferStateEvent::Run(TransferPhase::UploadingRemote),
                    )?;
                    emit_upload(
                        &app_spawn,
                        &UploadEvent::Resumed {
                            transfer_id: id_spawn.clone(),
                        },
                    );
                    was_paused = false;
                }

                let n = file.read(&mut buf).map_err(|e| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("read src: {e}"),
                    retry_after_ms: Some(200),
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                if n == 0 {
                    break;
                }

                writer.write(buf[..n].to_vec()).await.map_err(|e| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("writer write: {e}"),
                    retry_after_ms: Some(300),
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;

                mutate_upload(&id_spawn, |t| {
                    t.bytes_done = t.bytes_done.saturating_add(n as u64);
                    t.parts_completed += 1;
                })?;
                emit_upload(
                    &app_spawn,
                    &UploadEvent::PartProgress {
                        transfer_id: id_spawn.clone(),
                        progress: crate::types::UploadPartProgress {
                            part_number,
                            bytes_transferred: n as u64,
                        },
                    },
                );
                emit_upload(
                    &app_spawn,
                    &UploadEvent::PartDone {
                        transfer_id: id_spawn.clone(),
                        part_number,
                        etag: String::new(),
                    },
                );
                part_number += 1;
            }

            if cancelled.load(Ordering::Relaxed) {
                let _ = writer.close().await;
                let _ = client.op.delete(&key).await;
                transition_upload(&id_spawn, TransferStateEvent::CancelConfirm)?;
                emit_upload(
                    &app_spawn,
                    &UploadEvent::Cancelled {
                        transfer_id: id_spawn.clone(),
                    },
                );
                return Err(SpError {
                    kind: ErrorKind::Cancelled,
                    message: "cancelled".into(),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                });
            }

            transition_upload(
                &id_spawn,
                TransferStateEvent::Run(TransferPhase::FinalizingRemote),
            )?;
            writer.close().await.map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("writer close: {e}"),
                retry_after_ms: Some(300),
                context: None,
                at: now_ms(),
            })?;
            if should_upload_thumbnail {
                let thumb_temp_path = android_thumbnail_temp_path(&id_spawn, &key)?;
                let thumb_temp = thumb_temp_path.to_string_lossy().to_string();
                let thumb_key = crate::thumbnail::thumbnail_key_for(&key);
                let copy_res = crate::bridge::android_fs_copy(
                    app_spawn.clone(),
                    crate::bridge::AndroidFsCopyParams {
                        direction: "uri_to_sandbox".into(),
                        local_path: thumb_temp.clone(),
                        tree_uri: None,
                        relative_path: None,
                        mime: None,
                        uri: Some(uri.clone()),
                    },
                )
                .await;
                match copy_res {
                    Ok(()) => {
                        match crate::thumbnail::generate_thumbnail_bytes(
                            &thumb_temp,
                            128,
                            16 * 1024,
                        )
                        .await
                        {
                            Ok(Some(bytes)) => {
                                if let Err(e) = r2_client::put_object_bytes(
                                    &client, &thumb_key, bytes, None, false,
                                )
                                .await
                                {
                                    crate::logger::warn(
                                        "upload",
                                        &format!(
                                            "android thumbnail upload failed for {}: {}",
                                            key, e.message
                                        ),
                                    );
                                }
                            }
                            Ok(None) => {
                                crate::logger::info(
                                    "upload",
                                    &format!(
                                        "android thumbnail skipped for {}; unsupported file type",
                                        key
                                    ),
                                );
                            }
                            Err(e) => {
                                crate::logger::warn(
                                    "upload",
                                    &format!(
                                        "android thumbnail generation failed for {}: {}",
                                        key, e.message
                                    ),
                                );
                            }
                        }
                        let _ = tokio::fs::remove_file(&thumb_temp).await;
                    }
                    Err(e) => {
                        crate::logger::warn(
                            "upload",
                            &format!(
                                "android thumbnail source materialize failed for {}: {}",
                                key, e.message
                            ),
                        );
                    }
                }
            }
            transition_upload(&id_spawn, TransferStateEvent::Complete)?;
            emit_upload(
                &app_spawn,
                &UploadEvent::Completed {
                    transfer_id: id_spawn.clone(),
                },
            );
            Ok::<(), SpError>(())
        }
        .await;
        if let Err(e) = res {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
                t.last_error = Some(e.clone());
            });
            match e.kind {
                ErrorKind::Cancelled => {}
                _ => {
                    let _ = transition_upload(&id_spawn, TransferStateEvent::Fail);
                    emit_upload(
                        &app_spawn,
                        &UploadEvent::Failed {
                            transfer_id: id_spawn.clone(),
                            error: e,
                        },
                    );
                }
            }
        } else {
            let _ = mutate_upload(&id_spawn, |t| {
                t.worker_active = false;
            });
        }
    });

    Ok(id)
}

pub fn stream_write(id: &str, chunk: Vec<u8>) -> SpResult<()> {
    let g = USTREAMS.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload streams lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(tx) = g.get(id) {
        tx.try_send(Some(chunk)).map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("stream write: {e}"),
            retry_after_ms: Some(100),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        Ok(())
    } else {
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}

pub fn stream_finish(id: &str) -> SpResult<()> {
    let g = USTREAMS.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload streams lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(tx) = g.get(id) {
        tx.try_send(None).map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("stream finish: {e}"),
            retry_after_ms: Some(100),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        Ok(())
    } else {
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}

async fn run_upload(
    app: &tauri::AppHandle,
    id: &str,
    params: NewUploadParams,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> SpResult<()> {
    let should_upload_thumbnail = settings::get().upload_thumbnail;
    transition_upload(id, TransferStateEvent::Run(TransferPhase::PreparingSource))?;
    emit_upload(
        app,
        &UploadEvent::Started {
            transfer_id: id.to_string(),
        },
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let mut file = tokio::fs::File::open(&params.source_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open src: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let meta = file.metadata().await.map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("stat src: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let _size = meta.len();

    // Streaming upload via OpenDAL writer
    let mut writer = client.op.writer(&params.key).await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("open writer: {e}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })?;
    transition_upload(id, TransferStateEvent::Run(TransferPhase::UploadingRemote))?;

    let mut part_number: u32 = 1;
    let mut was_paused = false;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        while paused.load(Ordering::Relaxed) {
            if !was_paused {
                transition_upload(id, TransferStateEvent::Pause)?;
                emit_upload(
                    app,
                    &UploadEvent::Paused {
                        transfer_id: id.to_string(),
                    },
                );
                was_paused = true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if was_paused {
            transition_upload(id, TransferStateEvent::Run(TransferPhase::UploadingRemote))?;
            emit_upload(
                app,
                &UploadEvent::Resumed {
                    transfer_id: id.to_string(),
                },
            );
            was_paused = false;
        }
        let mut buf = vec![0u8; params.part_size as usize];
        let n = file.read(&mut buf).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("read src: {e}"),
            retry_after_ms: Some(200),
            context: None,
            at: now_ms(),
        })?;
        if n == 0 {
            break;
        }
        buf.truncate(n);
        writer.write(buf).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("writer write: {e}"),
            retry_after_ms: Some(300),
            context: None,
            at: now_ms(),
        })?;

        mutate_upload(id, |t| {
            t.bytes_done = t.bytes_done.saturating_add(n as u64);
            t.parts_completed += 1;
        })?;
        emit_upload(
            app,
            &UploadEvent::PartProgress {
                transfer_id: id.to_string(),
                progress: crate::types::UploadPartProgress {
                    part_number,
                    bytes_transferred: n as u64,
                },
            },
        );
        emit_upload(
            app,
            &UploadEvent::PartDone {
                transfer_id: id.to_string(),
                part_number,
                etag: String::new(),
            },
        );
        // Op and ingress/storage bytes tracked by HTTP layer.
        part_number += 1;
    }

    if cancelled.load(Ordering::Relaxed) {
        let _ = writer.close().await;
        let _ = client.op.delete(&params.key).await;
        transition_upload(id, TransferStateEvent::CancelConfirm)?;
        emit_upload(
            app,
            &UploadEvent::Cancelled {
                transfer_id: id.to_string(),
            },
        );
        return Err(SpError {
            kind: ErrorKind::Cancelled,
            message: "cancelled".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }

    // Complete writer
    transition_upload(id, TransferStateEvent::Run(TransferPhase::FinalizingRemote))?;
    writer.close().await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("writer close: {e}"),
        retry_after_ms: Some(300),
        context: None,
        at: now_ms(),
    })?;

    // Optionally generate and upload thumbnail in background (best-effort).
    if should_upload_thumbnail {
        let client2 = client.clone();
        let source_path = params.source_path.clone();
        let object_key = params.key.clone();
        let thumb_key = crate::thumbnail::thumbnail_key_for(&params.key);
        tokio::spawn(async move {
            match crate::thumbnail::generate_thumbnail_bytes(&source_path, 128, 16 * 1024).await {
                Ok(Some(bytes)) => {
                    if let Err(e) =
                        r2_client::put_object_bytes(&client2, &thumb_key, bytes, None, false).await
                    {
                        crate::logger::warn(
                            "upload",
                            &format!("thumbnail upload failed for {}: {}", object_key, e.message),
                        );
                    }
                }
                Ok(None) => {
                    crate::logger::info(
                        "upload",
                        &format!(
                            "thumbnail skipped for {}; unsupported file type",
                            object_key
                        ),
                    );
                }
                Err(e) => {
                    crate::logger::warn(
                        "upload",
                        &format!(
                            "thumbnail generation failed for {}: {}",
                            object_key, e.message
                        ),
                    );
                }
            }
        });
    }
    // Op tracked by HTTP layer.
    transition_upload(id, TransferStateEvent::Complete)?;
    emit_upload(
        app,
        &UploadEvent::Completed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

pub fn pause(app: &tauri::AppHandle, id: &str) -> SpResult<()> {
    let g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(id) {
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
    transition_upload(id, TransferStateEvent::Pause)?;
    emit_upload(
        app,
        &UploadEvent::Paused {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}
pub fn resume(app: &tauri::AppHandle, id: &str) -> SpResult<()> {
    let g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(id) {
        t.paused.store(false, Ordering::Relaxed);
    } else {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    let phase = g
        .get(id)
        .and_then(|t| t.phase)
        .ok_or_else(|| err_invalid("paused upload missing phase"))?;
    drop(g);
    transition_upload(id, TransferStateEvent::Run(phase))?;
    emit_upload(
        app,
        &UploadEvent::Resumed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}
pub fn cancel(app: &tauri::AppHandle, id: &str) -> SpResult<()> {
    let g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(id) {
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
    transition_upload(id, TransferStateEvent::CancelRequest)?;
    emit_upload(
        app,
        &UploadEvent::Cancelling {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}
pub fn status(id: &str) -> SpResult<UploadStatus> {
    let g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let t = g.get(id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(UploadStatus {
        transfer_id: id.into(),
        key: t.key.clone(),
        lifecycle_state: t.lifecycle_state.clone(),
        phase: t.phase,
        bytes_total: t.bytes_total,
        bytes_done: t.bytes_done,
        parts_completed: t.parts_completed,
        rate_bps: 0,
        eta_ms: None,
        last_error: t.last_error.clone(),
    })
}

pub fn list_active_snapshots() -> Vec<TransferSnapshot> {
    let g = match UL.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    g.iter()
        .filter_map(|(id, t)| {
            if t.lifecycle_state.is_terminal() {
                return None;
            }
            Some(snapshot_from_upload(id, t))
        })
        .collect()
}

pub fn remove(id: &str) -> SpResult<()> {
    let mut g = UL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "upload state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    if let Some(t) = g.get(id) {
        if !t.lifecycle_state.is_terminal() {
            return Err(err_invalid("cannot remove active upload"));
        }
    }
    g.remove(id);
    Ok(())
}
