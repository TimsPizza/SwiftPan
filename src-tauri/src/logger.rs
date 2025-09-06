use crate::types::*;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LEVEL: OnceLock<Mutex<LogLevel>> = OnceLock::new();
static LOG_CACHE: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
static LOG_PRIMED: OnceLock<Mutex<bool>> = OnceLock::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024; // 4MB
const MAX_CACHE_LINES: usize = 4000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    fn from_str(s: &str) -> LogLevel {
        match s.to_lowercase().as_str() {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

pub fn init(_app: AppHandle) -> SpResult<()> {
    let base = crate::sp_backend::vault_dir()?;
    let p = base.join("swiftpan.log");
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    LOG_PATH.set(p.clone()).ok();
    LOG_LEVEL.set(Mutex::new(LogLevel::Info)).ok();
    LOG_CACHE
        .set(Mutex::new(VecDeque::with_capacity(MAX_CACHE_LINES + 128)))
        .ok();
    LOG_PRIMED.set(Mutex::new(false)).ok();
    APP_HANDLE.set(_app).ok();
    // touch file
    let _ = OpenOptions::new().create(true).append(true).open(&p);
    Ok(())
}

fn log_path() -> SpResult<PathBuf> {
    LOG_PATH.get().cloned().ok_or_else(|| SpError {
        kind: ErrorKind::NotRetriable,
        message: "logger not initialized".into(),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

fn should_log(level: LogLevel) -> bool {
    if let Some(lock) = LOG_LEVEL.get() {
        let cur = lock.lock().unwrap_or_else(|p| p.into_inner());
        level >= *cur
    } else {
        true
    }
}

fn ts() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn push_cache(line: String) {
    if let Some(cache) = LOG_CACHE.get() {
        let mut q = cache.lock().unwrap_or_else(|p| p.into_inner());
        q.push_back(line);
        while q.len() > MAX_CACHE_LINES {
            q.pop_front();
        }
    }
}

fn clamp_file_size(p: &PathBuf) -> std::io::Result<()> {
    let meta = match fs::metadata(p) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };
    if meta.len() <= MAX_LOG_BYTES {
        return Ok(());
    }
    let mut f = OpenOptions::new().read(true).write(true).open(p)?;
    // Keep last half to preserve recent logs
    let keep: u64 = MAX_LOG_BYTES / 2;
    let len = meta.len();
    let start = len.saturating_sub(keep);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(keep as usize);
    f.read_to_end(&mut buf)?;
    f.set_len(0)?;
    f.seek(SeekFrom::Start(0))?;
    f.write_all(&buf)?;
    Ok(())
}

fn append_file(line: &str) {
    if let Ok(p) = log_path() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
            // opportunistically clamp
            let _ = clamp_file_size(&p);
        }
    }
}

pub fn log(level: LogLevel, target: &str, msg: &str) {
    println!("log: {} {} {}", level.as_str(), target, msg);
    if !should_log(level) {
        return;
    }
    let line = format!("{} [{}] {}: {}", ts(), level.as_str(), target, msg);
    push_cache(line.clone());
    append_file(&line);
    // Emit log event asynchronously to avoid potential blocking on Android
    if let Some(app) = APP_HANDLE.get() {
        let app_handle = app.clone();
        let payload = serde_json::json!({
            "ts": ts(),
            "level": level.as_str(),
            "target": target,
            "message": msg,
            "line": line,
        });
        tauri::async_runtime::spawn(async move {
            use tauri::Emitter;
            let _ = app_handle.emit("sp://log_event", payload);
        });
    }
}

pub fn trace(target: &str, msg: &str) {
    log(LogLevel::Trace, target, msg);
}
pub fn debug(target: &str, msg: &str) {
    log(LogLevel::Debug, target, msg);
}
pub fn info(target: &str, msg: &str) {
    log(LogLevel::Info, target, msg);
}
pub fn warn(target: &str, msg: &str) {
    log(LogLevel::Warn, target, msg);
}
pub fn error(target: &str, msg: &str) {
    log(LogLevel::Error, target, msg);
}

fn prime_cache_from_file() {
    if let (Some(cache), Some(primed)) = (LOG_CACHE.get(), LOG_PRIMED.get()) {
        let mut flag = primed.lock().unwrap_or_else(|p| p.into_inner());
        if *flag {
            return;
        }
        if let Ok(p) = log_path() {
            if let Ok(mut f) = OpenOptions::new().read(true).open(&p) {
                let mut s = String::new();
                if f.read_to_string(&mut s).is_ok() {
                    let mut q = cache.lock().unwrap_or_else(|p| p.into_inner());
                    for line in s.lines() {
                        q.push_back(line.to_string());
                        while q.len() > MAX_CACHE_LINES {
                            q.pop_front();
                        }
                    }
                }
            }
        }
        *flag = true;
    }
}

#[tauri::command]
pub async fn log_tail(lines: Option<u32>) -> SpResult<String> {
    prime_cache_from_file();
    if let Some(cache) = LOG_CACHE.get() {
        let q = cache.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(n) = lines {
            let n = n as usize;
            let len = q.len();
            let start = len.saturating_sub(n);
            let slice: Vec<String> = q.iter().skip(start).cloned().collect();
            return Ok(slice.join("\n"));
        } else {
            return Ok(q.iter().cloned().collect::<Vec<_>>().join("\n"));
        }
    }
    Ok(String::new())
}

#[tauri::command]
pub async fn log_clear() -> SpResult<()> {
    if let Some(cache) = LOG_CACHE.get() {
        cache.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }
    if let Ok(p) = log_path() {
        let _ = fs::write(&p, b"");
    }
    Ok(())
}

#[tauri::command]
pub async fn log_set_level(level: String) -> SpResult<()> {
    if let Some(lock) = LOG_LEVEL.get() {
        *lock.lock().unwrap_or_else(|p| p.into_inner()) = LogLevel::from_str(&level);
    }
    Ok(())
}

#[tauri::command]
pub async fn log_get_status() -> SpResult<serde_json::Value> {
    let level = if let Some(lock) = LOG_LEVEL.get() {
        let cur = lock.lock().unwrap_or_else(|p| p.into_inner());
        cur.as_str().to_string()
    } else {
        LogLevel::Info.as_str().to_string()
    };
    let cache_len = LOG_CACHE
        .get()
        .map(|c| {
            let q = c.lock().unwrap_or_else(|p| p.into_inner());
            q.len() as u32
        })
        .unwrap_or(0);
    let (file_path, file_size_bytes) = match log_path() {
        Ok(p) => {
            let sz = fs::metadata(&p).ok().map(|m| m.len()).unwrap_or(0);
            (p.to_string_lossy().to_string(), sz)
        }
        Err(_) => (String::new(), 0),
    };
    Ok(serde_json::json!({
        "level": level,
        "cache_lines": cache_len,
        "file_path": file_path,
        "file_size_bytes": file_size_bytes,
    }))
}
