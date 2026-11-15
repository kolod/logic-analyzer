//! Decoding in the background of a capture that grows, changes and ends (spec US3, SC-004).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell};
use la_decoders::decoders::clocked_bits::ID;
use la_decoders::run::ChunkLimits;
use la_decoders::setup::Value;
use la_decoders::testing::{self, SLOW};
use la_host::capture::CaptureEnd;
use la_host::decoders::InstanceState;
use la_host::{Activity, Command, ConfigEdit, Decision, Outcome, Shell, Tick};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn poll_until(shell: &mut Shell, what: &str, mut condition: impl FnMut(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !condition(shell) {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn busy(shell: &Shell) -> bool {
    shell
        .state()
        .decoders
        .instances
        .iter()
        .any(|i| matches!(i.state, InstanceState::Running | InstanceState::Waiting) && i.ready)
}

fn with_sample(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 20_000, 2);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    for (role, channel) in [("clock", 0), ("data", 1)] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: role.into(),
                channel: Some(channel),
            },
        );
    }
    poll_until(&mut shell, "the first pass", |s| !busy(s));
    (shell, id)
}

fn restart_capture(shell: &mut Shell, length: u64) {
    for edit in [ConfigEdit::Length(length), ConfigEdit::Seed(Some(8))] {
        done(shell, Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(shell, Command::Decide(Decision::Discard));
    }
}

#[test]
fn annotations_appear_while_the_capture_runs_and_the_capture_is_not_harmed() {
    // A long capture at full speed with a decoder following it.
    let length = 6_000_000;
    let with = {
        let dir = TempDir::new("growth-with");
        let (mut shell, id) = with_sample(&dir);
        restart_capture(&mut shell, length);
        assert_eq!(shell.state().activity, Activity::Capturing);
        assert_eq!(shell.wants_ticks(), Tick::Fast);
        let started = Instant::now();
        let mut first = None;
        while shell.state().activity == Activity::Capturing {
            shell.poll(Instant::now());
            if first.is_none() && shell.state().decoders.instance(id).unwrap().annotations > 0 {
                first = Some(started.elapsed());
            }
            std::thread::sleep(Duration::from_millis(2));
            assert!(started.elapsed() < Duration::from_secs(120));
        }
        // Annotations were there long before the capture ended, within a second of the data.
        let first = first.expect("annotations appeared during the capture");
        assert!(first < Duration::from_secs(1), "{first:?}");
        poll_until(&mut shell, "the pass", |s| !busy(s));
        let view = shell.state().decoders.instance(id).unwrap().clone();
        assert_eq!(view.state, InstanceState::Finished);
        assert_eq!(view.covered, length);
        let capture = shell.state().capture.as_ref().unwrap();
        (capture.snapshot.length(), capture.end)
    };
    // The same capture without a decoder ends the same way and has the same length.
    let without = {
        let dir = TempDir::new("growth-without");
        let mut shell = finished_shell(&dir, 0b11, 2_000, 2);
        restart_capture(&mut shell, length);
        poll_until(&mut shell, "the capture", |s| {
            s.state().activity == Activity::Idle
        });
        let capture = shell.state().capture.as_ref().unwrap();
        (capture.snapshot.length(), capture.end)
    };
    assert_eq!(with, without);
    assert_eq!(with.1, Some(CaptureEnd::Completed));
}

fn slow_shell(dir: &TempDir, instances: usize) -> (Shell, Vec<u32>) {
    let mut shell = finished_shell(dir, 0b11, 200_000, 4);
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_chunk_limits(ChunkLimits {
        max_edges: 400,
        max_span: 5_000,
    });
    let mut ids = Vec::new();
    for _ in 0..instances {
        done(
            &mut shell,
            Command::AddDecoder {
                decoder: SLOW.id.into(),
            },
        );
        let id = shell.state().decoders.instances.last().unwrap().id;
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: "clock".into(),
                channel: Some(0),
            },
        );
        ids.push(id);
    }
    (shell, ids)
}

