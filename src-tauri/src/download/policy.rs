//! Pure download policies and deterministic helper rules.
//!
//! This module contains side-effect-free decisions such as partial-file naming,
//! restart lifecycle mapping, artifact retention, failure-reason projection,
//! and range construction. It must not access files, SQLite, Tauri, clocks, or
//! remote storage so every rule remains cheap to test exhaustively.

use crate::transfer_db::TransferLifecycle;
use crate::types::{ErrorKind, SpError};
use std::path::{Path, PathBuf};

pub(super) fn part_path_for(temp_path: &Path) -> PathBuf {
    let mut part_name = temp_path.as_os_str().to_os_string();
    part_name.push(".part");
    PathBuf::from(part_name)
}

pub(super) fn last_fail_reason_for(
    lifecycle_state: TransferLifecycle,
    last_error: Option<&SpError>,
) -> Option<ErrorKind> {
    if !matches!(lifecycle_state, TransferLifecycle::Failed) {
        return None;
    }
    last_error.map(|error| error.kind.clone())
}

pub(super) fn should_keep_failed_artifacts(reason: Option<&ErrorKind>) -> bool {
    matches!(reason, Some(ErrorKind::RetryableNet))
}

pub(super) fn lifecycle_after_restart(lifecycle: &TransferLifecycle) -> TransferLifecycle {
    match lifecycle {
        TransferLifecycle::Queued | TransferLifecycle::Running => TransferLifecycle::Paused,
        TransferLifecycle::Cancelling => TransferLifecycle::Cancelled,
        other => other.clone(),
    }
}

pub(super) fn next_download_range(
    offset: u64,
    total: u64,
    chunk_size: u64,
) -> Option<std::ops::Range<u64>> {
    if offset >= total || chunk_size == 0 {
        return None;
    }
    Some(offset..offset.saturating_add(chunk_size).min(total))
}
