use super::super::*;
use crate::test_support::patterned_bytes;
use opendal::services::Memory;
use std::sync::{atomic::AtomicBool, Arc};

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
