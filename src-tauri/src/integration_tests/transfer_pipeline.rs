use crate::download::{
    download_to_stage_for_integration, IntegrationDownloadControl, IntegrationDownloadObserver,
    IntegrationDownloadRequest,
};
use crate::test_support::{inject_early_eof, patterned_bytes};
use crate::types::{ErrorKind, SpError, SpResult};
use crate::upload::{
    upload_file_for_integration, IntegrationUploadControl, IntegrationUploadObserver,
    IntegrationUploadRequest,
};
use opendal::services::Memory;
use sha2::{Digest, Sha256};
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Default)]
struct UploadObserver {
    uploaded_bytes: u64,
    finalized: bool,
}

impl IntegrationUploadObserver for UploadObserver {
    fn uploading(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn part_done(&mut self, _part_number: u32, bytes_transferred: u64) -> SpResult<()> {
        self.uploaded_bytes += bytes_transferred;
        Ok(())
    }

    fn finalizing(&mut self) -> SpResult<()> {
        self.finalized = true;
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        Ok(())
    }
}

#[derive(Default)]
struct DownloadObserver {
    remote_total: Option<u64>,
    downloaded_bytes: u64,
}

impl IntegrationDownloadObserver for DownloadObserver {
    fn remote_metadata(&mut self, total: u64, _observed_etag: Option<&str>) -> SpResult<()> {
        self.remote_total = Some(total);
        Ok(())
    }

    fn source_changed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn download_started(&mut self, _offset: u64) -> SpResult<()> {
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn chunk_done(&mut self, _range_start: u64, len: u64, _offset: u64) -> SpResult<()> {
        self.downloaded_bytes += len;
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        Ok(())
    }
}

#[derive(Default)]
struct InterruptAfterFirstChunk {
    persisted_bytes_done: u64,
}

impl IntegrationDownloadObserver for InterruptAfterFirstChunk {
    fn remote_metadata(&mut self, _total: u64, _observed_etag: Option<&str>) -> SpResult<()> {
        Ok(())
    }

