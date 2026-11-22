//! Nothing the user captured is lost without an explicit choice (spec SC-011, task T066).
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
use std::time::{Duration, Instant};

use common::{Lcg, TempDir, finished_shell, wait_operation};
use la_host::notices::NoticeContent;
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Guarded, NEEDS_PATH, Outcome, Shell,
    ShellConfig, Unsaved,
};
use la_session::harness::{FaultFs, FaultPlan, Op};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
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

/// A saved capture that has not changed since.
fn clean_shell(dir: &TempDir) -> (Shell, PathBuf) {
    let mut shell = finished_shell(dir, 0b11, 20_000, 31);
    let path = dir.path().join("clean.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
    (shell, path)
}

/// A finished capture that was never saved.
fn dirty_shell(dir: &TempDir) -> Shell {
    let shell = finished_shell(dir, 0b11, 20_000, 32);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    shell
}

/// A capture that is still running (paced in real time, long).
fn running_shell(dir: &TempDir) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0b11)),
        ConfigEdit::SampleRate(20_000),
        ConfigEdit::Length(10_000_000),
        ConfigEdit::Seed(Some(33)),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    let deadline = Instant::now() + Duration::from_secs(20);
    while shell
        .state()
        .capture
        .as_ref()
        .is_none_or(|c| c.snapshot.length() < 1_000)
    {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(shell.state().unsaved, Unsaved::Running);
    shell
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().activity != Activity::Idle || shell.operation_running() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "did not settle");
        std::thread::sleep(Duration::from_millis(3));
    }
    shell.poll(Instant::now());
}

fn started_at(shell: &Shell) -> Option<std::time::SystemTime> {
    shell
        .state()
        .capture
        .as_ref()
        .map(|c| c.description.started_at())
}

fn good_file(dir: &TempDir) -> PathBuf {
    let other = TempDir::new("guard-good");
    let (_, path) = clean_shell(&other);
    let target = dir.path().join("other.lasession");
    std::fs::copy(&path, &target).unwrap();
    target
}

// ---- the table: state x command ------------------------------------------------------------

#[test]
fn a_clean_capture_lets_every_command_run_at_once() {
    let dir = TempDir::new("guard-clean");
    let file = good_file(&dir);
    for command in ["new", "start", "open", "quit", "close"] {
        let (mut shell, _) = clean_shell(&TempDir::new("guard-clean-s"));
        let outcome = match command {
            "new" => shell.apply(Command::NewCapture),
            "start" => shell.apply(Command::StartCapture),
            "open" => shell.apply(Command::OpenSession(file.clone())),
            "quit" => shell.apply(Command::Quit),
            _ => shell.apply(Command::RequestClose),
        };
        match (command, outcome) {
            ("quit" | "close", Outcome::Done(c)) => assert!(c.contains(Changes::QUIT)),
            (_, Outcome::Done(_)) => {}
            (command, other) => panic!("{command}: {other:?}"),
        }
        assert!(shell.state().pending_decision.is_none());
        settle(&mut shell);
    }
}

#[test]
fn an_unsaved_or_running_capture_asks_before_any_command_that_would_lose_it() {
    let expected = [
        ("new", Guarded::NewCapture),
        ("open", Guarded::OpenSession(PathBuf::new())),
        ("quit", Guarded::Quit),
        ("close", Guarded::Close),
    ];
    let dir = TempDir::new("guard-ask");
    let file = good_file(&dir);
    for state in ["dirty", "running"] {
        for (name, guarded) in &expected {
            let work = TempDir::new("guard-ask-s");
            let mut shell = if state == "dirty" {
                dirty_shell(&work)
            } else {
                running_shell(&work)
            };
            let started = started_at(&shell);
            let command = match *name {
                "new" => Command::NewCapture,
                "open" => Command::OpenSession(file.clone()),
                "quit" => Command::Quit,
                _ => Command::RequestClose,
            };
            let outcome = shell.apply(command);
            let Outcome::NeedsDecision(got) = outcome else {
                panic!("{state} {name}: {outcome:?}");
            };
            match (guarded, &got) {
                (Guarded::OpenSession(_), Guarded::OpenSession(_)) => {}
                (a, b) => assert_eq!(a, b, "{state} {name}"),
            }
            // Nothing changed, and a running capture was stopped with its data kept.
            settle(&mut shell);
            assert_eq!(
                started_at(&shell),
                started,
                "{state} {name}: capture replaced"
            );
            let capture = shell.state().capture.as_ref().unwrap();
            assert!(capture.snapshot.length() > 0, "{state} {name}");
            assert_eq!(shell.state().unsaved, Unsaved::Dirty, "{state} {name}");
            assert_eq!(shell.state().pending_decision, Some(got), "{state} {name}");
        }
    }
}

