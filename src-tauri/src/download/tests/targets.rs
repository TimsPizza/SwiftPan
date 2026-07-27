use super::super::*;

fn params() -> NewDownloadParams {
    NewDownloadParams {
        key: "camera/photo.arw".into(),
        dest_path: None,
        chunk_size: 1024,
        expected_etag: None,
        android_tree_uri: None,
        android_relative_path: None,
        mime: None,
    }
}

#[test]
fn destination_path_accepts_plain_and_file_uri_paths() {
    assert_eq!(
        normalize_dest_path(" /downloads/photo.arw ").expect("plain path should parse"),
        PathBuf::from("/downloads/photo.arw")
    );
    assert_eq!(
        normalize_dest_path("file:///downloads/photo.arw").expect("file URI should parse"),
        PathBuf::from("/downloads/photo.arw")
    );
}

#[test]
fn destination_path_rejects_non_file_uris() {
    let error = normalize_dest_path("content://downloads/photo.arw")
        .expect_err("content URI should not become a desktop path");

    assert_eq!(error.kind.as_str(), "not_retriable");
    assert_eq!(error.message, "unsupported URI for download destination");
}

#[test]
fn target_requires_exactly_one_complete_destination_variant() {
    let mut desktop = params();
    desktop.dest_path = Some("/downloads/photo.arw".into());
    assert!(matches!(
        DownloadTarget::from_params(&desktop).expect("desktop target should parse"),
        DownloadTarget::FileSystem { .. }
    ));

    let mut android = params();
    android.android_tree_uri = Some("content://tree/downloads".into());
    android.android_relative_path = Some("camera/photo.arw".into());
    android.mime = Some("image/x-sony-arw".into());
    assert!(matches!(
        DownloadTarget::from_params(&android).expect("Android target should parse"),
        DownloadTarget::AndroidTree { .. }
    ));

    let empty = params();
    assert!(DownloadTarget::from_params(&empty).is_err());

    let mut mixed = desktop;
    mixed.android_tree_uri = Some("content://tree/downloads".into());
    mixed.android_relative_path = Some("camera/photo.arw".into());
    assert!(DownloadTarget::from_params(&mixed).is_err());
}

#[test]
fn filename_sanitization_replaces_reserved_characters_and_has_fallback() {
    assert_eq!(
        sanitize_filename("camera/a:b*c?d\"e<f>g|h.arw"),
        "camera_a_b_c_d_e_f_g_h.arw"
    );
    assert_eq!(sanitize_filename("..."), "download.bin");
    assert_eq!(sanitize_filename(""), "download.bin");
}
