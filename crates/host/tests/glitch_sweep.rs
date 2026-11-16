//! A one-sample glitch is visible at every zoom level (spec SC-003, constitution Principle VIII).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use la_host::plan::{ChannelView, FrameState, Segment, build_plan};
use la_host::presentation::Colour;
use la_host::window::{ViewWindow, WindowLimits};
use la_storage::{Capture, CaptureConfig, Limits, Snapshot};

const LENGTH: u64 = 2_000_000;
const GLITCH: u64 = 1_234_567;
const PIXELS: u32 = 800;

fn view() -> Vec<ChannelView> {
    vec![ChannelView {
        channel: 0,
        name: String::new(),
        colour: Colour::Palette(0),
        height: 0,
    }]
}

/// A long quiet low signal with a single high sample at `GLITCH`.
fn glitch_capture() -> Snapshot {
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut position = 0;
    while position < LENGTH {
        let length = (LENGTH - position).min(65_536);
        let chunk: Vec<u32> = (position..position + length)
            .map(|p| u32::from(p == GLITCH))
            .collect();
        capture.append(position, &chunk).unwrap();
        position += length;
    }
    capture.close().unwrap();
    capture.reader().snapshot()
}

fn shows_the_glitch(segments: &[Segment]) -> bool {
    segments.iter().any(|s| {
        matches!(
            s,
            Segment::Edge { .. } | Segment::Dense { .. } | Segment::Level { high: true, .. }
        )
    })
}

#[test]
fn the_glitch_is_visible_from_the_whole_capture_to_many_pixels_per_sample() {
    let snapshot = glitch_capture();
    let limits = WindowLimits {
        capture_length: LENGTH,
        pixels: PIXELS,
    };
    // Zoom in around the glitch, 60 steps from the whole capture to the minimum window.
    let mut window = ViewWindow::fit_all(&limits);
    let mut levels = 0;
    let mut lengths = Vec::new();
    for step in 0..60 {
        assert!(
            window.start <= GLITCH && GLITCH < window.end(),
            "step {step}"
        );
        let plan = build_plan(Some(&snapshot), &window, PIXELS, &view());
        assert_eq!(plan.state, FrameState::Ok);
        assert!(
            shows_the_glitch(&plan.channels[0].segments),
            "step {step}: window {window:?} hides the glitch: {:?}",
            plan.channels[0].segments
        );
        lengths.push(window.length);
        levels += 1;
        // Keep the glitch under the same fraction of the window while zooming in by 3/4.
        let fraction = (GLITCH - window.start, window.length);
        window = window.zoom(&limits, 3, 4, fraction);
    }
    assert_eq!(levels, 60);
    assert_eq!(*lengths.last().unwrap(), limits.min_length());
    // The sweep really went through very different scales.
    assert!(lengths[0] == LENGTH && lengths[59] <= 13);

    // Also with the glitch near the left and the right edge of the window at every scale.
    for fraction in [
        (0_u64, 1000_u64),
        (1, 1000),
        (500, 1000),
        (999, 1000),
        (1000, 1000),
    ] {
        let mut window = ViewWindow::fit_all(&limits);
        for _ in 0..60 {
            window = window.zoom(&limits, 3, 4, fraction);
            if window.start <= GLITCH && GLITCH < window.end() {
                let plan = build_plan(Some(&snapshot), &window, PIXELS, &view());
                assert!(
                    shows_the_glitch(&plan.channels[0].segments),
                    "{fraction:?} {window:?}"
                );
            }
        }
    }
}

#[test]
fn a_dense_column_reveals_every_transition_when_zoomed_in() {
    // Alternating levels for six samples: five transitions inside one tiny stretch.
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut samples = vec![0_u32; 1000];
    for (i, sample) in samples.iter_mut().enumerate().skip(500).take(6) {
        *sample = u32::try_from(i % 2).unwrap();
    }
    capture.append(0, &samples).unwrap();
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();

    // Zoomed out, the stretch is one dense column (several transitions in one pixel).
    let wide = ViewWindow {
        start: 0,
        length: 1000,
    };
    let plan = build_plan(Some(&snapshot), &wide, 100, &view());
    let dense: Vec<u32> = plan.channels[0]
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::Dense { count, .. } => Some(*count),
            _ => None,
        })
        .collect();
    assert_eq!(
        dense.iter().sum::<u32>() + edge_count(&plan.channels[0].segments),
        transitions(&snapshot)
    );

    // Zoomed in to a few pixels per sample, every transition is its own edge.
    let narrow = ViewWindow {
        start: 498,
        length: 12,
    };
    let plan = build_plan(Some(&snapshot), &narrow, 600, &view());
    let segments = &plan.channels[0].segments;
    assert!(!segments.iter().any(|s| matches!(s, Segment::Dense { .. })));
    assert_eq!(edge_count(segments), transitions(&snapshot));
}

fn edge_count(segments: &[Segment]) -> u32 {
    u32::try_from(
        segments
            .iter()
            .filter(|s| matches!(s, Segment::Edge { .. }))
            .count(),
    )
    .unwrap()
}

fn transitions(snapshot: &Snapshot) -> u32 {
    u32::try_from(
        snapshot
            .transitions(0, 0, snapshot.length())
            .unwrap()
            .count(),
    )
    .unwrap()
}
