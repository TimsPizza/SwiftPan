//! Download target model and target-path derivation.
//!
//! This module validates desktop versus Android target parameters, converts
//! persisted target fields, normalizes destination paths, and derives Android
//! staging paths. It does not copy bytes to the final target or check live SAF
//! permissions; those platform operations belong in `platform`.

use super::{now_ms, NewDownloadParams};
use crate::transfer_db::{TransferKind, TransferSnapshot};
use crate::types::{err_invalid, ErrorKind, SpError, SpResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) enum DownloadTarget {
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
    pub(super) fn from_params(params: &NewDownloadParams) -> SpResult<Self> {
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

    pub(super) fn from_snapshot(snapshot: &TransferSnapshot) -> SpResult<Self> {
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

    pub(super) fn temp_path_for(&self, transfer_id: &str, key: &str) -> SpResult<PathBuf> {
        match self {
            Self::FileSystem { dest } => Ok(dest.clone()),
            Self::AndroidTree { relative_path, .. } => {
                let mut dir = download_stage_dir()?;
                let fallback_name = sanitize_filename(key);
                let basename = Path::new(relative_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(fallback_name.as_str())
                    .to_string();
                dir.push(transfer_id);
                std::fs::create_dir_all(&dir).map_err(|error| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("create download stage dir: {error}"),
                    retry_after_ms: None,
                    context: None,
                    at: now_ms(),
                })?;
                dir.push(basename);
                Ok(dir)
            }
        }
    }

    pub(super) fn snapshot_fields(&self) -> (Option<String>, Option<String>, Option<String>) {
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

pub(super) fn normalize_dest_path(raw: &str) -> SpResult<PathBuf> {
    let value = raw.trim();
    let value = value.strip_prefix("file://").unwrap_or(value);
    if value.contains("://") {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "unsupported URI for download destination".into(),
            retry_after_ms: None,
            context: None,
            at: now_ms(),
        });
    }
    Ok(PathBuf::from(value))
}

pub(super) fn sanitize_filename(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => character,
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
    let mut path = crate::sp_backend::vault_dir()?;
    path.push("downloads");
    path.push("staging");
    std::fs::create_dir_all(&path).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create download stage dir: {error}"),
        retry_after_ms: None,
        context: None,
        at: now_ms(),
    })?;
    Ok(path)
}
