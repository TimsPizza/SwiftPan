use super::*;

fn config() -> R2Config {
    R2Config {
        endpoint: "https://account.r2.cloudflarestorage.com".into(),
        access_key_id: "access-key".into(),
        secret_access_key: "secret-key".into(),
        bucket: "photos".into(),
        region: None,
    }
}

#[test]
fn missing_region_and_explicit_auto_share_the_same_client_fingerprint() {
    let without_region = config();
    let mut explicit_auto = config();
    explicit_auto.region = Some("auto".into());

    assert_eq!(
        cfg_fingerprint(&without_region),
        cfg_fingerprint(&explicit_auto)
    );
}

#[test]
fn every_r2_configuration_field_participates_in_client_fingerprint() {
    let original = config();
    let baseline = cfg_fingerprint(&original);

    let mut variants = Vec::new();

    let mut endpoint = original.clone();
    endpoint.endpoint = "https://other.r2.cloudflarestorage.com".into();
    variants.push(endpoint);

    let mut access_key = original.clone();
    access_key.access_key_id = "other-access-key".into();
    variants.push(access_key);

    let mut secret_key = original.clone();
    secret_key.secret_access_key = "other-secret-key".into();
    variants.push(secret_key);

    let mut bucket = original.clone();
    bucket.bucket = "backups".into();
    variants.push(bucket);

    let mut region = original;
    region.region = Some("custom-region".into());
    variants.push(region);

    for variant in variants {
        assert_ne!(
            cfg_fingerprint(&variant),
            baseline,
            "configuration change reused the cached client fingerprint"
        );
    }
}
