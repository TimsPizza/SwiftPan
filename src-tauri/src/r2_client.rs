use crate::types::*;

pub struct R2Client; // placeholder until implementation

pub async fn build_client(_cfg: &R2Config) -> SpResult<R2Client> {
  Err(err_not_implemented("r2_client.build_client"))
}

pub async fn sanity_check(_client: &R2Client, _test_prefix: &str) -> SpResult<()> {
  Err(err_not_implemented("r2_client.sanity_check"))
}

pub async fn presign_get_url(
  _client: &R2Client,
  _key: &str,
  _ttl_secs: u64,
  _download_filename: Option<String>,
) -> SpResult<(String, i64)> {
  Err(err_not_implemented("r2_client.presign_get_url"))
}

