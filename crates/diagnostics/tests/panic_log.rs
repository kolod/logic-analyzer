//! A worker panic is recorded in the log with its details (spec US4, FR-013).
//!
//! One test per file: the log and the panic hook are process-wide.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_diagnostics::{LogConfig, init_logging, install_panic_hook, spawn_worker};
use serde_json::Value;

#[test]
fn worker_panic_produces_one_error_entry_with_worker_message_and_location() {
    let dir = common::temp_dir("panic-log");
    let _guard = init_logging(LogConfig {
        directory: dir.clone(),
        ..LogConfig::default()
    });
    install_panic_hook();
    // Installing twice must be harmless.
    install_panic_hook();

    let failure = spawn_worker("decoder-uart", || panic!("framing exploded"))
        .join()
        .unwrap_err();
    assert_eq!(failure.worker, "decoder-uart");

    let entries: Vec<Value> = common::all_lines(&dir)
        .iter()
        .filter(|l| l.contains("framing exploded"))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one entry per panic: {entries:?}");
    let entry = &entries[0];
    assert_eq!(entry["level"], "ERROR");
    assert_eq!(entry["fields"]["worker"], "decoder-uart");
    assert_eq!(entry["fields"]["panic_message"], "framing exploded");
    let location = entry["fields"]["location"].as_str().unwrap();
    assert!(location.contains("panic_log.rs"), "{location}");
}
