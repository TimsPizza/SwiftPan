use super::*;

#[test]
fn default_settings_are_safe_and_operational() {
    let settings = AppSettings::default();

    assert_eq!(settings.log_level, "info");
    assert_eq!(settings.max_concurrency, 2);
    assert!(settings.default_download_dir.is_none());
    assert!(settings.upload_thumbnail);
    assert!(settings.android_tree_uri.is_none());
}

#[test]
fn settings_json_uses_frontend_camel_case_contract() {
    let value = serde_json::to_value(AppSettings {
        log_level: "debug".into(),
        max_concurrency: 4,
        default_download_dir: Some("/downloads".into()),
        upload_thumbnail: false,
        android_tree_uri: Some("content://downloads".into()),
    })
    .expect("settings should serialize");

    assert_eq!(value["logLevel"], "debug");
    assert_eq!(value["maxConcurrency"], 4);
    assert_eq!(value["defaultDownloadDir"], "/downloads");
    assert_eq!(value["uploadThumbnail"], false);
    assert_eq!(value["androidTreeUri"], "content://downloads");

    for wrong_key in [
        "log_level",
        "max_concurrency",
        "default_download_dir",
        "upload_thumbnail",
        "android_tree_uri",
    ] {
        assert!(
            value.get(wrong_key).is_none(),
            "serialized settings leaked Rust key {wrong_key}"
        );
    }
}

#[test]
fn settings_survive_json_round_trip_without_field_loss() {
    let original = AppSettings {
        log_level: "warn".into(),
        max_concurrency: 7,
        default_download_dir: Some("/storage/photos".into()),
        upload_thumbnail: false,
        android_tree_uri: Some("content://tree/photos".into()),
    };

    let bytes = serde_json::to_vec(&original).expect("settings should serialize");
    let decoded =
        serde_json::from_slice::<AppSettings>(&bytes).expect("settings should deserialize");

    assert_eq!(decoded.log_level, original.log_level);
    assert_eq!(decoded.max_concurrency, original.max_concurrency);
    assert_eq!(decoded.default_download_dir, original.default_download_dir);
    assert_eq!(decoded.upload_thumbnail, original.upload_thumbnail);
    assert_eq!(decoded.android_tree_uri, original.android_tree_uri);
}
