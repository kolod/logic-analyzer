//! The table of commands, menu paths and shortcuts (contract `shell-layout.md`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod common;

use common::TempDir;
use la_host::shortcuts::{Accelerator, EnabledWhen, table};
use la_host::{Shell, ShellConfig};
use la_i18n::{Args, Key, Language, Localizer};

#[test]
fn every_accelerator_is_used_once() {
    let entries = table();
    assert!(!entries.is_empty());
    let mut seen: Vec<Accelerator> = Vec::new();
    for entry in &entries {
        if let Some(accelerator) = entry.accelerator {
            assert!(
                !seen.contains(&accelerator),
                "{accelerator:?} is used twice"
            );
            seen.push(accelerator);
        }
    }
}

#[test]
fn every_menu_path_key_has_a_message_in_every_language() {
    for language in Language::ALL {
        let localizer = Localizer::new(language);
        for entry in table() {
            for key in entry.menu {
                let text = localizer.message(&Key::from_static(key), &Args::new());
                assert!(
                    !text.is_empty() && text != key && !text.contains('\u{27E6}'),
                    "{language:?} {key}: {text}"
                );
            }
        }
        assert!(
            localizer.problems().is_empty(),
            "{:?}",
            localizer.problems()
        );
    }
}

#[test]
fn entries_are_enabled_according_to_the_state() {
    let dir = TempDir::new("shortcuts");
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let state = shell.state();
    assert!(EnabledWhen::Always.is_enabled(state));
    assert!(EnabledWhen::Idle.is_enabled(state));
    assert!(EnabledWhen::CanStart.is_enabled(state));
    assert!(!EnabledWhen::Capturing.is_enabled(state));
    assert!(!EnabledWhen::CaptureExists.is_enabled(state));
}

#[test]
fn menus_keep_the_same_name_across_their_entries() {
    // The first key of a path names the menu: an entry of another menu has another first key.
    let entries = table();
    for entry in &entries {
        assert!(entry.menu[0].starts_with("menu-"));
        assert!(entry.menu[1].starts_with(entry.menu[0]));
    }
}
