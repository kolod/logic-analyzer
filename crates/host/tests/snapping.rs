//! What a pointer means to a marker being placed: the sample under it and the transition it snaps
//! to (`specs/015-markers-rework`, research R3, user story 9). Expected values are written by hand
//! on small captures; a property test compares with a scan of every transition.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{Lcg, TempDir, shell_showing, snapshot_with_edges};
use la_host::markers::{Pointer, Snapped};
use la_host::{Command, Shell};

/// The whole capture of 10,000 samples on 800 pixels is 12.5 samples to a pixel, so the snapping
/// distance of 12 pixels is 150 samples.
const LENGTH: u64 = 10_000;

fn shell(name: &str) -> (TempDir, Shell) {
    let dir = TempDir::new(name);
    let snapshot = snapshot_with_edges(
        &[false, false, false],
        &[vec![1_000, 1_100, 5_000], vec![1_040], vec![3_000]],
        LENGTH,
    );
    let shell = shell_showing(&dir, snapshot, 3);
    (dir, shell)
}

/// The pointer in the middle of the column that is `sample` in the whole capture.
fn over(sample: u64, lane: Option<u8>) -> Pointer {
    Pointer {
        x: ((sample as f64 + 0.5) * 800.0 / LENGTH as f64) as f32,
        lane,
    }
}

#[test]
fn the_line_snaps_to_the_nearest_transition_of_the_lane_under_the_pointer() {
    let (_dir, shell) = shell("snap-nearest");
    // 1,060 is 40 from 1,100 and 60 from 1,000.
    let resolved = shell.resolve_pointer(over(1_060, Some(0)));
    assert_eq!(resolved.position, 1_100);
    assert_eq!(
        resolved.snapped,
        Some(Snapped {
            channel: 0,
            position: 1_100
        })
    );
    // Two edges in the zone: crowded.
    assert!(resolved.crowded);
}

#[test]
fn a_tie_goes_to_the_earlier_transition() {
    let (_dir, shell) = shell("snap-tie");
    let resolved = shell.resolve_pointer(over(1_050, Some(0)));
    assert_eq!(resolved.position, 1_000);
}

#[test]
fn only_the_lane_the_pointer_is_over_attracts_the_line() {
    let (_dir, shell) = shell("snap-lanes");
    let sample = 1_050;
    // Over the ruler, the legend, a decoder lane or empty space there is no lane.
    let plain = shell.resolve_pointer(over(sample, None));
    assert_eq!(
        (plain.position, plain.snapped, plain.crowded),
        (sample, None, false)
    );
    // Over a lane whose own signal has no edge within reach, the near edges of the other lanes
    // do not attract it.
    let far = shell.resolve_pointer(over(sample, Some(2)));
    assert_eq!((far.position, far.snapped), (sample, None));
    // Over the lane that has an edge close by, it snaps to that lane's edge, not the others'.
    let own = shell.resolve_pointer(over(sample, Some(1)));
    assert_eq!(own.position, 1_040);
    assert_eq!(
        own.snapped,
        Some(Snapped {
            channel: 1,
            position: 1_040
        })
    );
    // A lane that does not exist attracts nothing.
    let missing = shell.resolve_pointer(over(sample, Some(9)));
    assert_eq!((missing.position, missing.snapped), (sample, None));
}

#[test]
fn the_snapping_distance_is_twelve_pixels() {
    let (_dir, shell) = shell("snap-radius");
    // 12.5 samples to a pixel: 150 samples. The edge at 1,100 is reached from 1,250 and not 1,251.
    let near = shell.resolve_pointer(over(1_250, Some(0)));
    assert_eq!(near.snapped.map(|s| s.position), Some(1_100));
    let far = shell.resolve_pointer(over(1_251, Some(0)));
    assert_eq!(far.snapped, None);
    assert_eq!(far.position, 1_251);
}

#[test]
fn nothing_snaps_or_is_highlighted_while_snapping_is_off() {
    let (_dir, mut shell) = shell("snap-off");
    shell.apply(Command::SetSnap(false));
    let resolved = shell.resolve_pointer(over(1_060, Some(0)));
    assert_eq!(
        (resolved.position, resolved.snapped, resolved.crowded),
        (1_060, None, false)
    );
}

#[test]
fn a_lone_edge_is_not_a_crowded_zone() {
    let (_dir, shell) = shell("snap-lone");
    let lone = shell.resolve_pointer(over(3_000, Some(2)));
    assert_eq!(lone.snapped.map(|s| s.position), Some(3_000));
    assert!(!lone.crowded);
    let empty = shell.resolve_pointer(over(7_000, Some(0)));
    assert_eq!(empty.snapped, None);
    assert!(!empty.crowded);
}

