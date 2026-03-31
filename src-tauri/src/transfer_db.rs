use crate::types::*;
use once_cell::sync::OnceCell;
use sqlx::{Pool, Row, Sqlite};
use std::future::Future;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool, Migration, MigrationKind};

pub use crate::transfer_fsm::{TransferKind, TransferLifecycle, TransferPhase};

const DB_URL: &str = "sqlite:transfers.sqlite3";

static APP_HANDLE: OnceCell<tauri::AppHandle> = OnceCell::new();

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransferSnapshot {
    pub transfer_id: String,
    pub kind: TransferKind,
    pub key: String,
    pub lifecycle_state: TransferLifecycle,
    pub phase: Option<TransferPhase>,
    pub bytes_total: Option<u64>,
    pub bytes_done: u64,
    pub rate_bps: u64,
    pub last_error: Option<SpError>,
    pub dest_path: Option<String>,
    pub android_tree_uri: Option<String>,
    pub android_relative_path: Option<String>,
    pub temp_path: Option<String>,
    pub expected_etag: Option<String>,
    pub observed_etag: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub fn db_url() -> &'static str {
    DB_URL
}

pub fn migrations() -> Vec<Migration> {
    vec![Migration {
        version: 1,
        description: "create_transfer_snapshots",
        sql: r#"
CREATE TABLE IF NOT EXISTS transfer_snapshots (
  transfer_id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL,
  key TEXT NOT NULL,
  lifecycle_state TEXT NOT NULL,
  phase TEXT,
  bytes_total INTEGER,
  bytes_done INTEGER NOT NULL,
  rate_bps INTEGER NOT NULL,
  last_error_json TEXT,
  dest_path TEXT,
  android_tree_uri TEXT,
  android_relative_path TEXT,
  temp_path TEXT,
  expected_etag TEXT,
  observed_etag TEXT,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transfer_snapshots_active
  ON transfer_snapshots(lifecycle_state, updated_at_ms DESC);
        "#,
        kind: MigrationKind::Up,
    }]
}

pub fn init(app: &tauri::AppHandle) -> SpResult<()> {
    let _ = APP_HANDLE.set(app.clone());
    Ok(())
}

pub fn upsert_snapshot(snapshot: &TransferSnapshot) -> SpResult<()> {
    let last_error_json = snapshot
        .last_error
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(json_err)?;
    let bytes_total = snapshot.bytes_total.map(u64_to_i64).transpose()?;
    let bytes_done = u64_to_i64(snapshot.bytes_done)?;
    let rate_bps = u64_to_i64(snapshot.rate_bps)?;
    let phase = snapshot.phase.map(|value| value.as_str().to_string());
    let query = r#"
INSERT INTO transfer_snapshots (
  transfer_id,
  kind,
  key,
  lifecycle_state,
  phase,
  bytes_total,
  bytes_done,
  rate_bps,
  last_error_json,
  dest_path,
  android_tree_uri,
  android_relative_path,
  temp_path,
  expected_etag,
  observed_etag,
  created_at_ms,
  updated_at_ms
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(transfer_id) DO UPDATE SET
  kind = excluded.kind,
  key = excluded.key,
  lifecycle_state = excluded.lifecycle_state,
  phase = excluded.phase,
  bytes_total = excluded.bytes_total,
  bytes_done = excluded.bytes_done,
  rate_bps = excluded.rate_bps,
  last_error_json = excluded.last_error_json,
  dest_path = excluded.dest_path,
  android_tree_uri = excluded.android_tree_uri,
  android_relative_path = excluded.android_relative_path,
  temp_path = excluded.temp_path,
  expected_etag = excluded.expected_etag,
  observed_etag = excluded.observed_etag,
  created_at_ms = excluded.created_at_ms,
  updated_at_ms = excluded.updated_at_ms
"#;
    run_db(async move {
        let pool = load_pool().await?;
        sqlx::query(query)
            .bind(snapshot.transfer_id.clone())
            .bind(snapshot.kind.as_str())
            .bind(snapshot.key.clone())
            .bind(snapshot.lifecycle_state.as_str())
            .bind(phase)
            .bind(bytes_total)
            .bind(bytes_done)
            .bind(rate_bps)
            .bind(last_error_json)
            .bind(snapshot.dest_path.clone())
            .bind(snapshot.android_tree_uri.clone())
            .bind(snapshot.android_relative_path.clone())
            .bind(snapshot.temp_path.clone())
            .bind(snapshot.expected_etag.clone())
            .bind(snapshot.observed_etag.clone())
            .bind(snapshot.created_at_ms)
            .bind(snapshot.updated_at_ms)
            .execute(&pool)
            .await
            .map_err(db_err)?;
        Ok(())
    })
}

pub fn get_snapshot(transfer_id: &str) -> SpResult<Option<TransferSnapshot>> {
    let transfer_id = transfer_id.to_string();
    run_db(async move {
        let pool = load_pool().await?;
        let row = sqlx::query(
            r#"
SELECT
  transfer_id,
  kind,
  key,
  lifecycle_state,
  phase,
  bytes_total,
  bytes_done,
  rate_bps,
  last_error_json,
  dest_path,
  android_tree_uri,
  android_relative_path,
  temp_path,
  expected_etag,
  observed_etag,
  created_at_ms,
  updated_at_ms
FROM transfer_snapshots
WHERE transfer_id = ?
            "#,
        )
        .bind(transfer_id)
        .fetch_optional(&pool)
        .await
        .map_err(db_err)?;
        row.map(row_to_snapshot).transpose()
    })
}

pub fn list_active_snapshots() -> SpResult<Vec<TransferSnapshot>> {
    run_db(async move {
        let pool = load_pool().await?;
        let rows = sqlx::query(
            r#"
SELECT
  transfer_id,
  kind,
  key,
  lifecycle_state,
  phase,
  bytes_total,
  bytes_done,
  rate_bps,
  last_error_json,
  dest_path,
  android_tree_uri,
  android_relative_path,
  temp_path,
  expected_etag,
  observed_etag,
  created_at_ms,
  updated_at_ms
FROM transfer_snapshots
WHERE lifecycle_state NOT IN ('completed', 'failed', 'cancelled')
ORDER BY updated_at_ms DESC
            "#,
        )
        .fetch_all(&pool)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(row_to_snapshot).collect()
    })
}

fn row_to_snapshot(row: sqlx::sqlite::SqliteRow) -> SpResult<TransferSnapshot> {
    let phase: Option<String> = row.try_get("phase").map_err(db_err)?;
    let last_error_json: Option<String> = row.try_get("last_error_json").map_err(db_err)?;
    let last_error = last_error_json
        .as_deref()
        .map(serde_json::from_str::<SpError>)
        .transpose()
        .map_err(json_err)?;
    Ok(TransferSnapshot {
        transfer_id: row.try_get("transfer_id").map_err(db_err)?,
        kind: TransferKind::from_str(&row.try_get::<String, _>("kind").map_err(db_err)?)?,
        key: row.try_get("key").map_err(db_err)?,
        lifecycle_state: TransferLifecycle::from_str(
            &row.try_get::<String, _>("lifecycle_state")
                .map_err(db_err)?,
        )?,
        phase: TransferPhase::from_opt_str(phase)?,
        bytes_total: row
            .try_get::<Option<i64>, _>("bytes_total")
            .map_err(db_err)?
            .map(i64_to_u64)
            .transpose()?,
        bytes_done: i64_to_u64(row.try_get("bytes_done").map_err(db_err)?)?,
        rate_bps: i64_to_u64(row.try_get("rate_bps").map_err(db_err)?)?,
        last_error,
        dest_path: row.try_get("dest_path").map_err(db_err)?,
        android_tree_uri: row.try_get("android_tree_uri").map_err(db_err)?,
        android_relative_path: row.try_get("android_relative_path").map_err(db_err)?,
        temp_path: row.try_get("temp_path").map_err(db_err)?,
        expected_etag: row.try_get("expected_etag").map_err(db_err)?,
        observed_etag: row.try_get("observed_etag").map_err(db_err)?,
        created_at_ms: row.try_get("created_at_ms").map_err(db_err)?,
        updated_at_ms: row.try_get("updated_at_ms").map_err(db_err)?,
    })
}

async fn load_pool() -> SpResult<Pool<Sqlite>> {
    let app = APP_HANDLE.get().ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "transfer db not initialized".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    let instances = app.state::<DbInstances>();
    let guard = instances.0.read().await;
    let pool = guard.get(DB_URL).ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("plugin-sql database not loaded: {DB_URL}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    #[allow(unreachable_patterns)]
    match pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
        _ => Err(err_invalid("transfer db is not sqlite")),
    }
}

fn run_db<F, T>(future: F) -> SpResult<T>
where
    F: Future<Output = SpResult<T>>,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
    } else {
        tauri::async_runtime::block_on(future)
    }
}

fn u64_to_i64(value: u64) -> SpResult<i64> {
    i64::try_from(value).map_err(|_| err_invalid("u64 value overflowed sqlite integer"))
}

fn i64_to_u64(value: i64) -> SpResult<u64> {
    u64::try_from(value).map_err(|_| err_invalid("sqlite integer contained negative value"))
}

fn db_err(err: impl std::fmt::Display) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("transfer db: {err}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}

fn json_err(err: impl std::fmt::Display) -> SpError {
    SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("transfer db json: {err}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    }
}
