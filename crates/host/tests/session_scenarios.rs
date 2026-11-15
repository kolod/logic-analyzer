//! Saving and opening through the shell (spec US4, FR-018..FR-022).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::{SlowFs, TempDir, assert_same_snapshot, finished_shell, temp_files, wait_operation};
use la_host::notices::NoticeContent;
use la_host::presentation::Colour;
use la_host::{Command, Outcome, Shell, ShellConfig, Unsaved};

fn done(shell: &mut Shell, command: Command) {
    match shell.apply(command) {
        Outcome::Done(_) => {}
        other => panic!("{other:?}"),
    }
}

fn error_codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Error { code } => Some(code.clone()),
            NoticeContent::Info { .. } => None,
        })
        .collect()
}

fn load(path: &std::path::Path) -> la_session::Session {
    la_session::load_blocking(
        path,
        &la_session::LoadLimits::default(),
        &la_session::Control::new(),
    )
    .unwrap()
}

/// A capture with renamed channels, a colour, a hidden channel, a new order, two markers and a
/// zoomed window.
fn decorated_shell(dir: &TempDir, seed: u64) -> Shell {
    let mut shell = finished_shell(dir, 0b1111, 200_000, seed);
    done(&mut shell, Command::SetSnap(false));
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 1,
            name: "SDA".into(),
        },
    );
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 3,
            name: "Часы".into(),
        },
    );
    done(
        &mut shell,
        Command::SetChannelColour {
            channel: 2,
            colour: Colour::Rgb(10, 200, 30),
        },
    );
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 0,
            visible: false,
        },
    );
    done(&mut shell, Command::MoveChannel { from: 3, to: 0 });
    for position in [50_000, 120_000] {
        done(
            &mut shell,
            Command::PlaceMarker {
                position,
                channel: 1,
            },
        );
    }
    let first = shell.state().markers.list[0].id;
    done(
        &mut shell,
        Command::RenameMarker {
            id: first,
            name: "start".into(),
        },
    );
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 40_000,
            end: 90_000,
        },
    );
    shell
}

#[test]
fn a_saved_capture_opens_exactly_as_it_was() {
    let dir = TempDir::new("sess-roundtrip");
    let mut shell = decorated_shell(&dir, 5);
    let path = dir.path().join("a.lasession");
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert!(error_codes(&shell).is_empty(), "{:?}", error_codes(&shell));
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
    assert_eq!(shell.state().session_path.as_deref(), Some(path.as_path()));

    let other = TempDir::new("sess-roundtrip-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path.clone()));
    wait_operation(&mut fresh);
    assert!(error_codes(&fresh).is_empty(), "{:?}", error_codes(&fresh));

    let (a, b) = (shell.state(), fresh.state());
    let (ca, cb) = (a.capture.as_ref().unwrap(), b.capture.as_ref().unwrap());
    assert_same_snapshot(&ca.snapshot, &cb.snapshot);
    assert_eq!(ca.description.device_id(), cb.description.device_id());
    assert_eq!(ca.description.device_name(), cb.description.device_name());
    assert_eq!(ca.description.seed(), cb.description.seed());
    assert_eq!(
        ca.description.sample_rate_hz(),
        cb.description.sample_rate_hz()
    );
    assert_eq!(ca.description.enabled_mask(), cb.description.enabled_mask());
    assert!(cb.simulated, "the simulated mark survives");
    assert_eq!(ca.incomplete, cb.incomplete);
    assert_eq!(a.presentation, b.presentation);
    assert_eq!(a.markers.list, b.markers.list);
    assert_eq!(a.markers.selected, b.markers.selected);
    assert_eq!(a.window, b.window);
    assert!(!b.auto_fit);
    assert_eq!(b.unsaved, Unsaved::Clean);
    assert_eq!(b.session_path.as_deref(), Some(path.as_path()));
    assert_eq!(fresh.recent_files(), [path]);
}

#[test]
fn a_capture_with_the_default_look_has_no_view_section() {
    let dir = TempDir::new("sess-default");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 6);
    let path = dir.path().join("d.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert!(load(&path).extras().is_empty());
    // After a rename the section appears.
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 0,
            name: "x".into(),
        },
    );
    done(&mut shell, Command::SaveSession);
    wait_operation(&mut shell);
    let session = load(&path);
    let tags: Vec<&str> = session.extras().iter().map(|s| s.tag.as_str()).collect();
    assert_eq!(tags, ["VIEW"]);
}