#[test]
fn zoomed_in_the_distance_is_still_twelve_pixels() {
    let (_dir, mut shell) = shell("snap-zoomed");
    // 960..1,160 on 800 pixels: 4 pixels to a sample.
    shell.apply(Command::ZoomToRange {
        start: 960,
        end: 1_160,
    });
    // Twelve pixels are three samples; the pointer is in the middle of a column.
    let x = |offset: f64| (offset * 4.0 + 2.0) as f32;
    let at = |offset: f64| Pointer {
        x: x(offset),
        lane: Some(0),
    };
    // 1,098 is 2 from 1,100: snaps. 1,096 is 4 away: stays.
    assert_eq!(
        shell.resolve_pointer(at(138.0)).snapped.map(|s| s.position),
        Some(1_100)
    );
    assert_eq!(shell.resolve_pointer(at(136.0)).snapped, None);
    assert_eq!(shell.resolve_pointer(at(136.0)).position, 1_096);
}

#[test]
fn a_pointer_anywhere_gives_a_position_inside_the_capture() {
    let (_dir, shell) = shell("snap-total");
    for x in [
        -1.0e9,
        -1.0,
        0.0,
        799.0,
        800.0,
        1.0e9,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        for lane in [None, Some(0), Some(1), Some(2), Some(200)] {
            let resolved = shell.resolve_pointer(Pointer { x, lane });
            assert!(resolved.position < LENGTH, "x = {x}: {resolved:?}");
            if let Some(snapped) = resolved.snapped {
                assert_eq!(snapped.position, resolved.position);
            }
        }
    }
}

#[test]
fn an_edge_next_to_a_gap_is_not_a_measured_edge() {
    use la_storage::{Capture, CaptureConfig, Limits};
    let dir = TempDir::new("snap-gap");
    let mut capture = Capture::new(CaptureConfig {
        channels: 32,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    // Low for 2,000 samples, then a gap of 50, then high: the change across the gap is not a
    // measured edge; the real one is the fall at 3,000.
    capture.append(0, &vec![0_u32; 2_000]).unwrap();
    capture.record_gap(50).unwrap();
    capture.append(capture.length(), &vec![1_u32; 950]).unwrap();
    capture
        .append(capture.length(), &vec![0_u32; 7_000])
        .unwrap();
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    assert_eq!(snapshot.length(), LENGTH);
    let edges: Vec<u64> = snapshot
        .transitions(0, 0, LENGTH)
        .unwrap()
        .map(|e| e.position)
        .collect();
    assert!(
        edges.contains(&2_050),
        "the capture records the change after the gap at 2,050: {edges:?}"
    );
    let shell = shell_showing(&dir, snapshot, 3);
    let near_gap = shell.resolve_pointer(over(2_060, Some(0)));
    assert_eq!(near_gap.snapped, None, "{near_gap:?}");
    assert_eq!(near_gap.position, 2_060);
    let real = shell.resolve_pointer(over(3_000, Some(0)));
    assert_eq!(real.snapped.map(|s| s.position), Some(3_000));
}

#[test]
fn the_answer_is_the_nearest_transition_found_by_a_scan_of_every_one() {
    let mut rng = Lcg(0xFACADE);
    for round in 0..8 {
        let mut flips: Vec<Vec<u64>> = Vec::new();
        for _ in 0..3 {
            let mut at = 0;
            let mut list = Vec::new();
            while at < LENGTH - 400 {
                at += 1 + rng.below(if round % 2 == 0 { 40 } else { 600 });
                if at < LENGTH {
                    list.push(at);
                }
            }
            flips.push(list);
        }
        let dir = TempDir::new(&format!("snap-prop-{round}"));
        let snapshot = snapshot_with_edges(&[false, true, false], &flips, LENGTH);
        let shell = shell_showing(&dir, snapshot, 3);
        for _ in 0..300 {
            let sample = rng.below(LENGTH);
            let lane = rng.below(3) as u8;
            let resolved = shell.resolve_pointer(over(sample, Some(lane)));
            assert!(resolved.position < LENGTH);
            // The pointer's own sample, as the host takes it: floor(x * 12.5).
            let t = (f64::from(over(sample, Some(lane)).x) * LENGTH as f64 / 800.0).floor() as u64;
            let edges = &flips[usize::from(lane)];
            let nearest = edges
                .iter()
                .copied()
                .filter(|p| p.abs_diff(t) <= 150)
                .min_by_key(|p| (p.abs_diff(t), *p));
            match (nearest, resolved.snapped) {
                (Some(expected), Some(snapped)) => {
                    assert_eq!(snapped.channel, lane);
                    assert_eq!(snapped.position, expected, "round {round} pointer {t}");
                    assert_eq!(resolved.position, expected);
                }
                (None, None) => {}
                other => panic!("round {round} pointer {t} lane {lane}: {other:?}"),
            }
        }
    }
}
