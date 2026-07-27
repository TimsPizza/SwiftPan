//! Vault directory resolution and legacy data migration.
//!
//! This module owns application-data path selection and non-destructive copying
//! from legacy locations. It must not interpret credential files, encrypt data,
//! access device keys, call platform keystores, or mutate runtime credentials.

use crate::types::{ErrorKind, SpError, SpResult};
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

static APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();

pub fn init(app: &tauri::AppHandle) -> SpResult<()> {
    let _ = APP_HANDLE.set(app.clone());
    migrate_legacy_vault_dir()
}

pub(crate) fn vault_dir() -> SpResult<PathBuf> {
    if let Some(app) = APP_HANDLE.get() {
        return app.path().app_data_dir().map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("resolve app data dir failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    if let Some(project) = ProjectDirs::from("com", "swiftpan", "SwiftPan") {
        return Ok(project.data_dir().to_path_buf());
    }
    if let Ok(custom) = env::var("SWIFTPAN_DATA_DIR") {
        return Ok(PathBuf::from(custom));
    }
    Ok(env::temp_dir().join("swiftpan"))
}

fn migrate_legacy_vault_dir() -> SpResult<()> {
    let target_dir = vault_dir()?;
    fs::create_dir_all(&target_dir).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create app data dir failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    for legacy_dir in legacy_vault_dirs() {
        if legacy_dir == target_dir || !legacy_dir.exists() {
            continue;
        }
        migrate_dir_contents(&legacy_dir, &target_dir)?;
    }
    Ok(())
}

fn legacy_vault_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(project) = ProjectDirs::from("com", "swiftpan", "SwiftPan") {
        directories.push(project.data_dir().to_path_buf());
    }
    if let Ok(custom) = env::var("SWIFTPAN_DATA_DIR") {
        directories.push(PathBuf::from(custom));
    }
    directories.push(env::temp_dir().join("swiftpan"));
    directories
}

fn migrate_dir_contents(from: &Path, to: &Path) -> SpResult<()> {
    if !from.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(from).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read legacy data dir failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })? {
        let entry = entry.map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("read legacy dir entry failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        if source_path.is_dir() {
            fs::create_dir_all(&target_path).map_err(|error| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create migrated dir failed: {error}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
            migrate_dir_contents(&source_path, &target_path)?;
            continue;
        }
        if target_path.exists() {
            continue;
        }
        let _ = fs::copy(&source_path, &target_path);
    }
    Ok(())
}
