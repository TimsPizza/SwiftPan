//! Public facade and application-layer orchestration for uploads.
//!
//! This module owns bridge-facing DTOs, task spawning, credential/operator
//! coordination, Tauri event emission, and composition of upload adapters. It
//! must not contain local-file chunk loops, MIME rules, global registry
//! implementation, stream-channel mechanics, or Android SAF source handling.

use crate::settings;
use crate::transfer_db::{TransferLifecycle, TransferPhase, TransferSnapshot};
use crate::transfer_fsm::TransferStateEvent;
use crate::types::*;
use crate::{sp_backend::SpBackend, storage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use tauri::Emitter;
use tokio::sync::mpsc;

mod engine;
mod metadata;
mod platform;
mod runtime;
mod stream;

use engine::*;
use metadata::*;
use runtime::*;
use stream::*;

#[cfg(test)]
pub(crate) use engine::{
    upload_file as upload_file_for_integration, UploadControl as IntegrationUploadControl,
    UploadEngineObserver as IntegrationUploadObserver,
    UploadEngineRequest as IntegrationUploadRequest,
};

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

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn emit_upload(app: &tauri::AppHandle, event: &UploadEvent) {
    let _ = app.emit("sp://upload_event", event);
}

fn emit_part_events(
    app: &tauri::AppHandle,
    transfer_id: &str,
    part_number: u32,
    bytes_transferred: u64,
) {
    emit_upload(
        app,
        &UploadEvent::PartProgress {
            transfer_id: transfer_id.to_string(),
            progress: UploadPartProgress {
                part_number,
                bytes_transferred,
            },
        },
    );
    emit_upload(
        app,
        &UploadEvent::PartDone {
            transfer_id: transfer_id.to_string(),
            part_number,
            etag: String::new(),
        },
    );
}

struct RuntimeUploadObserver<'a> {
    app: &'a tauri::AppHandle,
    transfer_id: &'a str,
}

impl UploadEngineObserver for RuntimeUploadObserver<'_> {
    fn uploading(&mut self) -> SpResult<()> {
        transition_upload(
            self.transfer_id,
            TransferStateEvent::Run(TransferPhase::UploadingRemote),
        )?;
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        transition_upload(self.transfer_id, TransferStateEvent::Pause)?;
        emit_upload(
            self.app,
            &UploadEvent::Paused {
                transfer_id: self.transfer_id.to_string(),
            },
        );
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        transition_upload(
            self.transfer_id,
            TransferStateEvent::Run(TransferPhase::UploadingRemote),
        )?;
        emit_upload(
            self.app,
            &UploadEvent::Resumed {
                transfer_id: self.transfer_id.to_string(),
            },
        );
        Ok(())
    }

    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()> {
        mutate_upload(self.transfer_id, |transfer| {
            transfer.bytes_done = transfer.bytes_done.saturating_add(bytes_transferred);
            transfer.parts_completed += 1;
        })?;
        emit_part_events(self.app, self.transfer_id, part_number, bytes_transferred);
        Ok(())
    }

    fn finalizing(&mut self) -> SpResult<()> {
        transition_upload(
            self.transfer_id,
            TransferStateEvent::Run(TransferPhase::FinalizingRemote),
        )?;
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        transition_upload(self.transfer_id, TransferStateEvent::CancelConfirm)?;
        emit_upload(
            self.app,
            &UploadEvent::Cancelled {
                transfer_id: self.transfer_id.to_string(),
            },
        );
        Ok(())
    }
}

impl StreamUploadObserver for RuntimeUploadObserver<'_> {
    fn uploading(&mut self) -> SpResult<()> {
        UploadEngineObserver::uploading(self)
    }

    fn paused(&mut self) -> SpResult<()> {
        UploadEngineObserver::paused(self)
    }

    fn resumed(&mut self) -> SpResult<()> {
        UploadEngineObserver::resumed(self)
    }

    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()> {
        UploadEngineObserver::part_done(self, part_number, bytes_transferred)
    }

    fn finalizing(&mut self) -> SpResult<()> {
        UploadEngineObserver::finalizing(self)
    }

    fn cancelled(&mut self) -> SpResult<()> {
        UploadEngineObserver::cancelled(self)
    }
}

