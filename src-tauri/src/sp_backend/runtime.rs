//! In-process credential cache and derived backend status.
//!
//! This module owns the global decrypted bundle cache and lock handling. It
//! must not read vault files, encrypt packages, resolve paths, access device
//! keys, or call platform keystores.

use super::model::{BackendState, CredentialBundle, SpBackend};
use crate::types::{ErrorKind, SpError, SpResult};
use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Default)]
struct BackendMemory {
    credentials: Option<CredentialBundle>,
}

static STATE: Lazy<Mutex<BackendMemory>> = Lazy::new(|| Mutex::new(BackendMemory::default()));

pub(super) fn cached_credentials() -> SpResult<Option<CredentialBundle>> {
    STATE
        .lock()
        .map(|state| state.credentials.clone())
        .map_err(|_| {
            crate::logger::error(
                "sp_backend",
                "get_decrypted_bundle_if_unlocked backend state lock poisoned",
            );
            state_lock_error()
        })
}

pub(super) fn cache_credentials(bundle: CredentialBundle) -> SpResult<()> {
    STATE.lock().map_err(|_| state_lock_error())?.credentials = Some(bundle);
    Ok(())
}

pub(super) fn current_state() -> BackendState {
    let memory_bundle = match STATE.lock() {
        Ok(state) => state.credentials.clone(),
        Err(poisoned) => poisoned.into_inner().credentials.clone(),
    };
    let disk_bundle = if memory_bundle.is_some() {
        None
    } else {
        SpBackend::get_decrypted_bundle_if_unlocked().ok()
    };
    let bundle = memory_bundle.or(disk_bundle);
    let is_credential_completed = bundle.as_ref().is_some_and(|bundle| {
        !bundle.r2.endpoint.is_empty()
            && !bundle.r2.access_key_id.is_empty()
            && !bundle.r2.secret_access_key.is_empty()
            && !bundle.r2.bucket.is_empty()
    });
    BackendState {
        is_unlocked: bundle.is_some(),
        unlock_deadline_ms: None,
        device_id: "dev-removed".into(),
        is_credential_completed,
        is_credential_valid: is_credential_completed,
    }
}

fn state_lock_error() -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: "backend state lock poisoned".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}
