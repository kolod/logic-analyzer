//! Whole-process scenarios (spec edge cases, SC-007): the test binary re-executes itself
//! as a child process with the `LA_SCENARIO` environment variable set, so crashes and
//! failures can happen for real without affecting the test run.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use la_diagnostics::{LogConfig, init_logging, install_panic_hook};
use serde_json::Value;

const ENTRIES_BEFORE_ABORT: usize = 50;

/// The child side. In a normal test run `LA_SCENARIO` is not set and this does nothing.
#[test]
fn child() {
    let Ok(scenario) = std::env::var("LA_SCENARIO") else {
        return;
    };
    let dir = std::path::PathBuf::from(std::env::var("LA_DIR").unwrap());
    let _guard = init_logging(LogConfig {
        directory: dir,
        ..LogConfig::default()
    });
    install_panic_hook();
    match scenario.as_str() {
        "panic_on_non_worker_thread" => {
            tracing::info!("written before the panic");
            panic!("fatal on the main test thread");
        }
        "abort_mid_run" => {
            for i in 0..ENTRIES_BEFORE_ABORT {
                tracing::info!(entry = i, "progress");
            }
            std::process::abort();
        }
        "unwritable_directory" => {
            tracing::error!("logging while the log directory is unusable");
            println!("CHILD-ALIVE");
        }
        other => panic!("unknown scenario {other}"),
    }
}

fn run_child(scenario: &str, dir: &Path) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env("LA_SCENARIO", scenario)
        .env("LA_DIR", dir)
        .env_remove("RUST_BACKTRACE")
        .output()
        .unwrap()
}

#[test]
fn panic_on_a_non_worker_thread_is_logged_before_the_process_ends() {
    let dir = common::temp_dir("scenario-panic");
    let out = run_child("panic_on_non_worker_thread", &dir);
    assert!(!out.status.success(), "the child must fail");

    let lines = common::all_lines(&dir);
    let entries: Vec<Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("every line is valid JSON"))
        .collect();
    // Data written before the panic is intact.
    assert!(
        entries
            .iter()
            .any(|e| e["fields"]["message"] == "written before the panic")
    );
    // The panic itself was logged before the process ended.
    let panic_entry = entries
        .iter()
        .find(|e| e["fields"]["panic_message"] == "fatal on the main test thread")
        .expect("panic entry present");
    assert_eq!(panic_entry["level"], "ERROR");
    assert!(
        panic_entry["fields"]["location"]
            .as_str()
            .unwrap()
            .contains("process_scenarios.rs")
    );
}

#[test]
fn abrupt_termination_leaves_earlier_entries_readable() {
    let dir = common::temp_dir("scenario-abort");
    let out = run_child("abort_mid_run", &dir);
    assert!(!out.status.success(), "the child aborts");

    let lines = common::all_lines(&dir);
    assert!(!lines.is_empty());
    // At most the very last line may be damaged; everything before it must be valid.
    let (last, earlier) = lines.split_last().unwrap();
    for line in earlier {
        let _: Value = serde_json::from_str(line).expect("earlier entries are intact");
    }
    let _ = serde_json::from_str::<Value>(last); // may be partial; must not matter
    let progress = lines.iter().filter(|l| l.contains("\"progress\"")).count();
    assert!(
        progress >= ENTRIES_BEFORE_ABORT - 1,
        "entries logged before the abort are present, found {progress}"
    );
}

#[test]
fn unusable_log_directory_does_not_stop_the_process() {
    let base = common::temp_dir("scenario-unwritable");
    let file = base.join("a-file");
    std::fs::write(&file, b"x").unwrap();
    let out = run_child("unwritable_directory", &file.join("logs"));
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("CHILD-ALIVE"), "{stdout}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("logging.unavailable"), "{stderr}");
}
