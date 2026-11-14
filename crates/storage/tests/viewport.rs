//! Viewport semantics (spec US2, FR-007..FR-014; contracts/viewport-semantics.md).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{ReferenceCapture, Rng};
use la_storage::{
    Activity, Capture, CaptureConfig, LevelState, Limits, PixelSummary, Snapshot, error,
};

fn build(channels: u8, samples: &[u32], sizes: &[usize]) -> (Snapshot, ReferenceCapture) {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    common::feed(&mut capture, samples, sizes);
    capture.close().unwrap();
    let mut reference = ReferenceCapture::new(channels);
    reference.push_chunk(samples);
    (capture.reader().snapshot(), reference)
}

fn count_of(p: &PixelSummary) -> u64 {
    match p.activity {
        Activity::None => 0,
        Activity::One(_) => 1,
        Activity::Many { count } => u64::from(count),
    }
}

/// Invariants that must hold for any viewport result without gaps.
fn assert_invariants(pixels: &[PixelSummary], expected_total: Option<u64>) {
    for pair in pixels.windows(2) {
        assert_eq!(
            pair[0].end_level, pair[1].start_level,
            "boundary continuity"
        );
    }
    let mut total = 0;
    for p in pixels {
        total += count_of(p);
        if !p.missing_data
            && p.start_level != LevelState::Missing
            && p.end_level != LevelState::Missing
        {
            let flips = count_of(p) & 1 == 1;
            let changed = p.start_level != p.end_level;
            assert_eq!(flips, changed, "parity identity for {p:?}");
        }
        if let Activity::Many { count } = p.activity {
            assert!(count >= 2, "Many with fewer than two transitions: {p:?}");
        }
    }
    if let Some(expected) = expected_total {
        assert_eq!(total, expected, "columns partition the transitions");
    }
}

#[test]
fn random_windows_match_the_reference() {
    let mut rng = Rng::new(0xFEED);
    for round in 0..40 {
        let channels = [1_u8, 4, 16][round % 3];
        let per_mille = [5_u64, 50, 300, 800][round % 4];
        let len = rng.range(1, 3000) as usize;
        let samples = common::random(len, channels, per_mille, &mut rng);
        let whole_chunk = [len];
        let sizes = [&whole_chunk[..], &[7], &[1, 64, 1000]][round % 3];
        let (snap, reference) = build(channels, &samples, sizes);
        let length = snap.length();
        for _ in 0..25 {
            let start = rng.below(length + length / 2 + 2);
            let end = start + 1 + rng.below(length * 2 + 5);
            let pixels = [1_u32, 2, 3, 7, 64, 100, 1000, 2000][rng.below(8) as usize];
            for ch in 0..channels {
                let got = snap.viewport(ch, start, end, pixels).unwrap();
                let want = reference.viewport(ch, start, end, pixels);
                assert_eq!(got.pixels.len(), pixels as usize);
                assert_eq!(
                    got.pixels, want,
                    "round {round} ch {ch} window [{start}, {end}) pixels {pixels}"
                );
                let in_range = reference.transitions(ch, start, end).len() as u64;
                assert_invariants(&got.pixels, Some(in_range));
            }
        }
    }
}

#[test]
fn single_pixel_over_the_whole_capture() {
    let samples = common::alternating(100, 1);
    let (snap, _) = build(1, &samples, &[100]);
    let r = snap.viewport(0, 0, 100, 1).unwrap();
    assert_eq!(r.pixels.len(), 1);
    assert_eq!(r.pixels[0].activity, Activity::Many { count: 99 });
    assert_eq!(r.pixels[0].start_level, LevelState::Low);
    // The end boundary is position 100, which is beyond the capture (length 100).
    assert_eq!(r.pixels[0].end_level, LevelState::Missing);
}

