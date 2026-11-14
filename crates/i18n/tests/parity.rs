//! The translation parity check (spec US6, FR-023, FR-024, SC-004, SC-009).
//!
//! The real resources must pass; a small correct set of resources and sources is built in a
//! temporary directory and broken in one way per defect class, and the check must report exactly
//! that defect.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::workspace_root;
use la_i18n::Language;
use la_i18n::parity::{DefectKind, Report, check, registered_files};

/// A resource and source set that passes the check.
#[derive(Clone)]
struct Fixture {
    /// (language tag, file name) to text.
    resources: BTreeMap<(String, String), String>,
    /// File name to text, under `src/`.
    sources: BTreeMap<String, String>,
    registered: Vec<String>,
}

const APP: &str = "app.ftl";

fn app(language: Language) -> String {
    let (one, few, many, other, greet, broken) = match language {
        Language::En => ("thing", "", "", "things", "Hi { $name }!", "Broken."),
        Language::De => ("Ding", "", "", "Dinge", "Hallo { $name }!", "Kaputt."),
        Language::Fr => ("chose", "", "", "choses", "Salut { $name } !", "Cassé."),
        Language::Es => ("cosa", "", "", "cosas", "Hola { $name }!", "Roto."),
        Language::Ja => ("", "", "", "個", "こんにちは { $name }", "壊れています。"),
        Language::Uk => (
            "річ",
            "речі",
            "речей",
            "речі",
            "Привіт, { $name }!",
            "Зламано.",
        ),
    };
    let counted = match language {
        Language::Ja => format!("count-things = {{ FORMAT_NUMBER($count) }}{other}\n"),
        Language::Uk => format!(
            "count-things =\n    {{ $count ->\n        [one] {{ FORMAT_NUMBER($count) }} {one}\n        [few] {{ FORMAT_NUMBER($count) }} {few}\n        [many] {{ FORMAT_NUMBER($count) }} {many}\n       *[other] {{ FORMAT_NUMBER($count) }} {other}\n    }}\n"
        ),
        _ => format!(
            "count-things =\n    {{ $count ->\n        [one] {{ FORMAT_NUMBER($count) }} {one}\n       *[other] {{ FORMAT_NUMBER($count) }} {other}\n    }}\n"
        ),
    };
    format!(
        "### status: draft\n\nhello = Hello\n\n# A counted message.\n{counted}\ngreet = {greet}\n\nerror-demo-broken = {broken}\n"
    )
}

fn good() -> Fixture {
    let mut resources = BTreeMap::new();
    for language in Language::ALL {
        let mut text = app(language);
        if language != Language::En {
            text = text.replace("hello = Hello", "hello = Hello in another language");
        }
        resources.insert((language.tag().to_owned(), APP.to_owned()), text);
    }
    let mut sources = BTreeMap::new();
    sources.insert(
        "lib.rs".to_owned(),
        "use la_i18n::key;\nfn demo() {\n    let _ = key!(\"hello\");\n    let _ = key!(\"count-things\");\n    let _ = key!(\"greet\");\n}\n"
            .to_owned(),
    );
    sources.insert(
        "error.rs".to_owned(),
        "pub const BROKEN: &str = \"demo.broken\";\n".to_owned(),
    );
    Fixture {
        resources,
        sources,
        registered: vec![APP.to_owned()],
    }
}

impl Fixture {
    fn file_mut(&mut self, language: Language) -> &mut String {
        self.resources
            .get_mut(&(language.tag().to_owned(), APP.to_owned()))
            .unwrap()
    }

    fn run(&self) -> Report {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("la-i18n-parity-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for ((language, file), text) in &self.resources {
            let dir = root.join("resources").join(language);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(file), text).unwrap();
        }
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        for (file, text) in &self.sources {
            std::fs::write(src.join(file), text).unwrap();
        }
        let report = check(&root.join("resources"), &[src], &self.registered);
        let _ = std::fs::remove_dir_all(&root);
        report
    }
}

/// The defects as (kind, language tag, key), which is what each fixture is judged by.
fn summary(report: &Report) -> BTreeSet<(DefectKind, String, String)> {
    report
        .defects
        .iter()
        .map(|d| {
            (
                d.kind,
                d.language.map_or("-", Language::tag).to_owned(),
                d.key.clone(),
            )
        })
        .collect()
}

