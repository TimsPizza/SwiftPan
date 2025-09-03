use crate::types::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TaskKind { Upload, Download }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskSpec { pub kind: TaskKind, pub id: String, pub priority: i32 }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundStats {
  pub active_tasks: u32,
  pub moving_avg_bps: u64,
  pub cpu_hint: f32,
  pub io_hint: f32,
}

pub struct BackgroundManager;

impl BackgroundManager {
  pub fn submit(_task: TaskSpec) -> SpResult<()> { Err(err_not_implemented("bg.submit")) }
  pub fn set_limits(_limits: ConcurrencyLimits, _rate: RateLimitConfig) -> SpResult<()> { Err(err_not_implemented("bg.set_limits")) }
  pub fn global_pause() -> SpResult<()> { Err(err_not_implemented("bg.global_pause")) }
  pub fn global_resume() -> SpResult<()> { Err(err_not_implemented("bg.global_resume")) }
  pub fn clear_completed() -> SpResult<()> { Err(err_not_implemented("bg.clear_completed")) }
  pub fn stats() -> SpResult<BackgroundStats> { Err(err_not_implemented("bg.stats")) }
}

