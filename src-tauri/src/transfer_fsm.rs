use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferKind {
    Upload,
    Download,
}

impl TransferKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Download => "download",
        }
    }

    pub fn from_str(value: &str) -> SpResult<Self> {
        match value {
            "upload" => Ok(Self::Upload),
            "download" => Ok(Self::Download),
            _ => Err(err_invalid("invalid transfer kind")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferLifecycle {
    Queued,
    Running,
    Paused,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

impl TransferLifecycle {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Cancelling => "cancelling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(value: &str) -> SpResult<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "cancelling" => Ok(Self::Cancelling),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(err_invalid("invalid transfer lifecycle")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    PreparingSource,
    UploadingRemote,
    FinalizingRemote,
    PreparingTarget,
    DownloadingRemote,
    MaterializingTarget,
    CleaningUp,
}

impl TransferPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreparingSource => "preparing_source",
            Self::UploadingRemote => "uploading_remote",
            Self::FinalizingRemote => "finalizing_remote",
            Self::PreparingTarget => "preparing_target",
            Self::DownloadingRemote => "downloading_remote",
            Self::MaterializingTarget => "materializing_target",
            Self::CleaningUp => "cleaning_up",
        }
    }

    pub fn from_opt_str(value: Option<String>) -> SpResult<Option<Self>> {
        match value.as_deref() {
            None => Ok(None),
            Some("preparing_source") => Ok(Some(Self::PreparingSource)),
            Some("uploading_remote") => Ok(Some(Self::UploadingRemote)),
            Some("finalizing_remote") => Ok(Some(Self::FinalizingRemote)),
            Some("preparing_target") => Ok(Some(Self::PreparingTarget)),
            Some("downloading_remote") => Ok(Some(Self::DownloadingRemote)),
            Some("materializing_target") => Ok(Some(Self::MaterializingTarget)),
            Some("cleaning_up") => Ok(Some(Self::CleaningUp)),
            Some(_) => Err(err_invalid("invalid transfer phase")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferState {
    pub lifecycle: TransferLifecycle,
    pub phase: Option<TransferPhase>,
}

impl TransferState {
    pub fn queued(kind: TransferKind) -> Self {
        let phase = match kind {
            TransferKind::Upload => Some(TransferPhase::PreparingSource),
            TransferKind::Download => Some(TransferPhase::PreparingTarget),
        };
        Self {
            lifecycle: TransferLifecycle::Queued,
            phase,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStateEvent {
    Run(TransferPhase),
    Pause,
    CancelRequest,
    CancelConfirm,
    Complete,
    Fail,
}

fn phase_allowed(kind: TransferKind, phase: TransferPhase) -> bool {
    match kind {
        TransferKind::Upload => matches!(
            phase,
            TransferPhase::PreparingSource
                | TransferPhase::UploadingRemote
                | TransferPhase::FinalizingRemote
                | TransferPhase::CleaningUp
        ),
        TransferKind::Download => matches!(
            phase,
            TransferPhase::PreparingTarget
                | TransferPhase::DownloadingRemote
                | TransferPhase::MaterializingTarget
                | TransferPhase::CleaningUp
        ),
    }
}

fn is_first_phase(kind: TransferKind, phase: TransferPhase) -> bool {
    matches!(
        (kind, phase),
        (TransferKind::Upload, TransferPhase::PreparingSource)
            | (TransferKind::Download, TransferPhase::PreparingTarget)
    )
}

fn phase_transition_allowed(kind: TransferKind, from: TransferPhase, to: TransferPhase) -> bool {
    if from == to {
        return true;
    }
    match kind {
        TransferKind::Upload => matches!(
            (from, to),
            (
                TransferPhase::PreparingSource,
                TransferPhase::UploadingRemote
            ) | (
                TransferPhase::UploadingRemote,
                TransferPhase::FinalizingRemote
            ) | (TransferPhase::FinalizingRemote, TransferPhase::CleaningUp)
        ),
        TransferKind::Download => matches!(
            (from, to),
            (
                TransferPhase::PreparingTarget,
                TransferPhase::DownloadingRemote
            ) | (
                TransferPhase::DownloadingRemote,
                TransferPhase::MaterializingTarget
            ) | (
                TransferPhase::MaterializingTarget,
                TransferPhase::CleaningUp
            )
        ),
    }
}

fn complete_allowed(kind: TransferKind, phase: Option<TransferPhase>) -> bool {
    matches!(
        (kind, phase),
        (TransferKind::Upload, Some(TransferPhase::FinalizingRemote))
            | (TransferKind::Upload, Some(TransferPhase::CleaningUp))
            | (
                TransferKind::Download,
                Some(TransferPhase::MaterializingTarget)
            )
            | (TransferKind::Download, Some(TransferPhase::CleaningUp))
    )
}

pub fn apply_transfer_event(
    kind: TransferKind,
    current: &TransferState,
    event: TransferStateEvent,
) -> SpResult<TransferState> {
    match event {
        TransferStateEvent::Run(next_phase) => {
            if !phase_allowed(kind, next_phase) {
                return Err(err_invalid("phase not allowed for transfer kind"));
            }
            match current.lifecycle {
                TransferLifecycle::Queued => {
                    if !is_first_phase(kind, next_phase) {
                        return Err(err_invalid("queued transfer must start from first phase"));
                    }
                    Ok(TransferState {
                        lifecycle: TransferLifecycle::Running,
                        phase: Some(next_phase),
                    })
                }
                TransferLifecycle::Paused | TransferLifecycle::Running => {
                    let current_phase = current
                        .phase
                        .ok_or_else(|| err_invalid("running transfer missing phase"))?;
                    if !phase_transition_allowed(kind, current_phase, next_phase) {
                        return Err(err_invalid("invalid phase transition"));
                    }
                    Ok(TransferState {
                        lifecycle: TransferLifecycle::Running,
                        phase: Some(next_phase),
                    })
                }
                _ => Err(err_invalid("run transition not allowed from current state")),
            }
        }
        TransferStateEvent::Pause => match current.lifecycle {
            TransferLifecycle::Running => Ok(TransferState {
                lifecycle: TransferLifecycle::Paused,
                phase: current.phase,
            }),
            TransferLifecycle::Paused => Ok(current.clone()),
            _ => Err(err_invalid(
                "pause transition not allowed from current state",
            )),
        },
        TransferStateEvent::CancelRequest => match current.lifecycle {
            TransferLifecycle::Queued | TransferLifecycle::Running | TransferLifecycle::Paused => {
                Ok(TransferState {
                    lifecycle: TransferLifecycle::Cancelling,
                    phase: current.phase,
                })
            }
            TransferLifecycle::Cancelling => Ok(current.clone()),
            _ => Err(err_invalid(
                "cancel transition not allowed from current state",
            )),
        },
        TransferStateEvent::CancelConfirm => match current.lifecycle {
            TransferLifecycle::Cancelling
            | TransferLifecycle::Running
            | TransferLifecycle::Paused => Ok(TransferState {
                lifecycle: TransferLifecycle::Cancelled,
                phase: None,
            }),
            _ => Err(err_invalid("cancel confirm not allowed from current state")),
        },
        TransferStateEvent::Complete => match current.lifecycle {
            TransferLifecycle::Running if complete_allowed(kind, current.phase) => {
                Ok(TransferState {
                    lifecycle: TransferLifecycle::Completed,
                    phase: None,
                })
            }
            _ => Err(err_invalid(
                "complete transition not allowed from current state",
            )),
        },
        TransferStateEvent::Fail => match current.lifecycle {
            TransferLifecycle::Queued
            | TransferLifecycle::Running
            | TransferLifecycle::Paused
            | TransferLifecycle::Cancelling => Ok(TransferState {
                lifecycle: TransferLifecycle::Failed,
                phase: None,
            }),
            _ => Err(err_invalid(
                "fail transition not allowed from current state",
            )),
        },
    }
}