fn expect(report: &Report, wanted: &[(DefectKind, &str, &str)]) {
    let got = summary(report);
    let want: BTreeSet<(DefectKind, String, String)> = wanted
        .iter()
        .map(|(k, l, key)| (*k, (*l).to_owned(), (*key).to_owned()))
        .collect();
    assert_eq!(got, want, "report:\n{}", report.render());
}

#[test]
fn the_correct_fixture_passes() {
    let report = good().run();
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn a_key_missing_in_a_language_is_reported() {
    let mut f = good();
    let text = f
        .file_mut(Language::Ja)
        .replace("greet = こんにちは { $name }\n", "");
    *f.file_mut(Language::Ja) = text;
    expect(&f.run(), &[(DefectKind::MissingKey, "ja", "greet")]);
}

#[test]
fn a_key_that_english_lacks_is_reported() {
    let mut f = good();
    f.file_mut(Language::Fr).push_str("\nextra = x\n");
    expect(&f.run(), &[(DefectKind::ExtraKey, "fr", "extra")]);
}

#[test]
fn an_english_key_that_no_code_uses_is_reported_unless_it_is_reserved() {
    let mut f = good();
    for language in Language::ALL {
        f.file_mut(language).push_str("\nlonely = L\n");
    }
    expect(&f.run(), &[(DefectKind::UnusedKey, "en", "lonely")]);

    let mut f = good();
    for language in Language::ALL {
        f.file_mut(language).push_str("\n# @reserved\nfuture = F\n");
    }
    let report = f.run();
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn a_key_used_by_code_but_defined_nowhere_is_reported_with_file_and_line() {
    let mut f = good();
    f.sources.insert(
        "lib.rs".to_owned(),
        "use la_i18n::key;\nfn demo() {\n    let _ = key!(\"hello\");\n    let _ = key!(\"count-things\");\n    let _ = key!(\"greet\");\n    let _ = key!(\"missing-everywhere\");\n}\n"
            .to_owned(),
    );
    let report = f.run();
    expect(
        &report,
        &[(DefectKind::UndefinedKey, "-", "missing-everywhere")],
    );
    let defect = &report.defects[0];
    assert!(defect.file.ends_with("lib.rs"), "{}", defect.file);
    assert_eq!(defect.line, Some(6));
}

#[test]
fn an_error_code_without_a_message_is_reported() {
    let mut f = good();
    f.sources.insert(
        "error.rs".to_owned(),
        "pub const BROKEN: &str = \"demo.broken\";\npub const OTHER: &str = \"demo.other\";\n"
            .to_owned(),
    );
    expect(
        &f.run(),
        &[(DefectKind::UndefinedKey, "-", "error-demo-other")],
    );
}

#[test]
fn a_renamed_placeholder_is_reported() {
    let mut f = good();
    let text = f.file_mut(Language::De).replace("{ $name }", "{ $nom }");
    *f.file_mut(Language::De) = text;
    expect(
        &f.run(),
        &[(DefectKind::PlaceholderMismatch, "de", "greet")],
    );
}

#[test]
fn a_missing_plural_category_is_reported() {
    let mut f = good();
    let text = f
        .file_mut(Language::Uk)
        .replace("        [few] { FORMAT_NUMBER($count) } речі\n", "");
    *f.file_mut(Language::Uk) = text;
    expect(
        &f.run(),
        &[(DefectKind::MissingPluralCategory, "uk", "count-things")],
    );
    // A French message may also carry the `many` form the library does not select yet.
    let mut f = good();
    let text = f.file_mut(Language::Fr).replace(
        "       *[other]",
        "        [many] { FORMAT_NUMBER($count) } de choses\n       *[other]",
    );
    *f.file_mut(Language::Fr) = text;
    let report = f.run();
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn a_count_shown_without_the_number_function_is_reported() {
    let mut f = good();
    let text = f.file_mut(Language::Fr).replace(
        "[one] { FORMAT_NUMBER($count) } chose",
        "[one] { $count } chose",
    );
    *f.file_mut(Language::Fr) = text;
    expect(&f.run(), &[(DefectKind::RawNumber, "fr", "count-things")]);
}

#[test]
fn a_duplicate_key_is_reported() {
    let mut f = good();
    f.file_mut(Language::En).push_str("\nhello = Hello again\n");
    expect(&f.run(), &[(DefectKind::DuplicateKey, "en", "hello")]);
}

#[test]
fn an_empty_message_is_reported() {
    let mut f = good();
    for language in Language::ALL {
        f.file_mut(language)
            .push_str("\n# @reserved\nblank = { \" \" }\n");
    }
    let report = f.run();
    let kinds = summary(&report);
    assert_eq!(kinds.len(), 6, "{}", report.render());
    assert!(
        kinds
            .iter()
            .all(|(k, _, key)| *k == DefectKind::EmptyMessage && key == "blank")
    );
}

#[test]
fn a_syntax_error_is_reported_with_its_line() {
    let mut f = good();
    f.file_mut(Language::En)
        .push_str("\nbroken = { $unterminated\n");
    let report = f.run();
    assert!(
        report
            .defects
            .iter()
            .all(|d| d.kind == DefectKind::SyntaxError && d.language == Some(Language::En)),
        "{}",
        report.render()
    );
    assert!(!report.defects.is_empty());
    let text = f.resources[&("en".to_owned(), APP.to_owned())].clone();
    let broken_line = text.lines().position(|l| l.starts_with("broken")).unwrap() + 1;
    let line = report.defects[0].line.unwrap();
    assert!(
        (broken_line..=broken_line + 1).contains(&line),
        "line {line}, broken is at {broken_line}: {}",
        report.render()
    );
}

#[test]
fn a_file_that_is_not_registered_is_reported() {
    let mut f = good();
    f.resources.insert(
        ("en".to_owned(), "stray.ftl".to_owned()),
        "### status: draft\nstray = S\n".to_owned(),
    );
    let report = f.run();
    assert!(
        report
            .defects
            .iter()
            .any(|d| d.kind == DefectKind::UnregisteredFile && d.file.ends_with("stray.ftl")),
        "{}",
        report.render()
    );
}

#[test]
fn a_registered_file_missing_for_a_language_is_reported() {
    let mut f = good();
    f.resources.remove(&("es".to_owned(), APP.to_owned()));
    let report = f.run();
    let kinds: BTreeSet<DefectKind> = report.defects.iter().map(|d| d.kind).collect();
    assert!(
        kinds.contains(&DefectKind::MissingFile),
        "{}",
        report.render()
    );
    assert!(
        kinds.is_subset(&BTreeSet::from([
            DefectKind::MissingFile,
            DefectKind::MissingKey
        ])),
        "{}",
        report.render()
    );
    assert!(
        report
            .defects
            .iter()
            .all(|d| d.language == Some(Language::Es))
    );
}

#[test]
fn a_file_without_the_status_header_is_reported() {
    let mut f = good();
    let text = f.file_mut(Language::De).replace("### status: draft\n", "");
    *f.file_mut(Language::De) = text;
    expect(&f.run(), &[(DefectKind::MissingStatusHeader, "de", "")]);
}

#[test]
fn windows_line_endings_are_reported() {
    let mut f = good();
    let text = f.file_mut(Language::De).replace('\n', "\r\n");
    *f.file_mut(Language::De) = text;
    expect(&f.run(), &[(DefectKind::LineEndings, "de", "")]);
}

#[test]
fn the_report_is_stable_and_names_file_key_and_kind() {
    let mut f = good();
    f.file_mut(Language::Fr).push_str("\nextra = x\n");
    let (a, b) = (f.run().render(), f.run().render());
    assert_eq!(a, b);
    assert!(
        a.contains("extra") && a.contains("ExtraKey") && a.contains("[fr]"),
        "{a}"
    );
}

#[test]
fn the_real_resources_and_sources_pass_in_under_ten_seconds() {
    let root = workspace_root();
    let started = Instant::now();
    let report = check(
        &root.join("resources/i18n"),
        &[root.join("crates")],
        &registered_files(),
    );
    let elapsed = started.elapsed();
    assert!(
        report.is_clean(),
        "the translations are not in parity:\n{}",
        report.render()
    );
    let limit = if cfg!(debug_assertions) {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(10)
    };
    assert!(elapsed < limit, "the check took {elapsed:?}");
    let _ = PathBuf::new();
}

#[test]
fn a_placeholder_renamed_only_inside_a_plural_variant_is_reported() {
    let mut f = good();
    let text = f
        .file_mut(Language::De)
        .replace("[one] { FORMAT_NUMBER($count) } Ding", "[one] { FORMAT_NUMBER($anzahl) } Ding");
    *f.file_mut(Language::De) = text;
    expect(&f.run(), &[(DefectKind::PlaceholderMismatch, "de", "count-things")]);
}