#[test]
fn starting_a_capture_over_an_unsaved_one_asks_and_a_running_one_cannot_be_started_over() {
    let dir = TempDir::new("guard-start");
    let mut shell = dirty_shell(&dir);
    assert_eq!(
        shell.apply(Command::StartCapture),
        Outcome::NeedsDecision(Guarded::StartCapture)
    );
    let running_dir = TempDir::new("guard-start-r");
    let mut running = running_shell(&running_dir);
    assert!(matches!(
        running.apply(Command::StartCapture),
        Outcome::Ignored(_)
    ));
    assert_eq!(running.state().unsaved, Unsaved::Running);
}

// ---- the answers --------------------------------------------------------------------------

#[test]
fn cancel_drops_the_command_and_keeps_the_capture() {
    let dir = TempDir::new("guard-cancel");
    let mut shell = dirty_shell(&dir);
    let started = started_at(&shell);
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    done(&mut shell, Command::Decide(Decision::Cancel));
    assert!(shell.state().pending_decision.is_none());
    assert_eq!(started_at(&shell), started);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    // Nothing waits any more: a second answer is not valid.
    assert!(matches!(
        shell.apply(Command::Decide(Decision::Discard)),
        Outcome::Ignored(_)
    ));
}

#[test]
fn discard_runs_the_command() {
    let dir = TempDir::new("guard-discard");
    let mut shell = dirty_shell(&dir);
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    done(&mut shell, Command::Decide(Decision::Discard));
    assert!(shell.state().capture.is_none());

    let mut shell = dirty_shell(&TempDir::new("guard-discard-q"));
    assert!(matches!(
        shell.apply(Command::Quit),
        Outcome::NeedsDecision(_)
    ));
    let changes = done(&mut shell, Command::Decide(Decision::Discard));
    assert!(changes.contains(Changes::QUIT));

    let running_dir = TempDir::new("guard-discard-r");
    let mut running = running_shell(&running_dir);
    assert!(matches!(
        running.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    done(&mut running, Command::Decide(Decision::Discard));
    assert!(running.state().capture.is_none());
    settle(&mut running);
}

#[test]
fn save_with_a_known_path_saves_and_then_runs_the_command() {
    let dir = TempDir::new("guard-save");
    let (mut shell, path) = clean_shell(&dir);
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 0,
            name: "unsaved".into(),
        },
    );
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(Guarded::NewCapture)
    ));
    let before = std::fs::read(&path).unwrap();
    done(&mut shell, Command::Decide(Decision::Save));
    // The command waits for the save.
    assert!(shell.state().capture.is_some());
    wait_operation(&mut shell);
    assert_ne!(
        std::fs::read(&path).unwrap(),
        before,
        "the file was written"
    );
    assert!(shell.state().capture.is_none(), "then the command ran");
    assert!(shell.state().pending_decision.is_none());
    assert!(error_codes(&shell).is_empty());
}

