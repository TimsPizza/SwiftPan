use crate::settings;
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
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUploadParams {
    pub key: String,
    pub source_path: String,
    pub part_size: u64,
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatus {
    pub transfer_id: String,
    pub key: String,
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
    Completed {
        transfer_id: String,
    },
    Failed {
        transfer_id: String,
        error: SpError,
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
}

static UL: Lazy<Mutex<HashMap<String, UTransfer>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn emit_upload(app: &tauri::AppHandle, ev: &UploadEvent) {
    let _ = app.emit("sp://upload_event", ev);
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
            },
        );
    }
    let id_spawn = id.clone();
    let app_spawn = app.clone();
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
            let mut g = UL.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = g.get_mut(&id_spawn) {
                t.last_error = Some(e);
            }
        }
    });
    Ok(id)
}

async fn run_upload(
    app: &tauri::AppHandle,
    id: &str,
    params: NewUploadParams,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> SpResult<()> {
    let src_basename = std::path::Path::new(&params.source_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let maybe_thumb_local: Option<String> = {
        let s = settings::get();
        if !s.upload_thumbnail {
            None
        } else {
            let parent = std::path::Path::new(&params.source_path).parent();
            if let Some(dir) = parent {
                let thumb_name = format!("thumbnail_{}.jpg", src_basename);
                let p = dir.join(thumb_name);
                match tokio::fs::metadata(&p).await {
                    Ok(m) if m.is_file() => Some(p.to_string_lossy().to_string()),
                    _ => None,
                }
            } else {
                None
            }
        }
    };
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
        at: chrono::Utc::now().timestamp_millis(),
    })?;

    let mut part_number: u32 = 1;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        while paused.load(Ordering::Relaxed) {
            emit_upload(
                app,
                &UploadEvent::Paused {
                    transfer_id: id.to_string(),
                },
            );
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        emit_upload(
            app,
            &UploadEvent::Resumed {
                transfer_id: id.to_string(),
            },
        );
        let mut buf = vec![0u8; params.part_size as usize];
        let n = file.read(&mut buf).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("read src: {e}"),
            retry_after_ms: Some(200),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
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
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        {
            let mut g = UL.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = g.get_mut(id) {
                t.bytes_done = (t.bytes_done as u64 + n as u64) as u64;
                t.parts_completed += 1;
            }
        }
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
        emit_upload(
            app,
            &UploadEvent::Failed {
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

    // Complete writer
    writer.close().await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("writer close: {e}"),
        retry_after_ms: Some(300),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;

    // Optionally upload local thumbnail file in background (best-effort)
    if let Some(local_thumb) = maybe_thumb_local.clone() {
        let client2 = client.clone();
        let thumb_key = format!("thumbnail_{}.jpg", params.key);
        tokio::spawn(async move {
            if let Ok(mut f) = tokio::fs::File::open(&local_thumb).await {
                let mut writer = match client2.op.writer(&thumb_key).await {
                    Ok(w) => w,
                    Err(_) => return,
                };
                let mut buf = vec![0u8; 512 * 1024];
                loop {
                    match f.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = writer.write(buf[..n].to_vec()).await;
                        }
                        Err(_) => break,
                    }
                }
                let _ = writer.close().await;
            }
        });
    }
    // Op tracked by HTTP layer.
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
        emit_upload(
            app,
            &UploadEvent::Paused {
                transfer_id: id.to_string(),
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
        emit_upload(
            app,
            &UploadEvent::Resumed {
                transfer_id: id.to_string(),
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
        emit_upload(
            app,
            &UploadEvent::Failed {
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
        bytes_total: t.bytes_total,
        bytes_done: t.bytes_done,
        parts_completed: t.parts_completed,
        rate_bps: 0,
        eta_ms: None,
        last_error: t.last_error.clone(),
    })
}
