//! Every command a person can trigger is reachable from the keyboard (spec FR-036, task T076).
//!
//! The variants of `Command` are read from its source, so a new command cannot be forgotten: it
//! must be reachable from the menu and shortcut table, or be listed below with the reason why not.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::BTreeSet;

use la_host::shortcuts::{Action, table};

/// The variant names of `pub enum Command`.
fn command_variants() -> BTreeSet<String> {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/command.rs"),
    )
    .unwrap();
    let start = source.find("pub enum Command {").expect("the enum");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("the end of the enum");
    body[..end]
        .lines()
        .filter(|l| l.starts_with("    ") && !l.starts_with("     "))
        .filter_map(|l| {
            let name: String = l
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            name.chars()
                .next()
                .filter(char::is_ascii_uppercase)
                .map(|_| name)
        })
        .collect()
}

/// Commands the menu and shortcut table reaches, by variant name.
fn table_commands() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in table() {
        let names: &[&str] = match &entry.action {
            Action::Command(command) => {
                let debug = format!("{command:?}");
                let name: String = debug
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                set.insert(name);
                &[]
            }
            Action::OpenFile => &["OpenSession"],
            Action::SaveFile => &["SaveSession"],
            Action::SaveFileAs => &["SaveSessionAs"],
            Action::ToggleFollowLatest => &["SetFollowLatest"],
            Action::PlaceMarker => &["PlaceMarker"],
            Action::PreviousMarker | Action::NextMarker => &["GoToMarker"],
            Action::RemoveSelectedMarker => &["RemoveMarker"],
            Action::ToggleSnap => &["SetSnap"],
            Action::AddDecoder => &["AddDecoder"],
            Action::UiScaleUp | Action::UiScaleDown => &["SetUiScale"],
            // The dialogs reach the settings commands; see `SETTINGS_DIALOG`.
            Action::Settings | Action::About | Action::ShowLog => &[],
        };
        set.extend(names.iter().map(|n| (*n).to_owned()));
    }
    set
}

/// Reached through the settings dialog, which the table opens.
const SETTINGS_DIALOG: [&str; 4] = [
    "SetLanguage",
    "SetTheme",
    "SetMemoryLimit",
    "SetDecoderMemoryLimit",
];

/// Controls of the side panels and the toolbar: reachable with Tab and the keyboard in them.
const FOCUSABLE_CONTROLS: [&str; 24] = [
    "RefreshDevices",
    "SelectDevice",
    "EditConfig",
    "RenameChannel",
    "SetChannelColour",
    "SetChannelVisible",
    "MoveChannel",
    "ResetPresentation",
    "RenameMarker",
    "SelectMarkers",
    "DismissNotice",
    "DismissAllNotices",
    "CancelOperation",
    "ConfigureDecoder",
    "SetDecoderChannel",
    "SetDecoderOption",
    "RenameDecoder",
    "ShowDecoderLane",
    "RemoveDecoder",
    "RunDecoder",
    "StackDecoder",
    "CancelDecoder",
    "SelectAnnotation",
    "ClearAnnotationSelection",
];

/// The answer buttons of a question: Enter and Tab in the dialog.
const DIALOG_ANSWERS: [&str; 1] = ["Decide"];

/// Pointer gestures. Each has a menu or shortcut equivalent: wheel zoom and drag pan are
/// `ZoomStep` and `PanStep`; dragging a marker is removing it and placing another.
const POINTER_WITH_EQUIVALENT: [(&str, &str); 4] = [
    ("Zoom", "ZoomStep"),
    ("Pan", "PanStep"),
    ("ZoomToRange", "FitAll and ZoomStep"),
    ("MoveMarker", "RemoveMarker and PlaceMarker"),
];

/// Not something a person triggers: the window reports, the close button (the same as Quit) and
/// the commands that exist only to provoke failures in tests.
const NOT_USER_TRIGGERED: [&str; 5] = [
    "SetViewSize",
    "SetWindowGeometry",
    "RequestClose",
    "TestPanic",
    "TestError",
];

#[test]
fn every_command_is_reachable_and_the_lists_name_no_command_that_does_not_exist() {
    let variants = command_variants();
    assert!(
        variants.len() >= 40,
        "found only {} variants",
        variants.len()
    );
    let from_table = table_commands();
    let mut classified: BTreeSet<String> = BTreeSet::new();
    for group in [
        SETTINGS_DIALOG.to_vec(),
        FOCUSABLE_CONTROLS.to_vec(),
        DIALOG_ANSWERS.to_vec(),
        NOT_USER_TRIGGERED.to_vec(),
        POINTER_WITH_EQUIVALENT.iter().map(|(c, _)| *c).collect(),
    ] {
        for name in group {
            assert!(
                variants.contains(name),
                "{name} is not a command (stale list)"
            );
            assert!(
                classified.insert(name.to_owned()),
                "{name} is in more than one list"
            );
        }
    }
    for name in &from_table {
        assert!(
            variants.contains(name),
            "the table sends {name}, which is not a command"
        );
    }
    let unreachable: Vec<&String> = variants
        .iter()
        .filter(|v| !from_table.contains(*v) && !classified.contains(*v))
        .collect();
    assert!(
        unreachable.is_empty(),
        "commands neither in the shortcut table nor in an explicit list: {unreachable:?}"
    );
    // The equivalents named for the pointer gestures exist.
    for (_, equivalent) in POINTER_WITH_EQUIVALENT {
        let first = equivalent.split(" and ").next().unwrap();
        assert!(from_table.contains(first), "{first} is not in the table");
    }
}

#[test]
fn accelerators_are_unique_and_avoid_what_the_desktops_keep_for_themselves() {
    let entries = table();
    let mut seen = Vec::new();
    for entry in &entries {
        let Some(a) = entry.accelerator else { continue };
        assert!(!seen.contains(&a), "{a:?} is used twice");
        seen.push(a);
        let key = a.key.to_ascii_lowercase();
        // Windows and the Debian desktops (GNOME, KDE, Xfce) keep these.
        let reserved = (a.alt && matches!(key.as_str(), "f4" | "tab" | "space" | "escape" | "esc"))
            || (a.ctrl && matches!(key.as_str(), "escape" | "esc") && !a.alt)
            || (a.ctrl
                && a.alt
                && (matches!(
                    key.as_str(),
                    "delete" | "t" | "l" | "left" | "right" | "up" | "down"
                ) || key.starts_with('f')))
            || (a.alt && key.starts_with('f') && key.len() <= 3);
        assert!(!reserved, "{a:?} is reserved by the system");
    }
    assert!(seen.len() >= 20, "{} accelerators", seen.len());
}

#[test]
fn the_settings_and_about_entries_exist() {
    let actions: Vec<Action> = table().into_iter().map(|e| e.action).collect();
    for needed in [
        Action::Settings,
        Action::About,
        Action::UiScaleUp,
        Action::UiScaleDown,
    ] {
        assert!(actions.contains(&needed), "{needed:?}");
    }
}
