//! Every way a decoder can fail, contained (spec US2, SC-005, constitution Principle IX).
//!
//! Each broken fixture runs next to a healthy one on the same capture. The capture must not change,
//! the healthy results must be complete and equal to a run on their own, and the broken instance
//! keeps what it produced before the fault, marked incomplete.
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

use common::{Lcg, Part, capture_from_parts};
use la_decoders::descriptor::Descriptor;
use la_decoders::error;
use la_decoders::run::{
    ChunkLimits, Deadlines, Ended, Job, JobEnd, JobSpec, Outcome, Overdue, stuck_workers,
};
use la_decoders::store::{Budget, Full, Limits};
use la_decoders::testing::{
    self, GROWS_MEMORY, HEALTHY, INVALID_OUTPUT, NEVER_RETURNS, PANICS, RETURNS_ERROR,
    TOO_MANY_ANNOTATIONS,
};
use la_storage::{Capture, CaptureReader, Snapshot};

const CHUNK: ChunkLimits = ChunkLimits {
    max_edges: 400,
    max_span: 5_000,
};

fn capture() -> Capture {
    let mut rng = Lcg(4);
    let words: Vec<u32> = (0..100_000).map(|_| rng.below(2) as u32).collect();
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(words)]);
    capture.close().unwrap();
    capture
}

fn spawn(
    descriptor: &'static Descriptor,
    reader: &CaptureReader,
    limits: Limits,
    budget: Arc<Budget>,
    deadlines: Deadlines,
) -> Job {
    let setup = testing::setup(descriptor, 1);
    let run = testing::registry().create(&setup).unwrap();
    Job::spawn(JobSpec {
        descriptor,
        setup,
        run,
        capture: Arc::new(reader.clone()),
        chunk_limits: CHUNK,
        limits,
        budget,
        deadlines,
        output: la_decoders::stack::ItemStream::new(),
        input: None,
    })
}