#[test]
fn more_pixels_than_samples_gives_zero_width_columns() {
    let samples = vec![0, 0, 1, 1, 0];
    let (snap, reference) = build(1, &samples, &[5]);
    let r = snap.viewport(0, 0, 5, 1000).unwrap();
    assert_eq!(r.pixels, reference.viewport(0, 0, 5, 1000));
    assert_invariants(&r.pixels, Some(2));
    let with_activity = r
        .pixels
        .iter()
        .filter(|p| p.activity != Activity::None)
        .count();
    assert_eq!(with_activity, 2, "exactly the two transitions are visible");
}

#[test]
fn transition_at_window_start_is_not_counted_and_at_end_is() {
    // Transition at position 5 (sample 4 low, sample 5 high).
    let mut samples = vec![0_u32; 5];
    samples.extend(vec![1_u32; 5]);
    let (snap, _) = build(1, &samples, &[10]);
    // Window starts exactly at the transition: it belongs to the window before.
    let r = snap.viewport(0, 5, 10, 5).unwrap();
    assert!(r.pixels.iter().all(|p| p.activity == Activity::None));
    // Window ends exactly at the transition: counted in the last column.
    let r = snap.viewport(0, 0, 5, 5).unwrap();
    assert_eq!(
        r.pixels.last().unwrap().activity,
        Activity::One(la_storage::Edge {
            position: 5,
            rising: true
        })
    );
}

#[test]
fn windows_beyond_the_end_are_flagged_not_invented() {
    let (snap, _) = build(1, &[0, 1, 0, 1], &[4]);
    let r = snap.viewport(0, 4, 100, 4).unwrap();
    for p in &r.pixels {
        assert!(p.beyond_end);
        assert_eq!(p.start_level, LevelState::Missing);
        assert_eq!(p.end_level, LevelState::Missing);
        assert_eq!(p.activity, Activity::None);
    }
    let r = snap.viewport(0, 2, 100, 10).unwrap();
    assert!(!r.pixels[0].beyond_end);
    assert!(r.pixels[9].beyond_end);
}

#[test]
fn maximum_pixel_count_is_accepted() {
    let (snap, reference) = build(1, &common::alternating(50, 1), &[50]);
    let r = snap.viewport(0, 0, 50, 16_384).unwrap();
    assert_eq!(r.pixels, reference.viewport(0, 0, 50, 16_384));
}

#[test]
fn invalid_arguments_are_typed_errors() {
    let (snap, _) = build(2, &[0, 1, 2, 3], &[4]);
    let code = |r: la_core::Result<_>| match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.code(),
    };
    assert_eq!(
        code(snap.viewport(0, 0, 4, 0).map(|_| ())),
        error::INVALID_PIXELS
    );
    assert_eq!(
        code(snap.viewport(0, 0, 4, 16_385).map(|_| ())),
        error::INVALID_PIXELS
    );
    assert_eq!(
        code(snap.viewport(0, 0, 4, u32::MAX).map(|_| ())),
        error::INVALID_PIXELS
    );
    assert_eq!(
        code(snap.viewport(0, 4, 4, 10).map(|_| ())),
        error::INVALID_WINDOW
    );
    assert_eq!(
        code(snap.viewport(0, 5, 4, 10).map(|_| ())),
        error::INVALID_WINDOW
    );
    assert_eq!(
        code(snap.viewport(2, 0, 4, 10).map(|_| ())),
        error::CHANNEL_OUT_OF_RANGE
    );
    // Windows ending at the largest position are valid and do not overflow.
    assert!(snap.viewport(0, 0, u64::MAX, 100).is_ok());
    assert!(snap.viewport(0, u64::MAX - 1, u64::MAX, 100).is_ok());
}

#[test]
fn level_boundaries_are_consistent_across_zoom_levels() {
    let mut rng = Rng::new(31);
    let samples = common::random(5000, 1, 120, &mut rng);
    let (snap, _) = build(1, &samples, &[333]);
    for pixels in [1_u32, 2, 5, 50, 500, 5000, 9000] {
        let r = snap.viewport(0, 0, 5000, pixels).unwrap();
        assert_invariants(
            &r.pixels,
            Some(snap.transitions(0, 0, 5000).unwrap().count() as u64),
        );
    }
}