    fn source_changed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn download_started(&mut self, _offset: u64) -> SpResult<()> {
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn chunk_done(&mut self, _range_start: u64, _len: u64, offset: u64) -> SpResult<()> {
        self.persisted_bytes_done = offset;
        Err(SpError {
            kind: ErrorKind::RetryableNet,
            message: "simulated abrupt process termination after durable progress".into(),
            retry_after_ms: Some(500),
            context: None,
            at: 0,
        })
    }

    fn cancelled(&mut self) -> SpResult<()> {
        Ok(())
    }
}

fn control_flags() -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    )
}

#[tokio::test]
async fn download_resumes_with_fresh_runtime_after_interrupted_first_run() {
    const CHUNK: usize = 32 * 1024 + 5;
    let operator = opendal::Operator::new(Memory::default())
        .expect("memory service should build")
        .finish();
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let destination = directory.path().join("recovered-photo.arw");
    let part_path = destination.with_extension("arw.part");
    let key = "camera/recovered-photo.ARW";
    let original = patterned_bytes(3 * CHUNK + 17, 113);
    operator
        .write(key, original.clone())
        .await
        .expect("remote fixture should write");

    let persisted_bytes_done = {
        let (paused, cancelled) = control_flags();
        let mut interrupted_observer = InterruptAfterFirstChunk::default();
        let error = download_to_stage_for_integration(
            &operator,
            IntegrationDownloadRequest {
                key: key.into(),
                temp_path: destination.clone(),
                chunk_size: CHUNK as u64,
                expected_etag: None,
                recorded_bytes_done: 0,
            },
            IntegrationDownloadControl { paused, cancelled },
            &mut interrupted_observer,
        )
        .await
        .expect_err("first runtime must be interrupted");

        assert!(matches!(error.kind, ErrorKind::RetryableNet));
        assert_eq!(interrupted_observer.persisted_bytes_done, CHUNK as u64);
        assert_eq!(
            tokio::fs::read(&part_path)
                .await
                .expect("interrupted runtime must leave its staged prefix"),
            original[..CHUNK]
        );
        assert!(!destination.exists());
        interrupted_observer.persisted_bytes_done
    };

    // Everything above this point models the dead process. These controls and
    // observer are newly allocated; only remote storage, disk, and persisted
    // progress cross the restart boundary.
    let (paused, cancelled) = control_flags();
    let mut recovered_observer = DownloadObserver::default();
    let output = download_to_stage_for_integration(
        &operator,
        IntegrationDownloadRequest {
            key: key.into(),
            temp_path: destination.clone(),
            chunk_size: CHUNK as u64,
            expected_etag: None,
            recorded_bytes_done: persisted_bytes_done,
        },
        IntegrationDownloadControl { paused, cancelled },
        &mut recovered_observer,
    )
    .await
    .expect("fresh runtime should resume the staged download");

    let recovered = tokio::fs::read(&destination)
        .await
        .expect("recovered destination should exist");
    assert_eq!(output.total, original.len() as u64);
    assert_eq!(recovered_observer.downloaded_bytes, (2 * CHUNK + 17) as u64);
    assert_eq!(recovered, original);
    assert_eq!(Sha256::digest(&recovered), Sha256::digest(&original));
    assert!(!part_path.exists());
}

#[tokio::test]
async fn download_rejects_early_eof_instead_of_promoting_partial_file() {
    const CHUNK: usize = 16 * 1024 + 3;
    let original = patterned_bytes(2 * CHUNK, 127);
    let storage = opendal::Operator::new(Memory::default())
        .expect("memory service should build")
        .finish();
    storage
        .write("camera/truncated.ARW", original.clone())
        .await
        .expect("remote fixture should write");
    let operator = inject_early_eof(storage, CHUNK as u64);
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let destination = directory.path().join("truncated.arw");
    let part_path = destination.with_extension("arw.part");
    let (paused, cancelled) = control_flags();
    let mut observer = DownloadObserver::default();

    let error = download_to_stage_for_integration(
        &operator,
        IntegrationDownloadRequest {
            key: "camera/truncated.ARW".into(),
            temp_path: destination.clone(),
            chunk_size: CHUNK as u64,
            expected_etag: None,
            recorded_bytes_done: 0,
        },
        IntegrationDownloadControl { paused, cancelled },
        &mut observer,
    )
    .await
    .expect_err("early EOF must fail instead of publishing a corrupt file");

    assert!(matches!(error.kind, ErrorKind::RetryableNet));
    assert!(!destination.exists());
    assert_eq!(
        tokio::fs::read(part_path)
            .await
            .expect("retryable failure should retain the staged prefix"),
        original[..CHUNK]
    );
}

#[tokio::test]
async fn production_upload_and_download_engines_preserve_bytes_and_metadata() {
    const CHUNK: usize = 64 * 1024 + 3;

    for size in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK + 17] {
        let operator = opendal::Operator::new(Memory::default())
            .expect("memory service should build")
            .finish();
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let source = directory.path().join(format!("source-{size}.arw"));
        let destination = directory.path().join(format!("downloaded-{size}.arw"));
        let key = format!("camera/DSC-{size}.ARW");
        let original = patterned_bytes(size, 91);
        tokio::fs::write(&source, &original)
            .await
            .expect("source fixture should write");

        let (upload_paused, upload_cancelled) = control_flags();
        let mut upload_observer = UploadObserver::default();
        upload_file_for_integration(
            &operator,
            IntegrationUploadRequest {
                key: key.clone(),
                source_path: source,
                part_size: CHUNK as u64,
                content_type: None,
                content_disposition: Some("attachment; filename=\"DSC.ARW\"".into()),
            },
            IntegrationUploadControl {
                paused: upload_paused,
                cancelled: upload_cancelled,
            },
            &mut upload_observer,
        )
        .await
        .expect("production upload engine should complete");

        let remote_metadata = operator
            .stat(&key)
            .await
            .expect("uploaded object should exist");
        assert_eq!(remote_metadata.content_length(), size as u64);
        assert_eq!(remote_metadata.content_type(), Some("image/x-sony-arw"));
        assert_eq!(
            remote_metadata.content_disposition(),
            Some("attachment; filename=\"DSC.ARW\"")
        );
        assert_eq!(upload_observer.uploaded_bytes, size as u64);
        assert!(upload_observer.finalized);

        let (download_paused, download_cancelled) = control_flags();
        let mut download_observer = DownloadObserver::default();
        let output = download_to_stage_for_integration(
            &operator,
            IntegrationDownloadRequest {
                key,
                temp_path: destination.clone(),
                chunk_size: CHUNK as u64,
                expected_etag: None,
                recorded_bytes_done: 0,
            },
            IntegrationDownloadControl {
                paused: download_paused,
                cancelled: download_cancelled,
            },
            &mut download_observer,
        )
        .await
        .expect("production download engine should complete");

        let downloaded = tokio::fs::read(&destination)
            .await
            .expect("downloaded file should exist");
        assert_eq!(output.total, size as u64);
        assert_eq!(download_observer.remote_total, Some(size as u64));
        assert_eq!(download_observer.downloaded_bytes, size as u64);
        assert_eq!(downloaded, original);
        assert_eq!(Sha256::digest(&downloaded), Sha256::digest(&original));
        assert!(
            !destination.with_extension("arw.part").exists(),
            "completed transfer left a staged file"
        );
    }
}
