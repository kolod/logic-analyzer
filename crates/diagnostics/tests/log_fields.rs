//! Structured fields of log entries (spec FR-015, contracts/log-entry-format.md).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_diagnostics::{LogConfig, build_logging};
use serde_json::Value;

fn is_rfc3339_utc(s: &str) -> bool {
    // YYYY-MM-DDTHH:MM:SS[.fraction]Z
    let b = s.as_bytes();
    s.len() >= 20
        && s.ends_with('Z')
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && s.chars().take(4).all(|c| c.is_ascii_digit())
}

#[test]
fn entry_has_timestamp_level_target_message_and_fields() {
    let dir = common::temp_dir("fields");
    let (_guard, dispatch) = build_logging(LogConfig {
        directory: dir.clone(),
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::warn!(target: "decoder.i2c", answer = 42, name = "sda", "hello");
    });

    let line = common::all_lines(&dir)
        .into_iter()
        .find(|l| l.contains("hello"))
        .expect("entry written");
    let v: Value = serde_json::from_str(&line).unwrap();

    assert!(is_rfc3339_utc(v["timestamp"].as_str().unwrap()), "{v}");
    assert_eq!(v["level"], "WARN");
    assert_eq!(v["target"], "decoder.i2c");
    assert_eq!(v["fields"]["message"], "hello");
    assert_eq!(v["fields"]["answer"], 42);
    assert_eq!(v["fields"]["name"], "sda");
}

#[test]
fn events_below_the_configured_level_are_not_written() {
    let dir = common::temp_dir("fields-level");
    let (_guard, dispatch) = build_logging(LogConfig {
        directory: dir.clone(),
        level: tracing::Level::WARN,
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("quiet info");
        tracing::error!("loud error");
    });
    let all = common::all_lines(&dir).join("\n");
    assert!(!all.contains("quiet info"));
    assert!(all.contains("loud error"));
}

#[test]
fn each_entry_is_one_line() {
    let dir = common::temp_dir("fields-oneline");
    let (_guard, dispatch) = build_logging(LogConfig {
        directory: dir.clone(),
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("multi\nline\nmessage");
    });
    for line in common::all_lines(&dir) {
        let _: Value = serde_json::from_str(&line).unwrap();
    }
}
