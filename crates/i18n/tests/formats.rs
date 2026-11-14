//! Numbers, frequencies, sizes, durations and dates are written the way each language writes them
//! (spec US5, FR-017..FR-020, SC-003).
//!
//! The expected texts are written by hand from the CLDR conventions of each language.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::workspace_root;
use la_i18n::{Args, Key, Language, Localizer, ProblemKind};

/// What a language writes around a value: the space before the unit and the symbols.
struct Style {
    /// Between the value and the unit symbol.
    space: &'static str,
    /// Between the two parts of a duration.
    join: &'static str,
    hz: [&'static str; 4],
    bytes: [&'static str; 5],
    /// ns, µs, ms, s, min, h
    time: [&'static str; 6],
    /// The decimal separator.
    decimal: &'static str,
    /// 1,023 written in the language.
    thousand_twenty_three: &'static str,
}

fn style(language: Language) -> Style {
    match language {
        Language::En => Style {
            space: "\u{A0}",
            join: " ",
            hz: ["Hz", "kHz", "MHz", "GHz"],
            bytes: ["B", "KiB", "MiB", "GiB", "TiB"],
            time: ["ns", "µs", "ms", "s", "min", "h"],
            decimal: ".",
            thousand_twenty_three: "1,023",
        },
        Language::De => Style {
            space: "\u{A0}",
            join: " ",
            hz: ["Hz", "kHz", "MHz", "GHz"],
            bytes: ["B", "KiB", "MiB", "GiB", "TiB"],
            time: ["ns", "µs", "ms", "s", "min", "h"],
            decimal: ",",
            thousand_twenty_three: "1.023",
        },
        Language::Fr => Style {
            space: "\u{202F}",
            join: " ",
            hz: ["Hz", "kHz", "MHz", "GHz"],
            bytes: ["o", "Kio", "Mio", "Gio", "Tio"],
            time: ["ns", "µs", "ms", "s", "min", "h"],
            decimal: ",",
            thousand_twenty_three: "1\u{202F}023",
        },
        Language::Es => Style {
            space: "\u{A0}",
            join: " ",
            hz: ["Hz", "kHz", "MHz", "GHz"],
            bytes: ["B", "KiB", "MiB", "GiB", "TiB"],
            time: ["ns", "µs", "ms", "s", "min", "h"],
            decimal: ",",
            thousand_twenty_three: "1023",
        },
        Language::Ja => Style {
            space: "",
            join: "",
            hz: ["Hz", "kHz", "MHz", "GHz"],
            bytes: ["B", "KiB", "MiB", "GiB", "TiB"],
            time: ["ナノ秒", "マイクロ秒", "ミリ秒", "秒", "分", "時間"],
            decimal: ".",
            thousand_twenty_three: "1,023",
        },
        Language::Uk => Style {
            space: "\u{A0}",
            join: " ",
            hz: ["Гц", "кГц", "МГц", "ГГц"],
            bytes: ["Б", "КіБ", "МіБ", "ГіБ", "ТіБ"],
            time: ["нс", "мкс", "мс", "с", "хв", "год"],
            decimal: ",",
            thousand_twenty_three: "1\u{A0}023",
        },
    }
}

fn with(style: &Style, number: &str, symbol: &str) -> String {
    format!("{number}{}{symbol}", style.space)
}

#[test]
fn frequencies_are_scaled_and_written_in_each_language() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        let s = style(language);
        let d = s.decimal;
        let table: [(f64, String); 7] = [
            (1.0, with(&s, "1", s.hz[0])),
            (999.0, with(&s, "999", s.hz[0])),
            (1_000.0, with(&s, "1", s.hz[1])),
            (12_500_000.0, with(&s, &format!("12{d}5"), s.hz[2])),
            (500_000_000.0, with(&s, "500", s.hz[2])),
            (1_250_000_000.0, with(&s, &format!("1{d}25"), s.hz[3])),
            (0.0, with(&s, "0", s.hz[0])),
        ];
        for (hertz, expected) in table {
            assert_eq!(
                l.format_frequency(hertz),
                expected,
                "{language:?} {hertz} Hz"
            );
        }
        assert!(l.problems().is_empty(), "{language:?}: {:?}", l.problems());
    }
}

#[test]
fn sizes_use_binary_units_in_each_language() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        let s = style(language);
        let d = s.decimal;
        let table: [(u64, String); 5] = [
            (0, with(&s, "0", s.bytes[0])),
            (1_023, with(&s, s.thousand_twenty_three, s.bytes[0])),
            (1_024, with(&s, "1", s.bytes[1])),
            (31_981_568, with(&s, &format!("30{d}5"), s.bytes[2])),
            (5_368_709_120, with(&s, "5", s.bytes[3])),
        ];
        for (bytes, expected) in table {
            assert_eq!(l.format_size(bytes), expected, "{language:?} {bytes} B");
        }
        assert!(l.format_size(u64::MAX).len() < 40);
    }
}

