//! Tests for `LogConfig` defaults and validation (spec FR-016).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_diagnostics::{LogConfig, parse_level};
use tracing::Level;

#[test]
fn defaults_match_the_data_model() {
    let c = LogConfig::default();
    assert_eq!(c.max_total_bytes, 8 * 1024 * 1024);
    assert_eq!(c.segments, 4);
    assert_eq!(c.retained_instances, 5);
}

#[test]
fn total_size_is_clamped_to_the_minimum() {
    let c = LogConfig {
        max_total_bytes: 10,
        ..LogConfig::default()
    }
    .normalized();
    assert_eq!(c.max_total_bytes, 64 * 1024);
}

#[test]
fn segment_count_is_clamped_to_2_16() {
    let low = LogConfig {
        segments: 0,
        ..LogConfig::default()
    }
    .normalized();
    let high = LogConfig {
        segments: 200,
        ..LogConfig::default()
    }
    .normalized();
    assert_eq!(low.segments, 2);
    assert_eq!(high.segments, 16);
}

#[test]
fn retained_instances_are_clamped_to_1_50() {
    let low = LogConfig {
        retained_instances: 0,
        ..LogConfig::default()
    }
    .normalized();
    let high = LogConfig {
        retained_instances: 255,
        ..LogConfig::default()
    }
    .normalized();
    assert_eq!(low.retained_instances, 1);
    assert_eq!(high.retained_instances, 50);
}

#[test]
fn level_names_are_parsed_case_insensitively() {
    assert_eq!(parse_level("debug"), Some(Level::DEBUG));
    assert_eq!(parse_level("WARN"), Some(Level::WARN));
    assert_eq!(parse_level(" Info "), Some(Level::INFO));
    assert_eq!(parse_level("nonsense"), None);
    assert_eq!(parse_level(""), None);
}

#[test]
fn valid_values_are_left_unchanged() {
    let c = LogConfig {
        max_total_bytes: 1024 * 1024,
        segments: 8,
        retained_instances: 3,
        ..LogConfig::default()
    };
    let n = c.clone().normalized();
    assert_eq!(n.max_total_bytes, c.max_total_bytes);
    assert_eq!(n.segments, 8);
    assert_eq!(n.retained_instances, 3);
}
