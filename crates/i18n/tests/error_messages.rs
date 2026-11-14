//! Every stable error code has a message in every language (spec US1, FR-008..FR-010, SC-001).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{error_codes, key_of, localizer};
use la_core::{Error, ErrorKind};
use la_i18n::{Args, Language, ProblemKind};

#[test]
fn the_scan_finds_the_codes_of_the_product() {
    let codes: Vec<String> = error_codes().into_iter().map(|(c, _)| c).collect();
    assert!(codes.len() >= 44, "only {} codes found", codes.len());
    for expected in [
        "session.damaged",
        "storage.memory_limit",
        "device.disconnected",
        "device.driver_panic",
    ] {
        assert!(codes.iter().any(|c| c == expected), "{expected}");
    }
}

#[test]
fn every_code_has_a_message_in_every_language() {
    let codes = error_codes();
    for language in Language::ALL {
        let l = localizer(language);
        for (code, file) in &codes {
            let text = l.error_message_for_code(code);
            let key = key_of(code);
            assert!(
                !text.trim().is_empty(),
                "{language:?} {code} ({})",
                file.display()
            );
            assert_ne!(text, key.as_str(), "{language:?} {code}");
            assert!(!text.contains("error-"), "{language:?} {code}: {text}");
            assert!(
                !text.contains('\u{27E6}'),
                "{language:?} {code}: a placeholder"
            );
            assert!(
                !text.contains("{$") && !text.contains("{ $"),
                "{language:?} {code}: {text}"
            );
        }
        assert!(l.problems().is_empty(), "{language:?}: {:?}", l.problems());
    }
}

#[test]
fn the_english_messages_say_what_happened_and_what_to_do() {
    let l = localizer(Language::En);
    let jargon = [
        "invariant",
        "bytes of",
        "u64",
        "panic",
        "unwrap",
        "Error {",
        "::",
    ];
    for (code, _) in error_codes() {
        let text = l.error_message_for_code(&code);
        assert!(text.len() > 20, "{code}: {text}");
        assert!(text.ends_with('.'), "{code}: {text}");
        assert!(
            text.chars().next().is_some_and(char::is_uppercase),
            "{code}: {text}"
        );
        for word in jargon {
            assert!(!text.contains(word), "{code} contains `{word}`: {text}");
        }
        // Every message gives the user something to do or reassures about their data.
        let helpful = [
            "Check",
            "Choose",
            "Try",
            "Please",
            "Reconnect",
            "Unplug",
            "Wait",
            "Stop",
            "Update",
            "Open",
            "Select",
            "Enter",
            "Start",
            "Save",
            "Resize",
            "Allow",
            "If you",
            "Nothing was changed",
            "Everything captured",
        ];
        assert!(
            helpful.iter().any(|w| text.contains(w)),
            "{code} gives no advice: {text}"
        );
    }
}

#[test]
fn an_error_value_gives_the_message_of_its_code_and_never_its_detail() {
    let l = localizer(Language::De);
    for (code, _) in error_codes() {
        let code_static: &'static str = Box::leak(code.clone().into_boxed_str());
        for kind in [
            ErrorKind::Io,
            ErrorKind::Corrupt,
            ErrorKind::Device,
            ErrorKind::Resource,
            ErrorKind::Internal,
        ] {
            let error = Error::new(kind, code_static, "UNIQUE-TECHNICAL-DETAIL-1234");
            let text = l.error_message(&error);
            assert_eq!(text, l.error_message_for_code(&code));
            assert!(!text.contains("UNIQUE-TECHNICAL-DETAIL"), "{code}");
        }
    }
}

#[test]
fn a_code_without_a_message_gives_the_generic_text_with_the_code_and_a_problem() {
    for language in Language::ALL {
        let l = localizer(language);
        let text = l.error_message_for_code("feature.not_yet_defined");
        assert!(
            text.contains("feature.not_yet_defined"),
            "{language:?}: {text}"
        );
        assert!(!text.trim().is_empty());
        let problems = l.problems();
        assert!(
            problems
                .iter()
                .any(|p| p.kind == ProblemKind::UnknownErrorCode
                    && p.language == language
                    && p.subject == "feature.not_yet_defined"),
            "{language:?}: {problems:?}"
        );
        // The generic message does not look like a message of a known code.
        assert_ne!(text, l.error_message_for_code("session.damaged"));
    }
}

#[test]
fn hostile_codes_never_panic_and_give_readable_text() {
    let l = localizer(Language::Uk);
    let long = "x".repeat(10_000);
    for code in [
        "",
        ".",
        "..",
        "a b",
        "session.damaged\0",
        "ж.я",
        &long,
        "error-unexpected",
        "{ $code }",
    ] {
        let text = l.error_message_for_code(code);
        assert!(!text.trim().is_empty());
    }
    let _ = Args::new();
}

#[test]
fn the_code_to_key_mapping_is_injective_for_the_real_codes() {
    let codes = error_codes();
    let mut keys = std::collections::HashSet::new();
    for (code, _) in &codes {
        assert!(
            keys.insert(key_of(code)),
            "{code} collides with another code"
        );
    }
}
