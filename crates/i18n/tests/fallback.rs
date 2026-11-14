//! A missing or unusable translation never breaks the screen (spec US3, FR-004..FR-007, SC-007).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_i18n::{Args, Key, Language, Localizer, ProblemKind, key};

const ENGLISH: &str = "### status: reviewed\n\
hello = Hello\n\
only-english = Only in English\n\
greet = Hi { $name }!\n\
other = Other\n";

fn files(text: &str) -> Vec<(String, String)> {
    vec![("test.ftl".to_owned(), text.to_owned())]
}

fn with_german(german: &str) -> Localizer {
    Localizer::from_sources(
        Language::De,
        vec![
            (Language::En, files(ENGLISH)),
            (Language::De, files(german)),
        ],
    )
}

fn msg(l: &Localizer, key: Key) -> String {
    l.message(&key, &Args::new())
}

#[test]
fn a_key_missing_in_one_language_shows_english_and_names_the_language_and_key() {
    let l = with_german("### status: draft\nhello = Hallo\n");
    for _ in 0..3 {
        assert_eq!(msg(&l, key!("only-english")), "Only in English");
    }
    let problems = l.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].kind, ProblemKind::MissingTranslation);
    assert_eq!(problems[0].language, Language::De);
    assert_eq!(problems[0].subject, "only-english");
}

#[test]
fn a_key_missing_everywhere_shows_a_placeholder_with_the_key() {
    let l = with_german("hello = Hallo\n");
    let text = msg(&l, key!("nowhere-at-all"));
    assert_eq!(text, "\u{27E6}nowhere-at-all\u{27E7}");
    assert!(
        l.problems()
            .iter()
            .any(|p| p.kind == ProblemKind::MissingKey && p.subject == "nowhere-at-all")
    );
}

#[test]
fn a_damaged_language_falls_back_to_english_as_a_whole_and_the_others_are_untouched() {
    let broken_files = [
        "hello = Hallo\nbroken = { $unterminated\n",
        "hello = Hallo\nbroken = \"unterminated\n}\n",
        "hello = Hallo\n}\n",
        "hello = Hallo\n= no id\n",
        "hello = Hallo { \n",
    ];
    for broken in broken_files {
        let l = Localizer::from_sources(
            Language::De,
            vec![
                (Language::En, files(ENGLISH)),
                (Language::De, files(broken)),
                (Language::Fr, files("hello = Bonjour\n")),
            ],
        );
        // The whole of German is English now, including the message that parsed fine.
        assert_eq!(msg(&l, key!("hello")), "Hello", "{broken:?}");
        let damaged: Vec<_> = l
            .problems()
            .into_iter()
            .filter(|p| p.kind == ProblemKind::DamagedResource)
            .collect();
        assert_eq!(damaged.len(), 1, "{broken:?}: {damaged:?}");
        assert_eq!(
            (damaged[0].language, damaged[0].subject.as_str()),
            (Language::De, "test.ftl")
        );
        // French works as before.
        l.set_language(Language::Fr);
        assert_eq!(msg(&l, key!("hello")), "Bonjour");
    }
}

#[test]
fn files_that_are_unusual_but_valid_still_load() {
    // Empty file, byte order mark, Windows line endings.
    for (name, text) in [
        ("empty", String::new()),
        ("bom", "\u{FEFF}hello = Hallo\n".to_owned()),
        ("crlf", "hello = Hallo\r\nother = Anderes\r\n".to_owned()),
    ] {
        let l = with_german(&text);
        let hello = msg(&l, key!("hello"));
        if name == "empty" {
            assert_eq!(hello, "Hello", "{name}");
        } else {
            assert_eq!(hello, "Hallo", "{name}");
        }
        assert!(
            l.problems()
                .iter()
                .all(|p| p.kind != ProblemKind::DamagedResource),
            "{name}: {:?}",
            l.problems()
        );
    }
}

#[test]
fn a_duplicate_message_keeps_the_first_definition_and_is_reported() {
    let l = with_german("hello = Erstes\nhello = Zweites\n");
    assert_eq!(msg(&l, key!("hello")), "Erstes");
    assert!(
        l.problems()
            .iter()
            .any(|p| p.kind == ProblemKind::DamagedResource)
    );
}

#[test]
fn a_missing_argument_still_renders_the_message_and_is_reported() {
    let l = with_german("greet = Hallo { $name }!\n");
    let text = msg(&l, key!("greet"));
    assert!(text.starts_with("Hallo ") && text.ends_with('!'), "{text}");
    assert!(
        l.problems()
            .iter()
            .any(|p| p.kind == ProblemKind::FormatError && p.subject == "greet")
    );
}

#[test]
fn a_selector_given_text_instead_of_a_number_uses_the_default_variant() {
    let l = with_german("count = { $n ->\n    [one] ein Ding\n   *[other] viele Dinge\n}\n");
    let text = l.message(&key!("count"), &Args::new().trusted("n", "abc"));
    assert_eq!(text, "viele Dinge");
}

#[test]
fn cycles_long_chains_and_huge_expansions_terminate_with_readable_text() {
    let mut text = String::from("loop = { loop }\nself = Self { self }\n");
    for i in 0..1_000 {
        text.push_str(&format!("m{i} = x{{ m{} }}\n", i + 1));
    }
    text.push_str("m1000 = end\n");
    // Nested doubling: 2^40 characters if it were not bounded.
    text.push_str("d0 = ab\n");
    for i in 1..40 {
        text.push_str(&format!("d{i} = {{ d{} }}{{ d{} }}\n", i - 1, i - 1));
    }
    let l = with_german(&text);
    let started = std::time::Instant::now();
    for key in ["loop", "self", "m0", "d39"] {
        let out = l.message(&Key::from_static(key), &Args::new());
        assert!(!out.is_empty(), "{key}");
        assert!(out.len() < 10_000_000, "{key} produced {} bytes", out.len());
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(
        l.problems()
            .iter()
            .any(|p| p.kind == ProblemKind::FormatError)
    );
}

#[test]
fn no_problem_contains_an_argument_value() {
    let secret = "unique-argument-marker-9f3a";
    let l = with_german("greet = Hallo { $name }!\n");
    let _ = l.message(&key!("missing-key"), &Args::new().text("x", secret));
    let _ = l.message(&key!("greet"), &Args::new().text("other", secret));
    let _ = l.message(&key!("only-english"), &Args::new().text("x", secret));
    for p in l.problems() {
        assert!(!p.subject.contains(secret), "{p:?}");
    }
}

#[test]
fn one_broken_file_degrades_the_whole_language_even_if_its_other_files_are_fine() {
    let l = Localizer::from_sources(
        Language::De,
        vec![
            (Language::En, files(ENGLISH)),
            (
                Language::De,
                vec![
                    ("good.ftl".to_owned(), "hello = Hallo\n".to_owned()),
                    ("broken.ftl".to_owned(), "other = { $oops\n".to_owned()),
                ],
            ),
        ],
    );
    assert_eq!(
        msg(&l, key!("hello")),
        "Hello",
        "no mixture of languages inside one language"
    );
    let damaged: Vec<_> = l
        .problems()
        .into_iter()
        .filter(|p| p.kind == ProblemKind::DamagedResource)
        .collect();
    assert_eq!(damaged.len(), 1);
    assert_eq!(damaged[0].subject, "broken.ftl");
}