#[test]
fn more_instances_than_decode_threads_take_turns() {
    let dir = TempDir::new("growth-cap");
    let (mut shell, ids) = slow_shell(&dir, 3);
    // Redo it with a cap of one thread.
    for id in &ids {
        done(&mut shell, Command::RemoveDecoder(*id));
    }
    shell.set_decoder_thread_cap(1);
    let mut ids = Vec::new();
    for _ in 0..3 {
        done(
            &mut shell,
            Command::AddDecoder {
                decoder: SLOW.id.into(),
            },
        );
        let id = shell.state().decoders.instances.last().unwrap().id;
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: "clock".into(),
                channel: Some(0),
            },
        );
        ids.push(id);
    }
    let states: Vec<InstanceState> = ids
        .iter()
        .map(|id| shell.state().decoders.instance(*id).unwrap().state)
        .collect();
    assert_eq!(
        states
            .iter()
            .filter(|s| **s == InstanceState::Running)
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|s| **s == InstanceState::Waiting)
            .count(),
        2
    );
    assert_eq!(
        shell.wants_ticks(),
        Tick::Fast,
        "queued work needs polling too"
    );
    // Cancel the running one: the next in line starts.
    let running = ids
        .iter()
        .copied()
        .find(|id| shell.state().decoders.instance(*id).unwrap().state == InstanceState::Running)
        .unwrap();
    done(&mut shell, Command::CancelDecoder(running));
    poll_until(&mut shell, "the turn", |s| {
        ids.iter()
            .filter(|id| s.state().decoders.instance(**id).unwrap().state == InstanceState::Running)
            .count()
            == 1
            && s.state().decoders.instance(running).unwrap().state == InstanceState::Cancelled
    });
    poll_until(&mut shell, "all", |s| !busy(s));
    for id in ids.iter().filter(|id| **id != running) {
        assert_eq!(
            shell.state().decoders.instance(*id).unwrap().state,
            InstanceState::Finished
        );
    }
}

#[test]
fn a_setup_change_keeps_the_old_results_flagged_outdated_until_the_new_ones_arrive() {
    let dir = TempDir::new("growth-outdated");
    let (mut shell, id) = with_sample(&dir);
    let before = shell.decoder_results(id).unwrap().clone();
    done(
        &mut shell,
        Command::SetDecoderOption {
            id,
            option: "bits".into(),
            value: Value::Integer(4),
        },
    );
    // Right away: the old results are still there, flagged, and a pass runs.
    let view = shell.state().decoders.instance(id).unwrap().clone();
    assert_eq!(view.state, InstanceState::Running);
    assert!(view.outdated);
    let kept = shell.decoder_results(id).unwrap();
    assert_eq!(kept.lane.len(), before.lane.len(), "never a mixture");
    poll_until(&mut shell, "the swap", |s| !busy(s));
    let view = shell.state().decoders.instance(id).unwrap();
    assert!(!view.outdated);
    assert_ne!(
        shell.decoder_results(id).unwrap().lane.len(),
        before.lane.len()
    );
}

#[test]
fn saving_while_a_decoder_still_works_writes_the_capture_as_it_is() {
    let dir = TempDir::new("growth-save");
    let (mut shell, _) = slow_shell(&dir, 1);
    assert!(busy(&shell));
    let path = dir.path().join("during.las");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    poll_until(&mut shell, "the save", |s| s.state().operation.is_none());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    let mut reopened = Shell::new(la_host::ShellConfig::for_tests(dir.path()));
    done(&mut reopened, Command::OpenSession(path));
    poll_until(&mut reopened, "the load", |s| {
        s.state().operation.is_none() && s.state().capture.is_some()
    });
    assert_eq!(
        reopened.state().capture.as_ref().unwrap().snapshot.length(),
        200_000
    );
}

#[test]
fn a_new_capture_and_clearing_the_capture_end_every_pass() {
    let dir = TempDir::new("growth-clear");
    let (mut shell, ids) = slow_shell(&dir, 2);
    assert!(busy(&shell));
    let outcome = shell.apply(Command::NewCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(&mut shell, Command::Decide(Decision::Discard));
    }
    assert!(shell.state().decoders.instances.is_empty());
    assert!(ids.iter().all(|id| shell.decoder_results(*id).is_none()));
    assert_eq!(shell.wants_ticks(), Tick::Slow, "nothing runs any more");
}
