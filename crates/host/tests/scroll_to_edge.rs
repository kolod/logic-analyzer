//! Scrolling a signal to its next or previous edge (spec 014, user story 4, task T032).
//!
//! The captures are built by hand and opened as session files, so the edges are where the test says.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell, wait_operation};
use la_host::window::ViewSize;
use la_host::{Command, EdgeDirection, Outcome, Shell, ShellConfig};
use la_session::{Control, LoadLimits, SessionData};
use la_storage::{Capture, CaptureConfig, Limits};

const SIZE: ViewSize = ViewSize {
    pixels: 800,
    scale: 1.0,
};

/// A capture of `length` samples on two channels: `level(channel, position)` says the level, and a
/// gap of `gap.1` positions is recorded at `gap.0` when asked for.
fn snapshot(
    channels: u8,
    length: u64,
    gap: Option<(u64, u64)>,
    level: impl Fn(u8, u64) -> bool,
) -> la_storage::Snapshot {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut words = Vec::new();
    let mut position = 0_u64;
    let mut gap = gap;
    while position < length {
        if let Some((at, size)) = gap {
            if position == at {
                capture.record_gap(size).unwrap();
                position = capture.length();
                gap = None;
                continue;
            }
        }
        // A chunk ends at the gap, if one is still to come.
        let stop = gap.map_or(length, |(at, _)| at.min(length));
        let end = (position + 4096).min(stop);
        words.clear();
        for p in position..end {
            let mut word = 0_u32;
            for channel in 0..channels.min(32) {
                if level(channel, p) {
                    word |= 1 << channel;
                }
            }
            words.push(word);
        }
        capture.append(capture.length(), &words).unwrap();
        position = capture.length();
    }
    capture.close().unwrap();
    capture.reader().snapshot()
}

/// A shell showing `snapshot` as an opened session.
fn shell_with(dir: &TempDir, build: impl FnOnce(u8) -> la_storage::Snapshot) -> Shell {
    // Any finished capture gives a description with the two channels recorded.
    let mut model = finished_shell(dir, 0b11, 2_000, 1);
    let source = dir.path().join("model.lasession");
    let outcome = model.apply(Command::SaveSessionAs(source.clone()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut model);
    let session =
        la_session::load_blocking(&source, &LoadLimits::default(), &Control::new()).unwrap();
    let snapshot = build(session.description().channels());
    let data = SessionData::new(session.description().clone(), snapshot);
    let path = dir.path().join("built.lasession");
    la_session::save_blocking(&path, &data, &Control::new()).unwrap();
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let outcome = shell.apply(Command::OpenSession(path));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut shell);
    shell.apply(Command::SetViewSize(SIZE));
    shell
}

