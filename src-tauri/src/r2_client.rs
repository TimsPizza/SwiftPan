use crate::types::*;
use aws_config::Region;
use aws_sdk_s3 as s3;
use aws_sdk_s3::types::Object;

pub struct R2Client {
  pub s3: s3::Client,
  pub bucket: String,
}

pub async fn build_client(cfg: &R2Config) -> SpResult<R2Client> {
  let region = cfg.region.clone().unwrap_or_else(|| "auto".to_string());
  let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest()).region(Region::new(region));
  // Endpoint override for R2
  let endpoint_url = cfg.endpoint.clone();
  let endpoint = aws_sdk_s3::config::Region::new("auto");
  let config = aws_sdk_s3::config::Builder::from(&aws_sdk_s3::config::Builder::new().region(endpoint))
    .endpoint_url(endpoint_url)
    .credentials_provider(aws_credential_types::Credentials::new(
      cfg.access_key_id.clone(),
      cfg.secret_access_key.clone(),
      None,
      None,
      "swiftpan",
    ))
    .behavior_version_latest()
    .build();

  let s3 = s3::Client::from_conf(config);
  Ok(R2Client { s3, bucket: cfg.bucket.clone() })
}

pub async fn sanity_check(client: &R2Client, test_prefix: &str) -> SpResult<()> {
  // List on prefix (should succeed, even if empty)
  let _ = client
    .s3
    .list_objects_v2()
    .bucket(&client.bucket)
    .prefix(test_prefix)
    .max_keys(1)
    .send()
    .await
    .map_err(|e| SpError {
      kind: ErrorKind::NotRetriable,
      message: format!("ListObjectsV2 failed: {e}"),
      retry_after_ms: None,
      context: None,
      at: chrono::Utc::now().timestamp_millis(),
    })?;

  // Put -> Head -> Delete on a temp key
  let temp_key = format!("{}/swiftpan_sanity_{}.txt", test_prefix.trim_end_matches('/'), chrono::Utc::now().timestamp_millis());
  client
    .s3
    .put_object()
    .bucket(&client.bucket)
    .key(&temp_key)
    .body(s3::primitives::ByteStream::from_static(b"ok"))
    .send()
    .await
    .map_err(|e| SpError {
      kind: ErrorKind::NotRetriable,
      message: format!("PutObject failed: {e}"),
      retry_after_ms: None,
      context: None,
      at: chrono::Utc::now().timestamp_millis(),
    })?;

  let _ = client
    .s3
    .head_object()
    .bucket(&client.bucket)
    .key(&temp_key)
    .send()
    .await
    .map_err(|e| SpError {
      kind: ErrorKind::NotRetriable,
      message: format!("HeadObject failed: {e}"),
      retry_after_ms: None,
      context: None,
      at: chrono::Utc::now().timestamp_millis(),
    })?;

  let _ = client
    .s3
    .delete_object()
    .bucket(&client.bucket)
    .key(&temp_key)
    .send()
    .await
    .map_err(|e| SpError {
      kind: ErrorKind::RetryableNet,
      message: format!("DeleteObject failed: {e}"),
      retry_after_ms: Some(1000),
      context: None,
      at: chrono::Utc::now().timestamp_millis(),
    })?;
  Ok(())
}

pub async fn presign_get_url(
  client: &R2Client,
  key: &str,
  ttl_secs: u64,
  download_filename: Option<String>,
) -> SpResult<(String, i64)> {
  use aws_sdk_s3::types::SdkError;
  let mut get = client.s3.get_object().bucket(&client.bucket).key(key);
  if let Some(name) = download_filename {
    get = get.response_content_disposition(format!("attachment; filename=\"{}\"", name));
  }
  let conf = s3::presigning::PresigningConfig::expires_in(std::time::Duration::from_secs(ttl_secs))
    .map_err(|e| SpError {
      kind: ErrorKind::NotRetriable,
      message: format!("Invalid TTL: {e}"),
      retry_after_ms: None,
      context: None,
      at: chrono::Utc::now().timestamp_millis(),
    })?;
  let presigned = get
    .presigned(conf)
    .await
    .map_err(|e| SpError {
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
  let out = req
    .send()
    .await
    .map_err(|e| SpError { kind: ErrorKind::RetryableNet, message: format!("ListObjectsV2: {e}"), retry_after_ms: Some(500), context: None, at: chrono::Utc::now().timestamp_millis() })?;

  let mut items: Vec<FileEntry> = vec![];
  if let Some(objs) = out.contents() {
    for o in objs {
      let key = o.key().unwrap_or_default().to_string();
      items.push(FileEntry {
        key: key.clone(),
        size: o.size().map(|v| v as u64),
        last_modified_ms: o
          .last_modified()
          .and_then(|dt| dt.to_chrono_utc().ok())
          .map(|t| t.timestamp_millis()),
        etag: o.e_tag().map(|s| s.trim_matches('"').to_string()),
        is_prefix: false,
        protected: key.starts_with(ANALYTICS_PREFIX),
      });
    }
  }
  if let Some(prefixes) = out.common_prefixes() {
    for p in prefixes {
      let k = p.prefix().unwrap_or_default().to_string();
      items.push(FileEntry { key: k.clone(), size: None, last_modified_ms: None, etag: None, is_prefix: true, protected: k.starts_with(ANALYTICS_PREFIX) });
    }
  }
  // Stable order: prefixes first then objects by name
  items.sort_by(|a, b| match (a.is_prefix, b.is_prefix) {
    (true, false) => std::cmp::Ordering::Less,
    (false, true) => std::cmp::Ordering::Greater,
    _ => a.key.cmp(&b.key),
  });
  Ok(crate::types::ListPage { prefix: prefix.to_string(), items, next_token: out.next_continuation_token().map(|s| s.to_string()) })
}

pub async fn delete_object(client: &R2Client, key: &str) -> SpResult<()> {
  client
    .s3
    .delete_object()
    .bucket(&client.bucket)
    .key(key)
    .send()
    .await
    .map_err(|e| SpError { kind: ErrorKind::RetryableNet, message: format!("DeleteObject: {e}"), retry_after_ms: Some(500), context: None, at: chrono::Utc::now().timestamp_millis() })?;
  Ok(())
}
