//! Credential and backend-state Tauri commands.
//!
//! This module owns credential status, redacted views, package import/export,
//! mutation commands, and legacy vault shims. It must not perform object,
//! transfer, platform filesystem, thumbnail, sharing, or usage operations.

use crate::sp_backend::{
    BackendPackage, BackendState as BackendStatus, CredentialBundle, SpBackend,
};
use crate::types::{ErrorKind, SpError, SpResult};
use base64::Engine;

#[tauri::command]
pub async fn backend_status() -> SpResult<BackendStatus> {
    let result = SpBackend::status();
    match &result {
        Ok(_) => crate::logger::info("bridge", "backend_status ok"),
        Err(error) => {
            crate::logger::info("bridge", &format!("backend_status err: {}", error.message))
        }
    }
    result
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RedactedCredentials {
    pub endpoint: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
    pub region: Option<String>,
}

#[tauri::command]
pub async fn backend_credentials_redacted() -> SpResult<RedactedCredentials> {
    crate::logger::debug("bridge", "backend_credentials_redacted");
    let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
    Ok(RedactedCredentials {
        endpoint: redact_endpoint(&bundle.r2.endpoint),
        access_key_id: redact_key(&bundle.r2.access_key_id),
        secret_access_key: redact_key(&bundle.r2.secret_access_key),
        bucket: redact_key(&bundle.r2.bucket),
        region: bundle.r2.region,
    })
}

#[derive(serde::Serialize)]
pub struct CredentialExportPayload {
    pub encoded: String,
}

#[tauri::command]
pub async fn backend_export_credentials_package() -> SpResult<CredentialExportPayload> {
    crate::logger::info("bridge", "backend_export_credentials_package");
    let package = SpBackend::export_package()?;
    let json = serde_json::to_vec(&package).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize export package: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(json);
    Ok(CredentialExportPayload { encoded })
}

#[tauri::command]
pub async fn backend_import_credentials_package(encoded: String) -> SpResult<()> {
    crate::logger::info("bridge", "backend_import_credentials_package");
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(encoded.trim().as_bytes())
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode package payload: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let package: BackendPackage = serde_json::from_slice(&bytes).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("parse package payload: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    SpBackend::import_package(package)
}

fn redact_endpoint(endpoint: &str) -> String {
    if let Some(rest) = endpoint.strip_prefix("https://") {
        if let Some(index) = rest.find('.') {
            let host_tail = &rest[index..];
            return format!("https://{}{}", "*****", host_tail);
        }
    }
    let mut parts = endpoint.splitn(2, '.');
    if let Some(first) = parts.next() {
        let tail = parts.next().unwrap_or("");
        let masked = if first.starts_with("http") {
            format!("{}***", &first[..first.len().min(4)])
        } else {
            "*****".into()
        };
        if tail.is_empty() {
            return masked;
        }
        return format!("{masked}.{tail}");
    }
    "*****".into()
}

fn redact_key(value: &str) -> String {
    let length = value.len();
    if length <= 4 {
        return "****".into();
    }
    let keep = 4;
    let head = &value[..keep.min(length)];
    format!("{}{}", head, "*".repeat(length.saturating_sub(keep)))
}

#[tauri::command]
pub async fn backend_set_credentials(bundle: CredentialBundle) -> SpResult<()> {
    crate::logger::info("bridge", "backend_set_credentials called");
    let result = SpBackend::set_with_plaintext(bundle);
    match &result {
        Ok(_) => crate::logger::info("bridge", "backend_set_credentials ok"),
        Err(error) => crate::logger::info(
            "bridge",
            &format!("backend_set_credentials err: {}", error.message),
        ),
    }
    if result.is_ok() {
        crate::r2_client::invalidate_cached_client().await;
    }
    result
}

#[tauri::command]
pub async fn backend_patch_credentials(patch: crate::sp_backend::R2ConfigPatch) -> SpResult<()> {
    crate::logger::info("bridge", "backend_patch_credentials called");
    let result = SpBackend::patch_r2_config(patch);
    match &result {
        Ok(_) => crate::logger::info("bridge", "backend_patch_credentials ok"),
        Err(error) => crate::logger::info(
            "bridge",
            &format!("backend_patch_credentials err: {}", error.message),
        ),
    }
    if result.is_ok() {
        crate::r2_client::invalidate_cached_client().await;
    }
    result
}

#[tauri::command]
pub async fn vault_status() -> SpResult<BackendStatus> {
    backend_status().await
}

#[tauri::command]
pub async fn vault_set_manual(bundle: CredentialBundle) -> SpResult<()> {
    backend_set_credentials(bundle).await
}

#[cfg(test)]
mod tests;