#[test]
fn save_after_open_writes_the_same_path_atomically() {
    let dir = TempDir::new("sess-save-again");
    let mut shell = decorated_shell(&dir, 7);
    let path = dir.path().join("b.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    let other = TempDir::new("sess-save-again-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path.clone()));
    wait_operation(&mut fresh);
    done(
        &mut fresh,
        Command::RenameChannel {
            channel: 2,
            name: "changed".into(),
        },
    );
    assert_eq!(fresh.state().unsaved, Unsaved::Dirty);
    let before = std::fs::read(&path).unwrap();
    done(&mut fresh, Command::SaveSession);
    wait_operation(&mut fresh);
    assert_eq!(fresh.state().unsaved, Unsaved::Clean);
    assert_ne!(std::fs::read(&path).unwrap(), before);
    assert_eq!(temp_files(dir.path()), 0);

    let third = TempDir::new("sess-save-again-c");
    let mut again = Shell::new(ShellConfig::for_tests(third.path()));
    done(&mut again, Command::OpenSession(path));
    wait_operation(&mut again);
    let name = again
        .state()
        .presentation
        .channels
        .iter()
        .find(|c| c.index == 2)
        .unwrap()
        .name
        .clone();
    assert_eq!(name, "changed");
}

#[test]
fn saving_without_a_path_asks_for_one_and_nothing_is_saved_without_a_capture() {
    let dir = TempDir::new("sess-nopath");
    let mut shell = finished_shell(&dir, 0b1, 5_000, 8);
    assert_eq!(
        shell.apply(Command::SaveSession),
        Outcome::Ignored(la_host::NEEDS_PATH)
    );
    let empty_dir = TempDir::new("sess-nocapture");
    let mut empty = Shell::new(ShellConfig::for_tests(empty_dir.path()));
    assert!(matches!(
        empty.apply(Command::SaveSessionAs(empty_dir.path().join("x.lasession"))),
        Outcome::Ignored(_)
    ));
}

