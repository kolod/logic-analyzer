//! Every stable error code becomes one localized message and one log entry (spec SC-006).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, all_error_codes, read_logs};
use la_diagnostics::{LogConfig, build_logging};
use la_host::notices::NoticeContent;
use la_host::{Command, Outcome, Shell, ShellConfig};
use la_i18n::Language;

#[test]
fn the_scan_finds_the_codes_of_the_product() {
    let codes = all_error_codes();
    assert!(codes.len() >= 51, "only {} codes", codes.len());
    for expected in [
        "session.damaged",
        "device.disconnected",
        "storage.memory_limit",
        "host.internal",
    ] {
        assert!(codes.iter().any(|c| c == expected), "{expected}");
    }
}

#[test]
fn every_error_code_gives_one_localized_notice_and_a_log_entry() {
    let codes = all_error_codes();
    let log_dir = TempDir::new("sweep-log");
    let (guard, dispatch) = build_logging(LogConfig {
        directory: log_dir.path().to_path_buf(),
        level: tracing::Level::INFO,
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        for code in &codes {
            let dir = TempDir::new("sweep");
            let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
            let code: &'static str = Box::leak(code.clone().into_boxed_str());
            let outcome = shell.apply(Command::TestError(code));
            assert!(matches!(outcome, Outcome::Failed(_)), "{code}: {outcome:?}");
            assert_eq!(shell.state().notices.len(), 1, "{code}");
            let notice = shell.state().notices.iter().next().unwrap().clone();
            assert_eq!(
                notice.content,
                NoticeContent::Error {
                    code: code.to_owned()
                }
            );
            for language in Language::ALL {
                shell.localizer().set_language(language);
                let text = shell.notice_text(&notice);
                assert!(!text.trim().is_empty(), "{code} {language:?}");
                assert!(
                    !text.contains(code),
                    "{code} {language:?}: the code is shown: {text}"
                );
                assert!(
                    !text.starts_with("error-"),
                    "{code} {language:?}: a key is shown"
                );
                assert!(
                    !text.contains("provoked failure"),
                    "{code} {language:?}: the technical detail is shown"
                );
            }
        }
    });
    drop(guard);
    let logs = read_logs(log_dir.path());
    for code in &codes {
        assert!(logs.contains(code.as_str()), "{code} was not logged");
    }
    assert!(
        logs.contains("provoked failure"),
        "the technical detail goes to the log"
    );
}
