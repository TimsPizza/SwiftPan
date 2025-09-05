use crate::types::*;
use crate::usage::UsageSync;
use aws_config::Region;
use aws_sdk_s3 as s3;
use aws_sdk_s3::config::Credentials;

pub struct R2Client {
    pub s3: s3::Client,
    pub bucket: String,
}

pub async fn build_client(cfg: &R2Config) -> SpResult<R2Client> {
    crate::logger::debug(
        "r2",
        &format!(
            "build_client endpoint={} bucket={} region={}",
            cfg.endpoint,
            cfg.bucket,
            cfg.region.as_deref().unwrap_or("auto")
        ),
    );
    let region = cfg.region.clone().unwrap_or_else(|| "auto".to_string());
    let base = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(Region::new(region))
        .load()
        .await;
    let conf = s3::config::Builder::from(&base)
        .endpoint_url(cfg.endpoint.clone())
        .force_path_style(true)
        .credentials_provider(Credentials::new(
            cfg.access_key_id.clone(),
            cfg.secret_access_key.clone(),
            None,
            None,
            "swiftpan",
        ))
        .build();
    let s3 = s3::Client::from_conf(conf);
    Ok(R2Client {
        s3,
        bucket: cfg.bucket.clone(),
    })
}

pub async fn sanity_check(client: &R2Client, test_prefix: &str) -> SpResult<()> {
    crate::logger::debug("r2", &format!("sanity_check start prefix={}", test_prefix));
    // List on prefix (should succeed, even if empty)
    let _ = client
        .s3
        .list_objects_v2()
        .bucket(&client.bucket)
        .prefix(test_prefix)
        .max_keys(1)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("ListObjectsV2 failed: {}", e));
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("ListObjectsV2 failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    // Usage: B 类 ListObjectsV2 +1
    let mut b = std::collections::HashMap::new();
    b.insert("ListObjectsV2".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });

    // Put -> Head -> Delete on a temp key
    let temp_key = format!(
        "{}/swiftpan_sanity_{}.txt",
        test_prefix.trim_end_matches('/'),
        chrono::Utc::now().timestamp_millis()
    );
    client
        .s3
        .put_object()
        .bucket(&client.bucket)
        .key(&temp_key)
        .body(s3::primitives::ByteStream::from_static(b"ok"))
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("PutObject failed: {}", e));
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("PutObject failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    crate::logger::debug(
        "r2",
        &format!("sanity_check put_object ok key={}", temp_key),
    );
    // Usage: A 类 PutObject +1，入口字节很小（2B），计一次 ingress_bytes
    let mut a = std::collections::HashMap::new();
    a.insert("PutObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: a,
        class_b: Default::default(),
        ingress_bytes: 2,
        egress_bytes: 0,
        added_storage_bytes: 2,
        deleted_storage_bytes: 0,
    });

    let _ = client
        .s3
        .head_object()
        .bucket(&client.bucket)
        .key(&temp_key)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("HeadObject failed: {}", e));
            SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("HeadObject failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    crate::logger::debug(
        "r2",
        &format!("sanity_check head_object ok key={}", temp_key),
    );
    // Usage: B 类 HeadObject +1
    let mut b = std::collections::HashMap::new();
    b.insert("HeadObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });

    let _ = client
        .s3
        .delete_object()
        .bucket(&client.bucket)
        .key(&temp_key)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("DeleteObject failed: {}", e));
            SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("DeleteObject failed: {e}"),
                retry_after_ms: Some(1000),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    crate::logger::debug(
        "r2",
        &format!("sanity_check delete_object ok key={}", temp_key),
    );
    // Usage: A 类 DeleteObject +1；删除 2B
    let mut a = std::collections::HashMap::new();
    a.insert("DeleteObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: a,
        class_b: Default::default(),
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: 2,
    });
    Ok(())
}

