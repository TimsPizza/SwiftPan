//! Password-based key derivation primitives.
//!
//! This module owns the exact Argon2id parameters-to-key operation used by
//! portable credential packages. It must not choose package fields, read or
//! write vault files, access device keys, call platform keystores, or mutate
//! runtime credential state.

use super::KdfParams;
use crate::types::{ErrorKind, SpError, SpResult};
use argon2::{Algorithm, Argon2, Params, Version};

pub(super) fn derive_argon2_key(password: &str, params: &KdfParams) -> SpResult<[u8; 32]> {
    if params.mem_kib == 0 || params.iterations == 0 || params.parallelism == 0 {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "invalid argon2 params".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    let argon_params = Params::new(
        params.mem_kib,
        params.iterations,
        params.parallelism,
        Some(32),
    )
    .map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("argon2 params: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut key = [0; 32];
    argon2
        .hash_password_into(password.as_bytes(), &params.salt, &mut key)
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("argon2 derive failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    Ok(key)
}

#[cfg(test)]
mod tests;
