use super::super::*;
use crate::test_support::patterned_bytes;
use opendal::services::Memory;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Default)]
struct RecordingObserver {
    remote_total: Option<u64>,
    observed_etag: Option<String>,
    download_offsets: Vec<u64>,
    chunks: Vec<(u64, u64, u64)>,
    pause_count: usize,
    resume_count: usize,
    source_changed: bool,
    cancelled: bool,
}

impl DownloadEngineObserver for RecordingObserver {
    fn remote_metadata(&mut self, total: u64, observed_etag: Option<&str>) -> SpResult<()> {
        self.remote_total = Some(total);
        self.observed_etag = observed_etag.map(str::to_string);
        Ok(())
    }

    fn source_changed(&mut self) -> SpResult<()> {
        self.source_changed = true;
        Ok(())
    }

    fn download_started(&mut self, offset: u64) -> SpResult<()> {
        self.download_offsets.push(offset);
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        self.pause_count += 1;
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        self.resume_count += 1;
        Ok(())
    }

    fn chunk_done(&mut self, range_start: u64, len: u64, offset: u64) -> SpResult<()> {
        self.chunks.push((range_start, len, offset));
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

fn controls() -> (DownloadControl, Arc<AtomicBool>, Arc<AtomicBool>) {
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    (
        DownloadControl {
            paused: paused.clone(),
            cancelled: cancelled.clone(),
        },
        paused,
        cancelled,
    )
}

#[tokio::test]
async fn engine_downloads_boundary_sized_objects_exactly() {
    const CHUNK: usize = 64 * 1024 + 3;

    for size in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK + 17] {
        let operator = memory_operator();
        let original = patterned_bytes(size, 31);
        let key = format!("objects/{size}.bin");
        operator
            .write(&key, original.clone())
            .await
            .expect("fixture should upload");
        let temp = tempfile::tempdir().expect("temp directory should build");
        let destination = temp.path().join(format!("{size}.bin"));
        let (control, _, _) = controls();
        let mut observer = RecordingObserver::default();

        let output = download_to_stage(
            &operator,
            DownloadEngineRequest {
                key,
                temp_path: destination.clone(),
                chunk_size: CHUNK as u64,
                expected_etag: None,
                recorded_bytes_done: 0,
            },
            control,
            &mut observer,
        )
        .await
        .expect("download should complete");

        assert_eq!(output.total, size as u64);
        assert_eq!(
            tokio::fs::read(&destination)
                .await
                .expect("destination should exist"),
            original
        );
        assert!(!part_path_for(&destination).exists());
        assert_eq!(observer.remote_total, Some(size as u64));
    }
}

#[tokio::test]
async fn engine_resumes_from_existing_partial_file_without_rewriting_prefix() {
    const CHUNK: usize = 32 * 1024 + 5;
    let operator = memory_operator();
    let original = patterned_bytes(3 * CHUNK + 17, 44);
    operator
        .write("camera/photo.arw", original.clone())
        .await
        .expect("fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("photo.arw");
    let part_path = part_path_for(&destination);
    let resume_offset = CHUNK + 11;
    tokio::fs::write(&part_path, &original[..resume_offset])
        .await
        .expect("partial fixture should write");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "camera/photo.arw".into(),
            temp_path: destination.clone(),
            chunk_size: CHUNK as u64,
            expected_etag: None,
            recorded_bytes_done: resume_offset as u64,
        },
        control,
        &mut observer,
    )
    .await
    .expect("resumed download should complete");

    assert_eq!(
        tokio::fs::read(&destination)
            .await
            .expect("destination should exist"),
        original
    );
    assert_eq!(observer.download_offsets, vec![resume_offset as u64]);
    assert_eq!(
        observer.chunks.first().map(|chunk| chunk.0),
        Some(resume_offset as u64)
    );
}

#[tokio::test]
async fn oversized_partial_file_is_discarded_before_restarting_from_zero() {
    let operator = memory_operator();
    let original = patterned_bytes(1024, 55);
    operator
        .write("small.bin", original.clone())
        .await
        .expect("fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("small.bin");
    tokio::fs::write(part_path_for(&destination), patterned_bytes(2048, 9))
        .await
        .expect("oversized partial fixture should write");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "small.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 257,
            expected_etag: None,
            recorded_bytes_done: 2048,
        },
        control,
        &mut observer,
    )
    .await
    .expect("download should restart");

    assert_eq!(observer.download_offsets, vec![0]);
    assert_eq!(
        tokio::fs::read(destination)
            .await
            .expect("destination should exist"),
        original
    );
}

#[tokio::test]
async fn cancellation_removes_staged_artifacts_and_notifies_observer() {
    let operator = memory_operator();
    operator
        .write("cancel.bin", patterned_bytes(4096, 77))
        .await
        .expect("fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("cancel.bin");
    let part_path = part_path_for(&destination);
    tokio::fs::write(&part_path, patterned_bytes(1024, 77))
        .await
        .expect("partial fixture should write");
    let (control, _, cancelled) = controls();
    cancelled.store(true, Ordering::Relaxed);
    let mut observer = RecordingObserver::default();

    let error = download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "cancel.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 1024,
            expected_etag: None,
            recorded_bytes_done: 1024,
        },
        control,
        &mut observer,
    )
    .await
    .expect_err("cancelled download must fail");

    assert_eq!(error.kind.as_str(), "cancelled");
    assert!(observer.cancelled);
    assert!(!part_path.exists());
    assert!(!destination.exists());
}
