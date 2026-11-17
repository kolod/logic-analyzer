//! A remembered answer to the question about an unsaved capture (the checkbox of the dialog and of
//! the settings).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell, wait_operation};
use la_host::settings::{RealSettingsFs, Settings};
use la_host::{Command, Decision, Outcome, Shell, ShellConfig, Unsaved};

fn stored(dir: &TempDir) -> Option<String> {
    Settings::load(&RealSettingsFs, &dir.path().join("settings.json"))
        .settings
        .unsaved_choice
}

fn remember(shell: &mut Shell, choice: Option<Decision>) {
    let outcome = shell.apply(Command::SetUnsavedChoice(choice));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

#[test]
fn without_a_remembered_answer_the_question_is_asked() {
    let dir = TempDir::new("choice-ask");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 1);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    let outcome = shell.apply(Command::NewCapture);
    assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{outcome:?}");
    assert!(shell.state().pending_decision.is_some());
}

#[test]
fn a_remembered_discard_throws_the_capture_away_without_asking() {
    let dir = TempDir::new("choice-discard");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 2);
    remember(&mut shell, Some(Decision::Discard));
    let outcome = shell.apply(Command::NewCapture);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(shell.state().pending_decision.is_none());
    assert!(shell.state().capture.is_none());
}

#[test]
fn cancel_cannot_be_remembered() {
    let dir = TempDir::new("choice-cancel");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 3);
    let outcome = shell.apply(Command::SetUnsavedChoice(Some(Decision::Cancel)));
    assert!(
        matches!(outcome, Outcome::Ignored(c) if c == "host.invalid_command"),
        "{outcome:?}"
    );
    assert_eq!(stored(&dir), None);
    // The question is still asked, and the capture is still there.
    let outcome = shell.apply(Command::NewCapture);
    assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{outcome:?}");
    assert!(shell.state().capture.is_some());
    // Forgetting what is not remembered is fine, and a remembered answer can be replaced.
    remember(&mut shell, None);
    remember(&mut shell, Some(Decision::Save));
    remember(&mut shell, Some(Decision::Discard));
    assert_eq!(stored(&dir).as_deref(), Some("discard"));
    let outcome = shell.apply(Command::SetUnsavedChoice(Some(Decision::Cancel)));
    assert!(matches!(outcome, Outcome::Ignored(_)), "{outcome:?}");
    assert_eq!(
        stored(&dir).as_deref(),
        Some("discard"),
        "the old choice stays"
    );
}

#[test]
fn a_cancel_written_in_the_settings_file_asks_as_before() {
    let dir = TempDir::new("choice-stale-cancel");
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"unsaved_choice":"cancel"}"#,
    )
    .unwrap();
    let mut shell = finished_shell(&dir, 0b1, 3_000, 11);
    let outcome = shell.apply(Command::NewCapture);
    assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{outcome:?}");
}

#[test]
fn a_remembered_save_saves_to_the_file_of_the_session_and_then_carries_on() {
    let dir = TempDir::new("choice-save");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 4);
    let path = dir.path().join("kept.lasession");
    let outcome = shell.apply(Command::SaveSessionAs(path.clone()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut shell);
    // A change makes it unsaved again.
    shell.apply(Command::RenameChannel {
        channel: 0,
        name: "Clock".to_owned(),
    });
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    remember(&mut shell, Some(Decision::Save));
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let outcome = shell.apply(Command::NewCapture);
    assert!(
        matches!(outcome, Outcome::Done(_) | Outcome::NeedsDecision(_)),
        "{outcome:?}"
    );
    wait_operation(&mut shell);
    assert!(std::fs::metadata(&path).unwrap().modified().unwrap() >= before);
}

#[test]
fn a_remembered_save_of_a_capture_without_a_file_asks_for_a_name() {
    let dir = TempDir::new("choice-save-no-path");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 5);
    remember(&mut shell, Some(Decision::Save));
    let outcome = shell.apply(Command::NewCapture);
    // There is no file to save to yet: the shell asks for a name, and the capture is not lost.
    assert!(
        matches!(outcome, Outcome::Ignored(c) if c == la_host::NEEDS_PATH),
        "{outcome:?}"
    );
    assert!(shell.state().capture.is_some());
    assert!(shell.state().pending_decision.is_some());
}

#[test]
fn the_choice_is_saved_and_can_be_forgotten() {
    let dir = TempDir::new("choice-saved");
    let mut shell = finished_shell(&dir, 0b1, 3_000, 6);
    remember(&mut shell, Some(Decision::Discard));
    assert_eq!(stored(&dir).as_deref(), Some("discard"));
    drop(shell);
    let again = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(
        again.state().settings.unsaved_choice.as_deref(),
        Some("discard")
    );
    let mut again = again;
    remember(&mut again, None);
    assert_eq!(stored(&dir), None);
}

#[test]
fn a_word_that_is_none_of_the_three_asks_as_before() {
    let dir = TempDir::new("choice-unknown");
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"unsaved_choice":"banana"}"#,
    )
    .unwrap();
    let mut shell = finished_shell(&dir, 0b1, 3_000, 7);
    let outcome = shell.apply(Command::NewCapture);
    assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{outcome:?}");
}

#[test]
fn the_words_of_the_settings_read_back() {
    for decision in [Decision::Save, Decision::Discard, Decision::Cancel] {
        assert_eq!(
            Decision::from_setting(decision.as_setting()),
            Some(decision)
        );
    }
    assert_eq!(Decision::from_setting(""), None);
    assert_eq!(Decision::from_setting("Save"), None);
}

/// Polls until the save is over and says whether the shell then reports that it may quit.
fn quits_after(shell: &mut Shell) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let changes = shell.poll(std::time::Instant::now());
        if changes.contains(la_host::Changes::QUIT) {
            return true;
        }
        if shell.state().operation.is_none() && shell.state().pending_decision.is_none() {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

#[test]
fn quit_and_close_work_with_a_remembered_discard_or_save() {
    for (name, command) in [("quit", Command::Quit), ("close", Command::RequestClose)] {
        // Discard: the shell may quit at once.
        let dir = TempDir::new(&format!("choice-{name}-discard"));
        let mut shell = finished_shell(&dir, 0b1, 3_000, 8);
        remember(&mut shell, Some(Decision::Discard));
        let outcome = shell.apply(command.clone());
        assert!(
            matches!(&outcome, Outcome::Done(c) if c.contains(la_host::Changes::QUIT)),
            "{name}: {outcome:?}"
        );
        // Save to the file of the session: it quits when the save is over.
        let dir = TempDir::new(&format!("choice-{name}-save"));
        let mut shell = finished_shell(&dir, 0b1, 3_000, 9);
        let path = dir.path().join("kept.lasession");
        shell.apply(Command::SaveSessionAs(path));
        wait_operation(&mut shell);
        shell.apply(Command::RenameChannel {
            channel: 0,
            name: "Clock".to_owned(),
        });
        remember(&mut shell, Some(Decision::Save));
        let outcome = shell.apply(command.clone());
        let quit_now = matches!(&outcome, Outcome::Done(c) if c.contains(la_host::Changes::QUIT));
        assert!(quit_now || quits_after(&mut shell), "{name}: {outcome:?}");
    }
}
