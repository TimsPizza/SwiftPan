//! Platform-specific target access and final materialization.
//!
//! This module owns Android SAF permission checks and copying a completed staged
//! file into its final platform target. It may drive materialization-related
//! FSM transitions, but it must not download remote ranges, construct R2
//! clients, or own persistent/runtime transfer state.

use super::{transition_transfer, DownloadTarget};
use crate::bridge::AndroidFsCopyParams;
use crate::transfer_db::TransferPhase;
use crate::transfer_fsm::TransferStateEvent;
use crate::types::SpResult;
use std::path::Path;

#[cfg(target_os = "android")]
use super::now_ms;
#[cfg(target_os = "android")]
use crate::types::{ErrorKind, SpError};
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFsExt as _, FileUri, PersistableAccessMode};

pub(super) fn ensure_resume_target_access(
    _app: &tauri::AppHandle,
    _target: &DownloadTarget,
) -> SpResult<()> {
    #[cfg(target_os = "android")]
    {
        if let DownloadTarget::AndroidTree { tree_uri, .. } = _target {
            let api = _app.android_fs();
            let uri = FileUri::from_str(tree_uri).map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("tree uri parse: {error}"),
                retry_after_ms: None,
                context: None,
                at: now_ms(),
            })?;
            let has_permission = api
                .check_persisted_uri_permission(&uri, PersistableAccessMode::ReadAndWrite)
                .map_err(|error| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("check persisted SAF permission: {error}"),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                })?;
            if !has_permission {
                return Err(SpError {
                    kind: ErrorKind::NotRetriable,
                    message: "android download directory permission lost; choose directory again"
                        .into(),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                });
            }
        }
    }
    #[allow(unreachable_code)]
    Ok(())
}

pub(super) async fn materialize_target(
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
