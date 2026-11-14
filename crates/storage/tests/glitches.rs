//! Zoomed-out views never hide glitches (spec US3, FR-009..FR-012, SC-004).
//!
//! The expected answers are computed here independently of the reference model: from the
//! known transition positions and the column boundary formula of the semantics contract.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{ReferenceCapture, Rng};
use la_storage::testing::synthetic_capture;
use la_storage::{
    Activity, Capture, CaptureConfig, Edge, LevelState, Limits, PixelSummary, Snapshot,
};

fn build(channels: u8, samples: &[u32], sizes: &[usize]) -> Snapshot {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    common::feed(&mut capture, samples, sizes);
    capture.close().unwrap();
    capture.reader().snapshot()
}

fn boundary(start: u64, len: u64, i: u32, pixels: u32) -> u64 {
    start + (u128::from(len) * u128::from(i) / u128::from(pixels)) as u64
}

/// Expected activity of every column given the sorted transition positions.
fn expected_activity(
    trans: &[u64],
    rising_of: impl Fn(u64) -> bool,
    start: u64,
    end: u64,
    pixels: u32,
) -> Vec<Activity> {
    (0..pixels)
        .map(|i| {
            let a = boundary(start, end - start, i, pixels);
            let b = boundary(start, end - start, i + 1, pixels);
            let lo = trans.partition_point(|&p| p <= a);
            let hi = trans.partition_point(|&p| p <= b);
            match hi - lo {
                0 => Activity::None,
                1 => Activity::One(Edge {
                    position: trans[lo],
                    rising: rising_of(trans[lo]),
                }),
                n => Activity::Many { count: n as u32 },
            }
        })
        .collect()
}

fn assert_continuity(pixels: &[PixelSummary]) {
    for pair in pixels.windows(2) {
        assert_eq!(pair[0].end_level, pair[1].start_level);
    }
}

#[test]
fn a_single_sample_glitch_is_visible_at_every_zoom_level() {
    let len = 5_000_000_usize;
    let mut rng = Rng::new(99);
    let mut positions = vec![0_usize, 1, len / 2, len - 2, len - 1];
    positions.extend((0..7).map(|_| rng.below(len as u64) as usize));
    for at in positions {
        let samples = common::glitch(len, at, 1);
        let snap = build(1, &samples, &[65_536]);
        // The glitch is high for one sample: rising at `at` (unless at 0), falling at at+1.
        let mut trans = Vec::new();
        if at > 0 {
            trans.push(at as u64);
        }
        if at + 1 < len {
            trans.push(at as u64 + 1);
        }
        let rising_of = |p: u64| p == at as u64;

        // Whole capture at several pixel counts, then windows centred on the glitch at
        // zoom levels from one sample per pixel to the whole capture.
        let mut windows = vec![(0_u64, len as u64)];
        let centre = at as u64;
        for zoom in [1_u64, 3, 10, 1_000, 100_000, 3_000_000] {
            windows.push((centre.saturating_sub(zoom), centre + zoom + 1));
        }
        for (start, end) in windows {
            for pixels in [1_u32, 2, 3, 100, 2_000] {
                let got = snap.viewport(0, start, end, pixels).unwrap();
                let want = expected_activity(&trans, rising_of, start, end, pixels);
                let have: Vec<Activity> = got.pixels.iter().map(|p| p.activity).collect();
                assert_eq!(
                    have, want,
                    "glitch at {at}, window [{start}, {end}), {pixels} px"
                );
                assert_continuity(&got.pixels);
                // A column that contains a glitch edge can never look like a flat line.
                for (p, w) in got.pixels.iter().zip(&want) {
                    if *w != Activity::None {
                        assert_ne!(p.activity, Activity::None);
                    }
                }
            }
        }
    }
}

#[test]
fn bursts_report_many_with_the_exact_count() {
    for burst in [3_u64, 10, 1_000] {
        let len = 200_000_usize;
        let at = 50_000_u64;
        // `burst` consecutive toggles starting at position `at`.
        let mut samples = vec![0_u32; len];
        let mut level = 0;
        for k in 0..burst {
            level ^= 1;
            samples[(at + k) as usize] = level;
        }
        for s in samples.iter_mut().skip((at + burst) as usize) {
            *s = level;
        }
        let snap = build(1, &samples, &[4096]);
        let trans: Vec<u64> = (at..at + burst).collect();
        let rising_of = |p: u64| (p - at) % 2 == 0;
        for pixels in [1_u32, 7, 100, 2_000] {
            let got = snap.viewport(0, 0, len as u64, pixels).unwrap();
            let want = expected_activity(&trans, rising_of, 0, len as u64, pixels);
            let have: Vec<Activity> = got.pixels.iter().map(|p| p.activity).collect();
            assert_eq!(have, want, "burst of {burst}, {pixels} px");
        }
        // One pixel over the burst: Many with the exact count.
        let got = snap.viewport(0, at - 1, at + burst + 10, 1).unwrap();
        assert_eq!(
            got.pixels[0].activity,
            Activity::Many {
                count: burst as u32
            }
        );
    }
}

#[test]
fn classification_matches_exact_counts_on_random_signals() {
    let mut rng = Rng::new(4711);
    for per_mille in [2_u64, 20, 200, 700] {
        let samples = common::random(60_000, 1, per_mille, &mut rng);
        let snap = build(1, &samples, &[1000]);
        let mut reference = ReferenceCapture::new(1);
        reference.push_chunk(&samples);
        let trans = reference.all_transitions(0);
        let rising_of = |p: u64| reference.level(0, p) == LevelState::High;
        for _ in 0..20 {
            let start = rng.below(40_000);
            let end = start + 1 + rng.below(60_000);
            let pixels = [1_u32, 2, 5, 97, 1_000, 2_000][rng.below(6) as usize];
            let got = snap.viewport(0, start, end, pixels).unwrap();
            let want = expected_activity(&trans, rising_of, start, end, pixels);
            let have: Vec<Activity> = got.pixels.iter().map(|p| p.activity).collect();
            assert_eq!(
                have, want,
                "density {per_mille}, [{start}, {end}), {pixels} px"
            );
            assert_continuity(&got.pixels);
            for p in &got.pixels {
                // None, One and Many are mutually exclusive and Many always means >= 2.
                if let Activity::Many { count } = p.activity {
                    assert!(count >= 2);
                }
            }
        }
    }
}

#[test]
fn counts_saturate_instead_of_wrapping() {
    // More than u32::MAX transitions in a single column (documented saturation).
    let transitions = 4_500_000_000_u64;
    let snap = synthetic_capture(1, transitions).unwrap();
    // End one position before the capture end: that is exactly the last transition,
    // which is counted in the last (only) column.
    let end = snap.length() - 1;
    let r = snap.viewport(0, 0, end, 1).unwrap();
    assert_eq!(
        r.pixels[0].activity,
        Activity::Many {
            count: la_storage::MAX_REPORTED_TRANSITIONS
        }
    );
    assert_eq!(r.pixels[0].start_level, LevelState::Low);
    // The levels come from the exact rank parity, not from the saturated count: the
    // true count is even, so the level after it is the initial (low) level. The parity
    // identity of the semantics contract applies to the true count, not the saturated
    // one (u32::MAX is odd, which would wrongly suggest a flip).
    assert_eq!(r.pixels[0].end_level, LevelState::Low);
}