pub async fn presign_get_url(
    client: &R2Client,
    key: &str,
    ttl_secs: u64,
    download_filename: Option<String>,
) -> SpResult<(String, i64)> {
    let mut get = client.s3.get_object().bucket(&client.bucket).key(key);
    if let Some(name) = download_filename {
        get = get.response_content_disposition(format!("attachment; filename=\"{}\"", name));
    }
    let conf =
        s3::presigning::PresigningConfig::expires_in(std::time::Duration::from_secs(ttl_secs))
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("Invalid TTL: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;
    let presigned = get.presigned(conf).await.map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("Presign failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let url = presigned.uri().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_secs as i64);
    Ok((url, expires_at.timestamp_millis()))
}

pub async fn list_objects(
    client: &R2Client,
    prefix: &str,
    continuation: Option<String>,
    max_keys: i32,
) -> SpResult<crate::types::ListPage> {
    use crate::types::{FileEntry, ANALYTICS_PREFIX};
    let mut req = client
        .s3
        .list_objects_v2()
        .bucket(&client.bucket)
        .prefix(prefix)
        .delimiter("/")
        .max_keys(max_keys);
    if let Some(tok) = continuation.clone() {
        req = req.continuation_token(tok);
    }
    let out = req.send().await.map_err(|e| {
        crate::logger::error("r2", &format!("ListObjectsV2 error: {}", e));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("ListObjectsV2: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    })?;
    // Usage: B 类 ListObjectsV2 +1
    let mut b = std::collections::HashMap::new();
    b.insert("ListObjectsV2".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });

    let mut items: Vec<FileEntry> = vec![];
    for o in out.contents() {
        let key = o.key().unwrap_or_default().to_string();
        items.push(FileEntry {
            key: key.clone(),
            size: o.size().map(|v| v as u64),
            last_modified_ms: None,
            etag: o.e_tag().map(|s| s.trim_matches('"').to_string()),
            is_prefix: false,
            protected: key.starts_with(ANALYTICS_PREFIX),
        });
    }
    for p in out.common_prefixes() {
        let k = p.prefix().unwrap_or_default().to_string();
        items.push(FileEntry {
            key: k.clone(),
            size: None,
            last_modified_ms: None,
            etag: None,
            is_prefix: true,
            protected: k.starts_with(ANALYTICS_PREFIX),
        });
    }
    // Stable order: prefixes first then objects by name
    items.sort_by(|a, b| match (a.is_prefix, b.is_prefix) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.key.cmp(&b.key),
    });
    let page = crate::types::ListPage {
        prefix: prefix.to_string(),
        items,
        next_token: out.next_continuation_token().map(|s| s.to_string()),
    };
    crate::logger::info(
        "r2",
        &format!(
            "list_objects ok prefix={} items={} next_token_present={}",
            prefix,
            page.items.len(),
            page.next_token.is_some()
        ),
    );
    Ok(page)
}

pub async fn list_all_objects_flat(
    client: &R2Client,
    max_total: i32,
) -> SpResult<Vec<crate::types::FileEntry>> {
    use crate::types::{FileEntry, ANALYTICS_PREFIX};
    let mut items: Vec<FileEntry> = vec![];
    let mut token: Option<String> = None;
    loop {
        let mut req = client
            .s3
            .list_objects_v2()
            .bucket(&client.bucket)
            .max_keys(1000);
        if let Some(tok) = token.clone() {
            req = req.continuation_token(tok);
        }
        let out = req.send().await.map_err(|e| {
            crate::logger::error("r2", &format!("ListObjectsV2 (flat) error: {}", e));
            SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("ListObjectsV2: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
        for o in out.contents() {
            let key = o.key().unwrap_or_default().to_string();
            items.push(FileEntry {
                key: key.clone(),
                size: o.size().map(|v| v as u64),
                last_modified_ms: o
                    .last_modified()
                    .and_then(|dt| dt.secs().checked_mul(1000))
                    .map(|ms| ms as i64),
                etag: o.e_tag().map(|s| s.trim_matches('"').to_string()),
                is_prefix: false,
                protected: key.starts_with(ANALYTICS_PREFIX),
            });
        }
        if items.len() as i32 >= max_total {
            break;
        }
        token = out.next_continuation_token().map(|s| s.to_string());
        if token.is_none() {
            break;
        }
    }
    // Stable sort by key
    items.sort_by(|a, b| a.key.cmp(&b.key));
    crate::logger::info(
        "r2",
        &format!("list_all_objects_flat ok total_items={}", items.len()),
    );
    Ok(items)
}

pub async fn delete_object(client: &R2Client, key: &str) -> SpResult<()> {
    // Best-effort HEAD to capture size
    let size_opt = client
        .s3
        .head_object()
        .bucket(&client.bucket)
        .key(key)
        .send()
        .await
        .ok()
        .and_then(|h| h.content_length())
        .map(|v| v as u64);
    client
        .s3
        .delete_object()
        .bucket(&client.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("DeleteObject error: {}", e));
            SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("DeleteObject: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    // Usage: A 类 DeleteObject +1；无法获知对象大小，这里先只计操作，删除字节数需由上层传入或预先 HEAD
    let mut a = std::collections::HashMap::new();
    a.insert("DeleteObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: a,
        class_b: Default::default(),
        ingress_bytes: 0,
        egress_bytes: 0,
        added_storage_bytes: 0,
        deleted_storage_bytes: size_opt.unwrap_or(0),
    });
    Ok(())
}

pub async fn get_object_bytes(client: &R2Client, key: &str) -> SpResult<(Vec<u8>, Option<String>)> {
    let resp = client
        .s3
        .get_object()
        .bucket(&client.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("GetObject error: {}", e));
            SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("GetObject: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?;
    // Usage: B 类 GetObject +1；egress 按返回大小
    let mut b = std::collections::HashMap::new();
    b.insert("GetObject".into(), 1u64);
    let etag = resp.e_tag().map(|s| s.trim_matches('"').to_string());
    let data = resp
        .body
        .collect()
        .await
        .map_err(|e| {
            crate::logger::error("r2", &format!("GetObject read body error: {}", e));
            SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("read body: {e}"),
                retry_after_ms: Some(300),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            }
        })?
        .to_vec();
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: Default::default(),
        class_b: b,
        ingress_bytes: 0,
        egress_bytes: data.len() as u64,
        added_storage_bytes: 0,
        deleted_storage_bytes: 0,
    });
    Ok((data, etag))
}

pub async fn put_object_bytes(
    client: &R2Client,
    key: &str,
    bytes: Vec<u8>,
    if_match: Option<String>,
    if_none_match: bool,
) -> SpResult<()> {
    let write_len = bytes.len() as u64;
    let mut req = client
        .s3
        .put_object()
        .bucket(&client.bucket)
        .key(key)
        .body(s3::primitives::ByteStream::from(bytes));
    if let Some(tag) = if_match {
        req = req.if_match(tag);
    }
    if if_none_match {
        req = req.if_none_match("*");
    }
    req.send().await.map_err(|e| {
        crate::logger::error("r2", &format!("PutObject error: {}", e));
        SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("PutObject: {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        }
    })?;
    // Usage: A 类 PutObject +1；ingress 按写入大小
    let mut a = std::collections::HashMap::new();
    a.insert("PutObject".into(), 1u64);
    let _ = UsageSync::record_local_delta(UsageDelta {
        class_a: a,
        class_b: Default::default(),
        ingress_bytes: write_len,
        egress_bytes: 0,
        added_storage_bytes: write_len,
        deleted_storage_bytes: 0,
    });
    Ok(())
}
