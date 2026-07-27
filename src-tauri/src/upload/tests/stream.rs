use super::super::*;
use crate::test_support::patterned_bytes;
use opendal::services::Memory;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Default)]
struct StreamObserver {
    uploaded_parts: Vec<(u32, u64)>,
    finalizing: bool,
}

impl StreamUploadObserver for StreamObserver {
    fn uploading(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn part_done(&mut self, part_number: u32, bytes_transferred: u64) -> SpResult<()> {
        self.uploaded_parts.push((part_number, bytes_transferred));
        Ok(())
    }

    fn finalizing(&mut self) -> SpResult<()> {
        self.finalizing = true;
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn stream_engine_preserves_chunk_order_and_bytes() {
    let operator = opendal::Operator::new(Memory::default())
        .expect("memory operator should build")
        .finish();
    let chunks = [
        patterned_bytes(17, 3),
        patterned_bytes(64 * 1024 + 1, 5),
        patterned_bytes(29, 7),
    ];
    let expected = chunks.concat();
    let (sender, receiver) = tokio::sync::mpsc::channel(8);
    for chunk in chunks {
        sender
            .send(Some(chunk))
            .await
            .expect("stream chunk should queue");
    }
    sender.send(None).await.expect("stream finish should queue");
    let mut observer = StreamObserver::default();

    upload_stream(
        &operator,
        StreamUploadRequest {
            key: "stream/ordered.bin".into(),
            expected_bytes: expected.len() as u64,
            content_type: Some("application/x-stream-test".into()),
            content_disposition: None,
        },
        receiver,
        UploadControl {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
        },
        &mut observer,
    )
    .await
    .expect("stream upload should complete");

    let uploaded = operator
        .read("stream/ordered.bin")
        .await
        .expect("streamed object should exist")
        .to_bytes();
    assert_eq!(uploaded.as_ref(), expected);
    assert_eq!(
        observer.uploaded_parts,
        vec![(1, 17), (2, 64 * 1024 + 1), (3, 29)]
    );
    assert!(observer.finalizing);
}

fn memory_operator() -> opendal::Operator {
    opendal::Operator::new(Memory::default())
        .expect("memory operator should build")
        .finish()
}

fn stream_control() -> UploadControl {
    UploadControl {
        paused: Arc::new(AtomicBool::new(false)),
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}

#[tokio::test]
async fn sender_disconnect_without_finish_cannot_publish_partial_stream() {
    let operator = memory_operator();
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender
        .send(Some(patterned_bytes(1024, 21)))
        .await
        .expect("stream chunk should queue");
    drop(sender);
    let mut observer = StreamObserver::default();

    upload_stream(
        &operator,
        StreamUploadRequest {
            key: "stream/disconnected.bin".into(),
            expected_bytes: 1024,
            content_type: None,
            content_disposition: None,
        },
        receiver,
        stream_control(),
        &mut observer,
    )
    .await
    .expect_err("channel disconnect is not an explicit successful finish");

    assert!(!observer.finalizing);
    assert!(!operator
        .exists("stream/disconnected.bin")
        .await
        .expect("existence check should work"));
}

#[tokio::test]
async fn explicit_finish_rejects_stream_shorter_than_declared_total() {
    const DECLARED_TOTAL: usize = 4096;
    let operator = memory_operator();
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender
        .send(Some(patterned_bytes(1024, 31)))
        .await
        .expect("stream chunk should queue");
    sender.send(None).await.expect("stream finish should queue");
    let mut observer = StreamObserver::default();

    let result = upload_stream(
        &operator,
        StreamUploadRequest {
            key: "stream/too-short.bin".into(),
            expected_bytes: DECLARED_TOTAL as u64,
            content_type: None,
            content_disposition: None,
        },
        receiver,
        stream_control(),
        &mut observer,
    )
    .await;

    assert!(
        result.is_err(),
        "declared {DECLARED_TOTAL} bytes but stream finalized after only 1024"
    );
}

#[tokio::test]
async fn stream_rejects_bytes_beyond_declared_total() {
    const DECLARED_TOTAL: usize = 1024;
    let operator = memory_operator();
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender
        .send(Some(patterned_bytes(DECLARED_TOTAL + 1, 41)))
        .await
        .expect("oversized stream chunk should queue");
    sender.send(None).await.expect("stream finish should queue");
    let mut observer = StreamObserver::default();

    let result = upload_stream(
        &operator,
        StreamUploadRequest {
            key: "stream/too-long.bin".into(),
            expected_bytes: DECLARED_TOTAL as u64,
            content_type: None,
            content_disposition: None,
        },
        receiver,
        stream_control(),
        &mut observer,
    )
    .await;

    assert!(
        result.is_err(),
        "stream accepted more than the declared {DECLARED_TOTAL} bytes"
    );
}

#[tokio::test]
async fn empty_stream_chunks_do_not_create_false_part_progress() {
    let operator = memory_operator();
    let payload = patterned_bytes(513, 51);
    let (sender, receiver) = tokio::sync::mpsc::channel(3);
    sender
        .send(Some(Vec::new()))
        .await
        .expect("empty chunk should queue");
    sender
        .send(Some(payload.clone()))
        .await
        .expect("payload should queue");
    sender.send(None).await.expect("stream finish should queue");
    let mut observer = StreamObserver::default();

    upload_stream(
        &operator,
        StreamUploadRequest {
            key: "stream/empty-chunk.bin".into(),
            expected_bytes: payload.len() as u64,
            content_type: None,
            content_disposition: None,
        },
        receiver,
        stream_control(),
        &mut observer,
    )
    .await
    .expect("empty chunk should be ignored without corrupting the stream");

    assert_eq!(observer.uploaded_parts, vec![(1, payload.len() as u64)]);
    assert_eq!(
        operator
            .read("stream/empty-chunk.bin")
            .await
            .expect("streamed object should exist")
            .to_bytes()
            .as_ref(),
        payload
    );
}

struct CancelStreamWhenPaused {
    cancelled: Arc<AtomicBool>,
}

impl StreamUploadObserver for CancelStreamWhenPaused {
    fn uploading(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn paused(&mut self) -> SpResult<()> {
        self.cancelled.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn resumed(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn part_done(&mut self, _part_number: u32, _bytes_transferred: u64) -> SpResult<()> {
        Ok(())
    }

    fn finalizing(&mut self) -> SpResult<()> {
        Ok(())
    }

    fn cancelled(&mut self) -> SpResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn paused_stream_upload_observes_cancellation_without_requiring_resume() {
    let operator = memory_operator();
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender
        .send(Some(patterned_bytes(128, 61)))
        .await
        .expect("stream chunk should queue");
    let paused = Arc::new(AtomicBool::new(true));
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut observer = CancelStreamWhenPaused {
        cancelled: cancelled.clone(),
    };

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(750),
        upload_stream(
            &operator,
            StreamUploadRequest {
                key: "stream/paused-cancel.bin".into(),
                expected_bytes: 128,
                content_type: None,
                content_disposition: None,
            },
            receiver,
            UploadControl { paused, cancelled },
            &mut observer,
        ),
    )
    .await
    .expect("paused stream must react to cancellation promptly")
    .expect_err("cancelled paused stream must fail");

    assert!(matches!(result.kind, ErrorKind::Cancelled));
    drop(sender);
}
