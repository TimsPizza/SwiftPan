use crate::types::*;
use crate::{credential_vault::CredentialVault, r2_client};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

pub struct UsageSync;

impl UsageSync {
    pub fn record_local_delta(delta: UsageDelta) -> SpResult<()> {
        let p = local_delta_path_for_today()?;
        let mut cur: UsageDelta = if p.exists() {
            serde_json::from_slice(&fs::read(&p).map_err(ioe)?).unwrap_or(UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
            })
        } else {
            UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
            }
        };
        for (k, v) in delta.class_a {
            *cur.class_a.entry(k).or_insert(0) += v;
        }
        for (k, v) in delta.class_b {
            *cur.class_b.entry(k).or_insert(0) += v;
        }
        cur.ingress_bytes += delta.ingress_bytes;
        cur.egress_bytes += delta.egress_bytes;
        fs::create_dir_all(p.parent().unwrap()).map_err(ioe)?;
        fs::write(p, serde_json::to_vec(&cur).unwrap()).map_err(ioe)?;
        Ok(())
    }

    pub async fn merge_and_write_day(date: &str) -> SpResult<DailyLedger> {
        let p = local_delta_path(date)?;
        let local: UsageDelta = if p.exists() {
            serde_json::from_slice(&fs::read(&p).map_err(ioe)?).unwrap_or(UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
            })
        } else {
            UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
            }
        };

        let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
        let client = r2_client::build_client(&bundle.r2).await?;
        let key = format!("{}{}.json", ANALYTICS_PREFIX, date);

        // Ensure file exists
        let _ = r2_client::put_object_bytes(
            &client,
            &key,
            serde_json::to_vec(&DailyLedger {
                date: date.into(),
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
                storage_bytes: 0,
                rev: 1,
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap(),
            None,
            true,
        )
        .await;

        // GET current
        let (bytes, etag) = r2_client::get_object_bytes(&client, &key).await?;
        let mut day: DailyLedger = serde_json::from_slice(&bytes).unwrap_or(DailyLedger {
            date: date.into(),
            class_a: Default::default(),
            class_b: Default::default(),
            ingress_bytes: 0,
            egress_bytes: 0,
            storage_bytes: 0,
            rev: 1,
            updated_at: chrono::Utc::now().to_rfc3339(),
        });
        for (k, v) in local.class_a {
            *day.class_a.entry(k).or_insert(0) += v;
        }
        for (k, v) in local.class_b {
            *day.class_b.entry(k).or_insert(0) += v;
        }
        day.ingress_bytes += local.ingress_bytes;
        day.egress_bytes += local.egress_bytes;
        day.updated_at = chrono::Utc::now().to_rfc3339();
        day.rev += 1;

        // PUT If-Match
        r2_client::put_object_bytes(
            &client,
            &key,
            serde_json::to_vec(&day).unwrap(),
            etag,
            false,
        )
        .await?;

        // Clear local
        let _ = fs::remove_file(p);
        Ok(day)
    }

    pub async fn list_month(prefix: &str) -> SpResult<Vec<DailyLedger>> {
        // Simple: iterate days 01..31 and GET existing
        let bundle = CredentialVault::get_decrypted_bundle_if_unlocked()?;
        let client = r2_client::build_client(&bundle.r2).await?;
        let mut out = vec![];
        for day in 1..=31u32 {
            let key = format!("{}{}-{:02}.json", ANALYTICS_PREFIX, prefix, day);
            if let Ok((bytes, _)) = r2_client::get_object_bytes(&client, &key).await {
                if let Ok(v) = serde_json::from_slice::<DailyLedger>(&bytes) {
                    out.push(v);
                }
            }
        }
        Ok(out)
    }
}

fn app_dir() -> SpResult<PathBuf> {
    let proj = ProjectDirs::from("com", "swiftpan", "SwiftPan").ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "project dirs not available".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    Ok(proj.data_dir().to_path_buf())
}

fn local_delta_path_for_today() -> SpResult<PathBuf> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    local_delta_path(&today)
}

fn local_delta_path(date: &str) -> SpResult<PathBuf> {
    let dir = app_dir()?.join("usage_deltas");
    Ok(dir.join(format!("{}.json", date)))
}

fn ioe(e: std::io::Error) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: e.to_string(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}
