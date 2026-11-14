//! The initial language comes from the settings or the system (spec US4, FR-013, FR-014, FR-016,
//! SC-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_i18n::{Language, Localizer, ProblemKind};

#[test]
fn system_language_tags_map_to_the_expected_language() {
    let long = "x".repeat(1_000);
    let table: Vec<(&str, Language)> = vec![
        ("en", Language::En),
        ("en-US", Language::En),
        ("en_GB.UTF-8", Language::En),
        ("de", Language::De),
        ("de-AT", Language::De),
        ("de_CH", Language::De),
        ("fr-CA", Language::Fr),
        ("fr_BE.UTF-8", Language::Fr),
        ("es-ES", Language::Es),
        ("es-419", Language::Es),
        ("es_MX", Language::Es),
        ("ja", Language::Ja),
        ("ja-JP", Language::Ja),
        ("uk", Language::Uk),
        ("uk-UA", Language::Uk),
        ("it", Language::En),
        ("pt-BR", Language::En),
        ("zh-Hant", Language::En),
        ("ru", Language::En),
        ("C", Language::En),
        ("POSIX", Language::En),
        ("", Language::En),
        ("-", Language::En),
        ("xx_YY", Language::En),
        (long.as_str(), Language::En),
    ];
    assert!(table.len() >= 20);
    for (tag, expected) in table {
        assert_eq!(Language::negotiate([tag]), expected, "{tag:?}");
        let l = Localizer::from_settings(None, &[tag]);
        assert_eq!(l.language(), expected, "{tag:?}");
    }
}

#[test]
fn the_first_supported_system_tag_wins() {
    assert_eq!(
        Localizer::from_settings(None, &["it", "de-AT"]).language(),
        Language::De
    );
    assert_eq!(
        Localizer::from_settings(None, &["it", "ru"]).language(),
        Language::En
    );
    assert_eq!(Localizer::from_settings(None, &[]).language(), Language::En);
    assert_eq!(
        Localizer::from_settings(None, &["", "-", "uk"]).language(),
        Language::Uk
    );
}

#[test]
fn a_stored_choice_wins_over_the_system_language() {
    for stored in Language::ALL {
        let system = if stored == Language::Ja { "de" } else { "ja" };
        let l = Localizer::from_settings(Some(stored.tag()), &[system]);
        assert_eq!(l.language(), stored);
        assert!(l.problems().is_empty(), "{:?}", l.problems());
    }
    // A stored tag in a variant form is accepted as well.
    assert_eq!(
        Localizer::from_settings(Some("DE_at"), &["ja"]).language(),
        Language::De
    );
}

#[test]
fn an_invalid_stored_choice_falls_back_to_the_system_language_and_is_reported() {
    for stored in [
        "",
        "xx",
        "klingon",
        "zh-Hant",
        "de\0",
        "ж",
        &"d".repeat(500),
    ] {
        let l = Localizer::from_settings(Some(stored), &["fr-FR"]);
        assert_eq!(l.language(), Language::Fr, "{stored:?}");
        let problems = l.problems();
        assert!(
            problems
                .iter()
                .any(|p| p.kind == ProblemKind::InvalidLanguageTag),
            "{stored:?}: {problems:?}"
        );
        // The problem never repeats the stored value.
        assert!(problems.iter().all(|p| !p.subject.contains("klingon")));
    }
    // With no usable system language either, English.
    assert_eq!(
        Localizer::from_settings(Some("xx"), &["yy"]).language(),
        Language::En
    );
}

#[test]
fn the_real_system_language_is_always_a_supported_language() {
    let l = Localizer::from_settings(None, &[]);
    assert_eq!(l.language(), Language::En);
    assert!(Language::ALL.contains(&Language::from_system()));
}
