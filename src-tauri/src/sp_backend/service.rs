//! Credential-vault application service.
//!
//! This module implements the stable [`SpBackend`] facade: credential
//! mutation, encrypted vault persistence/loading, package import/export, and
//! coordination with device keys and runtime memory. It must not own platform
//! keystore JNI, directory migration, KDF primitives, or global cache details.

use super::crypto::derive_argon2_key;
use super::device_key::{load_existing_device_key, load_or_create_device_key};
use super::model::{
    BackendPackage, BackendState, CredentialBundle, KdfParams, R2ConfigPatch, SpBackend,
    EXPORT_SECRET, VAULT_FILE_NAME, VAULT_META_FILE_NAME,
};
use super::paths::vault_dir;
use super::runtime::{cache_credentials, cached_credentials, current_state};
use crate::types::*;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use rand::{rngs::OsRng, RngCore};
use std::fs;

impl SpBackend {
    pub fn status() -> SpResult<BackendState> {
        Ok(current_state())
    }

    pub fn set_with_plaintext(bundle: CredentialBundle) -> SpResult<()> {
        let dir = vault_dir()?;
        fs::create_dir_all(&dir).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("create_dir_all failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        // Serialize bundle
        let plaintext = serde_json::to_vec(&bundle).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize bundle: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        // Derive/store a per-device key (no user unlock required)
        let key = load_or_create_device_key()?;
        // Marker KDF params for compatibility
        let zero_salt = [0u8; 16];
        // keep zero
        let kdf_params = KdfParams {
            algo: "device-key".into(),
            mem_kib: 0,
            iterations: 0,
            parallelism: 0,
            salt: zero_salt,
        };

        // Encrypt
        let cipher = XChaCha20Poly1305::new((&key).into());
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt((&nonce).into(), plaintext.as_slice())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("encrypt failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        let pkg = BackendPackage {
            version: 1,
            kdf: kdf_params,
            nonce_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(nonce),
            ciphertext_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(ciphertext),
        };

        // Write files (avoid unwrap to prevent panic)
        let pkg_bytes = serde_json::to_vec(&pkg).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize credentials package failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        fs::write(dir.join(VAULT_FILE_NAME), pkg_bytes).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("write credentials file failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let meta = serde_json::json!({"version": 1});
        let meta_bytes = serde_json::to_vec_pretty(&meta).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize meta failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        fs::write(dir.join(VAULT_META_FILE_NAME), meta_bytes).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("write credentials meta failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        cache_credentials(bundle)
    }

    pub fn patch_r2_config(patch: R2ConfigPatch) -> SpResult<()> {
        // Load current bundle (from mem or disk). If vault doesn't exist yet, start from defaults
        let mut cur = match Self::get_decrypted_bundle_if_unlocked() {
            Ok(b) => b,
            Err(e) => {
                let dir = vault_dir()?;
                let vault_exists = dir.join(VAULT_FILE_NAME).exists();
                if !vault_exists {
                    // Start from an empty/default R2 config and apply patch below
                    CredentialBundle {
                        r2: R2Config {
                            endpoint: String::new(),
                            access_key_id: String::new(),
                            secret_access_key: String::new(),
                            bucket: String::new(),
                            region: None,
                        },
                    }
                } else {
                    // If a vault exists but couldn't be read/decrypted, bubble up the error
                    return Err(e);
                }
            }
        };
        // Apply provided fields
        if let Some(v) = patch.endpoint {
            cur.r2.endpoint = v;
        }
        if let Some(v) = patch.access_key_id {
            cur.r2.access_key_id = v;
        }
        if let Some(v) = patch.secret_access_key {
            cur.r2.secret_access_key = v;
        }
        if let Some(v) = patch.bucket {
            cur.r2.bucket = v;
        }
        if let Some(v) = patch.region {
            cur.r2.region = Some(v);
        }
        // Persist via existing set logic
        Self::set_with_plaintext(cur)
    }
    fn err_not_implemented(func: &str) -> SpError {
        crate::logger::error("sp_backend", format!("{func} not implemented").as_str());
        SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("{func} not implemented"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    }
    pub fn export_package() -> SpResult<BackendPackage> {
        let bundle = Self::get_decrypted_bundle_if_unlocked()?;
        let plaintext = serde_json::to_vec(&bundle).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize bundle: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        const MEM_KIB: u32 = 32 * 1024;
        const ITER: u32 = 3;
        const PAR: u32 = 1;
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        let kdf = KdfParams {
            algo: "argon2id".into(),
            mem_kib: MEM_KIB,
            iterations: ITER,
            parallelism: PAR,
            salt,
        };
        let key = derive_argon2_key(EXPORT_SECRET, &kdf)?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt((&nonce).into(), plaintext.as_slice())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("encrypt failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        Ok(BackendPackage {
            version: 1,
            kdf,
            nonce_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(nonce),
            ciphertext_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(ciphertext),
        })
    }

    pub fn import_package(pkg: BackendPackage) -> SpResult<()> {
        if pkg.kdf.algo != "argon2id" {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("unsupported kdf algo: {}", pkg.kdf.algo),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }

        let key = derive_argon2_key(EXPORT_SECRET, &pkg.kdf)?;
        let cipher = XChaCha20Poly1305::new((&key).into());

        let nonce = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(pkg.nonce_b64.as_bytes())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("decode nonce: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        let ciphertext = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(pkg.ciphertext_b64.as_bytes())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("decode ciphertext: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        let plaintext = cipher
            .decrypt((&*nonce).into(), ciphertext.as_slice())
            .map_err(|e| SpError {
                kind: ErrorKind::RetryableAuth,
                message: format!("decrypt failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        let bundle: CredentialBundle = serde_json::from_slice(&plaintext).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode bundle json: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;

        Self::set_with_plaintext(bundle)
    }

    pub fn rotate_password(_old_pw: &str, _new_pw: &str) -> SpResult<()> {
        Err(err_not_implemented("backend.rotate_password"))
    }

    pub fn get_decrypted_bundle_if_unlocked() -> SpResult<CredentialBundle> {
        if let Some(bundle) = cached_credentials()? {
            return Ok(bundle);
        }
        // Attempt lazy load from disk using device key
        crate::logger::debug(
            "sp_backend",
            "get_decrypted_bundle_if_unlocked attempting lazy load from disk",
        );
        let dir = vault_dir()?;
        let pkg_bytes = fs::read(dir.join(VAULT_FILE_NAME)).map_err(|e| {
            crate::logger::error(
                "sp_backend",
                "get_decrypted_bundle_if_unlocked read vault.sp failed",
            );
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("read vault.sp failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
        let pkg: BackendPackage = serde_json::from_slice(&pkg_bytes).map_err(|e| {
            crate::logger::error(
                "sp_backend",
                "get_decrypted_bundle_if_unlocked parse credentials package failed",
            );
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("parse credentials package failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
        let key = load_existing_device_key()?;
        let cipher = XChaCha20Poly1305::new((&key).into());
        let nonce = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(pkg.nonce_b64.as_bytes())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("decode nonce: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        let ct = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(pkg.ciphertext_b64.as_bytes())
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("decode ciphertext: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        let pt = cipher
            .decrypt((&*nonce).into(), ct.as_slice())
            .map_err(|e| SpError {
                kind: ErrorKind::RetryableAuth,
                message: format!("decrypt failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        let bundle: CredentialBundle = serde_json::from_slice(&pt).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode bundle json: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        cache_credentials(bundle.clone())?;
        Ok(bundle)
    }
}
