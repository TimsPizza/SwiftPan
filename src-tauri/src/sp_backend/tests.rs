use super::*;

fn lightweight_kdf(salt: [u8; 16]) -> KdfParams {
    KdfParams {
        algo: "argon2id".into(),
        mem_kib: 32,
        iterations: 1,
        parallelism: 1,
        salt,
    }
}

#[test]
fn argon2_derivation_is_deterministic_and_returns_256_bit_key() {
    let params = lightweight_kdf([7; 16]);

    let first = derive_argon2_key("correct horse battery staple", &params)
        .expect("valid Argon2 parameters should derive a key");
    let second = derive_argon2_key("correct horse battery staple", &params)
        .expect("same inputs should derive a key again");

    assert_eq!(first.len(), 32);
    assert_eq!(first, second);
}

#[test]
fn changing_password_or_salt_changes_derived_key() {
    let params = lightweight_kdf([7; 16]);
    let baseline =
        derive_argon2_key("password-a", &params).expect("baseline key should derive successfully");
    let changed_password = derive_argon2_key("password-b", &params)
        .expect("changed password should still derive successfully");
    let changed_salt = derive_argon2_key("password-a", &lightweight_kdf([8; 16]))
        .expect("changed salt should still derive successfully");

    assert_ne!(baseline, changed_password);
    assert_ne!(baseline, changed_salt);
}

#[test]
fn zero_argon2_cost_parameters_are_rejected_before_derivation() {
    let invalid_params = [
        KdfParams {
            mem_kib: 0,
            ..lightweight_kdf([1; 16])
        },
        KdfParams {
            iterations: 0,
            ..lightweight_kdf([1; 16])
        },
        KdfParams {
            parallelism: 0,
            ..lightweight_kdf([1; 16])
        },
    ];

    for params in invalid_params {
        let error = derive_argon2_key("password", &params)
            .expect_err("zero cost parameter must be rejected");
        assert_eq!(error.kind.as_str(), "not_retriable");
        assert_eq!(error.message, "invalid argon2 params");
    }
}

#[test]
fn structurally_invalid_argon2_parameters_return_error_instead_of_panicking() {
    let params = KdfParams {
        mem_kib: 1,
        iterations: 1,
        parallelism: 4,
        ..lightweight_kdf([2; 16])
    };

    let error = derive_argon2_key("password", &params)
        .expect_err("Argon2 should reject insufficient memory for parallelism");

    assert_eq!(error.kind.as_str(), "not_retriable");
    assert!(error.message.starts_with("argon2 params:"));
}

#[test]
fn backend_package_survives_json_round_trip_without_field_loss() {
    let original = BackendPackage {
        version: 1,
        kdf: KdfParams {
            algo: "argon2id".into(),
            mem_kib: 32 * 1024,
            iterations: 3,
            parallelism: 1,
            salt: [9; 16],
        },
        nonce_b64: "bm9uY2U".into(),
        ciphertext_b64: "Y2lwaGVydGV4dA".into(),
    };

    let bytes = serde_json::to_vec(&original).expect("backend package should serialize");
    let decoded =
        serde_json::from_slice::<BackendPackage>(&bytes).expect("backend package should decode");

    assert_eq!(decoded.version, original.version);
    assert_eq!(decoded.kdf.algo, original.kdf.algo);
    assert_eq!(decoded.kdf.mem_kib, original.kdf.mem_kib);
    assert_eq!(decoded.kdf.iterations, original.kdf.iterations);
    assert_eq!(decoded.kdf.parallelism, original.kdf.parallelism);
    assert_eq!(decoded.kdf.salt, original.kdf.salt);
    assert_eq!(decoded.nonce_b64, original.nonce_b64);
    assert_eq!(decoded.ciphertext_b64, original.ciphertext_b64);
}