#[test]
fn progress_is_reported_and_a_cancelled_save_leaves_the_old_file_byte_identical() {
    let dir = TempDir::new("sess-cancel-save");
    let mut shell = finished_shell(&dir, 0xffff, 1_500_000, 9);
    let path = dir.path().join("c.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    let original = std::fs::read(&path).unwrap();

    shell.set_session_fs(Arc::new(SlowFs));
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 0,
            name: "edited".into(),
        },
    );
    done(&mut shell, Command::SaveSession);
    // While it runs the state says so, with growing progress, and content edits wait.
    let mut seen_progress = false;
    for _ in 0..400 {
        shell.poll(std::time::Instant::now());
        if let Some(op) = shell.state().operation {
            assert_eq!(op.kind, la_host::OperationKind::Save);
            seen_progress |= op.done_bytes > 0 && op.total_bytes >= op.done_bytes;
            if seen_progress {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(seen_progress, "no progress was reported");
    assert!(matches!(
        shell.apply(Command::RenameChannel {
            channel: 1,
            name: "late".into()
        }),
        Outcome::Ignored(_)
    ));
    done(&mut shell, Command::CancelOperation);
    wait_operation(&mut shell);
    assert!(
        error_codes(&shell).is_empty(),
        "a cancel is not reported as a failure"
    );
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(temp_files(dir.path()), 0);
}

#[test]
fn a_cancelled_load_changes_nothing() {
    let dir = TempDir::new("sess-cancel-load");
    let mut shell = finished_shell(&dir, 0xffff, 1_500_000, 10);
    let path = dir.path().join("l.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    let other = TempDir::new("sess-cancel-load-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    fresh.set_session_fs(Arc::new(SlowFs));
    done(&mut fresh, Command::OpenSession(path));
    for _ in 0..200 {
        fresh.poll(std::time::Instant::now());
        if fresh.state().operation.is_some_and(|o| o.done_bytes > 0) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    done(&mut fresh, Command::CancelOperation);
    wait_operation(&mut fresh);
    assert!(fresh.state().capture.is_none());
    assert!(error_codes(&fresh).is_empty());
}

#[test]
fn bad_files_give_their_stable_code_and_leave_the_open_capture_alone() {
    let dir = TempDir::new("sess-bad");
    let mut shell = finished_shell(&dir, 0b11, 30_000, 11);
    let good = dir.path().join("good.lasession");
    done(&mut shell, Command::SaveSessionAs(good.clone()));
    wait_operation(&mut shell);
    let bytes = std::fs::read(&good).unwrap();

    let mut damaged = bytes.clone();
    let middle = damaged.len() / 2;
    damaged[middle] ^= 0xff;
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("damaged", damaged, la_session::error::DAMAGED),
        (
            "truncated",
            bytes[..bytes.len() - 40].to_vec(),
            la_session::error::TRUNCATED,
        ),
        ("empty", Vec::new(), la_session::error::NOT_A_SESSION),
        (
            "foreign",
            b"this is not a session file at all, just text".to_vec(),
            la_session::error::NOT_A_SESSION,
        ),
    ];
    let length = shell.state().capture.as_ref().unwrap().snapshot.length();
    for (name, content, code) in cases {
        let path = dir.path().join(format!("{name}.lasession"));
        std::fs::write(&path, &content).unwrap();
        // The capture on screen is saved, so opening is not guarded.
        assert_eq!(shell.state().unsaved, Unsaved::Clean);
        done(&mut shell, Command::OpenSession(path));
        wait_operation(&mut shell);
        let codes = error_codes(&shell);
        assert_eq!(codes.last().map(String::as_str), Some(code), "{name}");
        let capture = shell.state().capture.as_ref().unwrap();
        assert_eq!(
            capture.snapshot.length(),
            length,
            "{name}: the capture changed"
        );
        assert_eq!(shell.state().unsaved, Unsaved::Clean, "{name}");
        done(&mut shell, Command::DismissAllNotices);
    }
    // A missing file.
    done(
        &mut shell,
        Command::OpenSession(dir.path().join("missing.lasession")),
    );
    wait_operation(&mut shell);
    assert_eq!(
        error_codes(&shell).last().map(String::as_str),
        Some(la_session::error::IO_READ)
    );
}

#[test]
fn a_file_with_an_unknown_critical_section_is_refused_with_its_code() {
    let dir = TempDir::new("sess-critical");
    let shell = finished_shell(&dir, 0b11, 20_000, 15);
    let capture = shell.state().capture.as_ref().unwrap();
    let data = la_session::SessionData::new(capture.description.clone(), capture.snapshot.clone())
        .with_section(la_session::Section {
            tag: la_session::SectionTag::new("ZZZZ").unwrap(),
            critical: true,
            payload: vec![1, 2, 3],
        })
        .unwrap();
    let path = dir.path().join("critical.lasession");
    la_session::save_blocking(&path, &data, &la_session::Control::new()).unwrap();
    let other = TempDir::new("sess-critical-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path));
    wait_operation(&mut fresh);
    assert_eq!(
        error_codes(&fresh),
        [la_session::error::UNKNOWN_CRITICAL_SECTION]
    );
    assert!(fresh.state().capture.is_none());
}

#[test]
fn an_invalid_view_section_opens_with_defaults_a_warning_and_is_dropped_on_the_next_save() {
    let dir = TempDir::new("sess-invalid-view");
    let shell = finished_shell(&dir, 0b11, 30_000, 12);
    // A VIEW section the reader rejects: one marker beyond the end of the capture.
    let capture = shell.state().capture.as_ref().unwrap();
    let bad_payload = vec![1_u8, 0, 1, 1, 0xff, 0xff, 0x7f, 0, 0, 0, 0, 0, 0];
    let data = la_session::SessionData::new(capture.description.clone(), capture.snapshot.clone())
        .with_section(la_session::Section {
            tag: la_session::SectionTag::new("VIEW").unwrap(),
            critical: false,
            payload: bad_payload.clone(),
        })
        .unwrap();
    let path: PathBuf = dir.path().join("bad-view.lasession");
    la_session::save_blocking(&path, &data, &la_session::Control::new()).unwrap();

    let other = TempDir::new("sess-invalid-view-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path.clone()));
    wait_operation(&mut fresh);
    assert_eq!(error_codes(&fresh), [la_host::error::VIEW_INVALID]);
    let state = fresh.state();
    assert!(state.capture.is_some());
    assert!(state.markers.list.is_empty());
    assert!(state.auto_fit);
    assert_eq!(state.unsaved, Unsaved::Clean);
    // Saved again, the invalid section is gone.
    done(
        &mut fresh,
        Command::RenameChannel {
            channel: 0,
            name: "n".into(),
        },
    );
    done(&mut fresh, Command::SaveSession);
    wait_operation(&mut fresh);
    let session = load(&path);
    let view = session
        .extras()
        .iter()
        .find(|s| s.tag.as_str() == "VIEW")
        .unwrap();
    assert_ne!(view.payload, bad_payload, "the old payload is not carried");
    let snapshot = &fresh.state().capture.as_ref().unwrap().snapshot;
    let bounds = la_host::view_section::Bounds {
        channels: snapshot.channels(),
        length: snapshot.length(),
    };
    assert!(la_host::view_section::decode(&view.payload, bounds).is_ok());
}

#[test]
fn a_newer_view_section_is_kept_untouched_with_an_info_message() {
    let dir = TempDir::new("sess-newer-view");
    let shell = finished_shell(&dir, 0b11, 20_000, 16);
    let capture = shell.state().capture.as_ref().unwrap();
    let newer = vec![9_u8, 1, 2, 3, 4];
    let data = la_session::SessionData::new(capture.description.clone(), capture.snapshot.clone())
        .with_section(la_session::Section {
            tag: la_session::SectionTag::new("VIEW").unwrap(),
            critical: false,
            payload: newer.clone(),
        })
        .unwrap();
    let path = dir.path().join("newer.lasession");
    la_session::save_blocking(&path, &data, &la_session::Control::new()).unwrap();
    let other = TempDir::new("sess-newer-view-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path.clone()));
    wait_operation(&mut fresh);
    assert!(error_codes(&fresh).is_empty(), "not a warning");
    assert_eq!(fresh.state().notices.len(), 1, "one info message");
    done(
        &mut fresh,
        Command::RenameChannel {
            channel: 0,
            name: "z".into(),
        },
    );
    done(&mut fresh, Command::SaveSession);
    wait_operation(&mut fresh);
    let session = load(&path);
    assert_eq!(session.extras().len(), 1);
    assert_eq!(session.extras()[0].payload, newer);
}

#[test]
fn unknown_sections_of_an_opened_file_are_written_back() {
    let dir = TempDir::new("sess-extras");
    let shell = finished_shell(&dir, 0b11, 20_000, 13);
    let capture = shell.state().capture.as_ref().unwrap();
    let data = la_session::SessionData::new(capture.description.clone(), capture.snapshot.clone())
        .with_section(la_session::Section {
            tag: la_session::SectionTag::new("NOTE").unwrap(),
            critical: false,
            payload: b"a note from another version".to_vec(),
        })
        .unwrap();
    let path = dir.path().join("extras.lasession");
    la_session::save_blocking(&path, &data, &la_session::Control::new()).unwrap();
    let other = TempDir::new("sess-extras-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path.clone()));
    wait_operation(&mut fresh);
    done(
        &mut fresh,
        Command::RenameChannel {
            channel: 1,
            name: "q".into(),
        },
    );
    done(&mut fresh, Command::SaveSession);
    wait_operation(&mut fresh);
    let session = load(&path);
    let tags: Vec<&str> = session.extras().iter().map(|s| s.tag.as_str()).collect();
    assert_eq!(tags, ["NOTE", "VIEW"]);
    assert_eq!(session.extras()[0].payload, b"a note from another version");
}

#[test]
fn recent_files_are_newest_first_limited_to_ten_and_missing_ones_are_skipped() {
    let dir = TempDir::new("sess-recent");
    let mut shell = finished_shell(&dir, 0b1, 5_000, 14);
    let mut paths = Vec::new();
    for i in 0..12 {
        let path = dir.path().join(format!("f{i}.lasession"));
        done(&mut shell, Command::SaveSessionAs(path.clone()));
        wait_operation(&mut shell);
        paths.push(path);
    }
    assert_eq!(shell.state().settings.recent_files.len(), 10);
    let recent = shell.recent_files();
    assert_eq!(recent[0], paths[11]);
    assert_eq!(recent[9], paths[2]);
    std::fs::remove_file(&paths[11]).unwrap();
    assert_eq!(
        shell.recent_files()[0],
        paths[10],
        "a missing file is skipped"
    );
    // The list is remembered: a new shell reads it from the settings file.
    let again = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(again.state().settings.recent_files.len(), 10);
}

#[test]
fn opening_a_file_sets_the_flags_and_what_the_next_capture_records_together() {
    let dir = TempDir::new("sess-flags");
    let mut shell = finished_shell(&dir, 0b11, 10_000, 17);
    // Every channel switched off: nothing shown and nothing recorded next time.
    for channel in 0..2 {
        done(
            &mut shell,
            Command::SetChannelVisible {
                channel,
                visible: false,
            },
        );
    }
    let path = dir.path().join("all-off.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    let other = TempDir::new("sess-flags-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path));
    wait_operation(&mut fresh);
    let state = fresh.state();
    assert_eq!(
        state.config.enabled_channels, 0,
        "the next capture records nothing yet"
    );
    assert!(
        !state.config_problems.is_empty(),
        "and Start says that no channel is chosen"
    );
    assert_eq!(state.presentation.visible_mask() & 0b11, 0);
    // One flag turns the channel on for both: it is recorded next time and its lane is back.
    done(
        &mut fresh,
        Command::SetChannelVisible {
            channel: 1,
            visible: true,
        },
    );
    assert_eq!(fresh.state().config.enabled_channels, 0b10);
    assert!(fresh.state().config_problems.is_empty());
}
