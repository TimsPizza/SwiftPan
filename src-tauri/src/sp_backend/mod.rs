use crate::types::*;
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use directories::ProjectDirs;
use once_cell::sync::{Lazy, OnceCell};
use rand::{rngs::OsRng, RngCore};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const EXPORT_SECRET: &str = "swiftpan-export-v1";
const VAULT_FILE_NAME: &str = "vault.sp";
const VAULT_META_FILE_NAME: &str = "vault.meta.json";
const DEVICE_KEY_FILE_NAME: &str = "device.key";
#[cfg(target_os = "android")]
const DEVICE_KEY_WRAPPED_FILE_NAME: &str = "device.key.enc";
#[cfg(target_os = "android")]
const ANDROID_KEY_ALIAS: &str = "com.timspizza.swiftpan.device_key.v1";

static APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();

#[cfg(target_os = "android")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WrappedDeviceKey {
    version: u8,
    iv_b64: String,
    ciphertext_b64: String,
}

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
    pub is_credential_completed: bool, // all fields non-empty
    pub is_credential_valid: bool,     // validated by testing r2 client
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackendPackage {
    pub version: u16,
    pub kdf: KdfParams, // kept for compatibility; now marks device-key
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

pub struct SpBackend;

pub fn init(app: &tauri::AppHandle) -> SpResult<()> {
    let _ = APP_HANDLE.set(app.clone());
    migrate_legacy_vault_dir()?;
    Ok(())
}

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
            return Ok(b.clone());
        }
        drop(st);
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
    let key_path = dir.join(DEVICE_KEY_FILE_NAME);
    if let Ok(key) = load_existing_device_key() {
        return Ok(key);
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
    #[cfg(target_os = "android")]
    {
        let wrapped_path = dir.join(DEVICE_KEY_WRAPPED_FILE_NAME);
        store_android_wrapped_device_key(&wrapped_path, &key)?;
        return Ok(key);
    }
    #[cfg(not(target_os = "android"))]
    fs::write(&key_path, &key).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("write device.key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(key)
}

fn load_existing_device_key() -> SpResult<[u8; 32]> {
    let dir = vault_dir()?;
    let key_path = dir.join(DEVICE_KEY_FILE_NAME);
    #[cfg(target_os = "android")]
    {
        let wrapped_path = dir.join(DEVICE_KEY_WRAPPED_FILE_NAME);
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
    let data = fs::read(key_path).map_err(|e| SpError {
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
    let bytes = serde_json::to_vec(&wrapped).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize wrapped device key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(path, bytes).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("write wrapped device key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

#[cfg(target_os = "android")]
fn load_android_wrapped_device_key(path: &Path) -> SpResult<[u8; 32]> {
    let bytes = fs::read(path).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read wrapped device key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let wrapped: WrappedDeviceKey = serde_json::from_slice(&bytes).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("parse wrapped device key failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let iv = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(wrapped.iv_b64.as_bytes())
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode wrapped key iv failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let ciphertext = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(wrapped.ciphertext_b64.as_bytes())
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("decode wrapped key ciphertext failed: {e}"),
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
    let mut key = [0u8; 32];
    key.copy_from_slice(&plaintext);
    Ok(key)
}

#[cfg(target_os = "android")]
fn android_keystore_encrypt(plaintext: &[u8]) -> SpResult<(Vec<u8>, Vec<u8>)> {
    android_with_env(|env| {
        let secret_key = android_keystore_secret_key(env)?;
        let cipher = android_cipher(env)?;
        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;)V",
            &[
                jni::objects::JValue::Int(1),
                jni::objects::JValue::Object(&secret_key),
            ],
        )
        .map_err(android_jni_err)?;
        let input = env
            .byte_array_from_slice(plaintext)
            .map_err(android_jni_err)?;
        let ciphertext = env
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                    input,
                ))],
            )
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        let iv = env
            .call_method(&cipher, "getIV", "()[B", &[])
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        Ok((
            env.convert_byte_array(jni::objects::JByteArray::from(iv))
                .map_err(android_jni_err)?,
            env.convert_byte_array(jni::objects::JByteArray::from(ciphertext))
                .map_err(android_jni_err)?,
        ))
    })
}

#[cfg(target_os = "android")]
fn android_keystore_decrypt(iv: &[u8], ciphertext: &[u8]) -> SpResult<Vec<u8>> {
    android_with_env(|env| {
        let secret_key = android_keystore_secret_key(env)?;
        let cipher = android_cipher(env)?;
        let iv_array = env.byte_array_from_slice(iv).map_err(android_jni_err)?;
        let spec = env
            .new_object(
                "javax/crypto/spec/GCMParameterSpec",
                "(I[B)V",
                &[
                    jni::objects::JValue::Int(128),
                    jni::objects::JValue::Object(&jni::objects::JObject::from(iv_array)),
                ],
            )
            .map_err(android_jni_err)?;
        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[
                jni::objects::JValue::Int(2),
                jni::objects::JValue::Object(&secret_key),
                jni::objects::JValue::Object(&spec),
            ],
        )
        .map_err(android_jni_err)?;
        let input = env
            .byte_array_from_slice(ciphertext)
            .map_err(android_jni_err)?;
        let plaintext = env
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                    input,
                ))],
            )
            .map_err(android_jni_err)?
            .l()
            .map_err(android_jni_err)?;
        env.convert_byte_array(jni::objects::JByteArray::from(plaintext))
            .map_err(android_jni_err)
    })
}

