//! Entering the length of a capture in seconds (spec 014, user story 3, task T023).
//!
//! Examples are worked out by hand: samples = round half up of rate x seconds, computed exactly.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::Lcg;
use la_host::config::{SecondsError, samples_for_seconds, seconds_for_samples};

#[test]
fn seconds_become_samples_exactly_and_round_half_up() {
    assert_eq!(samples_for_seconds(1_000_000, "2.5"), Ok(2_500_000));
    assert_eq!(samples_for_seconds(1_000_000, "1"), Ok(1_000_000));
    assert_eq!(samples_for_seconds(1_000_000, "0.000001"), Ok(1));
    // 3 samples a second for half a second is 1.5 samples: rounded up to 2.
    assert_eq!(samples_for_seconds(3, "0.5"), Ok(2));
    // 0.166666667 s at 3 Hz is 0.500000001 samples: rounded to 1.
    assert_eq!(samples_for_seconds(3, "0.166666667"), Ok(1));
    assert_eq!(samples_for_seconds(1_000_000_000, "0.000000001"), Ok(1));
    assert_eq!(samples_for_seconds(1_000, "120"), Ok(120_000));
    assert_eq!(
        samples_for_seconds(1, "007"),
        Ok(7),
        "leading zeros are fine"
    );
    assert_eq!(samples_for_seconds(1, "0.9"), Ok(1));
    // The largest that fits.
    assert_eq!(samples_for_seconds(1, "18446744073709551615"), Ok(u64::MAX));
}

#[test]
fn what_is_not_a_positive_amount_of_time_is_refused() {
    // Less than half a sample is no sample at all.
    assert_eq!(
        samples_for_seconds(1_000, "0.0004"),
        Err(SecondsError::TooSmall)
    );
    assert_eq!(samples_for_seconds(1_000, "0"), Err(SecondsError::TooSmall));
    assert_eq!(
        samples_for_seconds(1_000, "0.000"),
        Err(SecondsError::TooSmall)
    );
    assert_eq!(samples_for_seconds(0, "1"), Err(SecondsError::NoRate));
    // Too large for a count of samples.
    assert_eq!(
        samples_for_seconds(u64::MAX, "2"),
        Err(SecondsError::TooLarge)
    );
    assert_eq!(
        samples_for_seconds(1, "18446744073709551616"),
        Err(SecondsError::TooLarge)
    );
    assert_eq!(
        samples_for_seconds(1_000_000, "99999999999999999999"),
        Err(SecondsError::TooLarge)
    );
    for text in ["", " ", "  "] {
        assert_eq!(
            samples_for_seconds(1_000, text),
            Err(SecondsError::Empty),
            "{text:?}"
        );
    }
    for text in [
        "-1", "+1", "1e3", "abc", "1,5", "1.", ".5", "1.2.3", "1 2", "0x10", "\u{660}", "١٢",
        "1\u{0}", "..", "-", ".",
    ] {
        assert_eq!(
            samples_for_seconds(1_000, text),
            Err(SecondsError::NotANumber),
            "{text:?}"
        );
    }
    // Ten decimals are more than the resolution of a nanosecond.
    assert_eq!(
        samples_for_seconds(1_000, "0.1234567891"),
        Err(SecondsError::TooManyDecimals)
    );
    // A text of any length is refused without trouble.
    let long = "9".repeat(5_000);
    assert_eq!(
        samples_for_seconds(1_000, &long),
        Err(SecondsError::NotANumber)
    );
    let long_fraction = format!("0.{}", "1".repeat(5_000));
    assert!(samples_for_seconds(1_000, &long_fraction).is_err());
}

#[test]
fn samples_are_shown_as_exact_seconds_with_only_the_digits_they_need() {
    assert_eq!(seconds_for_samples(1_000_000, 2_500_000), "2.5");
    assert_eq!(seconds_for_samples(1_000_000, 1), "0.000001");
    assert_eq!(seconds_for_samples(1_000, 1), "0.001");
    assert_eq!(seconds_for_samples(1_000, 1_000), "1");
    assert_eq!(seconds_for_samples(1_000, 1_500), "1.5");
    assert_eq!(seconds_for_samples(1, 7), "7");
    assert_eq!(seconds_for_samples(3, 1), "0.333333333");
    assert_eq!(seconds_for_samples(3, 2), "0.666666667");
    assert_eq!(seconds_for_samples(1_000_000_000, 1), "0.000000001");
    assert_eq!(seconds_for_samples(1_000_000, 0), "0");
    assert_eq!(seconds_for_samples(0, 5), "0", "no rate, nothing to show");
    // The largest count and the lowest rate.
    assert_eq!(seconds_for_samples(1, u64::MAX), "18446744073709551615");
}

#[test]
fn what_is_shown_is_read_back_exactly_when_it_is_exact_and_within_one_sample_otherwise() {
    let mut random = Lcg(2026);
    // Rates that divide 10^9 print exactly: the round trip is exact.
    for rate in [
        1_u64,
        2,
        4,
        5,
        8,
        10,
        100,
        1_000,
        20_000,
        1_000_000,
        8_000_000,
        100_000_000,
        1_000_000_000,
    ] {
        for _ in 0..400 {
            let samples = 1 + random.below(10_000_000_000);
            let text = seconds_for_samples(rate, samples);
            assert_eq!(
                samples_for_seconds(rate, &text),
                Ok(samples),
                "rate {rate}, samples {samples}, text {text}"
            );
        }
    }
    // Other rates (up to 10^9) print rounded to a nanosecond: within one sample.
    for rate in [3_u64, 7, 12_345, 48_000, 44_100, 999_999_999, 333_333_333] {
        for _ in 0..400 {
            let samples = 1 + random.below(1_000_000_000);
            let text = seconds_for_samples(rate, samples);
            let back = samples_for_seconds(rate, &text).unwrap();
            assert!(
                back.abs_diff(samples) <= 1,
                "rate {rate}, samples {samples}, text {text}, back {back}"
            );
        }
    }
}
