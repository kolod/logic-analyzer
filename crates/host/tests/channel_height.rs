//! The height of the signal of a channel (spec 014, user story 1, task T005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell};
use la_host::window::ViewSize;
use la_host::{Activity, Changes, Command, ConfigEdit, Outcome, Shell, ShellConfig, error};

const SIZE: ViewSize = ViewSize {
    pixels: 400,
    scale: 1.0,
};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

fn height_of(shell: &Shell, channel: u8) -> u16 {
    shell
        .state()
        .presentation
        .channels
        .iter()
        .find(|s| s.index == channel)
        .unwrap()
        .height
}

fn set(channel: u8, height: u16) -> Command {
    Command::SetChannelHeight { channel, height }
}

#[test]
fn a_height_is_stored_clamped_and_zero_means_automatic() {
    let dir = TempDir::new("height-set");
    let mut shell = finished_shell(&dir, 0b111, 2_000, 1);
    assert_eq!(height_of(&shell, 0), 0, "automatic by default");
    let changes = done(&mut shell, set(0, 60));
    assert!(changes.contains(Changes::PRESENTATION));
    assert_eq!(height_of(&shell, 0), 60);
    assert_eq!(height_of(&shell, 1), 0, "the others are not touched");
    done(&mut shell, set(0, 10));
    assert_eq!(height_of(&shell, 0), 24);
    done(&mut shell, set(0, 999));
    assert_eq!(height_of(&shell, 0), 240);
    done(&mut shell, set(0, 24));
    assert_eq!(height_of(&shell, 0), 24);
    done(&mut shell, set(0, 240));
    assert_eq!(height_of(&shell, 0), 240);
    done(&mut shell, set(0, 0));
    assert_eq!(height_of(&shell, 0), 0);
}

#[test]
fn an_unknown_channel_is_refused_and_nothing_changes() {
    let dir = TempDir::new("height-unknown");
    let mut shell = finished_shell(&dir, 0b1, 2_000, 1);
    let version = shell.state().version;
    match shell.apply(set(40, 50)) {
        Outcome::Ignored(code) => assert_eq!(code, error::INVALID_COMMAND),
        other => panic!("{other:?}"),
    }
    assert_eq!(shell.state().version, version);
}

#[test]
fn it_works_without_a_capture_and_while_one_runs() {
    let dir = TempDir::new("height-timing");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert!(shell.state().capture.is_none());
    done(&mut shell, set(3, 80));
    assert_eq!(height_of(&shell, 3), 80);
    // A capture that runs: the heights, like names and colours, can still be edited.
    for edit in [
        ConfigEdit::Realtime(true),
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0b1111)),
        ConfigEdit::SampleRate(1_000),
        ConfigEdit::Length(100_000),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    assert_eq!(shell.state().activity, Activity::Capturing);
    done(&mut shell, set(3, 100));
    assert_eq!(height_of(&shell, 3), 100);
    done(&mut shell, Command::StopCapture);
}

#[test]
fn the_height_follows_its_channel_when_it_is_moved_and_is_kept_for_the_next_capture() {
    let dir = TempDir::new("height-move");
    let mut shell = finished_shell(&dir, 0b111, 2_000, 1);
    done(&mut shell, set(2, 90));
    done(&mut shell, Command::MoveChannel { from: 2, to: 0 });
    let first = &shell.state().presentation.channels[0];
    assert_eq!((first.index, first.height), (2, 90));
    // The plan carries the height of every channel it shows, in display order.
    let plan = shell.frame_plan(SIZE);
    let heights: Vec<(u8, u16)> = plan
        .channels
        .iter()
        .map(|c| (c.channel, c.height))
        .collect();
    assert_eq!(heights, vec![(2, 90), (0, 0), (1, 0)]);
    // The next capture keeps it.
    for edit in [ConfigEdit::Length(3_000), ConfigEdit::Seed(Some(9))] {
        done(&mut shell, Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    if matches!(outcome, Outcome::NeedsDecision(_)) {
        done(&mut shell, Command::Decide(la_host::Decision::Discard));
    }
    while shell.state().activity != Activity::Idle {
        shell.poll(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(height_of(&shell, 2), 90);
}

#[test]
fn resetting_the_presentation_makes_every_height_automatic() {
    let dir = TempDir::new("height-reset");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 1);
    done(&mut shell, set(0, 50));
    done(&mut shell, set(1, 150));
    done(&mut shell, Command::ResetPresentation);
    assert_eq!(height_of(&shell, 0), 0);
    assert_eq!(height_of(&shell, 1), 0);
}
