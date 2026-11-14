//! Per-process log files and retention of old instances (research R6, spec edge cases).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use la_diagnostics::{LogConfig, build_logging};

/// Instance prefix of a log file name: everything before the segment suffix.
fn instances(dir: &Path) -> BTreeSet<String> {
    common::log_files(dir)
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            name.rsplitn(3, '.').nth(2).unwrap().to_owned()
        })
        .collect()
}

#[test]
fn two_instances_in_one_directory_use_distinct_valid_files() {
    let dir = common::temp_dir("instances-two");
    let config = || LogConfig {
        directory: dir.clone(),
        retained_instances: 5,
        ..LogConfig::default()
    };
    let (_g1, d1) = build_logging(config());
    let (_g2, d2) = build_logging(config());
    tracing::dispatcher::with_default(&d1, || tracing::info!("from one"));
    tracing::dispatcher::with_default(&d2, || tracing::info!("from two"));

    assert_eq!(instances(&dir).len(), 2, "each instance has its own files");
    for line in common::all_lines(&dir) {
        let _: serde_json::Value = serde_json::from_str(&line).unwrap();
    }
    let all = common::all_lines(&dir).join("\n");
    assert!(all.contains("from one") && all.contains("from two"));
}

#[test]
fn old_instances_beyond_the_retention_count_are_deleted_at_start() {
    let dir = common::temp_dir("instances-prune");
    // Four prior instances with older (zero-padded unix-millisecond) names.
    for ts in 1..=4_u32 {
        std::fs::write(dir.join(format!("la-{ts:013}-99-0.0.log")), "{}\n").unwrap();
    }
    let (_g, _d) = build_logging(LogConfig {
        directory: dir.clone(),
        retained_instances: 2,
        ..LogConfig::default()
    });
    let remaining = instances(&dir);
    assert_eq!(remaining.len(), 2, "{remaining:?}");
    // The current instance plus the newest prior one survive.
    assert!(remaining.contains("la-0000000000004-99-0"), "{remaining:?}");
    assert!(
        !remaining.contains("la-0000000000001-99-0"),
        "{remaining:?}"
    );
}

#[test]
fn unrelated_files_are_never_deleted() {
    let dir = common::temp_dir("instances-unrelated");
    std::fs::write(dir.join("notes.txt"), "keep me").unwrap();
    std::fs::write(dir.join("other.log"), "keep me too").unwrap();
    let (_g, _d) = build_logging(LogConfig {
        directory: dir.clone(),
        retained_instances: 1,
        ..LogConfig::default()
    });
    assert!(dir.join("notes.txt").exists());
    assert!(dir.join("other.log").exists());
}
