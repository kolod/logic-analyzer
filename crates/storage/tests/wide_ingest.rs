//! Captures of more than 32 channels (feature 017, FR-025, tasks T006 and T007).
//!
//! A sample of `n` channels takes `ceil(n / 32)` words; word `k` holds channels
//! `32k..32k+31`. Expected values come from a per-channel model written here, not from
//! the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_storage::{Capture, CaptureConfig, LevelState, Limits, MAX_CHANNELS};

fn config(channels: u8) -> CaptureConfig {
    CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    }
}

/// Deterministic level of `channel` at `sample`: a square wave whose half period depends
/// on the channel, with a phase, so that channels differ and several change together.
fn level(channel: u8, sample: usize) -> bool {
    let half = 2 + usize::from(channel) % 7;
    (sample / half + usize::from(channel) / 3) % 2 == 1
}

/// The words of `samples` samples for `channels` channels; `noise` sets the bits above the
/// channel count in the last word of every sample.
fn words(channels: u8, samples: usize, noise: bool) -> Vec<u32> {
    let per = usize::from(channels).div_ceil(32);
    let mut out = vec![0_u32; samples * per];
    for s in 0..samples {
        for c in 0..channels {
            if level(c, s) {
                out[s * per + usize::from(c) / 32] |= 1 << (c % 32);
            }
        }
        if noise && channels % 32 != 0 {
            out[s * per + per - 1] |= u32::MAX << (channels % 32);
        }
    }
    out
}

fn ingest(channels: u8, all: &[u32], plan: &[usize]) -> Capture {
    let per = usize::from(channels).div_ceil(32);
    let mut capture = Capture::new(config(channels)).unwrap();
    let total = all.len() / per;
    let (mut at, mut i) = (0, 0);
    while at < total {
        let n = plan[i % plan.len()].min(total - at);
        capture
            .append(at as u64, &all[at * per..(at + n) * per])
            .unwrap();
        at += n;
        i += 1;
    }
    capture
}

fn check(channels: u8, samples: usize, capture: &Capture) {
    let snapshot = capture.reader().snapshot();
    assert_eq!(snapshot.length(), samples as u64);
    for c in 0..channels {
        let mut expected = 0_u64;
        for s in 1..samples {
            if level(c, s) != level(c, s - 1) {
                expected += 1;
            }
        }
        assert_eq!(snapshot.transition_count(c).unwrap(), expected, "ch {c}");
        assert_eq!(snapshot.initial_high(c).unwrap(), level(c, 0), "ch {c}");
        for s in [0, 1, 5, samples / 2, samples - 1] {
            let want = if level(c, s) {
                LevelState::High
            } else {
                LevelState::Low
            };
            assert_eq!(
                snapshot.level_at(c, s as u64).unwrap(),
                want,
                "ch {c} at {s}"
            );
        }
    }
}

const PLANS: [&[usize]; 4] = [&[1], &[3, 1, 7], &[64], &[1000]];

#[test]
fn wide_captures_equal_the_model_in_any_chunking() {
    for channels in [33_u8, 40, 64, 65, 100, 127, 128, 160] {
        let samples = 700;
        let all = words(channels, samples, false);
        for plan in PLANS {
            check(channels, samples, &ingest(channels, &all, plan));
        }
    }
}

#[test]
fn bits_above_the_channel_count_are_ignored() {
    for channels in [33_u8, 50, 100, 159] {
        let samples = 300;
        let plain = ingest(channels, &words(channels, samples, false), &[17]);
        let noisy = ingest(channels, &words(channels, samples, true), &[17]);
        check(channels, samples, &noisy);
        let (a, b) = (plain.reader().snapshot(), noisy.reader().snapshot());
        for c in 0..channels {
            assert_eq!(
                a.transition_count(c).unwrap(),
                b.transition_count(c).unwrap()
            );
        }
    }
}

#[test]
fn a_chunk_that_does_not_hold_whole_samples_is_refused() {
    let mut capture = Capture::new(config(40)).unwrap();
    // 40 channels take two words per sample: three words are one and a half samples.
    assert!(capture.append(0, &[0, 0, 0]).is_err());
    assert_eq!(capture.length(), 0);
    capture.append(0, &[1, 0, 0, 0]).unwrap();
    assert_eq!(capture.length(), 2);
}

#[test]
fn the_largest_count_is_160_and_one_more_is_refused() {
    assert_eq!(MAX_CHANNELS, 160);
    assert!(Capture::new(config(160)).is_ok());
    assert!(Capture::new(config(161)).is_err());
    assert!(Capture::new(config(0)).is_err());
}

#[test]
fn a_gap_inside_a_wide_capture_compares_the_whole_sample() {
    let mut capture = Capture::new(config(70)).unwrap();
    capture.append(0, &[0, 0, 0, 0, 0, 0]).unwrap();
    capture.record_gap(5).unwrap();
    // After the gap channel 69 (word 2, bit 5) is high: a transition at the first position.
    capture.append(7, &[0, 0, 1 << 5]).unwrap();
    let snapshot = capture.reader().snapshot();
    assert_eq!(snapshot.transition_count(69).unwrap(), 1);
    assert_eq!(snapshot.transition_count(0).unwrap(), 0);
}

#[test]
fn memory_grows_with_the_channels_that_have_transitions_only() {
    let samples = 5000;
    let mut quiet = Capture::new(config(160)).unwrap();
    quiet.append(0, &vec![0_u32; samples * 5]).unwrap();
    let mut busy = Capture::new(config(160)).unwrap();
    busy.append(0, &words(160, samples, false)).unwrap();
    assert!(quiet.memory_used() * 4 < busy.memory_used());
    assert!(quiet.memory_used() < 160 * 200);
}

#[test]
fn queries_and_crop_work_on_channels_above_31() {
    let channels = 160;
    let samples = 400;
    let capture = ingest(channels, &words(channels, samples, false), &[50]);
    let snapshot = capture.reader().snapshot();
    // Channel 150 (word 4, bit 22): the first and the next transition by hand from `level`.
    let first = (1..samples)
        .find(|&s| level(150, s) != level(150, s - 1))
        .unwrap();
    let edge = snapshot.next_transition(150, 0).unwrap().unwrap();
    assert_eq!(edge.position, first as u64);
    assert_eq!(edge.rising, level(150, first));
    // A copy of 100..300 answers like the source at the shifted positions.
    let copy = la_storage::crop(&snapshot, 100, 300, Limits::default(), &()).unwrap();
    let copy = copy.reader().snapshot();
    assert_eq!(copy.channels(), 160);
    for c in [0_u8, 31, 32, 150, 159] {
        for p in [0_u64, 1, 57, 199] {
            assert_eq!(
                copy.level_at(c, p).unwrap(),
                snapshot.level_at(c, p + 100).unwrap(),
                "ch {c} at {p}"
            );
        }
    }
}
