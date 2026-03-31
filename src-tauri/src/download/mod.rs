use crate::bridge::AndroidFsCopyParams;
use crate::transfer_db::{self, TransferKind, TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::{apply_transfer_event, TransferState, TransferStateEvent};
use crate::types::*;
use crate::usage::UsageSync;
use crate::{r2_client, sp_backend::SpBackend};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

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

#[derive(Debug, Clone)]
enum DownloadTarget {
    FileSystem {
        dest: PathBuf,
    },
    AndroidTree {
        tree_uri: String,
        relative_path: String,
        mime: Option<String>,
    },
}

impl DownloadTarget {
    fn from_params(params: &NewDownloadParams) -> SpResult<Self> {
        match (
            params.dest_path.as_deref(),
            params.android_tree_uri.as_deref(),
            params.android_relative_path.as_deref(),
        ) {
            (Some(dest), None, None) => Ok(Self::FileSystem {
                dest: normalize_dest_path(dest)?,
            }),
            (None, Some(tree_uri), Some(relative_path)) => {
                if relative_path.trim().is_empty() {
                    return Err(err_invalid("android_relative_path required"));
                }
                Ok(Self::AndroidTree {
                    tree_uri: tree_uri.to_string(),
                    relative_path: relative_path.to_string(),
                    mime: params.mime.clone(),
                })
            }
            _ => Err(err_invalid(
                "download target must be either dest_path or android target",
            )),
        }
    }

    fn from_snapshot(snapshot: &TransferSnapshot) -> SpResult<Self> {
        match snapshot.kind {
            TransferKind::Download => {}
            _ => return Err(err_invalid("snapshot kind mismatch")),
        }
        if let Some(dest_path) = snapshot.dest_path.as_deref() {
            return Ok(Self::FileSystem {
                dest: normalize_dest_path(dest_path)?,
            });
        }
        match (
            snapshot.android_tree_uri.as_deref(),
            snapshot.android_relative_path.as_deref(),
        ) {
            (Some(tree_uri), Some(relative_path)) => Ok(Self::AndroidTree {
                tree_uri: tree_uri.to_string(),
                relative_path: relative_path.to_string(),
                mime: None,
            }),
            _ => Err(err_invalid("snapshot missing download target")),
        }
    }

    fn temp_path_for(&self, transfer_id: &str, key: &str) -> SpResult<PathBuf> {
        match self {
            Self::FileSystem { dest } => Ok(dest.clone()),
            Self::AndroidTree { relative_path, .. } => {
                let mut dir = download_stage_dir()?;
                let fallback_name = sanitize_filename(key);
                let basename = Path::new(relative_path)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or(fallback_name.as_str())
                    .to_string();
                dir.push(transfer_id);
                std::fs::create_dir_all(&dir).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("create download stage dir: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                })?;
                dir.push(basename);
                Ok(dir)
            }
        }
    }

    fn snapshot_fields(&self) -> (Option<String>, Option<String>, Option<String>) {
        match self {
            Self::FileSystem { dest } => (Some(dest.to_string_lossy().to_string()), None, None),
            Self::AndroidTree {
                tree_uri,
                relative_path,
                ..
            } => (None, Some(tree_uri.clone()), Some(relative_path.clone())),
        }
    }
}

struct Transfer {
    key: String,
    target: DownloadTarget,
    temp_path: PathBuf,
    chunk: u64,
    expected_etag: Option<String>,
    observed_etag: Option<String>,
    bytes_total: Option<u64>,
    bytes_done: u64,
    last_error: Option<SpError>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    worker_active: bool,
    lifecycle_state: TransferLifecycle,
    phase: Option<TransferPhase>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

static DL: Lazy<Mutex<HashMap<String, Transfer>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn emit_download(app: &tauri::AppHandle, ev: &DownloadEvent) {
    let _ = app.emit("sp://download_event", ev);
}

fn normalize_dest_path(raw: &str) -> SpResult<PathBuf> {
    let s = raw.trim();
    let s = if let Some(rest) = s.strip_prefix("file://") {
        rest
    } else {
        s
    };
    if s.contains("://") {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "unsupported URI for download destination".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    Ok(PathBuf::from(s))
}

fn sanitize_filename(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

fn download_stage_dir() -> SpResult<PathBuf> {
    let mut p = crate::sp_backend::vault_dir()?;
    p.push("downloads");
    p.push("staging");
    std::fs::create_dir_all(&p).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create download stage dir: {e}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    Ok(p)
}

fn part_path_for(temp_path: &Path) -> PathBuf {
    temp_path.with_extension("part")
}

fn snapshot_from_transfer(id: &str, transfer: &Transfer) -> TransferSnapshot {
    let (dest_path, android_tree_uri, android_relative_path) = transfer.target.snapshot_fields();
    TransferSnapshot {
        transfer_id: id.to_string(),
        kind: TransferKind::Download,
        key: transfer.key.clone(),
        lifecycle_state: transfer.lifecycle_state.clone(),
        phase: transfer.phase.clone(),
        bytes_total: transfer.bytes_total,
        bytes_done: transfer.bytes_done,
        rate_bps: 0,
        last_error: transfer.last_error.clone(),
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

fn persist_transfer(id: &str) -> SpResult<()> {
    let snapshot = {
        let g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let transfer = g.get(id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        snapshot_from_transfer(id, transfer)
    };
    transfer_db::upsert_snapshot(&snapshot)
}

fn mutate_transfer<F>(id: &str, f: F) -> SpResult<()>
where
    F: FnOnce(&mut Transfer),
{
    {
        let mut g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let transfer = g.get_mut(id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        f(transfer);
        transfer.updated_at_ms = now_ms();
    }
    persist_transfer(id)
}

fn transition_transfer(id: &str, event: TransferStateEvent) -> SpResult<TransferState> {
    let next_state = {
        let mut g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
        let transfer = g.get_mut(id).ok_or_else(|| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download not found".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
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

fn load_runtime_fields(
    id: &str,
) -> SpResult<(
    String,
    DownloadTarget,
    PathBuf,
    u64,
    Option<String>,
    u64,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
)> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let transfer = g.get(id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download not found".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
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
            let _ = mutate_transfer(&transfer_id, |t| {
                t.worker_active = false;
                t.last_error = Some(e.clone());
            });
            match e.kind {
                ErrorKind::Cancelled => {}
                _ => {
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
        let paused = Arc::new(AtomicBool::new(matches!(
            snapshot.lifecycle_state,
            TransferLifecycle::Paused
        )));
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
                    lifecycle_state: if matches!(
                        snapshot.lifecycle_state,
                        TransferLifecycle::Cancelling
                    ) {
                        TransferLifecycle::Cancelled
                    } else {
                        snapshot.lifecycle_state.clone()
                    },
                    phase: snapshot.phase.clone(),
                    created_at_ms: snapshot.created_at_ms,
                    updated_at_ms: now_ms(),
                },
            );
        }
        if matches!(snapshot.lifecycle_state, TransferLifecycle::Paused) {
            let _ = persist_transfer(&snapshot.transfer_id);
            continue;
        }
        if matches!(snapshot.lifecycle_state, TransferLifecycle::Cancelling) {
            let _ = persist_transfer(&snapshot.transfer_id);
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

    let head = client.op.stat(&key).await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("Stat: {e}"),
        retry_after_ms: Some(500),
        context: None,
        at: now_ms(),
    })?;
    let mut b = std::collections::HashMap::new();
    b.insert("HeadObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });

    let total = head.content_length();
    let etag = head.etag().map(|s| s.to_string());
    mutate_transfer(id, |t| {
        t.bytes_total = Some(total);
        t.observed_etag = etag.clone();
        if total > 0 && t.bytes_done > total {
            t.bytes_done = 0;
        }
    })?;

    if let (Some(exp), Some(obs)) = (expected_etag.as_ref(), etag.as_ref()) {
        if exp != obs {
            emit_download(
                app,
                &DownloadEvent::SourceChanged {
                    transfer_id: id.to_string(),
                },
            );
            return Err(SpError {
                kind: ErrorKind::SourceChanged,
                message: "ETag mismatch".into(),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            });
        }
    }

    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create parent dir: {e}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
    }

    let part_path = part_path_for(&temp_path);
    let finished_local = match tokio::fs::metadata(&temp_path).await {
        Ok(meta) => meta.len() == total && bytes_done == total && total > 0,
        Err(_) => false,
    };

    if !finished_local {
        let mut offset = match tokio::fs::metadata(&part_path).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        if offset > total {
            let _ = tokio::fs::remove_file(&part_path).await;
            offset = 0;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("open temp: {e}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;

        transition_transfer(
            id,
            TransferStateEvent::Run(TransferPhase::DownloadingRemote),
        )?;
        mutate_transfer(id, |t| {
            t.bytes_done = offset;
        })?;

        let mut was_paused = false;
        while offset < total {
            if cancelled.load(Ordering::Relaxed) {
                return cancel_download(app, id, &part_path, &temp_path).await;
            }
            while paused.load(Ordering::Relaxed) {
                if !was_paused {
                    transition_transfer(id, TransferStateEvent::Pause)?;
                    emit_download(
                        app,
                        &DownloadEvent::Paused {
                            transfer_id: id.to_string(),
                        },
                    );
                    was_paused = true;
                }
                if cancelled.load(Ordering::Relaxed) {
                    return cancel_download(app, id, &part_path, &temp_path).await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            if was_paused {
                transition_transfer(
                    id,
                    TransferStateEvent::Run(TransferPhase::DownloadingRemote),
                )?;
                emit_download(
                    app,
                    &DownloadEvent::Resumed {
                        transfer_id: id.to_string(),
                    },
                );
                was_paused = false;
            }

            let end = (offset + chunk - 1).min(total.saturating_sub(1));
            let range_start = offset;
            let data = client
                .op
                .read_with(&key)
                .range(range_start..(end + 1))
                .await
                .map_err(|e| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("GetObject range: {e}"),
                    retry_after_ms: Some(500),
                    context: None,
                    at: now_ms(),
                })?;
            if data.is_empty() {
                break;
            }
            let chunk_bytes: bytes::Bytes = data.to_bytes();
            file.write_all(&chunk_bytes).await.map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("write: {e}"),
                retry_after_ms: Some(300),
                context: None,
                at: now_ms(),
            })?;
            let mut b = std::collections::HashMap::new();
            b.insert("GetObject".into(), 1u64);
            let _ = UsageSync::record_local_delta(UsageDelta {
                class_a: Default::default(),
                class_b: b,
                ingress_bytes: 0,
                egress_bytes: chunk_bytes.len() as u64,
                added_storage_bytes: 0,
                deleted_storage_bytes: 0,
            });
            offset = offset.saturating_add(chunk_bytes.len() as u64);
            mutate_transfer(id, |t| {
                t.bytes_done = offset;
            })?;
            emit_download(
                app,
                &DownloadEvent::ChunkDone {
                    transfer_id: id.to_string(),
                    range_start,
                    len: chunk_bytes.len() as u64,
                },
            );
        }

        file.flush().await.ok();
        if cancelled.load(Ordering::Relaxed) {
            return cancel_download(app, id, &part_path, &temp_path).await;
        }
        tokio::fs::rename(&part_path, &temp_path)
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("rename: {e}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
    }

    materialize_target(app, id, &target, &temp_path).await?;
    transition_transfer(id, TransferStateEvent::Complete)?;
    mutate_transfer(id, |t| {
        t.bytes_done = total;
    })?;
    emit_download(
        app,
        &DownloadEvent::Completed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

async fn cancel_download(
    app: &tauri::AppHandle,
    id: &str,
    part_path: &Path,
    temp_path: &Path,
) -> SpResult<()> {
    let _ = tokio::fs::remove_file(part_path).await;
    let _ = tokio::fs::remove_file(temp_path).await;
    let _ = transition_transfer(id, TransferStateEvent::CancelConfirm);
    mutate_transfer(id, |t| {
        t.last_error = Some(SpError {
            kind: ErrorKind::Cancelled,
            message: "cancelled".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    })?;
    emit_download(
        app,
        &DownloadEvent::Cancelled {
            transfer_id: id.to_string(),
        },
    );
    Err(SpError {
        kind: ErrorKind::Cancelled,
        message: "cancelled".into(),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })
}

async fn materialize_target(
    app: &tauri::AppHandle,
    id: &str,
    target: &DownloadTarget,
    temp_path: &Path,
) -> SpResult<()> {
    match target {
        DownloadTarget::FileSystem { .. } => {
            transition_transfer(
                id,
                TransferStateEvent::Run(TransferPhase::MaterializingTarget),
            )?;
            transition_transfer(id, TransferStateEvent::Run(TransferPhase::CleaningUp))?;
            Ok(())
        }
        DownloadTarget::AndroidTree {
            tree_uri,
            relative_path,
            mime,
        } => {
            transition_transfer(
                id,
                TransferStateEvent::Run(TransferPhase::MaterializingTarget),
            )?;
            crate::bridge::android_fs_copy(
                app.clone(),
                AndroidFsCopyParams {
                    direction: "sandbox_to_tree".into(),
                    local_path: temp_path.to_string_lossy().to_string(),
                    tree_uri: Some(tree_uri.clone()),
                    relative_path: Some(relative_path.clone()),
                    mime: mime.clone(),
                    uri: None,
                },
            )
            .await?;
            transition_transfer(id, TransferStateEvent::Run(TransferPhase::CleaningUp))?;
            let _ = tokio::fs::remove_file(temp_path).await;
            Ok(())
        }
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
    transfer_db::list_active_snapshots()
}