#[cfg(target_os = "android")]
fn android_with_env<T, F>(f: F) -> SpResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> SpResult<T>,
{
    use jni::JavaVM;

    unsafe {
        let ctx = ndk_context::android_context();
        let vm_ptr = ctx.vm();
        if vm_ptr.is_null() {
            return Err(err_invalid("android vm unavailable"));
        }
        let vm = JavaVM::from_raw(vm_ptr as *mut _).map_err(android_jni_err)?;
        let mut env = vm.attach_current_thread().map_err(android_jni_err)?;
        f(&mut env)
    }
}

#[cfg(target_os = "android")]
fn android_cipher(env: &mut jni::JNIEnv) -> SpResult<jni::objects::JObject> {
    let transformation = env
        .new_string("AES/GCM/NoPadding")
        .map_err(android_jni_err)?;
    env.call_static_method(
        "javax/crypto/Cipher",
        "getInstance",
        "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
        &[jni::objects::JValue::Object(&jni::objects::JObject::from(
            transformation,
        ))],
    )
    .map_err(android_jni_err)?
    .l()
    .map_err(android_jni_err)
}

#[cfg(target_os = "android")]
fn android_keystore_secret_key(env: &mut jni::JNIEnv) -> SpResult<jni::objects::JObject> {
    let alias = env.new_string(ANDROID_KEY_ALIAS).map_err(android_jni_err)?;
    let provider = env.new_string("AndroidKeyStore").map_err(android_jni_err)?;
    let keystore = env
        .call_static_method(
            "java/security/KeyStore",
            "getInstance",
            "(Ljava/lang/String;)Ljava/security/KeyStore;",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                provider,
            ))],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    env.call_method(
        &keystore,
        "load",
        "(Ljava/io/InputStream;[C)V",
        &[
            jni::objects::JValue::Object(&jni::objects::JObject::null()),
            jni::objects::JValue::Object(&jni::objects::JObject::null()),
        ],
    )
    .map_err(android_jni_err)?;
    let has_alias = env
        .call_method(
            &keystore,
            "containsAlias",
            "(Ljava/lang/String;)Z",
            &[jni::objects::JValue::Object(&jni::objects::JObject::from(
                alias,
            ))],
        )
        .map_err(android_jni_err)?
        .z()
        .map_err(android_jni_err)?;
    if !has_alias {
        android_generate_keystore_key(env)?;
    }
    let alias = env.new_string(ANDROID_KEY_ALIAS).map_err(android_jni_err)?;
    let entry = env
        .call_method(
            &keystore,
            "getEntry",
            "(Ljava/lang/String;Ljava/security/KeyStore$ProtectionParameter;)Ljava/security/KeyStore$Entry;",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::from(alias)),
                jni::objects::JValue::Object(&jni::objects::JObject::null()),
            ],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    env.call_method(&entry, "getSecretKey", "()Ljavax/crypto/SecretKey;", &[])
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)
}

#[cfg(target_os = "android")]
fn android_generate_keystore_key(env: &mut jni::JNIEnv) -> SpResult<()> {
    let algorithm = env.new_string("AES").map_err(android_jni_err)?;
    let provider = env.new_string("AndroidKeyStore").map_err(android_jni_err)?;
    let generator = env
        .call_static_method(
            "javax/crypto/KeyGenerator",
            "getInstance",
            "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::from(algorithm)),
                jni::objects::JValue::Object(&jni::objects::JObject::from(provider)),
            ],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    let purposes_encrypt = env
        .get_static_field(
            "android/security/keystore/KeyProperties",
            "PURPOSE_ENCRYPT",
            "I",
        )
        .map_err(android_jni_err)?
        .i()
        .map_err(android_jni_err)?;
    let purposes_decrypt = env
        .get_static_field(
            "android/security/keystore/KeyProperties",
            "PURPOSE_DECRYPT",
            "I",
        )
        .map_err(android_jni_err)?
        .i()
        .map_err(android_jni_err)?;
    let alias = env.new_string(ANDROID_KEY_ALIAS).map_err(android_jni_err)?;
    let builder = env
        .new_object(
            "android/security/keystore/KeyGenParameterSpec$Builder",
            "(Ljava/lang/String;I)V",
            &[
                jni::objects::JValue::Object(&jni::objects::JObject::from(alias)),
                jni::objects::JValue::Int(purposes_encrypt | purposes_decrypt),
            ],
        )
        .map_err(android_jni_err)?;
    let block_modes = env
        .new_object_array(1, "java/lang/String", jni::objects::JObject::null())
        .map_err(android_jni_err)?;
    let gcm = env.new_string("GCM").map_err(android_jni_err)?;
    env.set_object_array_element(&block_modes, 0, gcm)
        .map_err(android_jni_err)?;
    env.call_method(
        &builder,
        "setBlockModes",
        "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
        &[jni::objects::JValue::Object(&jni::objects::JObject::from(
            block_modes,
        ))],
    )
    .map_err(android_jni_err)?;
    let paddings = env
        .new_object_array(1, "java/lang/String", jni::objects::JObject::null())
        .map_err(android_jni_err)?;
    let no_padding = env.new_string("NoPadding").map_err(android_jni_err)?;
    env.set_object_array_element(&paddings, 0, no_padding)
        .map_err(android_jni_err)?;
    env.call_method(
        &builder,
        "setEncryptionPaddings",
        "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
        &[jni::objects::JValue::Object(&jni::objects::JObject::from(
            paddings,
        ))],
    )
    .map_err(android_jni_err)?;
    env.call_method(
        &builder,
        "setKeySize",
        "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
        &[jni::objects::JValue::Int(256)],
    )
    .map_err(android_jni_err)?;
    let spec = env
        .call_method(
            &builder,
            "build",
            "()Landroid/security/keystore/KeyGenParameterSpec;",
            &[],
        )
        .map_err(android_jni_err)?
        .l()
        .map_err(android_jni_err)?;
    env.call_method(
        &generator,
        "init",
        "(Ljava/security/spec/AlgorithmParameterSpec;)V",
        &[jni::objects::JValue::Object(&spec)],
    )
    .map_err(android_jni_err)?;
    env.call_method(&generator, "generateKey", "()Ljavax/crypto/SecretKey;", &[])
        .map_err(android_jni_err)?;
    Ok(())
}

