//! User-supplied content is data: never interpreted, never translated, never able to change a
//! message (spec US7, FR-021, FR-022).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::time::{Duration, Instant};

use la_i18n::{Args, Language, Localizer, ProblemKind, key};

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

fn localizer() -> Localizer {
    let files = |text: &str| vec![("t.ftl".to_owned(), text.to_owned())];
    let english = "greet = Hello, { $name }, welcome back.\nunit-mhz = { $value } MHz\n";
    let german = "greet = Hallo, { $name }, willkommen zurück.\n";
    Localizer::from_sources(
        Language::De,
        vec![
            (Language::En, files(english)),
            (Language::De, files(german)),
        ],
    )
}

/// The message with an ordinary value: the frame every hostile value must leave intact.
fn frame(l: &Localizer) -> (String, String) {
    let text = l.message(&key!("greet"), &Args::new().text("name", "XX"));
    let (before, after) = text.split_once("XX").unwrap();
    (before.to_owned(), after.to_owned())
}

#[test]
fn text_that_looks_like_message_syntax_is_output_unchanged() {
    let l = localizer();
    let (before, after) = frame(&l);
    assert_eq!(before, "Hallo, \u{2068}");
    assert_eq!(after, "\u{2069}, willkommen zurück.");
    let hostile = [
        "{ $count } samples",
        "{ NUMBER($count) }",
        "{ -term }",
        "{ \"\\u0000\" }",
        "{",
        "}",
        "\\",
        "{{{{{{{{{{{{{{{{{{{{",
        "{ greet }",
        "{ FORMAT_NUMBER($x) }",
        "*[other] x",
        "### status: reviewed",
        "name = injected",
    ];
    for value in hostile {
        let text = l.message(&key!("greet"), &Args::new().text("name", value));
        assert_eq!(text, format!("{before}{value}{after}"), "{value:?}");
    }
    assert!(l.problems().is_empty(), "{:?}", l.problems());
}

#[test]
fn bidirectional_and_control_characters_are_isolated_and_cannot_break_out() {
    let l = localizer();
    let (before, after) = frame(&l);
    let cases = [
        ("\u{202E}reversed", "reversed"),
        ("a\u{2069}b", "ab"),
        ("a\u{2066}b\u{2067}c\u{2068}d", "abcd"),
        ("\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}", ""),
        ("שלום עולם", "שלום עולם"),
        ("مرحبا", "مرحبا"),
        ("a\u{200F}b\u{200E}c", "a\u{200F}b\u{200E}c"),
        ("NUL\0here", "NUL\0here"),
        ("\u{FEFF}bom", "\u{FEFF}bom"),
        ("tab\tnewline\nreturn\r", "tab\tnewline\nreturn\r"),
        ("\u{FFFD}", "\u{FFFD}"),
    ];
    for (value, expected) in cases {
        let text = l.message(&key!("greet"), &Args::new().text("name", value));
        assert_eq!(text, format!("{before}{expected}{after}"), "{value:?}");
        assert_eq!(
            text.matches(FSI).count(),
            text.matches(PDI).count(),
            "{value:?}: balanced isolates"
        );
        // Every closing isolate comes after its opening one.
        let open = text.find(FSI).unwrap();
        let close = text.rfind(PDI).unwrap();
        assert!(open < close);
        assert_eq!(
            text.matches(PDI).count(),
            1,
            "{value:?}: the value cannot close the isolate early"
        );
    }
}

#[test]
fn a_huge_value_is_produced_without_a_panic_or_a_noticeable_delay() {
    let l = localizer();
    let big = "ж{ $x }".repeat(150_000); // about one million characters
    let started = Instant::now();
    let text = l.message(&key!("greet"), &Args::new().text("name", &big));
    let limit = if cfg!(debug_assertions) {
        Duration::from_secs(2)
    } else {
        Duration::from_millis(200)
    };
    assert!(started.elapsed() < limit, "took {:?}", started.elapsed());
    assert!(text.contains(&big));
    assert!(text.len() < big.len() + 200);
}

#[test]
fn a_value_equal_to_a_key_is_shown_as_text_not_looked_up() {
    let l = localizer();
    let text = l.message(&key!("greet"), &Args::new().text("name", "unit-mhz"));
    assert!(text.contains("unit-mhz"));
    assert!(!text.contains("MHz"));
    // And a value is never a key: the API has no way to ask for one.
    assert_eq!(
        l.problems()
            .iter()
            .filter(|p| p.kind == ProblemKind::MissingKey)
            .count(),
        0
    );
}

#[test]
fn a_new_message_in_every_language_needs_no_code_change() {
    // A key and its six texts are all a developer adds; the lookup finds it in every language.
    let files = |text: &str| vec![("new.ftl".to_owned(), text.to_owned())];
    let sources = Language::ALL
        .iter()
        .map(|l| (*l, files(&format!("brand-new = text in {}\n", l.tag()))))
        .collect();
    let l = Localizer::from_sources(Language::En, sources);
    for language in Language::ALL {
        let text = l.message_in(language, &key!("brand-new"), &Args::new());
        assert_eq!(text, format!("text in {}", language.tag()));
    }
    assert!(l.problems().is_empty());
}
