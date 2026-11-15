//! Running in the background: progress, cancel, growth (spec US3, SC-004).
#![cfg(feature = "test-support")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{Lcg, Part, append_parts, capture_from_parts};
use la_decoders::decoders::clocked_bits::{DESCRIPTOR, ID};
use la_decoders::model::{Kind, Text};
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, Deadlines, Ended, Job, JobEnd, JobSpec, Outcome, Results};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Limits};
use la_decoders::testing::{self, SLOW};

fn noisy(seed: u64, length: usize) -> Vec<u32> {
    let mut rng = Lcg(seed);
    (0..length).map(|_| rng.below(4) as u32).collect()
}

fn sample_setup() -> Setup {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("clock".into(), 0);
    setup.channels.insert("data".into(), 1);
    setup
}

fn spawn_sample(reader: la_storage::CaptureReader, chunk: ChunkLimits) -> Job {
    let setup = sample_setup();
    Job::spawn(JobSpec {
        descriptor: &DESCRIPTOR,
        run: Registry::builtin().create(&setup).unwrap(),
        setup,
        capture: Arc::new(reader),
        chunk_limits: chunk,
        limits: Limits::default(),
        budget: Budget::new(1 << 30),
        deadlines: Deadlines::default(),
    })
}

fn wait(job: &mut Job) -> JobEnd {
    let started = Instant::now();
    loop {
        if let Some(ended) = job.poll() {
            return match ended {
                Ended::Done(end) => end,
                Ended::Panicked { failure, .. } => panic!("{failure}"),
            };
        }
        assert!(started.elapsed() < Duration::from_secs(60), "did not end");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The annotations of a result as comparable values, in store order.
fn flat(results: &Results) -> Vec<(Kind, Text, bool)> {
    let mut all: Vec<(Kind, Text, bool)> = results
        .lane
        .window(0, u64::MAX)
        .iter()
        .map(|e| {
            (
                e.annotation.kind,
                e.annotation.text.clone(),
                e.annotation.error,
            )
        })
        .collect();
    for snapshot in results.overlays.values() {
        all.extend(snapshot.window(0, u64::MAX).iter().map(|e| {
            (
                e.annotation.kind,
                e.annotation.text.clone(),
                e.annotation.error,
            )
        }));
    }
    all.sort_by_key(|(k, t, e)| (format!("{k:?}"), format!("{t:?}"), *e));
    all
}

const SMALL: ChunkLimits = ChunkLimits {
    max_edges: 300,
    max_span: 4_000,
};

#[test]
fn progress_never_goes_backwards_and_ends_at_the_length() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(1, 300_000))]);
    capture.close().unwrap();
    let mut job = spawn_sample(capture.reader(), SMALL);
    let mut last = 0;
    let started = Instant::now();
    while !job.is_finished() {
        let progress = job.progress();
        assert!(progress.covered >= last, "{} < {last}", progress.covered);
        assert!(progress.covered <= 300_000);
        last = progress.covered;
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(1));
    }
    let end = wait(&mut job);
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(job.progress().covered, 300_000);
    assert_eq!(job.progress().length, 300_000);
}

#[test]
fn cancel_takes_effect_within_a_second_even_for_a_slow_decoder_and_keeps_the_results() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(2, 400_000))]);
    capture.close().unwrap();
    let setup = testing::setup(&SLOW, 1);
    let make = || {
        Job::spawn(JobSpec {
            descriptor: &SLOW,
            run: testing::registry().create(&setup).unwrap(),
            setup: setup.clone(),
            capture: Arc::new(capture.reader()),
            chunk_limits: ChunkLimits {
                max_edges: 100,
                max_span: 1_000,
            },
            limits: Limits::default(),
            budget: Budget::new(1 << 30),
            deadlines: Deadlines::default(),
        })
    };
    let mut job = make();
    std::thread::sleep(Duration::from_millis(200));
    let asked = Instant::now();
    job.cancel();
    let end = wait(&mut job);
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "{:?}",
        asked.elapsed()
    );
    assert_eq!(end.outcome.unwrap(), Outcome::Cancelled);
    assert!(end.results.lane.len() > 3, "what was done is kept");
    assert!(end.results.coverage.incomplete);
    // It can be started again.
    let mut again = make();
    again.cancel();
    assert_eq!(wait(&mut again).outcome.unwrap(), Outcome::Cancelled);
}

#[test]
fn a_growing_capture_gives_the_same_annotations_as_the_finished_one() {
    let first = noisy(3, 40_000);
    let second = noisy(4, 40_000);
    let third = noisy(5, 40_000);
    let parts_all = [
        Part::Samples(first.clone()),
        Part::Gap(500),
        Part::Samples(second.clone()),
        Part::Samples(third.clone()),
    ];
    // The reference: the whole capture decoded after it is closed.
    let mut whole = capture_from_parts(2, 1_000_000, &parts_all);
    whole.close().unwrap();
    let mut reference_job = spawn_sample(whole.reader(), SMALL);
    let reference = wait(&mut reference_job);
    // The same capture appended to while the pass runs, with a gap that appears on the way.
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(first)]);
    let mut job = spawn_sample(capture.reader(), SMALL);
    std::thread::sleep(Duration::from_millis(50));
    assert!(!job.is_finished(), "it waits for the capture to end");
    append_parts(&mut capture, &[Part::Gap(500)]);
    std::thread::sleep(Duration::from_millis(30));
    append_parts(&mut capture, &[Part::Samples(second)]);
    std::thread::sleep(Duration::from_millis(30));
    let mid = job.progress().covered;
    assert!(mid > 0, "the covered position advances while it grows");
    append_parts(&mut capture, &[Part::Samples(third)]);
    assert!(
        !job.is_finished(),
        "finish comes only when the capture is closed"
    );
    capture.close().unwrap();
    let end = wait(&mut job);
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(flat(&end.results), flat(&reference.results));
    assert_eq!(end.results.coverage.covered, 120_500);
}
