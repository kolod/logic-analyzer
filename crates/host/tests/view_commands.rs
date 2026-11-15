//! Zoom, pan, fit and follow through the shell (spec US2).
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
use la_host::window::{ViewSize, ViewWindow};
use la_host::{
    Changes, Command, ConfigEdit, Outcome, PanStep, Shell, ShellConfig, ZoomAnchor, ZoomDirection,
    ZoomFactor, error,
};

fn window(shell: &Shell) -> (u64, u64) {
    let w = shell.state().window;
    (w.start, w.length)
}

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

#[test]
fn without_a_capture_the_view_commands_are_ignored() {
    let dir = TempDir::new("view-none");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for command in [
        Command::ZoomStep(ZoomDirection::In),
        Command::Pan { pixels: 10 },
        Command::PanStep(PanStep::Left),
        Command::FitAll,
        Command::GoToStart,
        Command::GoToEnd,
        Command::ZoomToRange { start: 1, end: 5 },
    ] {
        assert_eq!(
            shell.apply(command.clone()),
            Outcome::Ignored(error::INVALID_COMMAND),
            "{command:?}"
        );
    }
}

#[test]
fn zoom_and_pan_move_the_window_by_the_documented_amounts() {
    let dir = TempDir::new("view-moves");
    let mut shell = finished_shell(&dir, 0b11, 100_000, 1);
    assert_eq!(window(&shell), (0, 100_000));
    assert!(shell.state().auto_fit);
    let before = shell.state().versions;

    let changes = done(&mut shell, Command::ZoomStep(ZoomDirection::In));
    assert_eq!(changes, Changes::WINDOW);
    assert_eq!(window(&shell), (25_000, 50_000));
    assert!(!shell.state().auto_fit);
    let after = shell.state().versions;
    assert_eq!(after.window, before.window + 1);
    assert_eq!(after.capture, before.capture);
    assert_eq!(after.presentation, before.presentation);

    // 80 pixels of 800 over 50,000 samples is 5,000 samples.
    done(&mut shell, Command::Pan { pixels: 80 });
    assert_eq!(window(&shell), (30_000, 50_000));
    done(&mut shell, Command::PanStep(PanStep::Left));
    assert_eq!(window(&shell), (25_000, 50_000));
    done(&mut shell, Command::PanStep(PanStep::PageRight));
    assert_eq!(window(&shell), (50_000, 50_000));
    done(&mut shell, Command::GoToStart);
    assert_eq!(window(&shell), (0, 50_000));
    done(&mut shell, Command::GoToEnd);
    assert_eq!(window(&shell), (50_000, 50_000));

    // Zoom out around the left edge, then the right edge.
    done(
        &mut shell,
        Command::Zoom {
            factor: ZoomFactor {
                numerator: 1,
                denominator: 2,
            },
            anchor: ZoomAnchor::Column(0),
        },
    );
    assert_eq!(window(&shell), (50_000, 25_000));
    done(
        &mut shell,
        Command::Zoom {
            factor: ZoomFactor {
                numerator: 2,
                denominator: 1,
            },
            anchor: ZoomAnchor::Column(800),
        },
    );
    assert_eq!(window(&shell), (25_000, 50_000));

    done(
        &mut shell,
        Command::ZoomToRange {
            start: 10_000,
            end: 20_000,
        },
    );
    assert_eq!(window(&shell), (10_000, 10_000));
    done(&mut shell, Command::FitAll);
    assert_eq!(window(&shell), (0, 100_000));
    assert!(shell.state().auto_fit);
}

#[test]
fn every_view_command_keeps_the_window_valid() {
    let dir = TempDir::new("view-valid");
    let mut shell = finished_shell(&dir, 0b1, 5_000, 2);
    let mut rng = common::Lcg(7);
    for step in 0..2_000 {
        let command = match rng.below(9) {
            0 => Command::ZoomStep(ZoomDirection::In),
            1 => Command::ZoomStep(ZoomDirection::Out),
            2 => Command::Pan {
                pixels: i64::try_from(rng.below(4000)).unwrap() - 2000,
            },
            3 => Command::PanStep(PanStep::PageLeft),
            4 => Command::PanStep(PanStep::Right),
            5 => Command::ZoomToRange {
                start: rng.below(6_000),
                end: rng.below(6_000),
            },
            6 => Command::GoToEnd,
            7 => Command::FitAll,
            _ => Command::SetViewSize(ViewSize {
                pixels: u32::try_from(1 + rng.below(3000)).unwrap(),
                scale: 1.0,
            }),
        };
        shell.apply(command);
        let state = shell.state();
        let limits = la_host::window::WindowLimits {
            capture_length: state.capture.as_ref().unwrap().snapshot.length(),
            pixels: state.view_size.pixels,
        };
        assert!(
            state.window.is_valid(&limits),
            "step {step}: {:?}",
            state.window
        );
    }
}

