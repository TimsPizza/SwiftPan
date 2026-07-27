use super::super::*;

#[test]
fn explicit_content_type_takes_precedence_after_trimming() {
    assert_eq!(
        inferred_content_type("camera/DSC00001.ARW", Some("  image/custom-raw  ")),
        "image/custom-raw"
    );
}

#[test]
fn empty_explicit_content_type_falls_back_to_case_insensitive_extension() {
    assert_eq!(
        inferred_content_type("camera/DSC00001.ARW", Some("  ")),
        "image/x-sony-arw"
    );
    assert_eq!(
        inferred_content_type("camera/preview.JpEg", None),
        "image/jpeg"
    );
}

#[test]
fn known_raw_camera_extensions_have_stable_content_types() {
    let cases = [
        ("photo.arw", "image/x-sony-arw"),
        ("photo.cr2", "image/x-canon-cr2"),
        ("photo.cr3", "image/x-canon-cr3"),
        ("photo.dng", "image/x-adobe-dng"),
        ("photo.nef", "image/x-nikon-nef"),
        ("photo.orf", "image/x-olympus-orf"),
        ("photo.raf", "image/x-fuji-raf"),
        ("photo.rw2", "image/x-panasonic-rw2"),
    ];

    for (key, expected) in cases {
        assert_eq!(
            inferred_content_type(key, None),
            expected,
            "wrong content type for {key}"
        );
    }
}

#[test]
fn unknown_or_missing_extension_uses_binary_content_type() {
    for key in ["archive.unknown", "README", ".hidden"] {
        assert_eq!(
            inferred_content_type(key, None),
            "application/octet-stream",
            "wrong fallback content type for {key}"
        );
    }
}
