use crate::types::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShareParams {
  pub key: String,
  pub ttl_secs: u64, // 900, 3600, 86400
  pub download_filename: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShareLink {
  pub url: String,
  pub expires_at_ms: i64,
}

pub async fn generate_share_link(_params: ShareParams) -> SpResult<ShareLink> {
  Err(err_not_implemented("share.generate_share_link"))
}