#[test]
fn a_new_view_size_reclamps_the_window() {
    let dir = TempDir::new("view-size");
    let mut shell = finished_shell(&dir, 0b1, 100_000, 3);
    done(&mut shell, Command::ZoomToRange { start: 0, end: 100 });
    // At most 64 pixels per sample: 16,000 pixels need a window of at least 250 samples.
    let changes = done(
        &mut shell,
        Command::SetViewSize(ViewSize {
            pixels: 16_000,
            scale: 1.0,
        }),
    );
    assert_eq!(changes, Changes::WINDOW);
    let w: ViewWindow = shell.state().window;
    assert_eq!(w.length, 250);
}

#[test]
fn following_the_newest_data_tracks_the_end_until_the_user_pans() {
    let dir = TempDir::new("view-follow");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Channels(0b1),
        ConfigEdit::SampleRate(20_000),
        ConfigEdit::Length(1_000_000),
        ConfigEdit::Seed(Some(4)),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    shell.apply(Command::StartCapture);
    let wait = |shell: &mut Shell, samples: u64| {
        let deadline = Instant::now() + Duration::from_secs(20);
        while shell.state().capture.as_ref().unwrap().snapshot.length() < samples {
            shell.poll(Instant::now());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait(&mut shell, 2_000);
    // Zoomed in to a fixed length, following the end.
    done(&mut shell, Command::SetFollowLatest(true));
    done(&mut shell, Command::ZoomToRange { start: 0, end: 800 });
    done(&mut shell, Command::GoToEnd);
    done(&mut shell, Command::SetFollowLatest(true));
    let length = shell.state().window.length;
    wait(&mut shell, 6_000);
    let capture_length = shell.state().capture.as_ref().unwrap().snapshot.length();
    let w = shell.state().window;
    assert_eq!(w.length, length);
    assert_eq!(
        w.start + w.length,
        capture_length,
        "the window follows the end"
    );

    // A manual pan turns following off: the window stays where the user put it.
    done(&mut shell, Command::Pan { pixels: -200 });
    assert!(!shell.state().follow_latest);
    let kept = shell.state().window;
    wait(&mut shell, capture_length + 2_000);
    assert_eq!(shell.state().window, kept);
    shell.apply(Command::StopCapture);
}

#[test]
fn a_huge_window_size_and_a_new_short_capture_keep_the_stored_window_valid() {
    let dir = TempDir::new("view-huge");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(0b1),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(10),
        ConfigEdit::Seed(Some(61)),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: u32::MAX,
        scale: 1.0,
    }));
    // Zoomed by hand (so the window does not follow "fit all"), then a capture of 10 samples.
    shell.apply(Command::StartCapture);
    // Right after the start, before anything was polled, the window is already valid.
    let state = shell.state();
    let limits = la_host::window::WindowLimits {
        capture_length: state.capture.as_ref().map_or(0, |c| c.snapshot.length()),
        pixels: state.view_size.pixels.clamp(1, 16_384),
    };
    assert!(
        state.window.is_valid(&limits),
        "{:?} for {limits:?}",
        state.window
    );
    shell.apply(Command::GoToStart);
    let deadline = Instant::now() + Duration::from_secs(20);
    while shell.state().activity != la_host::Activity::Idle {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let state = shell.state();
    let limits = la_host::window::WindowLimits {
        capture_length: state.capture.as_ref().unwrap().snapshot.length(),
        pixels: state.view_size.pixels.clamp(1, 16_384),
    };
    assert!(
        state.window.is_valid(&limits),
        "{:?} for {limits:?}",
        state.window
    );
}
