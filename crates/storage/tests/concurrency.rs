//! Reading while a capture is in progress (spec US5, FR-019..FR-021, SC-007).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{ReferenceCapture, Rng};
use la_diagnostics::{install_panic_hook, spawn_worker};
use la_storage::{Activity, Capture, CaptureConfig, LevelState, Limits, Snapshot};

const CHANNELS: u8 = 4;
const STREAM_LEN: usize = 3_000_000;
const READERS: usize = 4;
/// Reader queries in total (spec SC-007 asks for 1,000,000; debug builds are slower).
const TOTAL_QUERIES: u64 = if cfg!(debug_assertions) {
    120_000
} else {
    1_000_000
};
/// Generous guard on the producer's worst append delay while readers run flat out. The
/// producer never waits for readers (they take no lock while querying); typical appends
/// take far less than a millisecond, this bound only catches blocking.
const MAX_APPEND_DELAY: Duration = Duration::from_millis(50);

struct Fixture {
    stream: Vec<u32>,
    /// Chunk sizes used by the producer, and the resulting cumulative boundaries.
    sizes: Vec<usize>,
    boundaries: Vec<u64>,
    reference: ReferenceCapture,
    transitions: Vec<Vec<u64>>,
}

fn fixture(seed: u64) -> Fixture {
    let mut rng = Rng::new(seed);
    let stream = common::random(STREAM_LEN, CHANNELS, 20, &mut rng);
    let mut sizes = Vec::new();
    let mut boundaries = vec![0_u64];
    let mut total = 0_usize;
    while total < STREAM_LEN {
        let size = (rng.range(1, 5_000) as usize).min(STREAM_LEN - total);
        sizes.push(size);
        total += size;
        boundaries.push(total as u64);
    }
    let mut reference = ReferenceCapture::new(CHANNELS);
    reference.push_chunk(&stream);
    let transitions = (0..CHANNELS)
        .map(|c| reference.all_transitions(c))
        .collect();
    Fixture {
        stream,
        sizes,
        boundaries,
        reference,
        transitions,
    }
}

fn new_capture() -> Capture {
    Capture::new(CaptureConfig {
        channels: CHANNELS,
        sample_rate_hz: 100_000_000,
        limits: Limits::default(),
    })
    .unwrap()
}

/// One random reader query; asserts the result equals the data of a complete-chunk prefix.
fn query_once(fx: &Fixture, snap: &Snapshot, rng: &mut Rng) {
    let l = snap.length();
    assert!(
        fx.boundaries.binary_search(&l).is_ok(),
        "snapshot length {l} is not a complete-chunk boundary"
    );
    let ch = rng.below(u64::from(CHANNELS)) as u8;
    let trans = &fx.transitions[usize::from(ch)];
    match rng.below(3) {
        0 => {
            if l == 0 {
                assert_eq!(snap.level_at(ch, 0).unwrap(), LevelState::Missing);
            } else {
                let p = rng.below(l);
                assert_eq!(
                    snap.level_at(ch, p).unwrap(),
                    fx.reference.level(ch, p),
                    "pos {p}"
                );
                assert_eq!(
                    snap.level_at(ch, l + rng.below(100)).unwrap(),
                    LevelState::Missing
                );
            }
        }
        1 => {
            let a = rng.below(l + 10);
            let b = a + 1 + rng.below(20_000);
            let got: Vec<u64> = snap
                .transitions(ch, a, b)
                .unwrap()
                .map(|e| e.position)
                .collect();
            let want: Vec<u64> = trans
                .iter()
                .copied()
                .filter(|&p| p > a && p <= b && p < l)
                .collect();
            assert_eq!(got, want, "ch {ch} window ({a}, {b}] at length {l}");
        }
        _ => {
            if l == 0 {
                return;
            }
            let view = snap.viewport(ch, 0, l, 64).unwrap();
            let counted: u64 = view
                .pixels
                .iter()
                .map(|p| match p.activity {
                    Activity::None => 0,
                    Activity::One(_) => 1,
                    Activity::Many { count } => u64::from(count),
                })
                .sum();
            let want = trans.iter().filter(|&&p| p < l).count() as u64;
            assert_eq!(counted, want, "ch {ch} viewport total at length {l}");
            for pair in view.pixels.windows(2) {
                assert_eq!(pair[0].end_level, pair[1].start_level);
            }
        }
    }
}

