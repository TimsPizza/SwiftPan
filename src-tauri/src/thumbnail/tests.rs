use super::*;

fn digest_component(thumbnail_key: &str) -> &str {
    thumbnail_key
        .strip_prefix(THUMBNAIL_PREFIX)
        .expect("thumbnail key should use reserved prefix")
        .strip_suffix(".jpg")
        .expect("thumbnail key should use JPEG suffix")
}

#[test]
fn thumbnail_key_is_stable_and_uses_sha256_namespace() {
    let first = thumbnail_key_for("camera/2026/DSC00001.ARW");
    let second = thumbnail_key_for("camera/2026/DSC00001.ARW");
    let digest = digest_component(&first);

    assert_eq!(first, second);
    assert_eq!(digest.len(), 64);
    assert!(
        digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "thumbnail digest must be lowercase hexadecimal"
    );
    assert_eq!(digest, digest.to_ascii_lowercase());
}

#[test]
fn same_basename_in_different_directories_does_not_collide() {
    assert_ne!(
        thumbnail_key_for("camera-a/DSC00001.ARW"),
        thumbnail_key_for("camera-b/DSC00001.ARW")
    );
}

#[test]
fn empty_unicode_and_hidden_object_keys_have_stable_thumbnail_keys() {
    for object_key in ["", "照片/夏天.ARW", ".hidden", "目录/📷.jpg"] {
        let thumbnail_key = thumbnail_key_for(object_key);

        assert!(thumbnail_key.starts_with(THUMBNAIL_PREFIX));
        assert_eq!(digest_component(&thumbnail_key).len(), 64);
        assert_eq!(thumbnail_key, thumbnail_key_for(object_key));
    }
}

#[test]
fn thumbnail_namespace_detection_is_prefix_based() {
    for key in [
        "__thumbnail__/abc.jpg",
        "__thumbnail__/nested/abc.jpg",
        THUMBNAIL_PREFIX,
    ] {
        assert!(is_thumbnail_key(key), "expected thumbnail key: {key}");
    }

    for key in [
        "thumbnail/abc.jpg",
        "photos/__thumbnail__/abc.jpg",
        "__thumbnails__/abc.jpg",
        "abc.jpg",
        "",
    ] {
        assert!(!is_thumbnail_key(key), "unexpected thumbnail key: {key}");
    }
}