fn quick(descriptor: &'static Descriptor, reader: &CaptureReader) -> Job {
    spawn(
        descriptor,
        reader,
        Limits::default(),
        Budget::new(1 << 30),
        Deadlines::default(),
    )
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

fn done(ended: Ended) -> JobEnd {
    match ended {
        Ended::Done(end) => end,
        Ended::Panicked { failure, .. } => panic!("{failure}"),
    }
}

/// What a capture is made of, as far as a decoder could damage it.
fn fingerprint(snapshot: &Snapshot) -> (u64, usize, Vec<u64>) {
    let edges = (0..2_u8)
        .map(|c| {
            snapshot
                .transitions(c, 0, snapshot.length() - 1)
                .unwrap()
                .count() as u64
        })
        .collect();
    (snapshot.length(), snapshot.gaps().len(), edges)
}

/// The healthy decoder on its own: the reference every neighbour must still match.
fn healthy_alone(reader: &CaptureReader) -> usize {
    let end = done(wait(&mut quick(&HEALTHY, reader)));
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    assert!(!end.results.coverage.incomplete);
    end.results.lane.len()
}

#[test]
fn an_error_keeps_what_came_before_and_the_neighbour_is_untouched() {
    let capture = capture();
    let reader = capture.reader();
    let before = fingerprint(&reader.snapshot());
    let reference = healthy_alone(&reader);
    let mut healthy = quick(&HEALTHY, &reader);
    let mut broken = quick(&RETURNS_ERROR, &reader);
    let broken_end = done(wait(&mut broken));
    let healthy_end = done(wait(&mut healthy));
    let e = broken_end.outcome.unwrap_err();
    assert_eq!(e.code(), error::FAILED);
    assert!(e.detail().contains("planned error"));
    // Two chunks were done before the third failed.
    assert_eq!(broken_end.results.lane.len(), 2);
    assert!(broken_end.results.coverage.incomplete);
    assert_eq!(healthy_end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(healthy_end.results.lane.len(), reference);
    assert!(!healthy_end.results.coverage.incomplete);
    assert_eq!(
        fingerprint(&reader.snapshot()),
        before,
        "the capture did not change"
    );
    // It can be started again, and fails the same way.
    let again = done(wait(&mut quick(&RETURNS_ERROR, &reader)));
    assert_eq!(again.outcome.unwrap_err().code(), error::FAILED);
}

#[test]
fn a_panic_is_contained_and_keeps_what_came_before() {
    let capture = capture();
    let reader = capture.reader();
    let mut healthy = quick(&HEALTHY, &reader);
    let mut broken = quick(&PANICS, &reader);
    match wait(&mut broken) {
        Ended::Panicked { failure, results } => {
            assert!(failure.message.contains("planned panic"), "{failure}");
            // What the worker had published before the panic is kept (at least the first chunk).
            let results = results.expect("published results");
            assert!(results.lane.len() <= 2);
        }
        Ended::Done(end) => panic!("expected a panic: {end:?}"),
    }
    let end = done(wait(&mut healthy));
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(capture.length(), 100_000);
}

/// The stuck fixtures share one flag and the global count of stuck workers: they take turns.
fn stuck_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn wait_overdue(job: &Job) -> Overdue {
    let started = Instant::now();
    loop {
        if let Some(why) = job.overdue() {
            return why;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "never overdue");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_decoder_that_never_returns_is_abandoned_and_counted_while_the_neighbour_finishes() {
    let _turn = stuck_turn();
    testing::hold_stuck_decoders();
    let capture = capture();
    let reader = capture.reader();
    let reference = healthy_alone(&reader);
    let base = stuck_workers();
    let deadlines = Deadlines {
        chunk: Duration::from_millis(250),
        cancel: Duration::from_secs(10),
    };
    let stuck = spawn(
        &NEVER_RETURNS,
        &reader,
        Limits::default(),
        Budget::new(1 << 30),
        deadlines,
    );
    let mut healthy = quick(&HEALTHY, &reader);
    let end = done(wait(&mut healthy));
    assert_eq!(end.results.lane.len(), reference);
    assert_eq!(wait_overdue(&stuck), Overdue::Chunk);
    stuck.abandon();
    assert_eq!(stuck_workers(), base + 1);
    // When the thread finally returns it is not stuck any more.
    testing::release_stuck_decoders();
    let started = Instant::now();
    while stuck_workers() > base {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    testing::hold_stuck_decoders();
}

#[test]
fn a_decoder_that_ignores_a_cancel_is_overdue_after_the_cancel_deadline() {
    let _turn = stuck_turn();
    testing::hold_stuck_decoders();
    let capture = capture();
    let reader = capture.reader();
    let deadlines = Deadlines {
        chunk: Duration::from_secs(60),
        cancel: Duration::from_millis(150),
    };
    let stuck = spawn(
        &NEVER_RETURNS,
        &reader,
        Limits::default(),
        Budget::new(1 << 30),
        deadlines,
    );
    // Wait until it has reached the chunk where it sticks.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        stuck.overdue().is_none(),
        "no cancel, no chunk deadline yet"
    );
    stuck.cancel();
    assert_eq!(wait_overdue(&stuck), Overdue::Cancel);
    let base = stuck_workers();
    stuck.abandon();
    testing::release_stuck_decoders();
    let started = Instant::now();
    while stuck_workers() >= base && base > 0 {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    testing::hold_stuck_decoders();
}

#[test]
fn four_stuck_workers_reach_the_refusal_threshold() {
    let _turn = stuck_turn();
    testing::hold_stuck_decoders();
    let capture = capture();
    let reader = capture.reader();
    let base = stuck_workers();
    let deadlines = Deadlines {
        chunk: Duration::from_millis(100),
        cancel: Duration::from_secs(10),
    };
    for _ in 0..la_decoders::run::MAX_STUCK_WORKERS {
        let job = spawn(
            &NEVER_RETURNS,
            &reader,
            Limits::default(),
            Budget::new(1 << 30),
            deadlines,
        );
        wait_overdue(&job);
        job.abandon();
    }
    assert_eq!(stuck_workers(), base + la_decoders::run::MAX_STUCK_WORKERS);
    testing::release_stuck_decoders();
    let started = Instant::now();
    while stuck_workers() > base {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    testing::hold_stuck_decoders();
}

#[test]
fn the_memory_limit_stops_a_decoder_that_grows_and_keeps_what_fits() {
    let capture = capture();
    let reader = capture.reader();
    let budget = Budget::new(200_000);
    let mut job = spawn(
        &GROWS_MEMORY,
        &reader,
        Limits::default(),
        Arc::clone(&budget),
        Deadlines::default(),
    );
    let end = done(wait(&mut job));
    assert_eq!(end.outcome.unwrap(), Outcome::Limit(Full::Memory));
    assert!(end.results.lane.len() > 100);
    assert!(end.results.coverage.incomplete);
    assert!(budget.used() <= 200_000);
}

#[test]
fn the_annotation_limit_stops_a_decoder_that_emits_too_many() {
    let capture = capture();
    let reader = capture.reader();
    let mut job = spawn(
        &TOO_MANY_ANNOTATIONS,
        &reader,
        Limits {
            max_annotations: 500,
        },
        Budget::new(1 << 30),
        Deadlines::default(),
    );
    let end = done(wait(&mut job));
    assert_eq!(end.outcome.unwrap(), Outcome::Limit(Full::Count));
    assert_eq!(end.results.lane.len(), 500);
    assert!(end.results.coverage.incomplete);
}

#[test]
fn invalid_output_is_refused_counted_and_the_valid_output_kept() {
    let capture = capture();
    let reader = capture.reader();
    let mut job = quick(&INVALID_OUTPUT, &reader);
    let end = done(wait(&mut job));
    assert_eq!(end.outcome.unwrap(), Outcome::Complete);
    let chunks = end.results.lane.len();
    assert!(chunks > 10, "one valid annotation per chunk: {chunks}");
    // Every chunk emitted five invalid records (four annotations and one item).
    assert_eq!(end.results.coverage.refused, 5 * chunks as u64);
    assert!(!end.results.coverage.incomplete);
    assert!(
        end.results
            .lane
            .window(0, u64::MAX)
            .iter()
            .all(|e| e.annotation.text == la_decoders::model::Text::Literal("valid".into()))
    );
}
