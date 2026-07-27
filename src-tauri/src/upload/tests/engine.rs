use super::super::*;
use crate::test_support::patterned_bytes;
use opendal::services::Memory;
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Default)]
struct RecordingObserver {
    uploading: bool,
    finalizing: bool,
    cancelled: bool,
    parts: Vec<(u32, u64)>,
}

impl UploadEngineObserver for RecordingObserver {
    fn uploading(&mut self) -> SpResult<()> {
        self.uploading = true;
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()> {
        self.parts.push((part_number, bytes_transferred));
        Ok(())
    }

    fn finalizing(&mut self) -> SpResult<()> {
        self.finalizing = true;
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        self.cancelled = true;
        Ok(())
    }
}

fn memory_operator() -> opendal::Operator {
    opendal::Operator::new(Memory::default())
        .expect("memory operator should build")
        .finish()
}

fn controls(cancelled: bool) -> UploadControl {
    UploadControl {
        paused: Arc::new(AtomicBool::new(false)),
        cancelled: Arc::new(AtomicBool::new(cancelled)),
    }
}

#[tokio::test]
async fn engine_uploads_boundary_sized_files_exactly() {
    const PART_SIZE: usize = 64 * 1024 + 3;

    for size in [
        0,
        1,
        PART_SIZE - 1,
        PART_SIZE,
        PART_SIZE + 1,
        2 * PART_SIZE + 17,
    ] {
        let source = tempfile::NamedTempFile::new().expect("temp source should be created");
        let original = patterned_bytes(size, 31);
        std::fs::write(source.path(), &original).expect("fixture should be written");
        let operator = memory_operator();
        let key = format!("engine/{size}.bin");
        let mut observer = RecordingObserver::default();

        upload_file(
            &operator,
            UploadEngineRequest {
                key: key.clone(),
                source_path: source.path().to_path_buf(),
                part_size: PART_SIZE as u64,
                content_type: Some("application/x-engine-test".into()),
                content_disposition: Some("attachment; filename=\"fixture.bin\"".into()),
            },
            controls(false),
            &mut observer,
        )
        .await
        .expect("engine upload should complete");

        let metadata = operator.stat(&key).await.expect("object should exist");
        let uploaded = operator
            .read(&key)
            .await
            .expect("object should be readable")
            .to_bytes();
        let expected_parts = size.div_ceil(PART_SIZE);

        assert_eq!(uploaded.as_ref(), original);
        assert_eq!(metadata.content_length(), size as u64);
        assert_eq!(metadata.content_type(), Some("application/x-engine-test"));
        assert_eq!(
            metadata.content_disposition(),
            Some("attachment; filename=\"fixture.bin\"")
        );
        assert!(observer.uploading);
        assert!(observer.finalizing);
        assert!(!observer.cancelled);
        assert_eq!(observer.parts.len(), expected_parts);
        assert_eq!(
            observer.parts.iter().map(|(_, bytes)| *bytes).sum::<u64>(),
            size as u64
        );
    }
}

#[tokio::test]
async fn cancellation_removes_remote_object_and_notifies_observer() {
    let source = tempfile::NamedTempFile::new().expect("temp source should be created");
    std::fs::write(source.path(), patterned_bytes(1024, 47)).expect("fixture should be written");
    let operator = memory_operator();
    let key = "engine/cancelled.bin";
    let mut observer = RecordingObserver::default();

    let error = upload_file(
        &operator,
        UploadEngineRequest {
            key: key.into(),
            source_path: source.path().to_path_buf(),
            part_size: 256,
            content_type: None,
            content_disposition: None,
        },
        controls(true),
        &mut observer,
    )
    .await
    .expect_err("pre-cancelled upload should fail");

    assert!(matches!(error.kind, ErrorKind::Cancelled));
    assert!(observer.uploading);
    assert!(observer.cancelled);
    assert!(!observer.finalizing);
    assert!(!operator
        .exists(key)
        .await
        .expect("existence check should work"));
}
