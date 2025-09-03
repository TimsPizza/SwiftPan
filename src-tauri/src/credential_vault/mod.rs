use crate::types::*;
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use rand::{rngs::OsRng, RngCore};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CredentialBundle {
    pub r2: R2Config,
    pub device_id: DeviceId,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultState {
    pub is_unlocked: bool,
    pub unlock_deadline_ms: Option<u64>,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultExportPackage {
    pub version: u16,
    pub kdf: KdfParams,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

pub struct CredentialVault;

impl CredentialVault {
    pub fn status() -> SpResult<VaultState> {
        Ok(current_state())
    }

    pub fn set_with_plaintext(bundle: CredentialBundle, master_password: &str) -> SpResult<()> {
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

        // KDF params
        let mut s = [0u8; 16];
        OsRng.fill_bytes(&mut s);
        let kdf = KdfParams {
            algo: "argon2id".into(),
            mem_kib: 65536,
            iterations: 3,
            parallelism: 2,
            salt: s,
        };
        let (key, kdf_params) = derive_key(master_password, Some(&kdf))?;

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

        let pkg = VaultExportPackage {
            version: 1,
            kdf: kdf_params,
            nonce_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(nonce),
            ciphertext_b64: base64::engine::general_purpose::STANDARD_NO_PAD.encode(ciphertext),
        };

        // Write files
        fs::write(dir.join("vault.sp"), serde_json::to_vec(&pkg).unwrap()).map_err(|e| {
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("write vault.sp failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
        let meta =
            serde_json::json!({"device_id": bundle.device_id, "created_at": bundle.created_at});
        fs::write(
            dir.join("vault.meta.json"),
            serde_json::to_vec_pretty(&meta).unwrap(),
        )
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("write meta failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        Ok(())
    }

    pub fn test_connectivity(_master_password: &str) -> SpResult<()> {
        // Will be implemented after R2 wiring to vault
        Err(err_not_implemented("vault.test_connectivity"))
    }

    pub fn unlock(master_password: &str, hold_ms: u64) -> SpResult<()> {
        let dir = vault_dir()?;
        let pkg_bytes = fs::read(dir.join("vault.sp")).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("read vault.sp failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let pkg: VaultExportPackage = serde_json::from_slice(&pkg_bytes).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("parse vault package failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let key = derive_key(master_password, Some(&pkg.kdf))?.0;
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
                message: format!("decrypt failed (bad password?): {e}"),
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

        let deadline = (chrono::Utc::now() + chrono::Duration::milliseconds(hold_ms as i64))
            .timestamp_millis() as u64;
        {
            let mut guard = STATE.lock().unwrap();
            guard.unlocked = Some(bundle);
            guard.unlock_deadline_ms = Some(deadline);
        }
        Ok(())
    }

    pub fn lock() -> SpResult<()> {
        let mut g = STATE.lock().unwrap();
        g.unlocked = None;
        g.unlock_deadline_ms = None;
        Ok(())
    }

    pub fn export_package(_master_password: &str) -> SpResult<VaultExportPackage> {
        Err(err_not_implemented("vault.export_package"))
    }

    pub fn import_package(_pkg: VaultExportPackage, _master_password: &str) -> SpResult<()> {
        Err(err_not_implemented("vault.import_package"))
    }

    pub fn rotate_password(_old_pw: &str, _new_pw: &str) -> SpResult<()> {
        Err(err_not_implemented("vault.rotate_password"))
    }

    pub fn get_decrypted_bundle_if_unlocked() -> SpResult<CredentialBundle> {
        let st = STATE.lock().unwrap();
        if let (Some(b), Some(deadline)) = (&st.unlocked, st.unlock_deadline_ms) {
            if chrono::Utc::now().timestamp_millis() as u64 <= deadline {
                return Ok(b.clone());
            }
        }
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "vault locked".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KdfParams {
    pub algo: String,
    pub mem_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: [u8; 16],
}

fn derive_key(
    master_password: &str,
    preset: Option<&KdfParams>,
) -> SpResult<([u8; 32], KdfParams)> {
    let k = if let Some(p) = preset {
        (*p).clone()
    } else {
        KdfParams {
            algo: "argon2id".into(),
            mem_kib: 65536,
            iterations: 3,
            parallelism: 2,
            salt: {
                let mut s = [0u8; 16];
                OsRng.fill_bytes(&mut s);
                s
            },
        }
    };
    let params =
        argon2::Params::new(k.mem_kib * 1024, k.iterations, k.parallelism, None).map_err(|e| {
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("argon2 params: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    let a2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0u8; 32];
    a2.hash_password_into(master_password.as_bytes(), &k.salt, &mut key)
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("argon2 derive: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    Ok((key, k))
}

fn vault_dir() -> SpResult<PathBuf> {
    let proj = ProjectDirs::from("com", "swiftpan", "SwiftPan").ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "project dirs not available".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(proj.data_dir().to_path_buf())
}

#[derive(Default)]
struct InMemoryState {
    unlocked: Option<CredentialBundle>,
    unlock_deadline_ms: Option<u64>,
    device_id: Option<DeviceId>,
}

static STATE: Lazy<Mutex<InMemoryState>> = Lazy::new(|| Mutex::new(InMemoryState::default()));

fn current_state() -> VaultState {
    let g = STATE.lock().unwrap();
    VaultState {
        is_unlocked: g.unlocked.is_some(),
        unlock_deadline_ms: g.unlock_deadline_ms,
        device_id: g.device_id.clone().unwrap_or_else(|| "dev-unknown".into()),
    }
}
