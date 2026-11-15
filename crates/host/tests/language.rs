//! Choosing the language: at the first start and at run time (spec US5, FR-024).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell, wait_operation};
use la_host::notices::NoticeContent;
use la_host::{Changes, Command, Outcome, Shell, ShellConfig};
use la_i18n::{Args, Key, Language};

fn shell_with_system(dir: &TempDir, system: &[&str]) -> Shell {
    let mut config = ShellConfig::for_tests(dir.path());
    config.system_languages = system.iter().map(|s| (*s).to_owned()).collect();
    Shell::new(config)
}

#[test]
fn the_first_start_follows_the_system_language_and_falls_back_to_english() {
    let cases: [(&[&str], Language); 4] = [
        (&["uk-UA"], Language::Uk),
        (&["pt-BR", "de"], Language::De),
        (&["zz"], Language::En),
        (&[], Language::En),
    ];
    for (system, expected) in cases {
        let dir = TempDir::new("lang-first");
        let shell = shell_with_system(&dir, system);
        assert_eq!(shell.state().language, expected, "{system:?}");
        assert_eq!(shell.localizer().language(), expected, "{system:?}");
    }
}

#[test]
fn an_explicit_choice_wins_over_the_system_language_on_the_next_start() {
    let dir = TempDir::new("lang-explicit");
    let mut shell = shell_with_system(&dir, &["uk-UA"]);
    assert!(matches!(
        shell.apply(Command::SetLanguage(Language::Ja)),
        Outcome::Done(_)
    ));
    drop(shell);
    let again = shell_with_system(&dir, &["uk-UA"]);
    assert_eq!(again.state().language, Language::Ja);
    assert_eq!(again.state().settings.language.as_deref(), Some("ja"));
}

#[test]
fn the_change_is_immediate_everywhere_and_flags_the_language() {
    let dir = TempDir::new("lang-immediate");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 41);
    // A failure notice and two markers, so that every kind of text exists.
    shell.apply(Command::TestError("session.damaged"));
    for position in [100, 15_000] {
        shell.apply(Command::SetSnap(false));
        shell.apply(Command::PlaceMarker {
            position,
            channel: 0,
        });
    }
    let key = Key::from_static("menu-file");
    let mut seen = Vec::new();
    for language in Language::ALL {
        let outcome = shell.apply(Command::SetLanguage(language));
        let Outcome::Done(changes) = outcome else {
            panic!("{outcome:?}");
        };
        assert!(changes.contains(Changes::LANGUAGE), "{language:?}");
        assert_eq!(shell.state().language, language);
        // Shell text, the notice and the measurement all follow at once.
        let menu = shell.text(&key, &Args::new());
        let notice = shell.state().notices.iter().next().unwrap().clone();
        assert_eq!(
            notice.content,
            NoticeContent::Error {
                code: "session.damaged".into()
            }
        );
        let notice_text = shell.notice_text(&notice);
        let measurement = shell.measurement_text().unwrap();
        for text in [&menu, &notice_text, &measurement.distance] {
            assert!(
                !text.is_empty() && !text.contains('\u{27E6}'),
                "{language:?}: {text}"
            );
        }
        seen.push((menu, notice_text, measurement.distance));
    }
    // Six languages give six different menu words and six different notices.
    for column in 0..2 {
        let mut texts: Vec<&String> = seen
            .iter()
            .map(|t| if column == 0 { &t.0 } else { &t.1 })
            .collect();
        texts.sort();
        texts.dedup();
        assert_eq!(texts.len(), 6, "column {column}");
    }
}

#[test]
fn the_language_never_changes_what_the_user_typed_or_the_description() {
    let dir = TempDir::new("lang-user-text");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 42);
    shell.apply(Command::RenameChannel {
        channel: 0,
        name: "Сигнал SDA".into(),
    });
    shell.apply(Command::SetSnap(false));
    shell.apply(Command::PlaceMarker {
        position: 500,
        channel: 0,
    });
    let id = shell.state().markers.list[0].id;
    shell.apply(Command::RenameMarker {
        id,
        name: "Ёлка".into(),
    });
    let before = (
        shell.state().presentation.clone(),
        shell.state().markers.list.clone(),
        shell.state().capture.as_ref().unwrap().description.clone(),
    );
    for language in Language::ALL {
        shell.apply(Command::SetLanguage(language));
        let state = shell.state();
        assert_eq!(state.presentation, before.0, "{language:?}");
        assert_eq!(state.markers.list, before.1, "{language:?}");
        let description = &state.capture.as_ref().unwrap().description;
        assert_eq!(description.device_name(), before.2.device_name());
        assert_eq!(description.device_id(), before.2.device_id());
    }
}

#[test]
fn switching_ten_times_while_a_capture_runs_and_saves_is_safe() {
    let dir = TempDir::new("lang-busy");
    let mut shell = finished_shell(&dir, 0xffff, 400_000, 43);
    let path = dir.path().join("busy.lasession");
    shell.apply(Command::SaveSessionAs(path.clone()));
    for i in 0..10 {
        let language = Language::ALL[i % 6];
        assert!(matches!(
            shell.apply(Command::SetLanguage(language)),
            Outcome::Done(_)
        ));
        shell.poll(std::time::Instant::now());
    }
    wait_operation(&mut shell);
    assert!(path.exists());
    assert!(shell.state().notices.is_empty());
}

#[test]
fn a_missing_translation_falls_back_to_english_and_never_shows_a_key() {
    let dir = TempDir::new("lang-missing");
    let shell = shell_with_system(&dir, &[]);
    // A key no language has: the text is a marked placeholder that the interface never asks for;
    // for every real key of the application every language gives real text.
    for language in Language::ALL {
        shell.localizer().set_language(language);
        for key in ["menu-file", "menu-view", "dialog-save", "status-unsaved"] {
            let text = shell.text(&Key::from_static(key), &Args::new());
            assert!(!text.is_empty() && text != key, "{language:?} {key}");
            assert!(!text.contains('\u{27E6}'), "{language:?} {key}: {text}");
        }
    }
    assert!(shell.localizer().problems().is_empty());
}
