//! The I2C decoder in the host: added like any decoder, no change to the shell (spec 008, tasks
//! T014, T020, T027, T029).
//!
//! A capture of a bus is made by hand, saved as a session file and opened, so the host reads it like
//! any capture.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{TempDir, finished_shell};
use la_decoders::decoders::i2c::ID;
use la_decoders::model::{Kind, Target, Text};
use la_decoders::run::Results;
use la_devices::{CaptureDescription, SavedDescription};
use la_host::decoders::InstanceState;
use la_host::window::ViewSize;
use la_host::{Activity, Command, ConfigEdit, Decision, Outcome, Shell, ShellConfig};
use la_session::{Control, SessionData};
use la_storage::{Capture, CaptureConfig, Limits};

const SCL: u8 = 0;
const SDA: u8 = 1;

/// A bus signal under construction: `(sda, scl)` per sample, 16 samples to a clock period.
#[derive(Default)]
struct Bus(Vec<(bool, bool)>);

impl Bus {
    fn raw(&mut self, sda: bool, scl: bool, n: usize) {
        self.0.extend(std::iter::repeat_n((sda, scl), n));
    }

    fn bit(&mut self, bit: bool) {
        self.raw(bit, false, 4);
        self.raw(bit, true, 8);
        self.raw(bit, false, 4);
    }

    fn byte(&mut self, value: u8, ack: bool) {
        for i in 0..8 {
            self.bit(value >> (7 - i) & 1 != 0);
        }
        self.bit(!ack);
    }

    fn write(&mut self, address: u8, data: &[u8]) {
        self.raw(true, true, 64);
        self.raw(false, true, 4);
        self.byte(address << 1, true);
        for value in data {
            self.byte(*value, true);
        }
        self.raw(false, false, 4);
        self.raw(false, true, 4);
        self.raw(true, true, 64);
    }

    fn words(&self) -> Vec<u32> {
        self.0
            .iter()
            .map(|(sda, scl)| (u32::from(*scl) << SCL) | (u32::from(*sda) << SDA))
            .collect()
    }
}

