//! The view of the panel at the right edge: the command, the settings and the start-up
//! (spec 014, user story 2, task T016).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::TempDir;
use la_host::settings::{RealSettingsFs, Settings};
use la_host::{Command, Outcome, Shell, ShellConfig, SidePanel};

fn stored(dir: &TempDir) -> Settings {
    Settings::load(&RealSettingsFs, &dir.path().join("settings.json")).settings
}

fn start(dir: &TempDir) -> Shell {
    Shell::new(ShellConfig::for_tests(dir.path()))
}

#[test]
fn the_default_is_the_capture_view() {
    let dir = TempDir::new("panel-default");
    let shell = start(&dir);
    assert_eq!(shell.state().side_panel, SidePanel::Capture);
}

#[test]
fn every_view_can_be_chosen_without_a_capture_and_is_saved() {
    let dir = TempDir::new("panel-set");
    let mut shell = start(&dir);
    assert!(shell.state().capture.is_none());
    for (view, text) in [
        (SidePanel::Decoders, "decoders"),
        (SidePanel::Markers, "markers"),
        (SidePanel::Hidden, "hidden"),
        (SidePanel::Capture, "capture"),
    ] {
        let outcome = shell.apply(Command::SetSidePanel(view));
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
        assert_eq!(shell.state().side_panel, view);
        assert_eq!(stored(&dir).side_panel.as_deref(), Some(text));
    }
}

#[test]
fn the_choice_is_there_at_the_next_start() {
    let dir = TempDir::new("panel-restart");
    let mut shell = start(&dir);
    shell.apply(Command::SetSidePanel(SidePanel::Markers));
    drop(shell);
    assert_eq!(start(&dir).state().side_panel, SidePanel::Markers);
    let mut shell = start(&dir);
    shell.apply(Command::SetSidePanel(SidePanel::Hidden));
    drop(shell);
    assert_eq!(start(&dir).state().side_panel, SidePanel::Hidden);
}

#[test]
fn an_unknown_value_reads_as_the_default_and_a_wrong_type_does_not_stop_the_start() {
    let dir = TempDir::new("panel-unknown");
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"side_panel":"sideways"}"#,
    )
    .unwrap();
    assert_eq!(start(&dir).state().side_panel, SidePanel::Capture);
    // A value of the wrong type makes the file unusable as a whole (the settings rule): the
    // defaults apply, the application starts and says so.
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"side_panel":7}"#,
    )
    .unwrap();
    let shell = start(&dir);
    assert_eq!(shell.state().side_panel, SidePanel::Capture);
    assert!(!shell.state().notices.is_empty());
}

#[test]
fn a_field_of_a_newer_version_is_kept_when_the_choice_is_saved() {
    let dir = TempDir::new("panel-future");
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"side_panel":"markers","future_field":{"a":[1,2]}}"#,
    )
    .unwrap();
    let mut shell = start(&dir);
    assert_eq!(shell.state().side_panel, SidePanel::Markers);
    shell.apply(Command::SetSidePanel(SidePanel::Decoders));
    let text = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(text.contains("future_field"), "{text}");
    assert!(text.contains("\"decoders\""), "{text}");
}
