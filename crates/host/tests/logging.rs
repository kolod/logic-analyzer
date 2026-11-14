//! What the log contains and what it must never contain (spec FR-031, constitution Principle VI).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell, read_logs};
use la_diagnostics::{LogConfig, build_logging};
use la_host::{Command, Shell, ShellConfig};

#[test]
fn the_log_has_commands_and_codes_but_never_user_text_or_signal_data() {
    let log_dir = TempDir::new("log-content");
    let (guard, dispatch) = build_logging(LogConfig {
        directory: log_dir.path().to_path_buf(),
        level: tracing::Level::DEBUG,
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        let dir = TempDir::new("log-shell");
        let mut shell = finished_shell(&dir, 0b11, 5_000, 6);
        shell.apply(Command::RenameChannel {
            channel: 0,
            name: "MARKER-USER-TEXT-4711".into(),
        });
        shell.apply(Command::TestError("device.disconnected"));
        shell.apply(Command::FitAll);
    });
    drop(guard);
    let logs = read_logs(log_dir.path());
    // Commands, lifecycle and the failure with its code and technical detail.
    for expected in [
        "start_capture",
        "rename_channel",
        "fit_all",
        "device.disconnected",
        "provoked failure",
    ] {
        assert!(logs.contains(expected), "the log lacks {expected}");
    }
    // User text and signal data never appear.
    assert!(
        !logs.contains("MARKER-USER-TEXT-4711"),
        "a channel name was logged"
    );
}

#[test]
fn the_log_location_is_the_configured_one() {
    let dir = TempDir::new("log-location");
    let mut config = ShellConfig::for_tests(dir.path());
    config.log_location = Some(dir.path().join("logs"));
    let shell = Shell::new(config);
    assert_eq!(
        shell.log_location(),
        Some(dir.path().join("logs").as_path())
    );
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(shell.log_location(), None);
}
