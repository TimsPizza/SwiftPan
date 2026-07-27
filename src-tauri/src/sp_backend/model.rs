//! Stable credential-backend data contracts.
//!
//! These serialized structures define bridge payloads and durable encrypted
//! package envelopes. Field names, types, defaults, and version semantics are
//! compatibility boundaries; this module must not perform I/O, cryptography,
//! platform access, or mutate runtime state.

use crate::types::{DeviceId, R2Config};

pub(super) const EXPORT_SECRET: &str = "swiftpan-export-v1";
pub(super) const VAULT_FILE_NAME: &str = "vault.sp";
pub(super) const VAULT_META_FILE_NAME: &str = "vault.meta.json";
pub(super) const DEVICE_KEY_FILE_NAME: &str = "device.key";
#[cfg(target_os = "android")]
pub(super) const DEVICE_KEY_WRAPPED_FILE_NAME: &str = "device.key.enc";
#[cfg(target_os = "android")]
pub(super) const ANDROID_KEY_ALIAS: &str = "com.timspizza.swiftpan.device_key.v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CredentialBundle {
    pub r2: R2Config,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct R2ConfigPatch {
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackendState {
    pub is_unlocked: bool,
    pub unlock_deadline_ms: Option<u64>,
    pub device_id: DeviceId,
    pub is_credential_completed: bool,
    pub is_credential_valid: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackendPackage {
    pub version: u16,
    pub kdf: KdfParams,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KdfParams {
    pub algo: String,
    pub mem_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: [u8; 16],
}

pub struct SpBackend;