#[cfg(target_os = "android")]
fn android_jni_err(err: impl std::fmt::Display) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("android keystore: {err}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}

fn derive_argon2_key(password: &str, params: &KdfParams) -> SpResult<[u8; 32]> {
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
    .map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("argon2 params: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), &params.salt, &mut key)
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("argon2 derive failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    Ok(key)
}

pub(crate) fn vault_dir() -> SpResult<PathBuf> {
    if let Some(app) = APP_HANDLE.get() {
        return app.path().app_data_dir().map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("resolve app data dir failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }
    if let Some(proj) = ProjectDirs::from("com", "swiftpan", "SwiftPan") {
        return Ok(proj.data_dir().to_path_buf());
    }
    if let Ok(custom) = env::var("SWIFTPAN_DATA_DIR") {
        return Ok(PathBuf::from(custom));
    }
    Ok(env::temp_dir().join("swiftpan"))
}

fn migrate_legacy_vault_dir() -> SpResult<()> {
    let target_dir = vault_dir()?;
    fs::create_dir_all(&target_dir).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("create app data dir failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    for legacy_dir in legacy_vault_dirs() {
        if legacy_dir == target_dir || !legacy_dir.exists() {
            continue;
        }
        migrate_dir_contents(&legacy_dir, &target_dir)?;
    }
    Ok(())
}

fn legacy_vault_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(proj) = ProjectDirs::from("com", "swiftpan", "SwiftPan") {
        dirs.push(proj.data_dir().to_path_buf());
    }
    if let Ok(custom) = env::var("SWIFTPAN_DATA_DIR") {
        dirs.push(PathBuf::from(custom));
    }
    dirs.push(env::temp_dir().join("swiftpan"));
    dirs
}

fn migrate_dir_contents(from: &Path, to: &Path) -> SpResult<()> {
    if !from.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(from).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("read legacy data dir failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })? {
        let entry = entry.map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("read legacy dir entry failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        if from_path.is_dir() {
            fs::create_dir_all(&to_path).map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("create migrated dir failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
            migrate_dir_contents(&from_path, &to_path)?;
            continue;
        }
        if to_path.exists() {
            continue;
        }
        let _ = fs::copy(&from_path, &to_path);
    }
    Ok(())
}

#[derive(Default)]
struct BackendMemory {
    creds: Option<CredentialBundle>,
}

static STATE: Lazy<Mutex<BackendMemory>> = Lazy::new(|| Mutex::new(BackendMemory { creds: None }));

fn current_state() -> BackendState {
    let mem_bundle = match STATE.lock() {
        Ok(guard) => guard.creds.clone(),
        Err(poisoned) => poisoned.into_inner().creds.clone(),
    };
    let disk_bundle = if mem_bundle.is_some() {
        None
    } else {
        SpBackend::get_decrypted_bundle_if_unlocked().ok()
    };
    let bundle = mem_bundle.or(disk_bundle);
    let is_credential_completed = bundle.as_ref().map_or(false, |c| {
        !c.r2.endpoint.is_empty()
            && !c.r2.access_key_id.is_empty()
            && !c.r2.secret_access_key.is_empty()
            && !c.r2.bucket.is_empty()
    });
    BackendState {
        is_unlocked: bundle.is_some(),
        unlock_deadline_ms: None,
        device_id: "dev-removed".into(),
        is_credential_completed,
        is_credential_valid: is_credential_completed,
    }
}
