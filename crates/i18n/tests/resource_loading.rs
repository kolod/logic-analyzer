//! The embedded resources load cleanly (spec US3, FR-005, FR-006).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::workspace_root;
use fluent_syntax::ast::Entry;
use la_i18n::{Args, Key, Language, Localizer, ReviewStatus};

/// The message ids of every file of a language, read from the repository.
fn message_ids(language: Language) -> Vec<String> {
    let dir = workspace_root().join("resources/i18n").join(language.tag());
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let resource = fluent_syntax::parser::parse(text.as_str()).unwrap_or_else(|(r, _)| r);
        for entry in resource.body {
            if let Entry::Message(message) = entry {
                ids.push(message.id.name.to_owned());
            }
        }
    }
    ids.sort();
    ids
}

#[test]
fn every_message_of_every_language_formats_without_a_problem() {
    for language in Language::ALL {
        let l = Localizer::new(language);
        let ids = message_ids(language);
        assert!(!ids.is_empty(), "{language:?} has no messages");
        let args = Args::new()
            .uint("count", 3)
            .uint("number", 4)
            .trusted("code", "x.y")
            .text("name", "n")
            .trusted("value", "1")
            .trusted("first", "1")
            .trusted("second", "2")
            .trusted("used", "1")
            .trusted("limit", "2")
            .uint("min", 1)
            .uint("max", 2)
            .text("version", "0.1.0")
            .text("text", "t");
        for id in &ids {
            let text = l.message(
                &Key::from_static(Box::leak(id.clone().into_boxed_str())),
                &args,
            );
            assert!(!text.trim().is_empty(), "{language:?} {id}");
            assert!(!text.contains('\u{27E6}'), "{language:?} {id}: {text}");
        }
        assert!(l.problems().is_empty(), "{language:?}: {:?}", l.problems());
    }
}

#[test]
fn the_review_status_of_every_language_is_known() {
    let l = Localizer::new(Language::En);
    for language in Language::ALL {
        // All current translations are machine drafts until a native speaker reviews them.
        assert_eq!(
            l.review_status(language),
            ReviewStatus::Draft,
            "{language:?}"
        );
    }
    let reviewed = Localizer::from_sources(
        Language::En,
        vec![(
            Language::En,
            vec![(
                "a.ftl".to_owned(),
                "### status: reviewed\na = A\n".to_owned(),
            )],
        )],
    );
    assert_eq!(reviewed.review_status(Language::En), ReviewStatus::Reviewed);
    // The weakest file decides.
    let mixed = Localizer::from_sources(
        Language::En,
        vec![(
            Language::En,
            vec![
                (
                    "a.ftl".to_owned(),
                    "### status: reviewed\na = A\n".to_owned(),
                ),
                ("b.ftl".to_owned(), "### status: draft\nb = B\n".to_owned()),
            ],
        )],
    );
    assert_eq!(mixed.review_status(Language::En), ReviewStatus::Draft);
}

#[test]
fn the_resources_are_part_of_the_program() {
    // A localizer works with no resource directory on disk: nothing is read at run time.
    let l = Localizer::new(Language::Ja);
    assert!(!l.error_message_for_code("session.damaged").is_empty());
    assert_eq!(l.language(), Language::Ja);
}
