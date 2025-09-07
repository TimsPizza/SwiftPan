use crate::types::*;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use rand::{rngs::OsRng, RngCore};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

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
    // For compatibility, keep names but semantics:
    // - is_unlocked now means "configured/available for use" in this session
    // - unlock_deadline_ms is always None (no lock/ttl concept)
    pub is_unlocked: bool,
    pub unlock_deadline_ms: Option<u64>,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackendPackage {
    pub version: u16,
    pub kdf: KdfParams, // kept for compatibility; now marks device-key
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

pub struct SpBackend;

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
        fs::write(dir.join("vault.sp"), pkg_bytes).map_err(|e| SpError {
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
        fs::write(dir.join("vault.meta.json"), meta_bytes).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("write credentials meta failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        // Also populate in-memory bundle for immediate use (no separate unlock needed)
        {
            let mut guard = STATE.lock().map_err(|_| SpError {
                kind: ErrorKind::NotRetriable,
                message: "backend state lock poisoned".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
            guard.creds = Some(bundle);
        }
        Ok(())
    }

    pub fn patch_r2_config(patch: R2ConfigPatch) -> SpResult<()> {
        // Load current bundle (from mem or disk). If vault doesn't exist yet, start from defaults
        let mut cur = match Self::get_decrypted_bundle_if_unlocked() {
            Ok(b) => b,
            Err(e) => {
                let dir = vault_dir()?;
                let vault_exists = dir.join("vault.sp").exists();
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
    pub fn export_package(_master_password: &str) -> SpResult<BackendPackage> {
        Err(err_not_implemented("backend.export_package"))
    }

    pub fn import_package(_pkg: BackendPackage, _master_password: &str) -> SpResult<()> {
        Err(err_not_implemented("backend.import_package"))
    }

    pub fn rotate_password(_old_pw: &str, _new_pw: &str) -> SpResult<()> {
        Err(err_not_implemented("backend.rotate_password"))
    }

    pub fn get_decrypted_bundle_if_unlocked() -> SpResult<CredentialBundle> {
        let st = STATE.lock().map_err(|_| {
            crate::logger::error(
                "sp_backend",
                "get_decrypted_bundle_if_unlocked backend state lock poisoned",
            );
            SpError {
                kind: ErrorKind::NotRetriable,
                message: "backend state lock poisoned".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
        if let Some(b) = &st.creds {
            crate::logger::debug(
                "sp_backend",
                "get_decrypted_bundle_if_unlocked returning bundle",
            );
            return Ok(b.clone());
        }
        drop(st);
        // Attempt lazy load from disk using device key
        crate::logger::debug(
            "sp_backend",
            "get_decrypted_bundle_if_unlocked attempting lazy load from disk",
        );
        let dir = vault_dir()?;
        let pkg_bytes = fs::read(dir.join("vault.sp")).map_err(|e| {
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
        let key = load_or_create_device_key()?;
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
        let mut st2 = STATE.lock().map_err(|_| SpError {
            kind: ErrorKind::NotRetriable,
            message: "backend state lock poisoned".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        st2.creds = Some(bundle.clone());
        Ok(bundle)
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

fn load_or_create_device_key() -> SpResult<[u8; 32]> {
    let dir = vault_dir()?;
    let key_path = dir.join("device.key");
    if key_path.exists() {
        let data = fs::read(&key_path).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("read device.key failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let mut key = [0u8; 32];
        if data.len() == 32 {
            key.copy_from_slice(&data);
            return Ok(key);
        }
        // support base64 stored
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(&data) {
            if decoded.len() == 32 {
                key.copy_from_slice(&decoded);
                return Ok(key);
            }
        }
        return Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "invalid device.key".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    fs::create_dir_all(&dir).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create_dir_all failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(&key_path, &key).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("write device.key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(key)
}

pub(crate) fn vault_dir() -> SpResult<PathBuf> {
    if let Some(proj) = ProjectDirs::from("com", "swiftpan", "SwiftPan") {
        return Ok(proj.data_dir().to_path_buf());
    }
    // Fallbacks for platforms where ProjectDirs is unavailable (e.g., Android emulator)
    if let Ok(custom) = env::var("SWIFTPAN_DATA_DIR") {
        return Ok(PathBuf::from(custom));
    }
    // Last resort: use temp dir within app sandbox; callers will create it lazily
    Ok(env::temp_dir().join("swiftpan"))
}

#[derive(Default)]
struct BackendMemory {
    creds: Option<CredentialBundle>,
}

static STATE: Lazy<Mutex<BackendMemory>> = Lazy::new(|| Mutex::new(BackendMemory { creds: None }));

fn current_state() -> BackendState {
    // If poisoned, recover inner state instead of panicking
    let g = match STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let dir_exists = vault_dir()
        .ok()
        .map(|d| d.join("vault.sp").exists())
        .unwrap_or(false);
    BackendState {
        is_unlocked: g.creds.is_some() || dir_exists,
        unlock_deadline_ms: None,
        device_id: "dev-removed".into(),
    }
}
