//! A pass on a worker thread: progress, results, cancel and a panic (spec FR-012, FR-020).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{Lcg, Part, capture_from_parts};
use la_decoders::decoders::clocked_bits::{DESCRIPTOR, ID};
use la_decoders::input::{Chunk, SignalReader};
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, DecodeError, End, Ended, Job, JobSpec, Outcome, Run, Sink};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Limits};

fn setup() -> Setup {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("clock".into(), 0);
    setup.channels.insert("data".into(), 1);
    setup
}

fn noisy(length: usize) -> Vec<u32> {
    let mut rng = Lcg(3);
    (0..length).map(|_| rng.below(4) as u32).collect()
}

fn wait(job: &mut Job) -> Ended {
    let started = Instant::now();
    loop {
        if let Some(ended) = job.poll() {
            return ended;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the job did not end"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn spec(run: Box<dyn Run>, capture: &la_storage::Capture, chunk: ChunkLimits) -> JobSpec {
    JobSpec {
        descriptor: &DESCRIPTOR,
        setup: setup(),
        run,
        capture: capture.reader(),
        chunk_limits: chunk,
        limits: Limits::default(),
        budget: Budget::new(1 << 30),
    }
}

#[test]
fn a_job_runs_on_a_worker_and_hands_back_complete_results() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(200_000))]);
    capture.close().unwrap();
    let run = Registry::builtin().create(&setup()).unwrap();
    let mut job = Job::spawn(spec(
        run,
        &capture,
        ChunkLimits {
            max_edges: 500,
            max_span: 10_000,
        },
    ));
    let Ended::Done(end) = wait(&mut job) else {
        panic!("the worker panicked")
    };
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    assert!(end.results.lane.len() > 1_000);
    assert_eq!(end.results.coverage.covered, 200_000);
    assert!(!end.results.coverage.incomplete);
    assert_eq!(job.progress().covered, 200_000);
    // The finished store is compact.
    assert!(job.latest().is_some());
    assert!(job.poll().is_none(), "the outcome is handed over once");
}

#[test]
fn a_job_can_be_cancelled_while_it_follows_a_capture_that_never_ends() {
    // The capture is never closed: the job caught up and waits for more.
    let capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(5_000))]);
    let run = Registry::builtin().create(&setup()).unwrap();
    let mut job = Job::spawn(spec(run, &capture, ChunkLimits::default()));
    let started = Instant::now();
    while job.progress().covered < 5_000 {
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!job.is_finished(), "it waits for the capture to grow");
    assert!(job.latest().is_some() || job.progress().covered == 5_000);
    job.cancel();
    let Ended::Done(end) = wait(&mut job) else {
        panic!("the worker panicked")
    };
    assert_eq!(end.outcome.unwrap(), Outcome::Cancelled);
    assert!(end.results.coverage.incomplete);
    assert_eq!(end.results.coverage.covered, 5_000);
}

struct Panics;

impl Run for Panics {
    fn process(&mut self, _: &Chunk, _: &SignalReader, _: &mut Sink) -> Result<(), DecodeError> {
        panic!("a decoder that panics");
    }

    fn finish(&mut self, _: End, _: &SignalReader, _: &mut Sink) -> Result<(), DecodeError> {
        Ok(())
    }
}

#[test]
fn a_panic_in_a_decoder_is_contained_and_reported() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(1_000))]);
    capture.close().unwrap();
    let mut job = Job::spawn(spec(Box::new(Panics), &capture, ChunkLimits::default()));
    match wait(&mut job) {
        Ended::Panicked { failure, .. } => {
            assert!(
                failure.message.contains("a decoder that panics"),
                "{failure}"
            );
        }
        Ended::Done(end) => panic!("expected a panic, got {end:?}"),
    }
    // The capture is untouched.
    assert_eq!(capture.length(), 1_000);
}
