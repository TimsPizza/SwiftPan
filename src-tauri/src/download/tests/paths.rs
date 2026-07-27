use super::super::*;

#[test]
fn partial_download_suffix_does_not_replace_the_original_extension() {
    let raw_destination = Path::new("/downloads/photo.ARW");
    let jpeg_destination = Path::new("/downloads/photo.JPG");
    let raw_part = part_path_for(raw_destination);
    let jpeg_part = part_path_for(jpeg_destination);

    assert_eq!(raw_part, PathBuf::from("/downloads/photo.ARW.part"));
    assert_eq!(jpeg_part, PathBuf::from("/downloads/photo.JPG.part"));
    assert_ne!(raw_part, jpeg_part);
}

#[test]
fn partial_download_suffix_is_appended_for_extensionless_and_hidden_files() {
    let cases = [
        ("/downloads/archive", "/downloads/archive.part"),
        ("/downloads/.camera-raw", "/downloads/.camera-raw.part"),
        (
            "/downloads/archive.tar.zst",
            "/downloads/archive.tar.zst.part",
        ),
    ];

    for (destination, expected) in cases {
        assert_eq!(
            part_path_for(Path::new(destination)),
            PathBuf::from(expected),
            "wrong partial path for {destination}"
        );
    }
}
