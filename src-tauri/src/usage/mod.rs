use crate::types::*;
use crate::{r2_client, sp_backend::SpBackend};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
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
                added_storage_bytes: 0,
                deleted_storage_bytes: 0,
            })
        } else {
            UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
                added_storage_bytes: 0,
                deleted_storage_bytes: 0,
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
        cur.added_storage_bytes += delta.added_storage_bytes;
        cur.deleted_storage_bytes += delta.deleted_storage_bytes;
        // Ensure parent dir exists
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(ioe)?;
        } else {
            return Err(SpError {
                kind: ErrorKind::NotRetriable,
                message: "invalid usage delta path".into(),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            });
        }
        let cur_bytes = serde_json::to_vec(&cur).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize usage delta failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        fs::write(p, cur_bytes).map_err(ioe)?;
        Ok(())
    }

    pub async fn merge_and_write_day(date: &str) -> SpResult<DailyLedger> {
        // Fast-path: if already merged today, skip R2 ops entirely
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        if date == today {
            if let Ok(st) = read_usage_state() {
                if st.last_merge_date == date {
                    // No-op result to reduce R2 operations; caller typically ignores return and reloads separately
                    return Ok(DailyLedger {
                        date: date.into(),
                        class_a: Default::default(),
                        class_b: Default::default(),
                        ingress_bytes: 0,
                        egress_bytes: 0,
                        storage_bytes: 0,
                        peak_storage_bytes: 0,
                        deleted_storage_bytes: 0,
                        rev: 0,
                        updated_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
        }
        let p = local_delta_path(date)?;
        let local: UsageDelta = if p.exists() {
            serde_json::from_slice(&fs::read(&p).map_err(ioe)?).unwrap_or(UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
                added_storage_bytes: 0,
                deleted_storage_bytes: 0,
            })
        } else {
            UsageDelta {
                class_a: Default::default(),
                class_b: Default::default(),
                ingress_bytes: 0,
                egress_bytes: 0,
                added_storage_bytes: 0,
                deleted_storage_bytes: 0,
            }
        };

        let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
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
                peak_storage_bytes: 0,
                deleted_storage_bytes: 0,
                rev: 1,
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
            .map_err(|e| SpError {
                kind: ErrorKind::NotRetriable,
                message: format!("serialize empty ledger failed: {e}"),
                retry_after_ms: None,
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?,
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
            peak_storage_bytes: 0,
            deleted_storage_bytes: 0,
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
        // 存储计费：根据本地增量估算存量并更新峰值
        // storage_bytes(t) ≈ storage_bytes(t-1) + added - deleted
        // peak_storage_bytes(t) = max(peak_storage_bytes(t), storage_bytes(t))
        // 注意：初始 storage_bytes 需要已有账本提供基线
        let added = local.added_storage_bytes;
        let deleted = local.deleted_storage_bytes;
        let new_storage = day
            .storage_bytes
            .saturating_add(added)
            .saturating_sub(deleted);
        day.storage_bytes = new_storage;
        if new_storage > day.peak_storage_bytes {
            day.peak_storage_bytes = new_storage;
        }
        day.deleted_storage_bytes = day.deleted_storage_bytes.saturating_add(deleted);
        day.updated_at = chrono::Utc::now().to_rfc3339();
        day.rev += 1;

        // PUT If-Match
        let day_bytes = serde_json::to_vec(&day).map_err(|e| SpError {
            kind: ErrorKind::NotRetriable,
            message: format!("serialize merged ledger failed: {e}"),
            retry_after_ms: None,
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        r2_client::put_object_bytes(&client, &key, day_bytes, etag, false).await?;

        // Clear local
        let _ = fs::remove_file(p);
        // Persist state that we merged this date
        let _ = write_usage_state(date);
        Ok(day)
    }

    pub async fn list_month(prefix: &str) -> SpResult<Vec<DailyLedger>> {
        // Cache month data on disk to minimize R2 GETs.
        // Strategy:
        // - List existing days once using ListObjectsV2; don't probe 1..31 blindly.
        // - For current month: always fetch today's record fresh; earlier days from cache or fetched once and cached.
        // - For past months: serve from cache, fetch-and-cache missing only for days that exist.
        let today = chrono::Utc::now();
        let today_month = today.format("%Y-%m").to_string();
        let today_day: u32 = today.format("%d").to_string().parse().unwrap_or(32);
        let is_current_month = prefix == today_month;

        let bundle = SpBackend::get_decrypted_bundle_if_unlocked()?;
        let client = r2_client::build_client(&bundle.r2).await?;

        // List existing objects for this month
        let list_prefix = format!("{}{}-", ANALYTICS_PREFIX, prefix);
        let resp = client
            .s3
            .list_objects_v2()
            .bucket(&client.bucket)
            .prefix(&list_prefix)
            .max_keys(64)
            .send()
            .await
            .map_err(|e| SpError {
                kind: ErrorKind::RetryableNet,
                message: format!("ListObjectsV2 (usage month) failed: {e}"),
                retry_after_ms: Some(500),
                context: None,
                at: chrono::Utc::now().timestamp_millis(),
            })?;

        let mut days_present: Vec<u32> = vec![];
        for o in resp.contents() {
            if let Some(key) = o.key() {
                // Expect analytics/daily/YYYY-MM-DD.json
                if let Some(day_str) = key.strip_prefix(&list_prefix) {
                    if let Some(day_part) = day_str.strip_suffix(".json") {
                        if let Ok(d) = day_part.parse::<u32>() {
                            days_present.push(d);
                        }
                    }
                }
            }
        }
        days_present.sort_unstable();

        let mut out: Vec<DailyLedger> = Vec::new();
        let mut cache = read_month_cache(prefix).unwrap_or_default();
        let mut dirty = false;

        for day in days_present {
            if is_current_month && day == today_day {
                // Always fetch today's record fresh
                let key = format!("{}{}-{:02}.json", ANALYTICS_PREFIX, prefix, day);
                if let Ok((bytes, _)) = r2_client::get_object_bytes(&client, &key).await {
                    if let Ok(v) = serde_json::from_slice::<DailyLedger>(&bytes) {
                        out.push(v);
                    }
                }
                continue;
            }
            if let Some(v) = cache.days.get(&day) {
                out.push(v.clone());
                continue;
            }
            let key = format!("{}{}-{:02}.json", ANALYTICS_PREFIX, prefix, day);
            if let Ok((bytes, _)) = r2_client::get_object_bytes(&client, &key).await {
                if let Ok(v) = serde_json::from_slice::<DailyLedger>(&bytes) {
                    out.push(v.clone());
                    cache.days.insert(day, v);
                    dirty = true;
                }
            }
        }

        if dirty {
            let _ = write_month_cache(prefix, &cache);
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UsageState {
    last_merge_date: String,
}

fn usage_state_path() -> SpResult<PathBuf> {
    Ok(app_dir()?.join("usage_state.json"))
}

fn read_usage_state() -> SpResult<UsageState> {
    let p = usage_state_path()?;
    if !p.exists() {
        return Ok(UsageState::default());
    }
    let bytes = fs::read(p).map_err(ioe)?;
    let st: UsageState = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(st)
}

fn write_usage_state(date: &str) -> SpResult<()> {
    let p = usage_state_path()?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(ioe)?;
    }
    let st = UsageState {
        last_merge_date: date.into(),
    };
    let bytes = serde_json::to_vec(&st).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize usage state: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(p, bytes).map_err(ioe)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MonthCache {
    // map day -> ledger
    days: std::collections::BTreeMap<u32, DailyLedger>,
}

fn month_cache_path(prefix: &str) -> SpResult<PathBuf> {
    Ok(app_dir()?.join(format!("usage_cache_{}.json", prefix)))
}

fn read_month_cache(prefix: &str) -> SpResult<MonthCache> {
    let p = month_cache_path(prefix)?;
    println!("read_month_cache: {}", p.display());
    if !p.exists() {
        return Ok(MonthCache::default());
    }
    let bytes = fs::read(p).map_err(ioe)?;
    let st: MonthCache = serde_json::from_slice(&bytes).unwrap_or_default();
    Ok(st)
}

fn write_month_cache(prefix: &str, cache: &MonthCache) -> SpResult<()> {
    let p = month_cache_path(prefix)?;
    println!("write_month_cache: {}", p.display());
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(ioe)?;
    }
    let bytes = serde_json::to_vec(cache).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize month cache: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(p, bytes).map_err(ioe)?;
    Ok(())
}