/// Saves a session file holding a closed capture of these words (two channels).
fn session_file(dir: &TempDir, words: &[u32]) -> PathBuf {
    let mut capture = Capture::new(CaptureConfig {
        channels: 2,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut start = 0_u64;
    for chunk in words.chunks(60_000) {
        capture.append(start, chunk).unwrap();
        start += chunk.len() as u64;
    }
    capture.close().unwrap();
    let description = CaptureDescription::restore(SavedDescription {
        device_id: "test:bus".to_owned(),
        device_name: "Test bus".to_owned(),
        simulated: false,
        seed: None,
        requested_sample_rate_hz: 1_000_000,
        sample_rate_hz: 1_000_000,
        channels: 2,
        enabled_mask: 0b11,
        trigger: None,
        samples_before_trigger: 0,
        started_at: std::time::SystemTime::UNIX_EPOCH,
    })
    .unwrap();
    let path = dir.path().join("bus.las");
    let data = SessionData::new(description, capture.reader().snapshot());
    la_session::save_blocking(&path, &data, &Control::new()).unwrap();
    path
}

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

fn idle(shell: &Shell) -> bool {
    shell.state().decoders.instances.iter().all(|i| {
        !(i.state == InstanceState::Running || (i.state == InstanceState::Waiting && i.ready))
    })
}

fn opened(dir: &TempDir, words: &[u32]) -> Shell {
    let path = session_file(dir, words);
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    done(&mut shell, Command::OpenSession(path));
    poll_until(&mut shell, "the load", |s| {
        s.state().operation.is_none() && s.state().capture.is_some()
    });
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    shell
}

fn channels(shell: &mut Shell, id: u32, scl: Option<u8>, sda: Option<u8>) {
    // One at a time through "not assigned", so two roles never share a channel on the way.
    for (role, channel) in [("scl", None), ("sda", None), ("scl", scl), ("sda", sda)] {
        done(
            shell,
            Command::SetDecoderChannel {
                id,
                role: role.to_owned(),
                channel,
            },
        );
    }
}

fn keys(results: &Results, channel: Option<u8>) -> Vec<&'static str> {
    let snapshot = match channel {
        None => &results.lane,
        Some(c) => match results.overlays.get(&c) {
            Some(s) => s,
            None => return Vec::new(),
        },
    };
    snapshot
        .window(0, u64::MAX)
        .iter()
        .filter_map(|e| match &e.annotation.text {
            Text::Message { key, .. } => Some(*key),
            Text::Literal(_) => None,
        })
        .collect()
}

#[test]
fn the_i2c_decoder_is_in_the_catalogue_and_reads_a_capture_of_a_bus() {
    let dir = TempDir::new("i2c-host");
    let mut bus = Bus::default();
    bus.write(0x50, &[0x12, 0x34]);
    bus.write(0x51, &[0x56]);
    let mut shell = opened(&dir, &bus.words());
    let entry = shell
        .state()
        .decoders
        .catalogue
        .iter()
        .find(|c| c.id == ID)
        .expect("the decoder is registered");
    let roles: Vec<&str> = entry.signals.iter().map(|s| s.role.as_str()).collect();
    assert_eq!(roles, ["scl", "sda"]);
    assert!(entry.has_generator);

    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    channels(&mut shell, id, Some(SCL), Some(SDA));
    poll_until(&mut shell, "the pass", idle);
    let view = shell.state().decoders.instance(id).unwrap().clone();
    assert_eq!(view.state, InstanceState::Finished);
    assert!(!view.incomplete && view.annotations > 0);

    let results = shell.decoder_results(id).unwrap().clone();
    let lane = keys(&results, None);
    assert_eq!(
        lane.iter()
            .filter(|k| **k == "ann-i2c-address-write")
            .count(),
        2
    );
    assert_eq!(lane.iter().filter(|k| **k == "ann-i2c-data").count(), 3);
    assert_eq!(lane.iter().filter(|k| **k == "ann-i2c-stop").count(), 2);
    // Dots on SDA, arrows on SCL: 2 starts and 2 stops, and nine pulses for each of the 5 bytes.
    assert_eq!(keys(&results, Some(SDA)).len(), 4);
    assert_eq!(results.overlays[&SCL].len(), 45);

    // The frame plan has a lane named after the decoder and overlays on the two channels.
    let plan = shell.frame_plan(ViewSize {
        pixels: 800,
        scale: 1.0,
    });
    assert_eq!(plan.lanes.len(), 1);
    assert_eq!(plan.lanes[0].name, "I2C");
    let overlay_channels: Vec<u8> = plan.overlays.iter().map(|o| o.channel).collect();
    assert!(overlay_channels.contains(&SCL) && overlay_channels.contains(&SDA));

    // Swapping the channels runs the decoder again and gives another, valid result.
    channels(&mut shell, id, Some(SDA), Some(SCL));
    poll_until(&mut shell, "the second pass", idle);
    let view = shell.state().decoders.instance(id).unwrap().clone();
    assert_eq!(view.state, InstanceState::Finished);
    let swapped = shell.decoder_results(id).unwrap();
    assert_ne!(keys(swapped, None), lane);
    for entry in swapped.lane.window(0, u64::MAX) {
        match entry.annotation.kind {
            Kind::Range { start, end } => assert!(start <= end),
            Kind::Point { .. } | Kind::Edge { .. } => {}
        }
        assert!(matches!(
            entry.annotation.target,
            Target::Lane | Target::Channel(_)
        ));
    }
}

// ---------------------------------------------------------------------------------------------
// The virtual device makes traffic that is valid for the configured decoder (spec 008, T020).
// ---------------------------------------------------------------------------------------------

const SEED: u64 = 99;

fn wait_all(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        shell.poll(Instant::now());
        if shell.state().activity == Activity::Idle && idle(shell) {
            return;
        }
        assert!(Instant::now() < deadline, "did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A shell with a small capture and an I2C decoder on channels 0 and 1, ready for the next capture.
fn shell_with_decoder(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 2_000, 1);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    channels(&mut shell, id, Some(SCL), Some(SDA));
    wait_all(&mut shell);
    (shell, id)
}

fn capture_again(shell: &mut Shell, seed: u64, length: u64) {
    for edit in [ConfigEdit::Seed(Some(seed)), ConfigEdit::Length(length)] {
        done(shell, Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(shell, Command::Decide(Decision::Discard));
    } else {
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    }
}

/// The addresses and data bytes the decoder showed in the lane, in order: `(key, value)`.
fn decoded_bytes(shell: &Shell, id: u32) -> Vec<(String, u64)> {
    let results = shell.decoder_results(id).unwrap();
    let mut bytes: Vec<(u64, String, u64)> = results
        .lane
        .window(0, u64::MAX)
        .iter()
        .filter_map(|e| match (&e.annotation.kind, &e.annotation.text) {
            (Kind::Range { start, .. }, Text::Message { key, args })
                if key.starts_with("ann-i2c-address") || *key == "ann-i2c-data" =>
            {
                let value = u64::from_str_radix(args[0].trim_start_matches("0x"), 16).unwrap();
                let kind = if *key == "ann-i2c-data" {
                    "data"
                } else {
                    "address"
                };
                Some((*start, kind.to_owned(), value))
            }
            _ => None,
        })
        .collect();
    bytes.sort();
    bytes.into_iter().map(|(_, k, v)| (k, v)).collect()
}

fn expected_bytes(shell: &Shell, id: u32, seed: u64, length: u64) -> Vec<(String, u64)> {
    let setup = shell.state().decoders.instance(id).unwrap().setup.clone();
    la_decoders::generators::i2c::expected(&setup, seed, length)
        .into_iter()
        .filter_map(|item| {
            let la_decoders::model::Payload::Bytes(bytes) = &item.payload else {
                return None;
            };
            let kind = match bytes[0] {
                la_decoders::decoders::i2c::items::ADDRESS => "address",
                la_decoders::decoders::i2c::items::DATA => "data",
                _ => return None,
            };
            Some((
                kind.to_owned(),
                u64::from(bytes[4]) << 8 | u64::from(bytes[5]),
            ))
        })
        .collect()
}

#[test]
fn a_virtual_capture_decodes_to_the_traffic_that_was_generated() {
    let dir = TempDir::new("i2c-generated");
    let (mut shell, id) = shell_with_decoder(&dir);
    let length = 60_000;
    capture_again(&mut shell, SEED, length);
    wait_all(&mut shell);
    let shown = decoded_bytes(&shell, id);
    let wanted = expected_bytes(&shell, id, SEED, length);
    assert!(wanted.len() > 100, "too little traffic: {}", wanted.len());
    assert_eq!(shown, wanted);
    assert!(!shell.state().decoders.instance(id).unwrap().incomplete);

    // The same seed again gives the same signal and the same result; another seed another result.
    capture_again(&mut shell, SEED, length);
    wait_all(&mut shell);
    assert_eq!(decoded_bytes(&shell, id), shown);
    capture_again(&mut shell, SEED + 1, length);
    wait_all(&mut shell);
    assert_ne!(decoded_bytes(&shell, id), shown);
}

#[test]
fn channels_without_a_decoder_keep_the_ordinary_random_signal() {
    let dir = TempDir::new("i2c-other-channels");
    let mut shell = finished_shell(&dir, 0b1111, 5_000, 3);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    channels(&mut shell, id, Some(0), Some(1));
    wait_all(&mut shell);
    capture_again(&mut shell, 4, 5_000);
    wait_all(&mut shell);
    let capture = shell.state().capture.as_ref().unwrap();
    // Channels 2 and 3 were not claimed: they carry transitions of the random generator.
    for channel in [2_u8, 3] {
        let transitions = capture
            .snapshot
            .transitions(channel, 0, capture.snapshot.length())
            .unwrap()
            .count();
        assert!(transitions > 10, "channel {channel}: {transitions}");
    }
}

#[test]
fn ten_million_samples_decode_without_holding_the_interface() {
    let dir = TempDir::new("i2c-big");
    let (mut shell, id) = shell_with_decoder(&dir);
    shell.set_decoder_thread_cap(4);
    capture_again(&mut shell, 5, 10_000_000);
    let mut worst = Duration::ZERO;
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let t = Instant::now();
        shell.poll(Instant::now());
        worst = worst.max(t.elapsed());
        if shell.state().activity == Activity::Idle && idle(&shell) {
            break;
        }
        assert!(Instant::now() < deadline, "did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(worst < Duration::from_millis(100), "a poll took {worst:?}");
    let view = shell.state().decoders.instance(id).unwrap();
    assert_eq!(view.state, InstanceState::Finished);
    assert!(view.annotations > 100_000, "{}", view.annotations);
}
