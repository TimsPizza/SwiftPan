use super::super::*;

#[test]
fn interrupted_active_downloads_restart_paused() {
    for lifecycle in [
        TransferLifecycle::Queued,
        TransferLifecycle::Running,
        TransferLifecycle::Paused,
    ] {
        assert_eq!(
            lifecycle_after_restart(&lifecycle),
            TransferLifecycle::Paused,
            "unexpected recovery state for {lifecycle:?}"
        );
    }
}

#[test]
fn interrupted_cancellation_finishes_as_cancelled_on_restart() {
    assert_eq!(
        lifecycle_after_restart(&TransferLifecycle::Cancelling),
        TransferLifecycle::Cancelled
    );
}

#[test]
fn terminal_download_states_remain_terminal_on_restart() {
    for lifecycle in [
        TransferLifecycle::Completed,
        TransferLifecycle::Failed,
        TransferLifecycle::Cancelled,
    ] {
        assert_eq!(
            lifecycle_after_restart(&lifecycle),
            lifecycle,
            "terminal state changed during recovery"
        );
    }
}

#[test]
fn only_retryable_network_failures_keep_partial_download_artifacts() {
    assert!(should_keep_failed_artifacts(Some(&ErrorKind::RetryableNet)));

    for reason in [
        ErrorKind::Cancelled,
        ErrorKind::RetryableAuth,
        ErrorKind::NotRetriable,
        ErrorKind::SourceChanged,
        ErrorKind::DiskFull,
        ErrorKind::TaskExists,
        ErrorKind::NotImplemented,
    ] {
        assert!(
            !should_keep_failed_artifacts(Some(&reason)),
            "unexpectedly retained artifacts for {}",
            reason.as_str()
        );
    }
    assert!(!should_keep_failed_artifacts(None));
}

#[test]
fn failure_reason_is_exposed_only_for_failed_transfers() {
    let error = SpError {
        kind: ErrorKind::RetryableNet,
        message: "connection reset".into(),
        retry_after_ms: Some(500),
        context: None,
        at: 0,
    };

    assert_eq!(
        last_fail_reason_for(TransferLifecycle::Failed, Some(&error))
            .as_ref()
            .map(ErrorKind::as_str),
        Some("retryable_net")
    );
    assert!(last_fail_reason_for(TransferLifecycle::Running, Some(&error)).is_none());
    assert!(last_fail_reason_for(TransferLifecycle::Failed, None).is_none());
}
