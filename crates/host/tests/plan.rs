//! The frame plan against a brute-force reference (data-model "Frame plan").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains,
    clippy::panic
)]

mod common;

use common::{Shape, capture_snapshot};
use la_host::error;
use la_host::plan::{ChannelView, FrameState, Segment, build_plan};
use la_host::presentation::Colour;
use la_host::window::ViewWindow;
use la_storage::{Capture, CaptureConfig, Limits, Snapshot};

fn view(channels: &[u8]) -> Vec<ChannelView> {
    channels
        .iter()
        .map(|c| ChannelView {
            channel: *c,
            name: format!("ch{c}"),
            colour: Colour::Palette(*c % 16),
        })
        .collect()
}

/// What one pixel column shows.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Column {
    Level(bool),
    Edge { rising: bool, x: f64 },
    Dense(u32),
    Gap,
    Beyond,
}

/// Expands the segments of a channel back into one entry per column.
fn expand(segments: &[Segment], pixels: u32) -> Vec<Column> {
    let mut columns = Vec::new();
    for segment in segments {
        match *segment {
            Segment::Level { from, to, high } => {
                columns.extend((from..to).map(|_| Column::Level(high)));
            }
            Segment::Edge { x, rising, .. } => columns.push(Column::Edge { rising, x }),
            Segment::Dense { count, .. } => columns.push(Column::Dense(count)),
            Segment::Gap { from, to } => columns.extend((from..to).map(|_| Column::Gap)),
            Segment::Beyond { from, to } => columns.extend((from..to).map(|_| Column::Beyond)),
        }
    }
    assert_eq!(columns.len(), usize::try_from(pixels).unwrap());
    columns
}

/// The columns computed straight from the stored transitions and gaps (no viewport query).
fn reference(snapshot: &Snapshot, channel: u8, window: ViewWindow, pixels: u32) -> Vec<Column> {
    let length = snapshot.length();
    let initial = snapshot.initial_high(channel).unwrap();
    let transitions: Vec<(u64, bool)> = if length == 0 {
        Vec::new()
    } else {
        snapshot
            .transitions(channel, 0, length)
            .unwrap()
            .map(|e| (e.position, e.rising))
            .collect()
    };
    let gaps = snapshot.gaps();
    let level_at = |x: u64| {
        let flips = transitions.iter().filter(|(p, _)| *p <= x).count();
        initial ^ (flips % 2 == 1)
    };
    let boundary = |i: u32| -> u64 {
        window.start
            + u64::try_from(u128::from(i) * u128::from(window.length) / u128::from(pixels)).unwrap()
    };
    (0..pixels)
        .map(|i| {
            let a = boundary(i);
            let b = boundary(i + 1);
            if a >= length {
                return Column::Beyond;
            }
            // The column touches a gap if a gap overlaps the closed range [a, b].
            let touches = gaps.iter().any(|g| g.start <= b && g.start + g.length > a);
            if touches {
                return Column::Gap;
            }
            let inside: Vec<&(u64, bool)> = transitions
                .iter()
                .filter(|(p, _)| *p > a && *p <= b)
                .collect();
            match inside.len() {
                0 => Column::Level(level_at(a)),
                1 => {
                    let (p, rising) = *inside[0];
                    let x = (p - window.start) as f64 * f64::from(pixels) / window.length as f64;
                    Column::Edge { rising, x }
                }
                n => Column::Dense(u32::try_from(n).unwrap()),
            }
        })
        .collect()
}

fn assert_same(actual: &[Column], expected: &[Column], context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}");
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        match (a, e) {
            (Column::Edge { rising: ra, x: xa }, Column::Edge { rising: re, x: xe }) => {
                assert_eq!(ra, re, "{context} column {i}");
                assert!((xa - xe).abs() < 1e-6, "{context} column {i}: {xa} vs {xe}");
            }
            _ => assert_eq!(a, e, "{context} column {i}"),
        }
    }
}

