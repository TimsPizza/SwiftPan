//! Credential backend facade and stable public contract.
//!
//! Child modules separate durable formats, cryptography, device-key storage,
//! platform keystores, path migration, runtime memory, and service
//! orchestration. Public types and `SpBackend` methods are re-exported here so
//! existing callers retain `crate::sp_backend::*` compatibility.

#[cfg(target_os = "android")]
mod android_keystore;
mod crypto;
mod device_key;
mod model;
mod paths;
mod runtime;
mod service;

pub use model::{
    BackendPackage, BackendState, CredentialBundle, KdfParams, R2ConfigPatch, SpBackend,
};
pub use paths::init;
pub(crate) use paths::vault_dir;
