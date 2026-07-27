//! Stable Tauri command facade.
//!
//! Commands are implemented in feature-oriented child modules and re-exported
//! here so existing `crate::bridge::*` registrations and internal callers keep
//! their paths. This facade must not accumulate feature implementation logic.

mod android_fs;
mod android_uploads;
mod background;
mod credentials;
mod downloads;
mod objects;
mod sharing;
mod thumbnails;
mod transfers;
mod ui;
mod uploads;
mod usage;

pub use android_fs::*;
pub use android_uploads::*;
pub use background::*;
pub use credentials::*;
pub use downloads::*;
pub use objects::*;
pub use sharing::*;
pub use thumbnails::*;
pub use transfers::*;
pub use ui::*;
pub use uploads::*;
pub use usage::*;
