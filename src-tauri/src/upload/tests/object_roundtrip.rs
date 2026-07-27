use super::super::*;
use super::fixtures::raw_camera_fixture;
use crate::test_support::patterned_bytes;
use opendal::services::Memory;
use sha2::{Digest, Sha256};

async fn assert_object_round_trip(
    key: &str,
    original: Vec<u8>,
    chunk_size: usize,
    explicit_content_type: Option<&str>,
    expected_content_type: &str,
) {
    let operator = opendal::Operator::new(Memory::default())
        .expect("memory operator should build")
        .finish();
    let original_hash = Sha256::digest(&original);
    let mut writer = open_upload_writer(
        &operator,
        key,
        explicit_content_type,
        Some("attachment; filename=\"fixture.bin\""),
    )
    .await
    .expect("writer should open");

    for chunk in original.chunks(chunk_size) {
        writer
            .write(chunk.to_vec())
            .await
            .expect("every chunk should be written");
    }
    writer.close().await.expect("writer should close");

    let metadata = operator
        .stat(key)
        .await
        .expect("uploaded object should exist");
    let downloaded = operator
        .read(key)
        .await
        .expect("uploaded object should be readable")
        .to_bytes()
        .to_vec();

    assert_eq!(metadata.content_length(), original.len() as u64);
    assert_eq!(metadata.content_type(), Some(expected_content_type));
    assert_eq!(
        metadata.content_disposition(),
        Some("attachment; filename=\"fixture.bin\"")
    );
    assert_eq!(downloaded.len(), original.len());
    assert_eq!(Sha256::digest(&downloaded), original_hash);
    assert_eq!(downloaded, original);
}

#[tokio::test]
async fn raw_camera_bytes_and_metadata_survive_chunked_object_round_trip() {
    assert_object_round_trip(
        "camera/DSC00001.ARW",
        raw_camera_fixture(2 * 1024 * 1024),
        64 * 1024 + 3,
        None,
        "image/x-sony-arw",
    )
    .await;
}

#[tokio::test]
async fn boundary_sized_binary_objects_survive_chunked_round_trip() {
    const CHUNK: usize = 64 * 1024 + 3;

    for size in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK + 17] {
        assert_object_round_trip(
            &format!("fixtures/{size}.bin"),
            patterned_bytes(size, 23),
            CHUNK,
            Some("application/x-test-binary"),
            "application/x-test-binary",
        )
        .await;
    }
}
