use super::super::*;
use crate::test_support::{limit_read_responses, patterned_bytes, report_etag};
use opendal::services::Memory;
use sha2::{Digest, Sha256};
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

#[tokio::test]
async fn cancellation_preserves_destination_that_existed_before_download() {
    let operator = memory_operator();
    operator
        .write("replacement.bin", patterned_bytes(4096, 81))
        .await
        .expect("remote fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("existing.bin");
    let original_destination = patterned_bytes(777, 19);
    tokio::fs::write(&destination, &original_destination)
        .await
        .expect("existing destination should write");
    tokio::fs::write(part_path_for(&destination), patterned_bytes(1024, 81))
        .await
        .expect("partial fixture should write");
    let (control, _, cancelled) = controls();
    cancelled.store(true, Ordering::Relaxed);
    let mut observer = RecordingObserver::default();

    download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "replacement.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 1024,
            expected_etag: None,
            recorded_bytes_done: 1024,
        },
        control,
        &mut observer,
    )
    .await
    .expect_err("cancelled replacement must fail");

    assert_eq!(
        tokio::fs::read(destination)
            .await
            .expect("pre-existing destination must survive cancellation"),
        original_destination
    );
}

#[tokio::test]
async fn complete_but_stale_partial_file_is_not_promoted_by_length_alone() {
    let operator = memory_operator();
    let remote = patterned_bytes(4096, 33);
    operator
        .write("same-size.bin", remote.clone())
        .await
        .expect("remote fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("same-size.bin");
    tokio::fs::write(
        part_path_for(&destination),
        patterned_bytes(remote.len(), 99),
    )
    .await
    .expect("stale complete partial should write");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "same-size.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 1024,
            expected_etag: None,
            recorded_bytes_done: remote.len() as u64,
        },
        control,
        &mut observer,
    )
    .await
    .expect("engine should recover without publishing stale bytes");

    let downloaded = tokio::fs::read(destination)
        .await
        .expect("destination should exist");
    assert_eq!(downloaded.len(), remote.len());
    assert_eq!(Sha256::digest(downloaded), Sha256::digest(remote));
}

#[tokio::test]
async fn changed_remote_etag_rejects_resume_before_touching_partial_file() {
    let storage = memory_operator();
    let remote = patterned_bytes(4096, 41);
    storage
        .write("etag.bin", remote.clone())
        .await
        .expect("remote fixture should upload");
    let operator = report_etag(storage, "etag-new");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("etag.bin");
    let prefix = remote[..1024].to_vec();
    tokio::fs::write(part_path_for(&destination), &prefix)
        .await
        .expect("partial fixture should write");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    let error = download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "etag.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 1024,
            expected_etag: Some("etag-old".into()),
            recorded_bytes_done: 1024,
        },
        control,
        &mut observer,
    )
    .await
    .expect_err("changed source must reject resume");

    assert!(matches!(error.kind, ErrorKind::SourceChanged));
    assert!(observer.source_changed);
    assert_eq!(
        tokio::fs::read(part_path_for(&destination))
            .await
            .expect("source-change detection must not mutate partial bytes"),
        prefix
    );
}

#[tokio::test]
async fn required_etag_that_backend_omits_fails_closed() {
    let operator = memory_operator();
    operator
        .write("missing-etag.bin", patterned_bytes(1024, 51))
        .await
        .expect("remote fixture should upload");
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("missing-etag.bin");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    let error = download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "missing-etag.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 512,
            expected_etag: Some("required-etag".into()),
            recorded_bytes_done: 0,
        },
        control,
        &mut observer,
    )
    .await
    .expect_err("an unverifiable source version must not be downloaded");

    assert!(matches!(error.kind, ErrorKind::SourceChanged));
    assert!(!destination.exists());
}

#[tokio::test]
async fn repeated_nonempty_short_reads_still_reconstruct_the_object() {
    let storage = memory_operator();
    let original = patterned_bytes(8192 + 37, 61);
    storage
        .write("short-reads.bin", original.clone())
        .await
        .expect("remote fixture should upload");
    let operator = limit_read_responses(storage, 317);
    let temp = tempfile::tempdir().expect("temp directory should build");
    let destination = temp.path().join("short-reads.bin");
    let (control, _, _) = controls();
    let mut observer = RecordingObserver::default();

    download_to_stage(
        &operator,
        DownloadEngineRequest {
            key: "short-reads.bin".into(),
            temp_path: destination.clone(),
            chunk_size: 1024,
            expected_etag: None,
            recorded_bytes_done: 0,
        },
        control,
        &mut observer,
    )
    .await
    .expect("nonempty short reads should continue until total is reached");

    assert_eq!(
        tokio::fs::read(destination)
            .await
            .expect("destination should exist"),
        original
    );
    assert!(observer.chunks.iter().all(|(_, len, _)| *len <= 317));
}
