use crate::types::*;
pub mod http_instrument;
use crate::{r2_client, sp_backend::SpBackend};
use chrono::NaiveDate;
// use directories::ProjectDirs;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tokio::sync::Mutex as AsyncMutex;

pub struct UsageSync;

impl UsageSync {
    /// Flush all locally accumulated usage deltas to remote ledgers.
    ///
    /// Scans `usage_deltas/` for `YYYY-MM-DD.json` files and merges each
    /// into the corresponding daily ledger, clearing the local file after.
    /// Returns number of days merged.
    pub async fn sync_all_local_deltas() -> SpResult<usize> {
        let dir = app_dir()?.join("usage_deltas");
        if !dir.exists() {
            return Ok(0);
        }
        let mut dates: Vec<String> = Vec::new();
        let rd = std::fs::read_dir(&dir).map_err(ioe)?;
        for ent in rd {
            let ent = ent.map_err(ioe)?;
            let path = ent.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d").is_ok() {
                    dates.push(stem.to_string());
                }
            }
        }
        dates.sort_unstable();
        if dates.is_empty() {
            return Ok(0);
        }
        crate::logger::info(
            "usage",
            &format!("startup sync: pending days = {:?}", dates),
        );
        let mut merged = 0usize;
        for d in dates {
            match Self::merge_and_write_day(&d).await {
                Ok(_) => {
                    merged += 1;
                }
                Err(e) => {
                    crate::logger::error(
                        "usage",
                        &format!("startup sync failed for {}: {}", d, e.message),
                    );
                }
            }
        }
        Ok(merged)
    }
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
        // Serialize merges to avoid concurrent duplicate writes
        static MERGE_LOCK: Lazy<AsyncMutex<()>> = Lazy::new(|| AsyncMutex::new(()));
        let _guard = MERGE_LOCK.lock().await;
        crate::logger::info("usage", &format!("merge_and_write_day date:: {}", date));
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

        let (mut day, etag) = match r2_client::get_object_bytes_opt(&client, &key).await? {
            Some((bytes, etag)) => {
                let parsed: DailyLedger = serde_json::from_slice(&bytes).unwrap_or(DailyLedger {
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
                (parsed, etag)
            }
            None => {
                let seeded = initial_ledger_with_baseline(&client, date).await?;
                let seed_bytes = serde_json::to_vec(&seeded).map_err(|e| SpError {
                    kind: ErrorKind::NotRetriable,
                    message: format!("serialize empty ledger failed: {e}"),
                    retry_after_ms: None,
                    context: None,
                    at: chrono::Utc::now().timestamp_millis(),
                })?;
                r2_client::put_object_bytes(&client, &key, seed_bytes, None, true).await?;
                (seeded, None)
            }
        };
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

        // List existing objects for this month via OpenDAL
        let list_prefix = format!("{}{}-", ANALYTICS_PREFIX, prefix);
        let mut days_present: Vec<u32> = vec![];
        let l = client.op.list(&list_prefix).await.map_err(|e| SpError {
            kind: ErrorKind::RetryableNet,
            message: format!("list (usage month): {e}"),
            retry_after_ms: Some(500),
            context: None,
            at: chrono::Utc::now().timestamp_millis(),
        })?;
        for e in l.into_iter() {
            let key = e.path().to_string();
            if let Some(day_str) = key.strip_prefix(&list_prefix) {
                if let Some(day_part) = day_str.strip_suffix(".json") {
                    if let Ok(d) = day_part.parse::<u32>() {
                        days_present.push(d);
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

    pub async fn month_cost(prefix: &str) -> SpResult<serde_json::Value> {
        // Read month (cached) and compute cost per R2 logic
        let days = Self::list_month(prefix).await?;
        // Storage: sum daily peak_storage_bytes (GB) / 30 → avg GB-month; ceil to integer; subtract free 10 GB-month (floor at 0)
        let sum_peak_gb: f64 = days
            .iter()
            .map(|d| (d.peak_storage_bytes as f64) / (1024.0 * 1024.0 * 1024.0))
            .sum();
        let avg_gb_month = (sum_peak_gb / 30.0).ceil();
        let free_gb = 10.0_f64;
        let billable_gb = (avg_gb_month - free_gb).max(0.0);
        let storage_cost = billable_gb * 0.0; // If Standard cost per GB-month is desired later, set here; currently only usage breakdown

        // Ops: sum class_a/b; apply free tiers 1m/10m, then ceil to next million
        let total_a: u64 = days
            .iter()
            .map(|d| d.class_a.values().copied().sum::<u64>())
            .sum();
        let total_b: u64 = days
            .iter()
            .map(|d| d.class_b.values().copied().sum::<u64>())
            .sum();
        let free_a: u64 = 1_000_000;
        let free_b: u64 = 10_000_000;
        let over_a = total_a.saturating_sub(free_a);
        let over_b = total_b.saturating_sub(free_b);
        let units_a_m = if over_a == 0 {
            0
        } else {
            (over_a + 1_000_000 - 1) / 1_000_000
        } as u64;
        let units_b_m = if over_b == 0 {
            0
        } else {
            (over_b + 1_000_000 - 1) / 1_000_000
        } as u64;
        let cost_a = (units_a_m as f64) * 4.50;
        let cost_b = (units_b_m as f64) * 0.36;

        let report = serde_json::json!({
            "month": prefix,
            "storage": {
                "sum_peak_gb": sum_peak_gb,
                "avg_gb_month_ceil": avg_gb_month,
                "free_gb_month": free_gb,
                "billable_gb_month": billable_gb,
                "cost_usd": storage_cost,
            },
            "class_a": {
                "total_ops": total_a,
                "free_ops": free_a,
                "billable_millions": units_a_m,
                "unit_price": 4.50,
                "cost_usd": cost_a,
            },
            "class_b": {
                "total_ops": total_b,
                "free_ops": free_b,
                "billable_millions": units_b_m,
                "unit_price": 0.36,
                "cost_usd": cost_b,
            },
            "total_cost_usd": storage_cost + cost_a + cost_b,
        });
        Ok(report)
    }
}

fn app_dir() -> SpResult<PathBuf> {
    // Reuse sp_backend's robust directory resolution (ProjectDirs -> env var -> temp)
    crate::sp_backend::vault_dir()
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

async fn initial_ledger_with_baseline(
    client: &crate::r2_client::R2Client,
    date: &str,
) -> SpResult<DailyLedger> {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("invalid ledger date {date}: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;

    let baseline_storage = if let Some(prev) = latest_ledger_before(client, parsed).await? {
        prev.storage_bytes
    } else {
        compute_bucket_total_storage(client).await?
    };

    Ok(DailyLedger {
        date: date.into(),
        class_a: Default::default(),
        class_b: Default::default(),
        ingress_bytes: 0,
        egress_bytes: 0,
        storage_bytes: baseline_storage,
        peak_storage_bytes: baseline_storage,
        deleted_storage_bytes: 0,
        rev: 1,
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}

async fn latest_ledger_before(
    client: &crate::r2_client::R2Client,
    date: NaiveDate,
) -> SpResult<Option<DailyLedger>> {
    let mut probe = date;
    for _ in 0..62 {
        let Some(prev) = probe.pred_opt() else {
            break;
        };
        probe = prev;
        let key = format!("{}{}.json", ANALYTICS_PREFIX, probe.format("%Y-%m-%d"));
        if let Some((bytes, _)) = r2_client::get_object_bytes_opt(client, &key).await? {
            if let Ok(ledger) = serde_json::from_slice::<DailyLedger>(&bytes) {
                return Ok(Some(ledger));
            }
        }
    }
    Ok(None)
}

async fn compute_bucket_total_storage(client: &crate::r2_client::R2Client) -> SpResult<u64> {
    let mut total: u64 = 0;
    let l = client.op.list("").await.map_err(|e| SpError {
        kind: ErrorKind::RetryableNet,
        message: format!("list (total storage): {e}"),
        retry_after_ms: Some(500),
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    for e in l.into_iter() {
        let path = e.path();
        if path.ends_with('/') {
            continue;
        }
        if let Ok(meta) = client.op.stat(path).await {
            let sz = meta.content_length();
            total = total.saturating_add(sz);
        }
    }
    Ok(total)
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