#[test]
fn save_without_a_path_asks_for_one_and_the_command_runs_after_that_save() {
    let dir = TempDir::new("guard-nopath");
    let mut shell = dirty_shell(&dir);
    assert!(matches!(
        shell.apply(Command::Quit),
        Outcome::NeedsDecision(Guarded::Quit)
    ));
    assert_eq!(
        shell.apply(Command::Decide(Decision::Save)),
        Outcome::Ignored(NEEDS_PATH)
    );
    assert!(shell.state().pending_decision.is_some(), "still waiting");
    let path = dir.path().join("chosen.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert!(path.exists());
    assert!(shell.state().pending_decision.is_none());
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
}

#[test]
fn a_failed_save_keeps_the_command_pending_and_the_capture() {
    let dir = TempDir::new("guard-failed-save");
    let mut shell = dirty_shell(&dir);
    let started = started_at(&shell);
    let path = dir.path().join("fails.lasession");
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    shell.set_session_fs(Arc::new(FaultFs::failing(FaultPlan {
        op: Op::Rename,
        nth: 0,
        kind: std::io::ErrorKind::PermissionDenied,
        partial_write: false,
    })));
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert_eq!(error_codes(&shell), [la_session::error::IO_WRITE]);
    assert_eq!(started_at(&shell), started, "the capture is still there");
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    assert_eq!(
        shell.state().pending_decision,
        Some(Guarded::NewCapture),
        "the command still waits"
    );
    assert!(!path.exists());
}

#[test]
fn cancelling_the_question_while_the_save_runs_does_not_run_the_command() {
    let dir = TempDir::new("guard-cancel-during-save");
    let mut shell = finished_shell(&dir, 0xffff, 1_500_000, 34);
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    shell.set_session_fs(Arc::new(common::SlowFs));
    done(
        &mut shell,
        Command::SaveSessionAs(dir.path().join("s.lasession")),
    );
    done(&mut shell, Command::Decide(Decision::Cancel));
    wait_operation(&mut shell);
    assert!(shell.state().capture.is_some(), "the user said no");
    assert_eq!(
        shell.state().unsaved,
        Unsaved::Clean,
        "but the save finished"
    );
}

#[test]
fn discarding_a_running_capture_leaves_the_shell_idle_and_able_to_open_a_file() {
    let dir = TempDir::new("guard-discard-running");
    let file = good_file(&dir);
    for command in ["open", "quit"] {
        let work = TempDir::new("guard-discard-running-s");
        let mut shell = running_shell(&work);
        let outcome = match command {
            "open" => shell.apply(Command::OpenSession(file.clone())),
            _ => shell.apply(Command::Quit),
        };
        assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{command}");
        done(&mut shell, Command::Decide(Decision::Discard));
        // Not stuck in "capturing", whatever the command was.
        assert_eq!(shell.state().activity, Activity::Idle, "{command}");
        wait_operation(&mut shell);
        if command == "open" {
            assert_eq!(shell.state().unsaved, Unsaved::Clean);
            assert!(shell.state().session_path.is_some());
        } else {
            // Quitting keeps the data on screen, now unsaved and no longer running.
            assert_eq!(shell.state().unsaved, Unsaved::Dirty);
            assert!(shell.state().capture.is_some());
        }
    }
}

// ---- a random walk -----------------------------------------------------------------------

#[test]
fn no_sequence_of_commands_discards_unsaved_data_without_a_decision() {
    let dir = TempDir::new("guard-walk");
    let file = good_file(&dir);
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0b11)),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(3_000),
        ConfigEdit::Seed(Some(35)),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    let save_paths: Vec<PathBuf> = (0..3)
        .map(|i| dir.path().join(format!("walk{i}.lasession")))
        .collect();
    let seed = std::env::var("LA_WALK_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2026);
    let mut rng = Lcg(seed);
    let mut history: Vec<String> = Vec::new();
    let mut discard_open = false;
    let mut steps_with_risk = 0_u32;
    for step in 0..20_000_u32 {
        let before = started_at(&shell);
        let at_risk = before.is_some() && shell.state().unsaved != Unsaved::Clean;
        let saving_before = shell
            .state()
            .operation
            .is_some_and(|o| o.kind == la_host::OperationKind::Save);
        let errors_before = error_codes(&shell).len();
        let was_operating = shell.operation_running();
        if !was_operating {
            discard_open = false;
        }
        let pending_before = shell.state().pending_decision.clone();

        let command = match rng.below(16) {
            0 => Command::NewCapture,
            1 | 2 => Command::StartCapture,
            3 => Command::OpenSession(file.clone()),
            4 => Command::Quit,
            5 => Command::RequestClose,
            6 => Command::Decide(Decision::Save),
            7 => Command::Decide(Decision::Discard),
            8 => Command::Decide(Decision::Cancel),
            9 => Command::SaveSessionAs(save_paths[rng.below(3) as usize].clone()),
            10 => Command::SaveSession,
            11 => Command::CancelOperation,
            12 => Command::StopCapture,
            13 => Command::RenameChannel {
                channel: rng.below(2) as u8,
                name: "w".into(),
            },
            14 => Command::BeginPlacement(la_host::markers::Kind::Single),
            _ => Command::RefreshDevices,
        };
        let discarding = matches!(command, Command::Decide(Decision::Discard));
        let described = format!("{step}: {command:?}");
        let outcome = shell.apply(command);
        history.push(format!(
            "{described} -> {outcome:?}; activity {:?}, unsaved {:?}, pending {:?}",
            shell.state().activity,
            shell.state().unsaved,
            shell.state().pending_decision
        ));
        if discarding && matches!(outcome, Outcome::Done(_)) {
            if let Some(Guarded::OpenSession(_)) = pending_before {
                discard_open = true;
            }
        }
        // Let background work move on a little, often enough to finish things.
        if rng.below(3) == 0 {
            shell.poll(Instant::now());
        }
        if rng.below(40) == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }

        let after = started_at(&shell);
        let discarded_now = discarding && matches!(outcome, Outcome::Done(_));
        // A save that ended without a failure (a save to the same path leaves the list of
        // recent files as it was, so that list cannot tell).
        let saved_now = saving_before
            && !shell.operation_running()
            && error_codes(&shell).len() <= errors_before;
        if at_risk {
            steps_with_risk += 1;
            let replaced = after != before;
            assert!(
                !replaced || discarded_now || discard_open || saved_now,
                "step {step}: unsaved data was replaced without a decision or a save; last steps: {:#?}",
                &history[history.len().saturating_sub(8)..]
            );
        }
    }
    assert!(steps_with_risk > 2_000, "the walk rarely held unsaved data");
    settle(&mut shell);
}
