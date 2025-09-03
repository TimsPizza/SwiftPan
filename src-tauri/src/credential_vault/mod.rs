use crate::types::*;

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
  pub kdf: serde_json::Value,
  pub nonce_b64: String,
  pub ciphertext_b64: String,
}

pub struct CredentialVault;

impl CredentialVault {
  pub fn status() -> SpResult<VaultState> {
    Err(err_not_implemented("vault.status"))
  }

  pub fn set_with_plaintext(_bundle: CredentialBundle, _master_password: &str) -> SpResult<()> {
    Err(err_not_implemented("vault.set_with_plaintext"))
  }

  pub fn test_connectivity(_master_password: &str) -> SpResult<()> {
    Err(err_not_implemented("vault.test_connectivity"))
  }

  pub fn unlock(_master_password: &str, _hold_ms: u64) -> SpResult<()> {
    Err(err_not_implemented("vault.unlock"))
  }

  pub fn lock() -> SpResult<()> {
    Err(err_not_implemented("vault.lock"))
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
    Err(err_not_implemented("vault.get_decrypted_bundle_if_unlocked"))
  }
}

