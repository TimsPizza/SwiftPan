use crate::settings::{load_from_path, save_to_path, AppSettings};

fn configured_settings() -> AppSettings {
    AppSettings {
        log_level: "debug".into(),
        max_concurrency: 6,
        default_download_dir: Some("/storage/photos".into()),
        upload_thumbnail: false,
        android_tree_uri: Some("content://tree/photos".into()),
    }
}

#[test]
fn settings_survive_real_file_persistence_and_fresh_reload() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let path = directory.path().join("nested/sp-settings.json");
    let expected = configured_settings();

    save_to_path(&path, &expected).expect("settings should persist");
    let persisted_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("settings file should exist"))
            .expect("settings file should contain JSON");
    let reloaded = load_from_path(&path);

    assert_eq!(persisted_json["logLevel"], "debug");
    assert_eq!(persisted_json["maxConcurrency"], 6);
    assert_eq!(persisted_json["defaultDownloadDir"], "/storage/photos");
    assert_eq!(persisted_json["uploadThumbnail"], false);
    assert_eq!(persisted_json["androidTreeUri"], "content://tree/photos");
    assert_eq!(reloaded.log_level, expected.log_level);
    assert_eq!(reloaded.max_concurrency, expected.max_concurrency);
    assert_eq!(reloaded.default_download_dir, expected.default_download_dir);
    assert_eq!(reloaded.upload_thumbnail, expected.upload_thumbnail);
    assert_eq!(reloaded.android_tree_uri, expected.android_tree_uri);
}

#[test]
fn missing_or_corrupt_settings_file_recovers_to_defaults() {
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let path = directory.path().join("sp-settings.json");

    let missing = load_from_path(&path);
    assert_eq!(missing.log_level, "info");
    assert_eq!(missing.max_concurrency, 2);

    std::fs::write(&path, b"{not valid json").expect("corrupt fixture should write");
    let corrupt = load_from_path(&path);
    assert_eq!(corrupt.log_level, "info");
    assert_eq!(corrupt.max_concurrency, 2);
    assert!(corrupt.upload_thumbnail);
}
