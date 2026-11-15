//! A capture on the virtual device is valid for the configured decoders (spec US6, SC-006;
//! constitution Principle XI).
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
use la_decoders::generators::clocked_bits::{
    OPTION_ERROR_RATE, OPTION_ERRORS, expected, expected_errors,
};
use la_decoders::model::{Kind, Text};
use la_host::decoders::InstanceState;
use la_host::{Activity, Command, ConfigEdit, Decision, Outcome, Shell};

/// 100 frames of the default setup (8 bits, idle 100): 8 · 10 + 100 + 10 = 190 samples each.
const LENGTH: u64 = 19_000;
const SEED: u64 = 99;

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn wait(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        shell.poll(Instant::now());
        let decoding = shell
            .state()
            .decoders
            .instances
            .iter()
            .any(|i| i.state == InstanceState::Running);
        if shell.state().activity == Activity::Idle && !decoding {
            return;
        }
        assert!(Instant::now() < deadline, "did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A shell with one capture and the decoder on channels 0 and 1, ready for the next capture.
fn shell_with_decoder(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 2_000, 1);
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
    wait(&mut shell);
    (shell, id)
}

/// Starts the next capture with the seed and length of the tests, discarding the old one.
fn capture_again(shell: &mut Shell) {
    for edit in [ConfigEdit::Seed(Some(SEED)), ConfigEdit::Length(LENGTH)] {
        done(shell, Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(shell, Command::Decide(Decision::Discard));
    } else {
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    }
    wait(shell);
}

/// The bytes the decoder showed in the lane: `(start, end, value)`.
fn decoded_bytes(shell: &Shell, id: u32) -> Vec<(u64, u64, u64)> {
    let results = shell.decoder_results(id).unwrap();
    results
        .lane
        .window(0, u64::MAX)
        .iter()
        .filter_map(|e| match (&e.annotation.kind, &e.annotation.text) {
            (Kind::Range { start, end }, Text::Message { key, args }) if key.ends_with("-byte") => {
                let value = u64::from_str_radix(args[0].trim_start_matches("0x"), 16).unwrap();
                Some((*start, *end, value))
            }
            _ => None,
        })
        .collect()
}

fn error_ranges(shell: &Shell, id: u32) -> Vec<(u64, u64)> {
    let results = shell.decoder_results(id).unwrap();
    results
        .lane
        .window(0, u64::MAX)
        .iter()
        .filter(|e| e.annotation.error)
        .filter_map(|e| match (&e.annotation.kind, &e.annotation.text) {
            (Kind::Range { start, end }, Text::Message { key, .. })
                if key.ends_with("incomplete") =>
            {
                Some((*start, *end))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_simulated_capture_decodes_to_exactly_what_the_generator_expects() {
    let dir = TempDir::new("gen-exact");
    let (mut shell, id) = shell_with_decoder(&dir);
    capture_again(&mut shell);
    let capture = shell.state().capture.as_ref().unwrap();
    assert!(capture.simulated, "the capture stays labelled simulated");
    assert_eq!(capture.snapshot.length(), LENGTH);
    let setup = shell.state().decoders.instances[0].setup.clone();
    let want: Vec<(u64, u64, u64)> = expected(&setup, SEED, LENGTH)
        .iter()
        .map(|item| match item.payload {
            la_decoders::model::Payload::Integer(v) => (item.start, item.end, v),
            _ => panic!("integer expected"),
        })
        .collect();
    assert_eq!(want.len(), 100);
    assert_eq!(decoded_bytes(&shell, id), want);
    assert!(
        error_ranges(&shell, id).is_empty(),
        "no errors unless asked for"
    );
}

#[test]
fn the_same_seed_gives_the_same_capture() {
    let a = {
        let dir = TempDir::new("gen-same-a");
        let (mut shell, id) = shell_with_decoder(&dir);
        capture_again(&mut shell);
        decoded_bytes(&shell, id)
    };
    let b = {
        let dir = TempDir::new("gen-same-b");
        let (mut shell, id) = shell_with_decoder(&dir);
        capture_again(&mut shell);
        decoded_bytes(&shell, id)
    };
    assert_eq!(a, b);
}

#[test]
fn without_a_decoder_the_capture_is_the_plain_random_one() {
    let edges = |shell: &Shell| {
        let snapshot = &shell.state().capture.as_ref().unwrap().snapshot;
        (0..2_u8)
            .map(|c| snapshot.transitions(c, 0, LENGTH - 1).unwrap().count())
            .collect::<Vec<_>>()
    };
    let plain = |name: &str| {
        let dir = TempDir::new(name);
        let mut shell = finished_shell(&dir, 0b11, 2_000, 1);
        capture_again(&mut shell);
        edges(&shell)
    };
    let first = plain("gen-plain-a");
    assert_eq!(first, plain("gen-plain-b"));
    // With the decoder the clock channel is no longer random: it has exactly the edges of the
    // generated frames (two per bit, 800 in all), not what the random source would give.
    let dir = TempDir::new("gen-plain-c");
    let (mut shell, _) = shell_with_decoder(&dir);
    capture_again(&mut shell);
    assert_eq!(edges(&shell)[0], 100 * 8 * 2);
    assert_ne!(edges(&shell)[0], first[0]);
}

#[test]
fn deliberate_errors_are_planted_and_reported_where_they_were_planted() {
    let dir = TempDir::new("gen-errors");
    let (mut shell, id) = shell_with_decoder(&dir);
    let mut setup = shell.state().decoders.instances[0].setup.clone();
    setup.generator.insert(OPTION_ERRORS.into(), 1);
    setup.generator.insert(OPTION_ERROR_RATE.into(), 4);
    done(
        &mut shell,
        Command::ConfigureDecoder {
            id,
            setup: setup.clone(),
        },
    );
    wait(&mut shell);
    capture_again(&mut shell);
    let planted = expected_errors(&setup, SEED, LENGTH);
    assert!(planted.len() > 5, "{}", planted.len());
    assert_eq!(error_ranges(&shell, id), planted);
    let want = expected(&setup, SEED, LENGTH);
    assert_eq!(decoded_bytes(&shell, id).len(), want.len());
    assert_eq!(want.len() + planted.len(), 100);
}

#[test]
fn a_stop_during_generation_takes_effect_at_once() {
    let dir = TempDir::new("gen-stop");
    let (mut shell, _) = shell_with_decoder(&dir);
    for edit in [
        ConfigEdit::Realtime(true),
        ConfigEdit::SampleRate(1_000),
        ConfigEdit::Length(10_000_000),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(&mut shell, Command::Decide(Decision::Discard));
    }
    assert_eq!(shell.state().activity, Activity::Capturing);
    std::thread::sleep(Duration::from_millis(200));
    done(&mut shell, Command::StopCapture);
    let started = Instant::now();
    wait(&mut shell);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(shell.state().activity, Activity::Idle);
}
