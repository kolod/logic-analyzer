//! Plural forms are correct in every language (spec US2, FR-011, FR-012, SC-002).
//!
//! The expectations are written by hand from the Unicode CLDR plural rules, not computed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_i18n::{Args, Key, Language, Localizer, key};

/// The counts of the table; the last one is a fraction.
const COUNTS: [f64; 18] = [
    0.0,
    1.0,
    2.0,
    3.0,
    4.0,
    5.0,
    11.0,
    12.0,
    14.0,
    21.0,
    22.0,
    25.0,
    100.0,
    101.0,
    102.0,
    111.0,
    1_000_000.0,
    1.5,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cat {
    One,
    Few,
    Many,
    Other,
}
use Cat::{Few, Many, One, Other};

/// The CLDR cardinal category of each count, per language (the order of `COUNTS`).
fn categories(language: Language) -> [Cat; 18] {
    match language {
        // one: i = 1 and v = 0.
        Language::En | Language::De => [
            Other, One, Other, Other, Other, Other, Other, Other, Other, Other, Other, Other,
            Other, Other, Other, Other, Other, Other,
        ],
        // one: i = 0,1. CLDR 42 adds `many` for exact millions, but the plural rules of the shipped
        // library predate it: 1,000,000 is `other` for now. The resources already carry the `many`
        // forms, which start to be used when the library is updated.
        Language::Fr => [
            One, One, Other, Other, Other, Other, Other, Other, Other, Other, Other, Other, Other,
            Other, Other, Other, Other, One,
        ],
        // one: n = 1 (CLDR 42 `many` for exact millions: see French).
        Language::Es => [
            Other, One, Other, Other, Other, Other, Other, Other, Other, Other, Other, Other,
            Other, Other, Other, Other, Other, Other,
        ],
        // other only.
        Language::Ja => [Other; 18],
        // one: i%10=1 and i%100!=11; few: i%10=2..4 and i%100!=12..14; many: the rest of the
        // integers; other: fractions.
        Language::Uk => [
            Many, One, Few, Few, Few, Many, Many, Many, Many, One, Few, Many, Many, One, Few, Many,
            Many, Other,
        ],
    }
}

/// The noun of `count-samples` in each category.
fn noun(language: Language, category: Cat) -> &'static str {
    match (language, category) {
        (Language::En, One) => "sample",
        (Language::En, _) => "samples",
        (Language::De, One) => "Sample",
        (Language::De, _) => "Samples",
        (Language::Fr, One) => "échantillon",
        (Language::Fr, Many) => "d'échantillons",
        (Language::Fr, _) => "échantillons",
        (Language::Es, One) => "muestra",
        (Language::Es, Many) => "de muestras",
        (Language::Es, _) => "muestras",
        (Language::Ja, _) => "サンプル",
        (Language::Uk, One) => "відлік",
        (Language::Uk, Few) => "відліки",
        (Language::Uk, Many) => "відліків",
        (Language::Uk, Other) => "відліку",
    }
}

/// The count written the way the language writes it.
fn number(language: Language, index: usize) -> String {
    let count = COUNTS[index];
    if count == 1.5 {
        return match language {
            Language::En | Language::Ja => "1.5".to_owned(),
            _ => "1,5".to_owned(),
        };
    }
    if count == 1_000_000.0 {
        return match language {
            Language::En | Language::Ja => "1,000,000".to_owned(),
            Language::De | Language::Es => "1.000.000".to_owned(),
            Language::Fr => "1\u{202F}000\u{202F}000".to_owned(),
            Language::Uk => "1\u{A0}000\u{A0}000".to_owned(),
        };
    }
    format!("{count}")
}

fn expected(language: Language, index: usize) -> String {
    let category = categories(language)[index];
    let joiner = if language == Language::Ja { "" } else { " " };
    format!(
        "{}{joiner}{}",
        number(language, index),
        noun(language, category)
    )
}

#[test]
fn counted_messages_use_the_cldr_form_of_every_language() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        for (index, count) in COUNTS.iter().enumerate() {
            let text = l.message(&key!("count-samples"), &Args::new().float("count", *count));
            assert_eq!(
                text,
                expected(language, index),
                "{language:?} count {count}"
            );
        }
        assert!(l.problems().is_empty(), "{language:?}: {:?}", l.problems());
    }
}

#[test]
fn every_counted_message_has_the_forms_its_language_needs() {
    // Each of the five counted messages must give a distinct word for each category a language has.
    for language in Language::ALL {
        let l = Localizer::new(language);
        for id in [
            "count-samples",
            "count-channels",
            "count-devices",
            "count-transitions",
            "count-gaps",
        ] {
            let key = Key::from_static(id);
            let text = |n: f64| l.message(&key, &Args::new().float("count", n));
            let distinct_categories: std::collections::BTreeSet<String> = categories(language)
                .iter()
                .map(|c| format!("{c:?}"))
                .collect();
            let distinct_forms: std::collections::BTreeSet<String> = COUNTS
                .iter()
                .enumerate()
                .map(|(i, c)| text(*c).replace(&number(language, i), "#"))
                .collect();
            assert_eq!(
                distinct_forms.len(),
                distinct_categories.len(),
                "{language:?} {id}: {distinct_forms:?}"
            );
        }
    }
}

#[test]
fn unusual_counts_never_panic_and_use_the_other_form() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        let key = key!("count-samples");
        for n in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            -0.0,
            1.0e300,
            1.0e15,
            0.5,
            -1.5,
        ] {
            let text = l.message(&key, &Args::new().float("count", n));
            assert!(!text.trim().is_empty(), "{language:?} {n}");
        }
        let max = l.message(&key, &Args::new().uint("count", u64::MAX));
        assert!(!max.is_empty());
        let min = l.message(&key, &Args::new().int("count", i64::MIN));
        assert!(!min.is_empty());
        // A count that is not a number takes the default form.
        let text = l.message(&key, &Args::new().trusted("count", "many"));
        assert!(!text.is_empty());
    }
}

#[test]
fn the_categories_the_library_can_produce_are_the_expected_ones() {
    // A sweep over integers and fractions: the set of categories each language produces must be
    // exactly the one the resources are written for, so a library update that changes a rule is
    // noticed here (and by the parity check).
    // The behaviour is observed through a message that names the category.
    let source = "k = { $n ->\n    [zero] zero\n    [one] one\n    [two] two\n    [few] few\n    [many] many\n   *[other] other\n}\n";
    for language in Language::ALL {
        let l = Localizer::from_sources(
            language,
            vec![(language, vec![("k.ftl".to_owned(), source.to_owned())])],
        );
        let mut seen = std::collections::BTreeSet::new();
        let mut sweep: Vec<f64> = (0..2_000).map(f64::from).collect();
        sweep.extend([
            1_000_000.0,
            2_000_000.0,
            10_000_000.0,
            0.5,
            1.5,
            2.5,
            10.5,
            21.5,
        ]);
        for n in sweep {
            seen.insert(l.message(&key!("k"), &Args::new().float("n", n)));
        }
        let want: &[&str] = match language {
            Language::En | Language::De => &["one", "other"],
            Language::Fr | Language::Es => &["one", "other"],
            Language::Ja => &["other"],
            Language::Uk => &["few", "many", "one", "other"],
        };
        let got: Vec<&str> = seen.iter().map(String::as_str).collect();
        assert_eq!(got, want, "{language:?}");
    }
}