fn start_event(app: &tauri::AppHandle, id: &str) -> SpResult<()> {
    transition_upload(id, TransferStateEvent::Run(TransferPhase::PreparingSource))?;
    emit_upload(
        app,
        &UploadEvent::Started {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

fn finish_upload_task(app: &tauri::AppHandle, id: &str, result: SpResult<()>) {
    if let Err(error) = result {
        let _ = mutate_upload(id, |transfer| {
            transfer.worker_active = false;
            transfer.last_error = Some(error.clone());
        });
        if !matches!(error.kind, ErrorKind::Cancelled) {
            let _ = transition_upload(id, TransferStateEvent::Fail);
            emit_upload(
                app,
                &UploadEvent::Failed {
                    transfer_id: id.to_string(),
                    error,
                },
            );
        }
    } else {
        let _ = mutate_upload(id, |transfer| transfer.worker_active = false);
    }
}

async fn complete_file_upload(
    app: &tauri::AppHandle,
    id: &str,
    params: &NewUploadParams,
    operator: &opendal::Operator,
    should_upload_thumbnail: bool,
) -> SpResult<()> {
    if should_upload_thumbnail {
        let thumbnail_operator = operator.clone();
        let source_path = params.source_path.clone();
        let object_key = params.key.clone();
        tokio::spawn(async move {
            match crate::thumbnail::generate_and_store(
                &thumbnail_operator,
                &object_key,
                &source_path,
                128,
                16 * 1024,
            )
            .await
            {
                Ok(Some(_)) => {}
                Ok(None) => crate::logger::info(
                    "upload",
                    &format!("thumbnail skipped for {object_key}; unsupported file type"),
                ),
                Err(error) => crate::logger::warn(
                    "upload",
                    &format!(
                        "thumbnail generation failed for {object_key}: {}",
                        error.message
                    ),
                ),
            }
        });
    }
    transition_upload(id, TransferStateEvent::Complete)?;
    emit_upload(
        app,
        &UploadEvent::Completed {
            transfer_id: id.to_string(),
        },
    );
    Ok(())
}

pub async fn start_upload(app: tauri::AppHandle, params: NewUploadParams) -> SpResult<String> {
    let metadata = tokio::fs::metadata(&params.source_path)
        .await
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("stat src: {error}"),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        })?;
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    register_upload(
        &id,
        params.key.clone(),
        PathBuf::from(&params.source_path),
        params.part_size.max(8 * 1024 * 1024),
        metadata.len(),
        paused.clone(),
        cancelled.clone(),
    )?;

    let task_id = id.clone();
    let task_app = app.clone();
    let _ = mutate_upload(&id, |transfer| transfer.worker_active = true);
    tokio::spawn(async move {
        let result = async {
            let should_upload_thumbnail = settings::get().upload_thumbnail;
            start_event(&task_app, &task_id)?;
            let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
            let operator = storage::build_operator(&bundle.r2).await?;
            let mut observer = RuntimeUploadObserver {
                app: &task_app,
                transfer_id: &task_id,
            };
            upload_file(
                &operator,
                UploadEngineRequest {
                    key: params.key.clone(),
                    source_path: PathBuf::from(&params.source_path),
                    part_size: params.part_size,
                    content_type: params.content_type.clone(),
                    content_disposition: params.content_disposition.clone(),
                },
                UploadControl { paused, cancelled },
                &mut observer,
            )
            .await?;
            complete_file_upload(
                &task_app,
                &task_id,
                &params,
                &operator,
                should_upload_thumbnail,
            )
            .await
        }
        .await;
        finish_upload_task(&task_app, &task_id, result);
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
    register_upload(
        &id,
        params.key.clone(),
        PathBuf::new(),
        params.part_size.max(512 * 1024),
        params.bytes_total,
        paused.clone(),
        cancelled.clone(),
    )?;
    let (sender, receiver) = mpsc::channel(8);
    register_stream(id.clone(), sender)?;

    let task_id = id.clone();
    let task_app = app.clone();
    let _ = mutate_upload(&id, |transfer| transfer.worker_active = true);
    tokio::spawn(async move {
        let result = async {
            start_event(&task_app, &task_id)?;
            if settings::get().upload_thumbnail {
                crate::logger::warn(
                    "sp.backend",
                    "upload_thumbnail=true; streaming mode does not auto-upload thumbnails",
                );
            }
            let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
            let operator = storage::build_operator(&bundle.r2).await?;
            let mut observer = RuntimeUploadObserver {
                app: &task_app,
                transfer_id: &task_id,
            };
            upload_stream(
                &operator,
                StreamUploadRequest {
                    key: params.key,
                    content_type: params.content_type,
                    content_disposition: params.content_disposition,
                },
                receiver,
                UploadControl { paused, cancelled },
                &mut observer,
            )
            .await?;
            transition_upload(&task_id, TransferStateEvent::Complete)?;
            emit_upload(
                &task_app,
                &UploadEvent::Completed {
                    transfer_id: task_id.clone(),
                },
            );
            Ok(())
        }
        .await;
        finish_upload_task(&task_app, &task_id, result);
        unregister_stream(&task_id);
    });
    Ok(id)
}

#[cfg(target_os = "android")]
pub async fn start_upload_android_uri(
    app: tauri::AppHandle,
    key: String,
    uri: String,
    part_size: u64,
    content_type: Option<String>,
) -> SpResult<String> {
    platform::start_upload_android_uri(app, key, uri, part_size, content_type).await
}

pub fn stream_write(id: &str, chunk: Vec<u8>) -> SpResult<()> {
    runtime::stream_write(id, chunk)
}

pub fn stream_finish(id: &str) -> SpResult<()> {
    runtime::stream_finish(id)
}

pub fn pause(app: &tauri::AppHandle, id: &str) -> SpResult<()> {
    pause_upload(id)?;
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
    let phase = resume_upload(id)?;
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
    cancel_upload(id)?;
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
    upload_status(id)
}

pub fn list_active_snapshots() -> Vec<TransferSnapshot> {
    runtime::list_active_snapshots()
}

pub fn remove(id: &str) -> SpResult<()> {
    remove_upload(id)
}

#[cfg(test)]
mod tests;
