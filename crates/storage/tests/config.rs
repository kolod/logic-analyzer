//! Configuration validation (spec FR-001, US7).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_storage::error;
use la_storage::{Capture, CaptureConfig, Limits};

fn config() -> CaptureConfig {
    CaptureConfig {
        channels: 8,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    }
}

fn code_of(result: la_core::Result<Capture>) -> &'static str {
    result.unwrap_err().code()
}

#[test]
fn defaults_match_the_plan() {
    let l = Limits::default();
    assert_eq!(l.memory_budget_bytes, 1024 * 1024 * 1024);
    assert_eq!(l.max_chunk_samples, 65_536);
    assert_eq!(l.max_pixels, 16_384);
}

#[test]
fn valid_configurations_are_accepted() {
    for channels in [1_u8, 2, 16, 32] {
        let c = CaptureConfig {
            channels,
            ..config()
        };
        assert!(c.validate().is_ok());
        assert!(Capture::new(c).is_ok());
    }
}

#[test]
fn channel_count_outside_range_is_rejected() {
    for channels in [0_u8, 33, 255] {
        let c = CaptureConfig {
            channels,
            ..config()
        };
        assert_eq!(
            c.validate().unwrap_err().code(),
            error::INVALID_CHANNEL_COUNT
        );
        assert_eq!(code_of(Capture::new(c)), error::INVALID_CHANNEL_COUNT);
    }
}

#[test]
fn zero_sample_rate_is_rejected() {
    let c = CaptureConfig {
        sample_rate_hz: 0,
        ..config()
    };
    assert_eq!(c.validate().unwrap_err().code(), error::INVALID_SAMPLE_RATE);
    assert_eq!(code_of(Capture::new(c)), error::INVALID_SAMPLE_RATE);
}

#[test]
fn limits_outside_range_are_rejected() {
    let bad = [
        Limits {
            memory_budget_bytes: 1024 * 1024 - 1,
            ..Limits::default()
        },
        Limits {
            memory_budget_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_chunk_samples: 0,
            ..Limits::default()
        },
        Limits {
            max_chunk_samples: 1_048_577,
            ..Limits::default()
        },
        Limits {
            max_chunk_samples: u32::MAX,
            ..Limits::default()
        },
        Limits {
            max_pixels: 0,
            ..Limits::default()
        },
        Limits {
            max_pixels: 65_537,
            ..Limits::default()
        },
        Limits {
            max_pixels: u32::MAX,
            ..Limits::default()
        },
    ];
    for limits in bad {
        let c = CaptureConfig { limits, ..config() };
        assert_eq!(
            c.validate().unwrap_err().code(),
            error::INVALID_LIMITS,
            "{limits:?}"
        );
        assert_eq!(code_of(Capture::new(c)), error::INVALID_LIMITS);
    }
}

#[test]
fn boundary_limit_values_are_accepted() {
    let limits = Limits {
        memory_budget_bytes: 1024 * 1024,
        max_chunk_samples: 1_048_576,
        max_pixels: 65_536,
    };
    assert!(CaptureConfig { limits, ..config() }.validate().is_ok());
    let limits = Limits {
        memory_budget_bytes: u64::MAX,
        max_chunk_samples: 1,
        max_pixels: 1,
    };
    assert!(CaptureConfig { limits, ..config() }.validate().is_ok());
}
