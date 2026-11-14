//! Logging must never prevent the application from running (spec FR-017, SC-007).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_diagnostics::{LogConfig, LogStatus, build_logging};

#[test]
fn unusable_directory_falls_back_to_stderr_without_panicking() {
    let base = common::temp_dir("fallback");
    // A regular file where a directory is needed: creating `<file>/logs` must fail
    // on every operating system.
    let file = base.join("not-a-directory");
    std::fs::write(&file, b"x").unwrap();
    let config = LogConfig {
        directory: file.join("logs"),
        ..LogConfig::default()
    };

    let (guard, dispatch) = build_logging(config);

    assert!(
        matches!(guard.status(), LogStatus::Fallback { .. }),
        "expected fallback, got {:?}",
        guard.status()
    );
    // Logging after the fallback must still work and must not fail.
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("still running");
        tracing::error!(code = "x.y", "an error after fallback");
    });
}

#[test]
fn usable_directory_reports_file_status() {
    let dir = common::temp_dir("fallback-ok");
    let (guard, _dispatch) = build_logging(LogConfig {
        directory: dir.clone(),
        ..LogConfig::default()
    });
    assert_eq!(guard.status(), LogStatus::File { directory: dir });
}

#[test]
fn fallback_reason_is_not_empty() {
    let base = common::temp_dir("fallback-reason");
    let file = base.join("f");
    std::fs::write(&file, b"x").unwrap();
    let (guard, _d) = build_logging(LogConfig {
        directory: file.join("sub"),
        ..LogConfig::default()
    });
    match guard.status() {
        LogStatus::Fallback { reason } => assert!(!reason.is_empty()),
        other => panic!("expected fallback, got {other:?}"),
    }
}
