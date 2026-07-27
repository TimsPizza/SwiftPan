//! Per-device vault-key storage and compatibility migration.
//!
//! This module owns creation/loading of the 256-bit device key, legacy
//! plaintext key decoding, and Android wrapped-key envelope serialization. File
//! names and wrapped envelope fields are durable compatibility contracts. It
//! must not encrypt credential packages or implement Android Keystore JNI.

#[cfg(target_os = "android")]
use super::android_keystore::{android_keystore_decrypt, android_keystore_encrypt};
use super::model::DEVICE_KEY_FILE_NAME;
#[cfg(target_os = "android")]
use super::model::DEVICE_KEY_WRAPPED_FILE_NAME;
use super::paths::vault_dir;
use crate::types::{ErrorKind, SpError, SpResult};
use base64::Engine;
use rand::{rngs::OsRng, RngCore};
use std::fs;
use std::path::Path;

#[cfg(target_os = "android")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WrappedDeviceKey {
    version: u8,
    iv_b64: String,
    ciphertext_b64: String,
}

pub(super) fn load_or_create_device_key() -> SpResult<[u8; 32]> {
    let directory = vault_dir()?;
    #[cfg(not(target_os = "android"))]
    let key_path = directory.join(DEVICE_KEY_FILE_NAME);
    if let Ok(key) = load_existing_device_key() {
        return Ok(key);
    }
    let mut key = [0; 32];
    OsRng.fill_bytes(&mut key);
    fs::create_dir_all(&directory).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create_dir_all failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    #[cfg(target_os = "android")]
    {
        let wrapped_path = directory.join(DEVICE_KEY_WRAPPED_FILE_NAME);
        store_android_wrapped_device_key(&wrapped_path, &key)?;
        return Ok(key);
    }
    #[cfg(not(target_os = "android"))]
    {
        fs::write(&key_path, key).map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("write device.key failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        Ok(key)
    }
}

pub(super) fn load_existing_device_key() -> SpResult<[u8; 32]> {
    let directory = vault_dir()?;
    let key_path = directory.join(DEVICE_KEY_FILE_NAME);
    #[cfg(target_os = "android")]
    {
        let wrapped_path = directory.join(DEVICE_KEY_WRAPPED_FILE_NAME);
        if wrapped_path.exists() {
            return load_android_wrapped_device_key(&wrapped_path);
        }
        if key_path.exists() {
            let key = load_plaintext_device_key(&key_path)?;
            store_android_wrapped_device_key(&wrapped_path, &key)?;
            let _ = fs::remove_file(&key_path);
            return Ok(key);
        }
    }
    #[cfg(not(target_os = "android"))]
    if key_path.exists() {
        return load_plaintext_device_key(&key_path);
    }
    Err(SpError {
        kind: ErrorKind::NotRetriable,
        message: "device key not found".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

fn load_plaintext_device_key(key_path: &Path) -> SpResult<[u8; 32]> {
    let data = fs::read(key_path).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read device.key failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let mut key = [0; 32];
    if data.len() == 32 {
        key.copy_from_slice(&data);
        return Ok(key);
    }
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(&data) {
        if decoded.len() == 32 {
            key.copy_from_slice(&decoded);
            return Ok(key);
        }
    }
    Err(SpError {
        kind: ErrorKind::NotRetriable,
        message: "invalid device.key".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

#[cfg(target_os = "android")]
fn store_android_wrapped_device_key(path: &Path, key: &[u8; 32]) -> SpResult<()> {
    let (iv, ciphertext) = android_keystore_encrypt(key)?;
    let wrapped = WrappedDeviceKey {
        version: 1,
        iv_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(iv),
        ciphertext_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(ciphertext),
    };
    let bytes = serde_json::to_vec(&wrapped).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize wrapped device key failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(path, bytes).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("write wrapped device key failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

#[cfg(target_os = "android")]
fn load_android_wrapped_device_key(path: &Path) -> SpResult<[u8; 32]> {
    let bytes = fs::read(path).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read wrapped device key failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let wrapped: WrappedDeviceKey = serde_json::from_slice(&bytes).map_err(|error| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("parse wrapped device key failed: {error}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let iv = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(wrapped.iv_b64.as_bytes())
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode wrapped key iv failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let ciphertext = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(wrapped.ciphertext_b64.as_bytes())
        .map_err(|error| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode wrapped key ciphertext failed: {error}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let plaintext = android_keystore_decrypt(&iv, &ciphertext)?;
    if plaintext.len() != 32 {
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "invalid wrapped device key length".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    let mut key = [0; 32];
    key.copy_from_slice(&plaintext);
    Ok(key)
}
