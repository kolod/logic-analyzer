//! The text of the status line: activity, counts, memory, end reasons (spec US1, FR-004/005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod common;

use std::time::{Duration, Instant};

use common::TempDir;
use la_host::status::{memory_text, samples_text, status_line};
use la_host::{Activity, Command, ConfigEdit, Shell, ShellConfig};
use la_i18n::{Language, Localizer};

#[test]
fn sample_counts_use_the_plural_forms_and_separators_of_the_language() {
    let english = Localizer::new(Language::En);
    assert_eq!(samples_text(&english, 1), "1 sample");
    assert_eq!(samples_text(&english, 0), "0 samples");
    assert_eq!(samples_text(&english, 21), "21 samples");
    assert_eq!(samples_text(&english, 1_234_567), "1,234,567 samples");

    let ukrainian = Localizer::new(Language::Uk);
    assert_eq!(samples_text(&ukrainian, 1), "1 відлік");
    assert_eq!(samples_text(&ukrainian, 2), "2 відліки");
    assert_eq!(samples_text(&ukrainian, 5), "5 відліків");
    assert_eq!(samples_text(&ukrainian, 21), "21 відлік");
    assert_eq!(samples_text(&ukrainian, 25), "25 відліків");

    // Japanese has a single form.
    let japanese = Localizer::new(Language::Ja);
    assert_eq!(
        samples_text(&japanese, 1),
        samples_text(&japanese, 2).replace('2', "1")
    );
}

#[test]
fn memory_is_shown_as_used_over_limit() {
    let english = Localizer::new(Language::En);
    assert_eq!(
        memory_text(&english, 31_981_568, 1_073_741_824),
        "30.5\u{a0}MiB / 1\u{a0}GiB"
    );
    assert_eq!(memory_text(&english, 0, 1_048_576), "0\u{a0}B / 1\u{a0}MiB");
}

#[test]
fn an_idle_shell_has_only_the_activity() {
    let dir = TempDir::new("status-idle");
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let line = shell.status_line();
    assert_eq!(line.activity, "Idle");
    assert_eq!(line.samples, None);
    assert_eq!(line.end_reason, None);
    assert_eq!(line.memory, None);
    assert!(!line.simulated && !line.incomplete && !line.unsaved);
}

#[test]
fn the_activity_names_follow_the_language() {
    let dir = TempDir::new("status-activity");
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let mut state = shell.state().clone();
    let english = Localizer::new(Language::En);
    let names = [
        (Activity::Idle, "Idle"),
        (Activity::Capturing, "Capturing"),
        (Activity::Saving, "Saving"),
        (Activity::Loading, "Loading"),
    ];
    for (activity, expected) in names {
        state.activity = activity;
        assert_eq!(status_line(&state, &english).activity, expected);
    }
    // Every language gives a text for every activity (not the key, not empty).
    for language in Language::ALL {
        let localizer = Localizer::new(language);
        for (activity, _) in names {
            state.activity = activity;
            let text = status_line(&state, &localizer).activity;
            assert!(
                !text.is_empty() && !text.starts_with("status-"),
                "{language:?} {text}"
            );
        }
    }
}

#[test]
fn a_finished_capture_shows_samples_end_reason_rate_and_flags() {
    let dir = TempDir::new("status-capture");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(0b1),
        ConfigEdit::SampleRate(1000),
        ConfigEdit::Length(21),
        ConfigEdit::Seed(Some(1)),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::StartCapture);
    let deadline = Instant::now() + Duration::from_secs(10);
    while shell.state().activity != Activity::Idle {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let line = shell.status_line();
    assert_eq!(line.activity, "Idle");
    assert_eq!(line.samples.as_deref(), Some("21 samples"));
    assert_eq!(line.end_reason.as_deref(), Some("Capture completed"));
    assert_eq!(line.rate.as_deref(), Some("1\u{a0}kHz"));
    assert!(line.memory.is_some());
    assert!(line.simulated);
    assert!(!line.incomplete);
    assert!(line.unsaved);

    shell.localizer().set_language(Language::Uk);
    let line = shell.status_line();
    assert_eq!(line.samples.as_deref(), Some("21 відлік"));
    assert_ne!(line.end_reason.as_deref(), Some("Capture completed"));
}
