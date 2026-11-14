//! Complexity class of viewport queries (spec US2, SC-003, SC-010; research R9).
//!
//! Merge-blocking and deterministic: it counts probes (block-index and offset
//! comparisons), not time, so it cannot be flaky on shared CI runners. If a change makes
//! the cost grow linearly with the capture size, these tests fail.
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
use la_storage::{Capture, CaptureConfig, Limits, Snapshot};

const PIXELS: u32 = 2_000;

/// Probes of one full-window viewport query on channel 0.
fn probes_of(snap: &Snapshot, start: u64, end: u64, pixels: u32) -> u64 {
    snap.viewport(0, start, end, pixels).unwrap().stats.probes
}

fn whole(snap: &Snapshot, pixels: u32) -> u64 {
    probes_of(snap, 0, snap.length(), pixels)
}

/// The documented upper bound: `c * (log2 N + W * (1 + log2(1 + N_w / W)))`.
fn bound(n: u64, n_w: u64, w: u32) -> f64 {
    let (n, n_w, w) = (n as f64, n_w as f64, f64::from(w));
    C * (n.max(2.0).log2() + w * (1.0 + (1.0 + n_w / w).log2()))
}

/// Constant of the bound; the measured probes are well below it.
const C: f64 = 12.0;

#[test]
fn probes_grow_logarithmically_with_capture_size() {
    let mut results = Vec::new();
    for exp in 3..=9_u32 {
        let n = 10_u64.pow(exp);
        let snap = synthetic_capture(1, n).unwrap();
        let probes = whole(&snap, PIXELS);
        results.push((n, probes));
        assert!(
            (probes as f64) <= bound(n, n, PIXELS),
            "N = {n}: {probes} probes exceed the bound {}",
            bound(n, n, PIXELS)
        );
    }
    let at = |n: u64| results.iter().find(|r| r.0 == n).unwrap().1 as f64;
    let growth = at(1_000_000_000) / at(1_000_000);
    println!("probes by size: {results:?}; growth 1e6 -> 1e9 = {growth:.2}");
    assert!(
        growth < 3.0,
        "1,000x more transitions must cost under 3x ({growth:.2})"
    );
    // Never linear: for large captures the probes are a tiny fraction of N.
    for &(n, probes) in &results {
        if n >= 100_000_000 {
            assert!(probes < n / 1000, "N = {n}: {probes} probes look linear");
        }
    }
}

#[test]
fn probes_are_linear_in_the_pixel_count() {
    let snap = synthetic_capture(1, 100_000_000).unwrap();
    let p1 = whole(&snap, 1_000) as f64;
    let p2 = whole(&snap, 2_000) as f64;
    let p4 = whole(&snap, 4_000) as f64;
    println!("W=1000: {p1}, W=2000: {p2}, W=4000: {p4}");
    assert!((1.5..2.5).contains(&(p2 / p1)), "doubling W: {}", p2 / p1);
    assert!((1.5..2.5).contains(&(p4 / p2)), "doubling W: {}", p4 / p2);
}

#[test]
fn cost_does_not_depend_on_the_number_of_transitions_inside_the_window() {
    let n = 1_000_000_000;
    let snap = synthetic_capture(1, n).unwrap();
    let full = whole(&snap, PIXELS);
    // Synthetic spacing is uniform: a window of length / 1000 holds about 1e6 transitions.
    let small = probes_of(&snap, 0, snap.length() / 1000, PIXELS);
    let tiny = probes_of(&snap, 5_000_000, 5_000_000 + 4_000, PIXELS);
    println!("window 1e9: {full}, 1e6: {small}, ~1e3: {tiny}");
    assert!(full as f64 / small as f64 > 0.5 && (full as f64 / small as f64) < 3.0);
    assert!((small as f64) <= bound(n, 1_000_000, PIXELS));
    assert!((tiny as f64) <= bound(n, 2_000, PIXELS));
}

#[test]
fn a_start_deep_into_a_huge_capture_costs_the_same_as_the_start() {
    let snap = synthetic_capture(1, 1_000_000_000).unwrap();
    let len = snap.length();
    let a = probes_of(&snap, 0, len / 10, PIXELS) as f64;
    let b = probes_of(&snap, len / 2, len / 2 + len / 10, PIXELS) as f64;
    assert!(
        (0.5..2.0).contains(&(a / b)),
        "start offset changes cost: {}",
        a / b
    );
}

#[test]
fn alternating_worst_case_on_real_data_stays_within_the_bound() {
    // A transition at every sample: the worst case for memory and for query cost.
    let len = 1_000_000;
    let samples = common::alternating(len, 1);
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    common::feed(&mut capture, &samples, &[65_536]);
    let snap = capture.reader().snapshot();
    let transitions = (len - 1) as u64;
    for pixels in [100_u32, 2_000, 16_384] {
        let probes = whole(&snap, pixels);
        assert!(
            (probes as f64) <= bound(transitions, transitions, pixels),
            "W = {pixels}: {probes} probes"
        );
    }
}

#[test]
fn real_ten_million_transition_capture_matches_the_exact_answer_on_sampled_columns() {
    let mut rng = Rng::new(2024);
    let len = 10_000_000;
    let samples = common::random(len, 1, 900, &mut rng);
    let mut capture = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    common::feed(&mut capture, &samples, &[65_536]);
    let snap = capture.reader().snapshot();
    let mut reference = ReferenceCapture::new(1);
    reference.push_chunk(&samples);
    let trans = reference.all_transitions(0);
    assert!(
        trans.len() > 8_000_000,
        "about nine million transitions expected"
    );

    let (start, end, pixels) = (0_u64, len as u64, 2_000_u32);
    let got = snap.viewport(0, start, end, pixels).unwrap();
    let bound_of =
        |i: u32| start + (u128::from(end - start) * u128::from(i) / u128::from(pixels)) as u64;
    let rank = |x: u64| trans.partition_point(|&p| p <= x) as u64;
    for i in (0..pixels).step_by(37) {
        let (a, b) = (bound_of(i), bound_of(i + 1));
        let want = rank(b) - rank(a);
        let pixel = &got.pixels[i as usize];
        let have = match pixel.activity {
            la_storage::Activity::None => 0,
            la_storage::Activity::One(_) => 1,
            la_storage::Activity::Many { count } => u64::from(count),
        };
        assert_eq!(have, want, "column {i}");
        assert_eq!(pixel.start_level, reference.level(0, a), "column {i} start");
    }
    let probes = got.stats.probes as f64;
    assert!(probes <= bound(trans.len() as u64, trans.len() as u64, pixels));
}