fn assert_invariants(segments: &[Segment], pixels: u32) {
    let mut next = 0_u32;
    let mut previous: Option<&Segment> = None;
    for segment in segments {
        let (from, to) = segment.range();
        assert_eq!(from, next, "contiguous: {segments:?}");
        assert!(to > from, "non-empty: {segment:?}");
        next = to;
        if let Some(prev) = previous {
            match (prev, segment) {
                (Segment::Level { high: a, .. }, Segment::Level { high: b, .. }) => {
                    assert_ne!(a, b, "adjacent equal levels are merged");
                }
                (Segment::Gap { .. }, Segment::Gap { .. })
                | (Segment::Beyond { .. }, Segment::Beyond { .. }) => {
                    panic!("adjacent equal segments are merged: {prev:?} {segment:?}");
                }
                _ => {}
            }
        }
        previous = Some(segment);
    }
    assert_eq!(next, pixels, "covers every column");
}

#[test]
fn the_plan_equals_the_brute_force_reference_on_random_captures() {
    let shapes = [
        Shape {
            channels: 3,
            samples: 20_000,
            max_run: 40,
            gaps: 0,
        },
        Shape {
            channels: 8,
            samples: 60_000,
            max_run: 5,
            gaps: 3,
        },
        Shape {
            channels: 2,
            samples: 5_000,
            max_run: 1,
            gaps: 0,
        },
        Shape {
            channels: 4,
            samples: 30_000,
            max_run: 3_000,
            gaps: 2,
        },
    ];
    for (shape_index, shape) in shapes.iter().enumerate() {
        let snapshot = capture_snapshot(100 + shape_index as u64, shape);
        let length = snapshot.length();
        let windows = [
            ViewWindow { start: 0, length },
            ViewWindow {
                start: length / 4,
                length: length / 2,
            },
            ViewWindow {
                start: length / 3,
                length: 997.min(length / 3),
            },
            ViewWindow {
                start: length / 2,
                length: 40,
            },
            ViewWindow {
                start: length.saturating_sub(100),
                length: 100,
            },
        ];
        for window in windows {
            for pixels in [1_u32, 50, 333, 800] {
                let channels: Vec<u8> = (0..shape.channels).collect();
                let plan = build_plan(Some(&snapshot), &window, pixels, &view(&channels));
                assert_eq!(plan.state, FrameState::Ok);
                assert_eq!(plan.channels.len(), channels.len());
                for channel_plan in &plan.channels {
                    assert_invariants(&channel_plan.segments, pixels);
                    let expected = reference(&snapshot, channel_plan.channel, window, pixels);
                    let actual = expand(&channel_plan.segments, pixels);
                    assert_same(
                        &actual,
                        &expected,
                        &format!(
                            "shape {shape_index} {window:?} W={pixels} channel {}",
                            channel_plan.channel
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn a_quiet_channel_is_one_segment_however_long_the_capture_is() {
    // One channel that never changes level for a million samples.
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1000,
        limits: Limits::default(),
    })
    .unwrap();
    for _ in 0..16 {
        capture
            .append(capture.length(), &vec![1_u32; 65_536])
            .unwrap();
    }
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    let window = ViewWindow {
        start: 0,
        length: snapshot.length(),
    };
    let plan = build_plan(Some(&snapshot), &window, 1600, &view(&[0]));
    assert_eq!(
        plan.channels[0].segments,
        vec![Segment::Level {
            from: 0,
            to: 1600,
            high: true
        }]
    );
}

#[test]
fn a_gap_is_a_gap_and_never_a_level() {
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1000,
        limits: Limits::default(),
    })
    .unwrap();
    capture.append(0, &vec![0_u32; 1000]).unwrap();
    capture.record_gap(1000).unwrap();
    capture
        .append(capture.length(), &vec![1_u32; 1000])
        .unwrap();
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    let window = ViewWindow {
        start: 0,
        length: 3000,
    };
    let plan = build_plan(Some(&snapshot), &window, 300, &view(&[0]));
    let segments = &plan.channels[0].segments;
    assert!(segments.iter().any(|s| matches!(s, Segment::Gap { .. })));
    assert_invariants(segments, 300);
    // Columns 100..200 lie inside the gap (the columns touching its edges are gaps too).
    let columns = expand(segments, 300);
    assert!(columns[101..199].iter().all(|c| *c == Column::Gap));
    assert_eq!(columns[50], Column::Level(false));
    assert_eq!(columns[250], Column::Level(true));
}

#[test]
fn columns_past_the_end_are_beyond() {
    let snapshot = capture_snapshot(
        7,
        &Shape {
            channels: 1,
            samples: 1000,
            max_run: 30,
            gaps: 0,
        },
    );
    let window = ViewWindow {
        start: 900,
        length: 200,
    };
    let plan = build_plan(Some(&snapshot), &window, 200, &view(&[0]));
    assert_eq!(plan.state, FrameState::Ok);
    let segments = &plan.channels[0].segments;
    assert_invariants(segments, 200);
    assert!(matches!(
        segments.last().unwrap(),
        Segment::Beyond { to: 200, .. }
    ));
    let expected = reference(&snapshot, 0, window, 200);
    assert_same(&expand(segments, 200), &expected, "past the end");
}

#[test]
fn an_empty_capture_has_the_empty_state_and_beyond_lanes() {
    let capture = Capture::new(CaptureConfig {
        channels: 2,
        sample_rate_hz: 1000,
        limits: Limits::default(),
    })
    .unwrap();
    let snapshot = capture.reader().snapshot();
    let window = ViewWindow {
        start: 0,
        length: 13,
    };
    let plan = build_plan(Some(&snapshot), &window, 800, &view(&[0, 1]));
    assert_eq!(plan.state, FrameState::Empty);
    assert_eq!(plan.channels.len(), 2);
    assert_eq!(
        plan.channels[0].segments,
        vec![Segment::Beyond { from: 0, to: 800 }]
    );
}

#[test]
fn no_capture_gives_the_no_capture_state() {
    let window = ViewWindow {
        start: 0,
        length: 13,
    };
    let plan = build_plan(None, &window, 800, &view(&[0]));
    assert_eq!(plan.state, FrameState::NoCapture);
    assert!(plan.channels.is_empty());
    assert!(plan.axis.ticks.is_empty());
}

#[test]
fn only_the_given_channels_are_planned_in_the_given_order() {
    let snapshot = capture_snapshot(
        9,
        &Shape {
            channels: 4,
            samples: 2000,
            max_run: 20,
            gaps: 0,
        },
    );
    let window = ViewWindow {
        start: 0,
        length: 2000,
    };
    let plan = build_plan(Some(&snapshot), &window, 100, &view(&[3, 1]));
    let order: Vec<u8> = plan.channels.iter().map(|c| c.channel).collect();
    assert_eq!(order, [3, 1]);
    assert_eq!(plan.channels[0].name, "ch3");
    assert_eq!(plan.channels[1].colour, Colour::Palette(1));
}

#[test]
fn an_edge_reports_its_sub_pixel_position() {
    // A single rising edge at sample 500 of a 1000-sample window shown over 100 pixels.
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut samples = vec![0_u32; 500];
    samples.extend(vec![1_u32; 500]);
    capture.append(0, &samples).unwrap();
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    let window = ViewWindow {
        start: 0,
        length: 1000,
    };
    let plan = build_plan(Some(&snapshot), &window, 100, &view(&[0]));
    let edge = plan.channels[0]
        .segments
        .iter()
        .find_map(|s| match s {
            Segment::Edge { column, x, rising } => Some((*column, *x, *rising)),
            _ => None,
        })
        .unwrap();
    assert_eq!(edge.0, 49);
    assert!((edge.1 - 50.0).abs() < 1e-9);
    assert!(edge.2);
}

#[test]
fn a_failing_query_gives_the_cannot_draw_state_without_a_panic() {
    let snapshot = capture_snapshot(
        11,
        &Shape {
            channels: 2,
            samples: 1000,
            max_run: 10,
            gaps: 0,
        },
    );
    let window = ViewWindow {
        start: 0,
        length: 1000,
    };
    let cannot = FrameState::CannotDraw(error::RENDER_FAILED);
    // Zero pixels, more pixels than the storage allows, and a channel the capture does not have.
    assert_eq!(
        build_plan(Some(&snapshot), &window, 0, &view(&[0])).state,
        cannot
    );
    assert_eq!(
        build_plan(Some(&snapshot), &window, 100_000, &view(&[0])).state,
        cannot
    );
    assert_eq!(
        build_plan(Some(&snapshot), &window, 100, &view(&[0, 7])).state,
        cannot
    );
    // An invalid window (zero length).
    let bad = ViewWindow {
        start: 5,
        length: 0,
    };
    assert_eq!(
        build_plan(Some(&snapshot), &bad, 100, &view(&[0])).state,
        cannot
    );
}
