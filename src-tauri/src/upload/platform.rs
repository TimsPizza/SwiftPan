//! Platform-specific upload sources and staging.
//!
//! This module owns Android SAF source access and Android-only thumbnail
//! staging. It may coordinate the existing runtime and remote writer because
//! the platform file handle is not an async Rust source. It must not define the
//! desktop file engine, the stream-channel protocol, or public bridge DTOs.

#[cfg(target_os = "android")]
use super::*;
#[cfg(target_os = "android")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "android")]
fn android_thumbnail_temp_path(transfer_id: &str, object_key: &str) -> SpResult<PathBuf> {
    let mut dir = crate::sp_backend::vault_dir()?;
    dir.push("uploads");
    dir.push("thumbnail_staging");
    std::fs::create_dir_all(&dir).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create thumbnail staging dir: {error}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    let extension = std::path::Path::new(object_key)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("bin");
    dir.push(format!("{transfer_id}.{extension}"));
    Ok(dir)
}

#[cfg(target_os = "android")]
pub(super) async fn start_upload_android_uri(
    app: tauri::AppHandle,
    key: String,
    uri: String,
    part_size: u64,
    content_type: Option<String>,
) -> SpResult<String> {
    use std::io::Read;
    use tauri_plugin_android_fs::AndroidFsExt as _;

    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    register_upload(
        &id,
        key.clone(),
        PathBuf::new(),
        part_size.max(512 * 1024),
        0,
        paused.clone(),
        cancelled.clone(),
    )?;

    let task_id = id.clone();
    let task_app = app.clone();
    let _ = mutate_upload(&id, |transfer| transfer.worker_active = true);
    tokio::spawn(async move {
        let result = async {
            let should_upload_thumbnail = settings::get().upload_thumbnail;
            transition_upload(
                &task_id,
                TransferStateEvent::Run(TransferPhase::PreparingSource),
            )?;
            emit_upload(
                &task_app,
                &UploadEvent::Started {
                    transfer_id: task_id.clone(),
                },
            );

            let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
            let operator = crate::storage::build_operator(&bundle.r2).await?;
            let mut writer = open_upload_writer(&operator, &key, content_type.as_deref(), None)
                .await
                .map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("open writer: {error}"),
                    retry_after_ms: Some(500),
                    context: None,
                    at: now_ms(),
                })?;
            transition_upload(
                &task_id,
                TransferStateEvent::Run(TransferPhase::UploadingRemote),
            )?;

            let api = task_app.android_fs();
            let file_uri = tauri_plugin_android_fs::FileUri {
                uri: uri.clone(),
                document_top_tree_uri: None,
            };
            let mut file = api.open_file_readable(&file_uri).map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("open_file_readable: {error}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
            if let Ok(metadata) = file.metadata() {
                let _ = mutate_upload(&task_id, |transfer| transfer.bytes_total = metadata.len());
            }

            let mut part_number = 1;
            let mut buffer = vec![0; part_size.max(256 * 1024) as usize];
            let mut was_paused = false;
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                while paused.load(Ordering::Relaxed) {
                    if !was_paused {
                        transition_upload(&task_id, TransferStateEvent::Pause)?;
                        emit_upload(
                            &task_app,
                            &UploadEvent::Paused {
                                transfer_id: task_id.clone(),
                            },
                        );
                        was_paused = true;
                    }
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                if was_paused {
                    transition_upload(
                        &task_id,
                        TransferStateEvent::Run(TransferPhase::UploadingRemote),
                    )?;
                    emit_upload(
                        &task_app,
                        &UploadEvent::Resumed {
                            transfer_id: task_id.clone(),
                        },
                    );
                    was_paused = false;
                }

                let read = file.read(&mut buffer).map_err(|error| SpError {
                    kind: ErrorKind::RetryableNet,
                    message: format!("read src: {error}"),
                    retry_after_ms: Some(200),
                    context: None,
                    at: now_ms(),
                })?;
                if read == 0 {
                    break;
                }
                writer
                    .write(buffer[..read].to_vec())
                    .await
                    .map_err(|error| SpError {
                        kind: ErrorKind::RetryableNet,
                        message: format!("writer write: {error}"),
                        retry_after_ms: Some(300),
                        context: None,
                        at: now_ms(),
                    })?;
                mutate_upload(&task_id, |transfer| {
                    transfer.bytes_done = transfer.bytes_done.saturating_add(read as u64);
                    transfer.parts_completed += 1;
                })?;
                emit_part_events(&task_app, &task_id, part_number, read as u64);
                part_number += 1;
            }

            if cancelled.load(Ordering::Relaxed) {
                let _ = writer.abort().await;
                transition_upload(&task_id, TransferStateEvent::CancelConfirm)?;
                emit_upload(
                    &task_app,
                    &UploadEvent::Cancelled {
                        transfer_id: task_id.clone(),
                    },
                );
                return Err(cancelled_error());
            }

            transition_upload(
                &task_id,
                TransferStateEvent::Run(TransferPhase::FinalizingRemote),
            )?;
            writer.close().await.map_err(|error| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("writer close: {error}"),
                retry_after_ms: Some(300),
                context: None,
                at: now_ms(),
            })?;

            if should_upload_thumbnail {
                upload_android_thumbnail(&task_app, &task_id, &key, &uri, &operator).await?;
            }
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
    });
    Ok(id)
}

#[cfg(target_os = "android")]
async fn upload_android_thumbnail(
    app: &tauri::AppHandle,
    transfer_id: &str,
    key: &str,
    uri: &str,
    operator: &opendal::Operator,
) -> SpResult<()> {
    let temp_path = android_thumbnail_temp_path(transfer_id, key)?;
    let temp = temp_path.to_string_lossy().to_string();
    let copy_result = crate::bridge::android_fs_copy(
        app.clone(),
        crate::bridge::AndroidFsCopyParams {
            direction: "uri_to_sandbox".into(),
            local_path: temp.clone(),
            tree_uri: None,
            relative_path: None,
            mime: None,
            uri: Some(uri.to_string()),
        },
    )
    .await;
    match copy_result {
        Ok(()) => {
            match crate::thumbnail::generate_and_store(operator, key, &temp, 128, 16 * 1024).await {
                Ok(Some(_)) => {}
                Ok(None) => crate::logger::info(
                    "upload",
                    &format!("android thumbnail skipped for {key}; unsupported file type"),
                ),
                Err(error) => crate::logger::warn(
                    "upload",
                    &format!(
                        "android thumbnail generation failed for {key}: {}",
                        error.message
                    ),
                ),
            }
            let _ = tokio::fs::remove_file(&temp).await;
        }
        Err(error) => crate::logger::warn(
            "upload",
            &format!(
                "android thumbnail source materialize failed for {key}: {}",
                error.message
            ),
        ),
    }
    Ok(())
}
