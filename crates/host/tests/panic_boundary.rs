//! Commands run inside a panic boundary on a working copy of the state (research R9).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

mod common;

use common::TempDir;
use la_host::error;
use la_host::notices::NoticeContent;
use la_host::{Changes, Command, Outcome, Shell, ShellConfig, Tick};

fn shell(dir: &TempDir) -> Shell {
    Shell::new(ShellConfig::for_tests(dir.path()))
}

fn error_codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .map(|n| match &n.content {
            NoticeContent::Error { code } => code.clone(),
            NoticeContent::Info { key, .. } => (*key).to_owned(),
        })
        .collect()
}

#[test]
fn a_panicking_command_leaves_the_state_unchanged_and_reports_one_notice() {
    let dir = TempDir::new("panic");
    let mut shell = shell(&dir);
    let version = shell.state().version;
    let outcome = shell.apply(Command::TestPanic);
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert_eq!(
        shell.state().version,
        version,
        "the working copy is dropped"
    );
    assert_eq!(error_codes(&shell), [error::INTERNAL]);
    // The shell keeps working afterwards.
    let outcome = shell.apply(Command::DismissAllNotices);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(shell.state().notices.is_empty());
}

#[test]
fn a_typed_error_adds_a_notice_and_changes_nothing_else() {
    let dir = TempDir::new("typed");
    let mut shell = shell(&dir);
    let version = shell.state().version;
    let outcome = shell.apply(Command::TestError("session.damaged"));
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert_eq!(shell.state().version, version);
    assert_eq!(error_codes(&shell), ["session.damaged"]);
    // The same failure again is grouped into one notice with a count.
    shell.apply(Command::TestError("session.damaged"));
    assert_eq!(shell.state().notices.len(), 1);
    assert_eq!(shell.state().notices.iter().next().unwrap().count, 2);
}

#[test]
fn polling_with_nothing_to_do_reports_no_changes_and_asks_for_slow_ticks() {
    let dir = TempDir::new("idle");
    let mut shell = shell(&dir);
    assert_eq!(shell.wants_ticks(), Tick::Slow);
    let changes = shell.poll(std::time::Instant::now());
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(changes, Changes::NONE);
}

#[test]
fn a_command_that_is_not_valid_now_is_ignored_with_the_invalid_command_code() {
    let dir = TempDir::new("ignored");
    let mut shell = shell(&dir);
    let version = shell.state().version;
    // Stopping a capture that does not exist.
    let outcome = shell.apply(Command::StopCapture);
    assert_eq!(outcome, Outcome::Ignored(error::INVALID_COMMAND));
    assert_eq!(shell.state().version, version);
}

#[test]
fn the_text_helpers_follow_the_language() {
    let dir = TempDir::new("text");
    let mut shell = shell(&dir);
    let key = la_i18n::Key::from_static("app-title");
    assert_eq!(shell.text(&key, &la_i18n::Args::new()), "Logic Analyzer");
    shell.apply(Command::TestError("session.damaged"));
    let notice = shell.state().notices.iter().next().unwrap().clone();
    let english = shell.notice_text(&notice);
    assert!(!english.is_empty());
}