/// Runs a producer and `readers` busy readers; returns each append's duration.
fn run(fx: &Fixture, readers: usize, queries_total: u64) -> Vec<Duration> {
    let mut capture = new_capture();
    let reader = capture.reader();
    let done = AtomicBool::new(false);
    let queries = AtomicU64::new(0);
    let mut durations = Vec::new();
    std::thread::scope(|scope| {
        for r in 0..readers {
            let reader = reader.clone();
            let (done, queries) = (&done, &queries);
            scope.spawn(move || {
                let mut rng = Rng::new(1000 + r as u64);
                while !done.load(Ordering::Acquire)
                    || queries.load(Ordering::Relaxed) < queries_total
                {
                    query_once(fx, &reader.snapshot(), &mut rng);
                    if queries.fetch_add(1, Ordering::Relaxed) >= queries_total
                        && done.load(Ordering::Acquire)
                    {
                        break;
                    }
                }
            });
        }
        let mut at = 0;
        for &size in &fx.sizes {
            let start = capture.length();
            let t = Instant::now();
            capture.append(start, &fx.stream[at..at + size]).unwrap();
            durations.push(t.elapsed());
            at += size;
        }
        done.store(true, Ordering::Release);
    });
    let final_snap = reader.snapshot();
    assert_eq!(final_snap.length(), STREAM_LEN as u64);
    durations
}

#[test]
fn readers_always_see_a_prefix_of_complete_chunks() {
    let fx = fixture(7);
    let durations = run(&fx, READERS, TOTAL_QUERIES);
    assert_eq!(durations.len(), fx.sizes.len());
}

#[test]
fn a_held_snapshot_does_not_change_when_more_data_is_appended() {
    let fx = fixture(8);
    let mut capture = new_capture();
    let half = fx.sizes.len() / 2;
    let split: usize = fx.sizes[..half].iter().sum();
    let mut at = 0;
    for &size in &fx.sizes[..half] {
        capture
            .append(capture.length(), &fx.stream[at..at + size])
            .unwrap();
        at += size;
    }
    let held = capture.reader().snapshot();
    let describe = |s: &Snapshot| -> (u64, Vec<LevelState>, Vec<u64>) {
        (
            s.length(),
            (0..200).map(|i| s.level_at(0, i * 100).unwrap()).collect(),
            s.transitions(0, 0, s.length().max(1))
                .unwrap()
                .map(|e| e.position)
                .collect(),
        )
    };
    let before = describe(&held);
    for &size in &fx.sizes[half..] {
        capture
            .append(capture.length(), &fx.stream[at..at + size])
            .unwrap();
        at += size;
    }
    assert_eq!(describe(&held), before, "a held snapshot must never change");
    assert_eq!(held.length(), split as u64);
    assert_eq!(capture.reader().snapshot().length(), STREAM_LEN as u64);
}

#[test]
fn length_reports_only_fully_available_positions() {
    let fx = fixture(9);
    let mut capture = new_capture();
    let reader = capture.reader();
    let mut at = 0;
    for &size in fx.sizes.iter().take(50) {
        capture
            .append(capture.length(), &fx.stream[at..at + size])
            .unwrap();
        at += size;
        assert_eq!(reader.snapshot().length(), at as u64);
    }
}

#[test]
fn readers_never_delay_the_producer_beyond_the_documented_maximum() {
    let fx = fixture(10);
    let alone = run(&fx, 0, 0);
    let busy = run(&fx, READERS, TOTAL_QUERIES / 4);
    let worst = |d: &[Duration]| d.iter().max().copied().unwrap();
    let median = |d: &[Duration]| {
        let mut s = d.to_vec();
        s.sort();
        s[s.len() / 2]
    };
    println!(
        "append median/worst alone: {:?}/{:?}; with {READERS} busy readers: {:?}/{:?}",
        median(&alone),
        worst(&alone),
        median(&busy),
        worst(&busy)
    );
    assert!(
        worst(&busy) <= MAX_APPEND_DELAY,
        "worst append with readers took {:?} (limit {MAX_APPEND_DELAY:?})",
        worst(&busy)
    );
}

#[test]
fn a_panicking_reader_does_not_disturb_the_producer_or_later_readbacks() {
    install_panic_hook();
    let fx = fixture(11);
    let mut capture = new_capture();
    let mut at = 0;
    let half = fx.sizes.len() / 2;
    for &size in &fx.sizes[..half] {
        capture
            .append(capture.length(), &fx.stream[at..at + size])
            .unwrap();
        at += size;
    }
    let reader = capture.reader();
    let failure = spawn_worker("panicking-reader", move || {
        let snap = reader.snapshot();
        let _ = snap.viewport(0, 0, snap.length(), 100);
        panic!("a reader fails while holding a snapshot");
    })
    .join()
    .unwrap_err();
    assert!(failure.message.contains("a reader fails"));

    for &size in &fx.sizes[half..] {
        capture
            .append(capture.length(), &fx.stream[at..at + size])
            .unwrap();
        at += size;
    }
    let snap = capture.reader().snapshot();
    assert_eq!(snap.length(), STREAM_LEN as u64);
    let mut rng = Rng::new(5);
    for _ in 0..2_000 {
        query_once(&fx, &snap, &mut rng);
    }
}
