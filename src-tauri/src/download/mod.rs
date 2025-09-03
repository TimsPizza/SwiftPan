use crate::types::*;
use crate::usage::UsageSync;
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
use tokio::io::AsyncWriteExt;

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
    Completed {
        transfer_id: String,
    },
    Failed {
        transfer_id: String,
        error: SpError,
    },
    SourceChanged {
        transfer_id: String,
    },
}

struct Transfer {
    key: String,
    dest: PathBuf,
    chunk: u64,
    expected_etag: Option<String>,
    observed_etag: Option<String>,
    bytes_total: Option<u64>,
    bytes_done: u64,
    last_error: Option<SpError>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

static DL: Lazy<Mutex<HashMap<String, Transfer>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn emit_download(app: &tauri::AppHandle, ev: &DownloadEvent) {
    let _ = app.emit("sp://download_event", ev);
}

pub async fn start_download(app: tauri::AppHandle, params: NewDownloadParams) -> SpResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));

    let key = params.key.clone();
    let dest = PathBuf::from(params.dest_path.clone());
    let chunk = params.chunk_size.max(1024 * 1024);
    let expected = params.expected_etag.clone();

    {
        let mut g = DL.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "download state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        g.insert(
            id.clone(),
            Transfer {
                key: key.clone(),
                dest: dest.clone(),
                chunk,
                expected_etag: expected.clone(),
                observed_etag: None,
                bytes_total: None,
                bytes_done: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
            },
        );
    }

    let id_spawn = id.clone();
    let app_spawn = app.clone();
    tokio::spawn(async move {
        let res = run_download(
            &app_spawn,
            &id_spawn,
            &key,
            &dest,
            chunk,
            expected,
            paused.clone(),
            cancelled.clone(),
        )
        .await;
        if let Err(e) = res {
            let mut g = DL.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = g.get_mut(&id_spawn) {
                t.last_error = Some(e);
            }
        }
    });

    Ok(id)
}

async fn run_download(
    app: &tauri::AppHandle,
    id: &str,
    key: &str,
    dest: &PathBuf,
    chunk: u64,
    expected_etag: Option<String>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> SpResult<()> {
    emit_download(
        app,
        &DownloadEvent::Started {
            transfer_id: id.to_string(),
        },
    );
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;

    // Head to get size and etag
    let head = client
        .s3
        .head_object()
        .bucket(&client.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("HeadObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    // Usage: B 类 HeadObject +1
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

    let total = head.content_length().map(|v| v as u64);
    let etag = head.e_tag().map(|s| s.trim_matches('"').to_string());
    {
        let mut g = DL.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(t) = g.get_mut(id) {
            t.bytes_total = total;
            t.observed_etag = etag.clone();
        }
    }

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
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    // Temp file write
    let part_path = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open temp: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

    let mut offset: u64 = 0;
    let total_len = total.unwrap_or(0);
    while cancelled.load(Ordering::Relaxed) == false {
        while paused.load(Ordering::Relaxed) {
            emit_download(
                app,
                &DownloadEvent::Paused {
                    transfer_id: id.to_string(),
                },
            );
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        emit_download(
            app,
            &DownloadEvent::Resumed {
                transfer_id: id.to_string(),
            },
        );
        if total.is_some() && offset >= total_len {
            break;
        }
        let end = if total.is_some() {
            (offset + chunk - 1).min(total_len.saturating_sub(1))
        } else {
            offset + chunk - 1
        };
        let range = if total.is_some() {
            format!("bytes={}-{}", offset, end)
        } else {
            format!("bytes={}-{}", offset, offset + chunk - 1)
        };
        let mut get = client
            .s3
            .get_object()
            .bucket(&client.bucket)
            .key(key)
            .range(range);
        if let Some(exp) = expected_etag.clone() {
            get = get.if_match(exp);
        }
        let get = get.send().await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("GetObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let mut body = get.body.into_async_read();
        let copied = tokio::io::copy(&mut body, &mut file)
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("copy: {e}"),
                retry_after_ms: Some(300),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        // Usage: B 类 GetObject +1；egress 计本次 copied 字节
        let mut b = std::collections::HashMap::new();
        b.insert("GetObject".into(), 1u64);
        let _ = UsageSync::record_local_delta(UsageDelta {
            class_a: Default::default(),
            class_b: b,
            ingress_bytes: 0,
            egress_bytes: copied as u64,
            added_storage_bytes: 0,
            deleted_storage_bytes: 0,
        });
        offset = offset.saturating_add(copied as u64);
        emit_download(
            app,
            &DownloadEvent::ChunkDone {
                transfer_id: id.to_string(),
                range_start: offset.saturating_sub(copied as u64),
                len: copied as u64,
            },
        );
        {
            let mut g = DL.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = g.get_mut(id) {
                t.bytes_done = offset;
            }
        }
        if total.is_none() && copied == 0 {
            break;
        }
    }

    file.flush().await.ok();
    if cancelled.load(Ordering::Relaxed) {
        let _ = tokio::fs::remove_file(&part_path).await;
        emit_download(
            app,
            &DownloadEvent::Failed {
                transfer_id: id.to_string(),
                error: SpError {
                    kind: ErrorKind::Cancelled,
                    message: "cancelled".into(),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                },
            },
        );
        return Err(SpError {
            kind: ErrorKind::Cancelled,
            message: "cancelled".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    tokio::fs::rename(&part_path, &dest)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("rename: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    emit_download(
        app,
        &DownloadEvent::Completed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

pub fn pause(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(transfer_id) {
        t.paused.store(true, Ordering::Relaxed);
        emit_download(
            app,
            &DownloadEvent::Paused {
                transfer_id: transfer_id.to_string(),
            },
        );
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
pub fn resume(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(transfer_id) {
        t.paused.store(false, Ordering::Relaxed);
        emit_download(
            app,
            &DownloadEvent::Resumed {
                transfer_id: transfer_id.to_string(),
            },
        );
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
pub fn cancel(app: &tauri::AppHandle, transfer_id: &str) -> SpResult<()> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    if let Some(t) = g.get(transfer_id) {
        t.cancelled.store(true, Ordering::Relaxed);
        emit_download(
            app,
            &DownloadEvent::Failed {
                transfer_id: transfer_id.to_string(),
                error: SpError {
                    kind: ErrorKind::Cancelled,
                    message: "cancelled".into(),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                },
            },
        );
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

pub fn status(transfer_id: &str) -> SpResult<DownloadStatus> {
    let g = DL.lock().map_err(|_| SpError {
        kind: ErrorKind::NotRetriable,
        message: "download state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let t = g.get(transfer_id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(DownloadStatus {
        transfer_id: transfer_id.into(),
        key: t.key.clone(),
        bytes_total: t.bytes_total,
        bytes_done: t.bytes_done,
        rate_bps: 0,
        expected_etag: t.expected_etag.clone(),
        observed_etag: t.observed_etag.clone(),
        last_error: t.last_error.clone(),
    })
}
