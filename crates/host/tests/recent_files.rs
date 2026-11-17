//! The list of recent files: fifteen, newest first, with the files that are gone marked
//! (spec 014, user story 5, task T041).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;

use common::{TempDir, finished_shell, wait_operation};
use la_host::settings::{MAX_RECENT_FILES, RealSettingsFs, Settings};
use la_host::{Command, Outcome, Shell, ShellConfig};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

/// A shell that has saved `count` sessions, `f0` first.
fn saved(dir: &TempDir, count: usize) -> (Shell, Vec<PathBuf>) {
    let mut shell = finished_shell(dir, 0b1, 3_000, 5);
    let mut paths = Vec::new();
    for i in 0..count {
        let path = dir.path().join(format!("f{i}.lasession"));
        done(&mut shell, Command::SaveSessionAs(path.clone()));
        wait_operation(&mut shell);
        paths.push(path);
    }
    (shell, paths)
}

fn stored(dir: &TempDir) -> Vec<String> {
    Settings::load(&RealSettingsFs, &dir.path().join("settings.json"))
        .settings
        .recent_files
}

fn listed(shell: &Shell) -> Vec<PathBuf> {
    shell.recent_files().into_iter().map(|r| r.path).collect()
}

#[test]
fn fifteen_files_are_kept_newest_first() {
    assert_eq!(MAX_RECENT_FILES, 15);
    let dir = TempDir::new("recent-15");
    let (shell, paths) = saved(&dir, 16);
    let recent = shell.recent_files();
    assert_eq!(recent.len(), 15);
    assert_eq!(recent[0].path, paths[15]);
    assert_eq!(recent[14].path, paths[1]);
    assert!(recent.iter().all(|r| r.exists));
    assert_eq!(stored(&dir).len(), 15);
}

#[test]
fn using_an_old_file_again_moves_it_to_the_top_without_a_duplicate() {
    let dir = TempDir::new("recent-reuse");
    let (mut shell, paths) = saved(&dir, 4);
    done(&mut shell, Command::OpenSession(paths[1].clone()));
    wait_operation(&mut shell);
    assert_eq!(
        listed(&shell),
        vec![
            paths[1].clone(),
            paths[3].clone(),
            paths[2].clone(),
            paths[0].clone()
        ]
    );
}

#[test]
fn a_file_that_is_gone_stays_in_the_list_marked_missing() {
    let dir = TempDir::new("recent-missing");
    let (shell, paths) = saved(&dir, 3);
    std::fs::remove_file(&paths[2]).unwrap();
    let recent = shell.recent_files();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].path, paths[2]);
    assert!(!recent[0].exists);
    assert!(recent[1].exists && recent[2].exists);
}

#[test]
fn one_entry_can_be_removed_and_one_that_is_not_listed_is_ignored() {
    let dir = TempDir::new("recent-remove");
    let (mut shell, paths) = saved(&dir, 3);
    done(&mut shell, Command::RemoveRecentFile(paths[1].clone()));
    assert_eq!(listed(&shell), vec![paths[2].clone(), paths[0].clone()]);
    assert_eq!(stored(&dir).len(), 2, "saved to the settings");
    // The file itself is not touched.
    assert!(paths[1].exists());
    let before = shell.recent_files();
    let outcome = shell.apply(Command::RemoveRecentFile(
        dir.path().join("other.lasession"),
    ));
    assert!(
        matches!(outcome, Outcome::Ignored(c) if c == "host.invalid_command"),
        "ignored"
    );
    assert_eq!(shell.recent_files(), before);
}

#[test]
fn the_list_can_be_cleared() {
    let dir = TempDir::new("recent-clear");
    let (mut shell, _) = saved(&dir, 3);
    done(&mut shell, Command::ClearRecentFiles);
    assert!(shell.recent_files().is_empty());
    assert!(stored(&dir).is_empty());
    // An empty list is cleared without complaint.
    done(&mut shell, Command::ClearRecentFiles);
}

#[test]
fn a_settings_file_with_thirty_entries_is_cut_to_fifteen_on_load() {
    let dir = TempDir::new("recent-30");
    let names: Vec<String> = (0..30).map(|i| format!("\"/f{i}.lasession\"")).collect();
    std::fs::write(
        dir.path().join("settings.json"),
        format!(r#"{{"version":1,"recent_files":[{}]}}"#, names.join(",")),
    )
    .unwrap();
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let recent = shell.recent_files();
    assert_eq!(recent.len(), 15);
    assert_eq!(recent[0].path, PathBuf::from("/f0.lasession"));
    assert!(recent.iter().all(|r| !r.exists));
}