#[test]
fn durations_use_one_unit_below_a_minute_and_parts_above() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        let s = style(language);
        let d = s.decimal;
        let table: [(f64, String); 7] = [
            (1.5e-6, with(&s, &format!("1{d}5"), s.time[1])),
            (2.3e-3, with(&s, &format!("2{d}3"), s.time[2])),
            (42.0, with(&s, "42", s.time[3])),
            (0.0, with(&s, "0", s.time[3])),
            (
                125.0,
                format!(
                    "{}{}{}",
                    with(&s, "2", s.time[4]),
                    s.join,
                    with(&s, "5", s.time[3])
                ),
            ),
            (
                3_725.0,
                format!(
                    "{}{}{}",
                    with(&s, "1", s.time[5]),
                    s.join,
                    with(&s, "2", s.time[4])
                ),
            ),
            (7_200.0, with(&s, "2", s.time[5])),
        ];
        for (seconds, expected) in table {
            assert_eq!(
                l.format_duration(seconds),
                expected,
                "{language:?} {seconds} s"
            );
        }
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(l.format_duration(bad), "\u{2013}", "{language:?}");
        }
    }
}

#[test]
fn dates_and_times_follow_the_language_and_the_offset() {
    let epoch = 1_794_672_309; // 2026-11-14T16:05:09Z
    let expected = [
        (Language::En, "11/14/2026 4:05:09 PM"),
        (Language::De, "14.11.2026 16:05:09"),
        (Language::Fr, "14/11/2026 16:05:09"),
        (Language::Es, "14/11/2026 16:05:09"),
        (Language::Ja, "2026/11/14 16:05:09"),
        (Language::Uk, "14.11.2026 16:05:09"),
    ];
    for (language, text) in expected {
        let l = Localizer::new(language);
        assert_eq!(l.format_datetime(epoch), text, "{language:?}");
    }
    let l = Localizer::new(Language::En);
    l.set_utc_offset_seconds(7_200);
    assert_eq!(l.format_datetime(epoch), "11/14/2026 6:05:09 PM");
    l.set_utc_offset_seconds(-18_000);
    assert_eq!(l.format_datetime(epoch), "11/14/2026 11:05:09 AM");
    // Outside the years 1 to 9999 and beyond i64 range: the marker, no panic.
    assert_eq!(l.format_datetime(-62_135_596_801 + 18_000 - 1), "\u{2013}");
    assert_eq!(l.format_datetime(253_402_300_800 + 18_000), "\u{2013}");
    assert_eq!(l.format_datetime(i64::MAX), "\u{2013}");
    assert_eq!(l.format_datetime(i64::MIN), "\u{2013}");
}

#[test]
fn format_functions_in_resources_give_the_same_text_as_the_methods() {
    let test_messages = "t-number = { FORMAT_NUMBER($v) }\n\
        t-frequency = { FORMAT_FREQUENCY($v) }\n\
        t-size = { FORMAT_SIZE($v) }\n\
        t-duration = { FORMAT_DURATION($v) }\n\
        t-date = { FORMAT_DATE($v) }\n\
        t-time = { FORMAT_TIME($v) }\n\
        t-datetime = { FORMAT_DATETIME($v) }\n";
    for language in Language::ALL {
        let common = std::fs::read_to_string(
            workspace_root()
                .join("resources/i18n")
                .join(language.tag())
                .join("common.ftl"),
        )
        .unwrap();
        let l = Localizer::from_sources(
            language,
            vec![(
                language,
                vec![
                    ("common.ftl".to_owned(), common),
                    ("test.ftl".to_owned(), test_messages.to_owned()),
                ],
            )],
        );
        let call =
            |id: &'static str, v: f64| l.message(&Key::from_static(id), &Args::new().float("v", v));
        assert_eq!(
            call("t-number", 1_234_567.89),
            l.format_number(1_234_567.89),
            "{language:?}"
        );
        assert_eq!(
            call("t-frequency", 12_500_000.0),
            l.format_frequency(12_500_000.0),
            "{language:?}"
        );
        assert_eq!(
            call("t-size", 31_981_568.0),
            l.format_size(31_981_568),
            "{language:?}"
        );
        assert_eq!(
            call("t-duration", 3_725.0),
            l.format_duration(3_725.0),
            "{language:?}"
        );
        assert_eq!(
            call("t-datetime", 1_794_672_309.0),
            l.format_datetime(1_794_672_309),
            "{language:?}"
        );
        let date = call("t-date", 1_794_672_309.0);
        let time = call("t-time", 1_794_672_309.0);
        assert_eq!(
            format!("{date} {time}"),
            l.format_datetime(1_794_672_309),
            "{language:?}"
        );
        assert!(l.problems().is_empty(), "{language:?}: {:?}", l.problems());

        // A non-number argument gives the marker and a problem.
        let text = l.message(
            &Key::from_static("t-frequency"),
            &Args::new().trusted("v", "abc"),
        );
        assert_eq!(text, "\u{2013}");
        assert!(
            l.problems()
                .iter()
                .any(|p| p.kind == ProblemKind::FormatError && p.subject == "FORMAT_FREQUENCY")
        );
    }
}

#[test]
fn a_missing_unit_message_falls_back_to_an_english_symbol_and_is_reported() {
    let l = Localizer::from_sources(
        Language::De,
        vec![(
            Language::De,
            vec![("a.ftl".to_owned(), "x = y\n".to_owned())],
        )],
    );
    let text = l.format_frequency(12_500_000.0);
    assert_eq!(text, "12,5\u{A0}MHz");
    assert!(
        l.problems()
            .iter()
            .any(|p| p.kind == ProblemKind::MissingTranslation && p.subject == "unit-mhz")
    );
}
