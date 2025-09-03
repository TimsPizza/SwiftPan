use crate::types::*;
use crate::{credential_vault::CredentialVault, r2_client};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUploadParams {
    pub key: String,
    pub source_path: String,
    pub part_size: u64,
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStatus {
    pub transfer_id: String,
    pub key: String,
    pub bytes_total: u64,
    pub bytes_done: u64,
    pub parts_completed: u32,
    pub rate_bps: u64,
    pub eta_ms: Option<u64>,
    pub last_error: Option<SpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UploadEvent {
    Started {
        transfer_id: String,
    },
    PartProgress {
        transfer_id: String,
        progress: UploadPartProgress,
    },
    PartDone {
        transfer_id: String,
        part_number: u32,
        etag: String,
    },
    Paused {
        transfer_id: String,
    },
    Resumed {
        transfer_id: String,
    },
    Completed {
        transfer_id: String,
    },
    Failed {
        transfer_id: String,
        error: SpError,
    },
}

struct UTransfer {
    key: String,
    src: PathBuf,
    part_size: u64,
    bytes_total: u64,
    bytes_done: u64,
    parts_completed: u32,
    last_error: Option<SpError>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

static UL: Lazy<Mutex<HashMap<String, UTransfer>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn start_upload(params: NewUploadParams) -> SpResult<String> {
    let meta = tokio::fs::metadata(&params.source_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("stat src: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let total = meta.len();
    let id = uuid::Uuid::new_v4().to_string();
    let paused = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut g = UL.lock().unwrap();
        g.insert(
            id.clone(),
            UTransfer {
                key: params.key.clone(),
                src: PathBuf::from(&params.source_path),
                part_size: params.part_size.max(8 * 1024 * 1024),
                bytes_total: total,
                bytes_done: 0,
                parts_completed: 0,
                last_error: None,
                paused: paused.clone(),
                cancelled: cancelled.clone(),
            },
        );
    }
    let id_spawn = id.clone();
    tokio::spawn(async move {
        let res = run_upload(&id_spawn, params, paused.clone(), cancelled.clone()).await;
        if let Err(e) = res {
            let mut g = UL.lock().unwrap();
            if let Some(t) = g.get_mut(&id_spawn) {
                t.last_error = Some(e);
            }
        }
    });
    Ok(id)
}

async fn run_upload(
    id: &str,
    params: NewUploadParams,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> SpResult<()> {
    let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
    let client = r2_client::build_client(&bundle.r2).await?;
    let mut file = tokio::fs::File::open(&params.source_path)
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("open src: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    let meta = file.metadata().await.map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("stat src: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let size = meta.len();

    if size <= params.part_size {
        // Single PUT
        let mut buf = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buf).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("read src: {e}"),
            retry_after_ms: Some(200),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let mut req = client
            .s3
            .put_object()
            .bucket(&client.bucket)
            .key(&params.key)
            .body(aws_sdk_s3::primitives::ByteStream::from(buf));
        if let Some(ct) = params.content_type.clone() {
            req = req.content_type(ct);
        }
        if let Some(cd) = params.content_disposition.clone() {
            req = req.content_disposition(cd);
        }
        req.send().await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("PutObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        let mut g = UL.lock().unwrap();
        if let Some(t) = g.get_mut(id) {
            t.bytes_done = size;
            t.parts_completed = 1;
        }
        return Ok(());
    }

    // Multipart upload
    let mut create = client
        .s3
        .create_multipart_upload()
        .bucket(&client.bucket)
        .key(&params.key);
    if let Some(ct) = params.content_type.clone() {
        create = create.content_type(ct);
    }
    if let Some(cd) = params.content_disposition.clone() {
        create = create.content_disposition(cd);
    }
    let created = create.send().await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("CreateMultipartUpload: {e}"),
        retry_after_ms: Some(500),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let upload_id = created.upload_id().unwrap_or_default().to_string();

    let mut part_number: i32 = 1;
    let mut completed_parts: Vec<aws_sdk_s3::types::CompletedPart> = vec![];
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        while paused.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        let mut buf = vec![0u8; params.part_size as usize];
        let n = file.read(&mut buf).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("read src: {e}"),
            retry_after_ms: Some(200),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        if n == 0 {
            break;
        }
        buf.truncate(n);
        let out = client
            .s3
            .upload_part()
            .bucket(&client.bucket)
            .key(&params.key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(aws_sdk_s3::primitives::ByteStream::from(buf))
            .send()
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("UploadPart#{part_number}: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
        let etag = out.e_tag().map(|s| s.to_string()).unwrap_or_default();
        completed_parts.push(
            aws_sdk_s3::types::CompletedPart::builder()
                .e_tag(etag)
                .part_number(part_number)
                .build(),
        );
        {
            let mut g = UL.lock().unwrap();
            if let Some(t) = g.get_mut(id) {
                t.bytes_done = (t.bytes_done as u64 + n as u64) as u64;
                t.parts_completed += 1;
            }
        }
        part_number += 1;
    }

    if cancelled.load(Ordering::Relaxed) {
        let _ = client
            .s3
            .abort_multipart_upload()
            .bucket(&client.bucket)
            .key(&params.key)
            .upload_id(upload_id)
            .send()
            .await;
        return Err(SpError {
            kind: ErrorKind::Cancelled,
            message: "cancelled".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        });
    }

    // Complete
    let comp = aws_sdk_s3::types::CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();
    client
        .s3
        .complete_multipart_upload()
        .bucket(&client.bucket)
        .key(&params.key)
        .upload_id(upload_id)
        .multipart_upload(comp)
        .send()
        .await
        .map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("CompleteMultipartUpload: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
    Ok(())
}

pub fn pause(id: &str) -> SpResult<()> {
    let g = UL.lock().unwrap();
    if let Some(t) = g.get(id) {
        t.paused.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}
pub fn resume(id: &str) -> SpResult<()> {
    let g = UL.lock().unwrap();
    if let Some(t) = g.get(id) {
        t.paused.store(false, Ordering::Relaxed);
        Ok(())
    } else {
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}
pub fn cancel(id: &str) -> SpResult<()> {
    let g = UL.lock().unwrap();
    if let Some(t) = g.get(id) {
        t.cancelled.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err(SpError {
            kind: ErrorKind::NotRetriable,
            message: "not found".into(),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })
    }
}
pub fn status(id: &str) -> SpResult<UploadStatus> {
    let g = UL.lock().unwrap();
    let t = g.get(id).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "not found".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(UploadStatus {
        transfer_id: id.into(),
        key: t.key.clone(),
        bytes_total: t.bytes_total,
        bytes_done: t.bytes_done,
        parts_completed: t.parts_completed,
        rate_bps: 0,
        eta_ms: None,
        last_error: t.last_error.clone(),
    })
}