fn zoom(shell: &mut Shell, start: u64, end: u64) {
    let outcome = shell.apply(Command::ZoomToRange { start, end });
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn scroll(shell: &mut Shell, channel: u8, direction: EdgeDirection) -> Outcome {
    shell.apply(Command::ScrollToEdge { channel, direction })
}

fn window(shell: &Shell) -> (u64, u64) {
    let plan = shell.frame_plan(SIZE);
    (plan.window.start, plan.window.length)
}

/// Channel 0: low until 1,000, high until 90,000, low after; channel 1: low, high from 199,950.
fn two_edges(channels: u8) -> la_storage::Snapshot {
    snapshot(channels, 200_000, None, |channel, p| match channel {
        0 => (1_000..90_000).contains(&p),
        _ => p >= 199_950,
    })
}

#[test]
fn the_edge_lands_a_quarter_of_the_window_from_the_side_with_the_zoom_unchanged() {
    let dir = TempDir::new("edge-quarter");
    let mut shell = shell_with(&dir, two_edges);
    zoom(&mut shell, 0, 400);
    let outcome = scroll(&mut shell, 0, EdgeDirection::Next);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert_eq!(window(&shell), (900, 400));
    scroll(&mut shell, 0, EdgeDirection::Next);
    assert_eq!(window(&shell), (89_900, 400));
    scroll(&mut shell, 0, EdgeDirection::Previous);
    assert_eq!(window(&shell), (700, 400));
    assert!(!shell.state().follow_latest);
}

#[test]
fn a_window_near_an_end_is_clamped_and_a_long_window_starts_at_zero() {
    let dir = TempDir::new("edge-clamp");
    let mut shell = shell_with(&dir, two_edges);
    zoom(&mut shell, 50_000, 54_000);
    scroll(&mut shell, 0, EdgeDirection::Previous);
    assert_eq!(window(&shell), (0, 4_000), "1,000 - 3,000 is clamped to 0");
    zoom(&mut shell, 0, 400);
    scroll(&mut shell, 1, EdgeDirection::Next);
    assert_eq!(window(&shell), (199_600, 400), "199,950 - 100 is clamped");
}

#[test]
fn nothing_changes_without_an_edge_on_that_side_or_with_a_wrong_channel() {
    let dir = TempDir::new("edge-none");
    let mut shell = shell_with(&dir, two_edges);
    zoom(&mut shell, 100_000, 100_400);
    let before = (window(&shell), shell.state().follow_latest);
    // No edge of channel 0 after 90,000.
    for (channel, direction) in [
        (0, EdgeDirection::Next),
        (9, EdgeDirection::Next),
        (9, EdgeDirection::Previous),
    ] {
        let outcome = scroll(&mut shell, channel, direction);
        assert!(
            matches!(outcome, Outcome::Ignored(c) if c == "host.invalid_command"),
            "{channel} {outcome:?}"
        );
        assert_eq!((window(&shell), shell.state().follow_latest), before);
    }
    // A hidden channel is not scrolled.
    let hidden = shell.apply(Command::SetChannelVisible {
        channel: 0,
        visible: false,
    });
    assert!(matches!(hidden, Outcome::Done(_)), "{hidden:?}");
    zoom(&mut shell, 0, 400);
    let outcome = scroll(&mut shell, 0, EdgeDirection::Next);
    assert!(matches!(outcome, Outcome::Ignored(_)), "{outcome:?}");
    assert_eq!(window(&shell), (0, 400));
    // No capture, no scroll.
    let mut empty = Shell::new(ShellConfig::for_tests(dir.path()));
    let outcome = scroll(&mut empty, 0, EdgeDirection::Next);
    assert!(matches!(outcome, Outcome::Ignored(_)), "{outcome:?}");
}

#[test]
fn the_plan_says_where_edges_lie_for_every_shown_channel() {
    let dir = TempDir::new("edge-plan");
    let mut shell = shell_with(&dir, two_edges);
    zoom(&mut shell, 0, 400);
    let plan = shell.frame_plan(SIZE);
    let flags = |plan: &la_host::plan::FramePlan| {
        plan.channels
            .iter()
            .map(|c| (c.channel, c.edge_before, c.edge_after))
            .collect::<Vec<_>>()
    };
    assert_eq!(flags(&plan), vec![(0, false, true), (1, false, true)]);
    zoom(&mut shell, 50_000, 50_400);
    assert_eq!(
        flags(&shell.frame_plan(SIZE)),
        vec![(0, true, true), (1, false, true)]
    );
    zoom(&mut shell, 199_000, 199_400);
    assert_eq!(
        flags(&shell.frame_plan(SIZE)),
        vec![(0, true, false), (1, false, true)]
    );
    zoom(&mut shell, 199_960, 200_000);
    assert_eq!(
        flags(&shell.frame_plan(SIZE)),
        vec![(0, true, false), (1, true, false)]
    );
    // A hidden channel is not in the plan at all.
    shell.apply(Command::SetChannelVisible {
        channel: 1,
        visible: false,
    });
    assert_eq!(flags(&shell.frame_plan(SIZE)), vec![(0, true, false)]);
}

#[test]
fn an_edge_next_to_a_gap_is_not_an_edge() {
    let dir = TempDir::new("edge-gap");
    // Channel 0 is low before a gap of 100 positions at 1,000, high after it until 5,000. The change
    // at the gap is no edge; the fall at 5,000 is.
    let mut shell = shell_with(&dir, |channels| {
        snapshot(
            channels,
            8_000,
            Some((1_000, 100)),
            |channel, p| match channel {
                0 => (1_100..5_000).contains(&p),
                _ => false,
            },
        )
    });
    zoom(&mut shell, 0, 400);
    let plan = shell.frame_plan(SIZE);
    assert!(plan.channels[0].edge_after, "the fall at 5,000 lies ahead");
    scroll(&mut shell, 0, EdgeDirection::Next);
    assert_eq!(window(&shell), (4_900, 400));
    // Only the gap lies before: no edge on that side.
    let outcome = scroll(&mut shell, 0, EdgeDirection::Previous);
    assert!(matches!(outcome, Outcome::Ignored(_)), "{outcome:?}");
    assert!(!shell.frame_plan(SIZE).channels[0].edge_before);
}
