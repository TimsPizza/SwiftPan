use crate::types::*;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub log_level: String,
    pub max_concurrency: u32,
    pub default_download_dir: Option<String>,
    pub upload_thumbnail: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            max_concurrency: 2,
            default_download_dir: None,
            upload_thumbnail: false,
        }
    }
}

static SETTINGS: OnceCell<Mutex<AppSettings>> = OnceCell::new();

fn settings_path() -> SpResult<PathBuf> {
    let dir = crate::sp_backend::vault_dir()?;
    Ok(dir.join("sp-settings.json"))
}

fn load_from_disk() -> AppSettings {
    let p = match settings_path() {
        Ok(p) => p,
        Err(_) => return AppSettings::default(),
    };
    match fs::read(&p) {
        Ok(bytes) => match serde_json::from_slice::<AppSettings>(&bytes) {
            Ok(s) => s,
            Err(_) => AppSettings::default(),
        },
        Err(_) => AppSettings::default(),
    }
}

fn save_to_disk(s: &AppSettings) -> SpResult<()> {
    let p = settings_path()?;
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_vec_pretty(s).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("serialize settings failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })?;
    fs::write(&p, data).map_err(|e| SpError {
        kind: ErrorKind::NotRetriable,
        message: format!("write settings failed: {e}"),
        retry_after_ms: None,
        context: None,
        at: chrono::Utc::now().timestamp_millis(),
    })
}

pub fn init() -> SpResult<()> {
    let s = load_from_disk();
    let _ = SETTINGS.set(Mutex::new(s));
    if let Some(lock) = SETTINGS.get() {
        let cur = lock.lock().unwrap_or_else(|p| p.into_inner()).clone();
        crate::logger::set_level_str(&cur.log_level);
    }
    Ok(())
}

pub fn get() -> AppSettings {
    if let Some(lock) = SETTINGS.get() {
        return lock.lock().unwrap_or_else(|p| p.into_inner()).clone();
    }
    AppSettings::default()
}

pub fn set(new_settings: AppSettings) -> SpResult<()> {
    if let Some(lock) = SETTINGS.get() {
        {
            let mut g = lock.lock().unwrap_or_else(|p| p.into_inner());
            *g = new_settings.clone();
        }
        save_to_disk(&new_settings)?;
        crate::logger::set_level_str(&new_settings.log_level);
        Ok(())
    } else {
        let _ = SETTINGS.set(Mutex::new(new_settings.clone()));
        save_to_disk(&new_settings)?;
        crate::logger::set_level_str(&new_settings.log_level);
        Ok(())
    }
}

#[tauri::command]
pub async fn settings_get() -> SpResult<AppSettings> {
    Ok(get())
}

#[tauri::command]
pub async fn settings_set(settings: AppSettings) -> SpResult<()> {
    set(settings)
}
