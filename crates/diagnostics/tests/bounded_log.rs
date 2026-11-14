//! Tests for the size-bounded log writer (spec US5, FR-015, FR-016, SC-006).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_diagnostics::bounded_writer::BoundedLogWriter;
use serde_json::Value;

const MAX_TOTAL: u64 = 16 * 1024;
const SEGMENTS: usize = 4;

fn record(i: usize, pad: usize) -> String {
    format!(
        "{{\"timestamp\":\"2026-01-01T00:00:00.000000Z\",\"level\":\"INFO\",\"target\":\"t\",\"fields\":{{\"message\":\"m\",\"id\":{i},\"pad\":\"{}\"}}}}\n",
        "x".repeat(pad)
    )
}

/// Deterministic xorshift generator: no extra dependency.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn id_of(line: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(line).ok()?;
    v["fields"]["id"].as_u64()
}

#[test]
fn total_size_never_exceeds_limit_and_lines_are_never_split() {
    let dir = common::temp_dir("bounded-size");
    let mut w = BoundedLogWriter::open(&dir, "la-0000000000001-1-0", SEGMENTS, MAX_TOTAL).unwrap();
    let mut state = 0x1234_5678_9abc_def1_u64;
    let mut written_bytes = 0_u64;
    let mut i = 0_usize;
    while written_bytes < MAX_TOTAL * 12 {
        let pad = usize::try_from(next(&mut state) % 300).unwrap();
        let line = record(i, pad);
        written_bytes += u64::try_from(line.len()).unwrap();
        w.write_record(line.as_bytes()).unwrap();
        assert!(
            common::total_size(&dir) <= MAX_TOTAL,
            "size bound exceeded after {i} records"
        );
        i += 1;
    }
    // Every retained line is complete, valid JSON (nothing was split).
    for line in common::all_lines(&dir) {
        let v: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad line {line}: {e}"));
        for key in ["timestamp", "level", "target", "fields"] {
            assert!(v.get(key).is_some(), "missing {key} in {line}");
        }
    }
}

#[test]
fn newest_entries_are_retained_and_oldest_dropped_first() {
    let dir = common::temp_dir("bounded-newest");
    let mut w = BoundedLogWriter::open(&dir, "la-0000000000001-1-0", SEGMENTS, MAX_TOTAL).unwrap();
    let total = 2_000_usize;
    for i in 0..total {
        w.write_record(record(i, 40).as_bytes()).unwrap();
    }
    let mut ids: Vec<u64> = common::all_lines(&dir)
        .iter()
        .filter_map(|l| id_of(l))
        .collect();
    ids.sort_unstable();
    assert!(!ids.is_empty());
    assert_eq!(
        *ids.last().unwrap(),
        u64::try_from(total - 1).unwrap(),
        "newest present"
    );
    assert!(ids[0] > 0, "the oldest entries were discarded");
    // Retained ids form one contiguous run ending at the newest entry.
    for pair in ids.windows(2) {
        assert_eq!(pair[1], pair[0] + 1, "gap in retained entries");
    }
}

#[test]
fn every_segment_starts_with_the_log_started_header() {
    let dir = common::temp_dir("bounded-header");
    let mut w = BoundedLogWriter::open(&dir, "la-0000000000001-1-0", SEGMENTS, MAX_TOTAL).unwrap();
    for i in 0..1_000 {
        w.write_record(record(i, 60).as_bytes()).unwrap();
    }
    let files = common::log_files(&dir);
    assert!(files.len() > 1, "rotation must have used several segments");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(first["fields"]["message"], "log started", "{f:?}");
        assert_eq!(first["fields"]["format_version"], 1, "{f:?}");
    }
}

#[test]
fn oversized_record_is_replaced_by_a_notice_and_bound_holds() {
    let dir = common::temp_dir("bounded-oversize");
    let mut w = BoundedLogWriter::open(&dir, "la-0000000000001-1-0", SEGMENTS, MAX_TOTAL).unwrap();
    let huge = record(7, 100_000);
    w.write_record(huge.as_bytes()).unwrap();
    w.write_record(record(8, 10).as_bytes()).unwrap();
    assert!(common::total_size(&dir) <= MAX_TOTAL);
    let lines = common::all_lines(&dir);
    assert!(
        lines.iter().any(|l| l.contains("log entry too large")),
        "a dropped-entry notice is expected"
    );
    assert!(lines.iter().filter_map(|l| id_of(l)).any(|id| id == 8));
    assert!(!lines.iter().any(|l| l.contains(&"x".repeat(1000))));
}
