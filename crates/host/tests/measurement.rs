//! The distance and frequency between two markers, in six languages (spec US3, FR-017).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use la_host::markers::{measurement, measurement_text};
use la_i18n::{Language, Localizer};

const NBSP: char = '\u{a0}';
const NARROW: char = '\u{202f}';

fn texts(language: Language, rate: u64, a: u64, b: u64) -> (String, Option<String>) {
    let m = measurement(rate, a, b).unwrap();
    let t = measurement_text(&Localizer::new(language), &m);
    (t.distance, t.frequency)
}

#[test]
fn the_distance_is_exact_and_the_frequency_follows_from_it() {
    let m = measurement(1_000_000, 100, 1_600).unwrap();
    assert_eq!(m.distance_samples, 1_500);
    assert!((m.seconds - 0.0015).abs() < 1e-15);
    assert!((m.hertz.unwrap() - 666.666_666_666).abs() < 1e-6);
    // The order of the two markers does not matter.
    assert_eq!(measurement(1_000_000, 1_600, 100), Some(m));
    // Zero distance has no frequency.
    let zero = measurement(1_000_000, 70, 70).unwrap();
    assert_eq!(zero.distance_samples, 0);
    assert_eq!(zero.hertz, None);
    // No sample rate, no measurement.
    assert_eq!(measurement(0, 1, 2), None);
}

#[test]
fn one_and_a_half_milliseconds_in_every_language() {
    // 1,500 samples at 1 MHz: 1.5 ms; 666.67 Hz is shown with the four digits that 1,500 samples resolve.
    let expected = [
        (
            Language::En,
            format!("1.5{NBSP}ms"),
            format!("666.7{NBSP}Hz"),
        ),
        (
            Language::De,
            format!("1,5{NBSP}ms"),
            format!("666,7{NBSP}Hz"),
        ),
        (
            Language::Fr,
            format!("1,5{NARROW}ms"),
            format!("666,7{NARROW}Hz"),
        ),
        (
            Language::Es,
            format!("1,5{NBSP}ms"),
            format!("666,7{NBSP}Hz"),
        ),
        (Language::Ja, "1.5ミリ秒".to_owned(), "666.7Hz".to_owned()),
        (
            Language::Uk,
            format!("1,5{NBSP}мс"),
            format!("666,7{NBSP}Гц"),
        ),
    ];
    for (language, distance, frequency) in expected {
        let (d, f) = texts(language, 1_000_000, 100, 1_600);
        assert_eq!(d, distance, "{language:?}");
        assert_eq!(f.as_deref(), Some(frequency.as_str()), "{language:?}");
    }
}

#[test]
fn round_numbers_use_the_matching_units() {
    let (d, f) = texts(Language::En, 1_000_000, 0, 1_000);
    assert_eq!(d, format!("1{NBSP}ms"));
    assert_eq!(f.as_deref(), Some(format!("1{NBSP}kHz").as_str()));
    let (d, f) = texts(Language::Uk, 1_000_000, 0, 1_000);
    assert_eq!(d, format!("1{NBSP}мс"));
    assert_eq!(f.as_deref(), Some(format!("1{NBSP}кГц").as_str()));
}

#[test]
fn the_frequency_never_claims_more_digits_than_the_distance_resolves() {
    // Three samples apart at 100 MHz: 30 ns, and 33.33 MHz is only known to one digit.
    let (d, f) = texts(Language::En, 100_000_000, 10, 13);
    assert_eq!(d, format!("30{NBSP}ns"));
    assert_eq!(f.as_deref(), Some(format!("30{NBSP}MHz").as_str()));
    // 7 samples at 1 MHz: 142857.14 Hz is shown as 100 kHz (one digit, since 7 < 10).
    let (_, f) = texts(Language::En, 1_000_000, 0, 7);
    assert_eq!(f.as_deref(), Some(format!("100{NBSP}kHz").as_str()));
    // 12 samples: two digits.
    let (_, f) = texts(Language::En, 1_000_000, 0, 12);
    assert_eq!(f.as_deref(), Some(format!("83{NBSP}kHz").as_str()));
}

#[test]
fn no_distance_gives_zero_time_and_no_frequency() {
    let (d, f) = texts(Language::En, 1_000, 5, 5);
    assert_eq!(d, format!("0{NBSP}s"));
    assert_eq!(f, None);
}

#[test]
fn long_distances_use_minutes_and_seconds() {
    let (d, _) = texts(Language::En, 1_000, 0, 1_000_000);
    assert_eq!(d, format!("16{NBSP}min 40{NBSP}s"));
}
