//! Stacked decoders: an upper decoder reads what a lower one produced (spec US4).
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
use la_decoders::descriptor::Descriptor;
use la_decoders::error;
use la_decoders::model::Text;
use la_decoders::run::{ChunkLimits, Deadlines, Ended, Job, JobEnd, JobSpec, Outcome};
use la_decoders::setup::{Setup, Source};
use la_decoders::stack::{ItemStream, dependants, stack_order};
use la_decoders::store::{Budget, Limits};
use la_decoders::testing::{self, LOWER_BYTES, RETURNS_ERROR, UPPER_WORDS};
use la_storage::CaptureReader;

const CHUNK: ChunkLimits = ChunkLimits {
    max_edges: 300,
    max_span: 4_000,
};

fn noisy(seed: u64, length: usize) -> Vec<u32> {
    let mut rng = Lcg(seed);
    (0..length).map(|_| rng.below(2) as u32).collect()
}

fn job(
    descriptor: &'static Descriptor,
    setup: Setup,
    reader: &CaptureReader,
    output: ItemStream,
    input: Option<ItemStream>,
) -> Job {
    let run = testing::registry().create(&setup).unwrap();
    Job::spawn(JobSpec {
        descriptor,
        setup,
        run,
        capture: Arc::new(reader.clone()),
        chunk_limits: CHUNK,
        limits: Limits::default(),
        budget: Budget::new(1 << 30),
        deadlines: Deadlines::default(),
        output,
        input,
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

fn closed_capture() -> la_storage::Capture {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(1, 60_000))]);
    capture.close().unwrap();
    capture
}

fn pairs(end: &JobEnd) -> usize {
    end.results
        .lane
        .window(0, u64::MAX)
        .iter()
        .filter(|e| e.annotation.text == Text::Literal("pair".into()))
        .count()
}

#[test]
fn the_upper_decoder_sees_exactly_the_items_of_the_lower_one() {
    let capture = closed_capture();
    let reader = capture.reader();
    let stream = ItemStream::new();
    let mut lower = job(
        &LOWER_BYTES,
        testing::setup(&LOWER_BYTES, 1),
        &reader,
        stream.clone(),
        None,
    );
    let mut upper = job(
        &UPPER_WORDS,
        testing::setup_above(&UPPER_WORDS, 2, 1),
        &reader,
        ItemStream::new(),
        Some(stream.clone()),
    );
    let lower_end = wait(&mut lower);
    let upper_end = wait(&mut upper);
    assert_eq!(lower_end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(*upper_end.outcome.as_ref().unwrap(), Outcome::Complete);
    let items = stream.items();
    assert!(items.len() > 10);
    // The items of the lower decoder cover the capture without a gap, in order.
    assert!(items.windows(2).all(|w| w[0].end == w[1].start));
    assert_eq!(pairs(&upper_end), items.len() / 2);
    // An odd item is left over and reported as an error at the end.
    assert_eq!(upper_end.results.lane.errors(), (items.len() % 2) as u64);
    assert!(!upper_end.results.coverage.incomplete);
    assert_eq!(upper_end.results.coverage.covered, 60_000);
}

#[test]
fn a_fault_below_marks_the_upper_results_incomplete_and_does_not_fail_it() {
    let capture = closed_capture();
    let reader = capture.reader();
    let stream = ItemStream::new();
    let mut lower = job(
        &RETURNS_ERROR,
        testing::setup(&RETURNS_ERROR, 1),
        &reader,
        stream.clone(),
        None,
    );
    let mut upper = job(
        &UPPER_WORDS,
        testing::setup_above(&UPPER_WORDS, 2, 1),
        &reader,
        ItemStream::new(),
        Some(stream.clone()),
    );
    assert_eq!(wait(&mut lower).outcome.unwrap_err().code(), error::FAILED);
    let end = wait(&mut upper);
    assert_eq!(end.outcome.unwrap(), Outcome::Complete, "it does not fail");
    assert!(end.results.coverage.incomplete);
}

#[test]
fn the_upper_decoder_follows_the_watermark_of_a_growing_capture() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(2, 20_000))]);
    let reader = capture.reader();
    let stream = ItemStream::new();
    let mut lower = job(
        &LOWER_BYTES,
        testing::setup(&LOWER_BYTES, 1),
        &reader,
        stream.clone(),
        None,
    );
    let mut upper = job(
        &UPPER_WORDS,
        testing::setup_above(&UPPER_WORDS, 2, 1),
        &reader,
        ItemStream::new(),
        Some(stream.clone()),
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(!upper.is_finished(), "it waits for the capture to end");
    for part in 0..3 {
        // The upper decoder never gets ahead of the lower one.
        let upper_covered = upper.progress().covered;
        assert!(upper_covered <= lower.progress().covered.max(20_000 * (part + 1)));
        append_parts(&mut capture, &[Part::Samples(noisy(10 + part, 20_000))]);
        std::thread::sleep(Duration::from_millis(30));
    }
    capture.close().unwrap();
    let lower_end = wait(&mut lower);
    let upper_end = wait(&mut upper);
    assert_eq!(lower_end.outcome.unwrap(), Outcome::Complete);
    assert_eq!(*upper_end.outcome.as_ref().unwrap(), Outcome::Complete);
    assert_eq!(upper_end.results.coverage.covered, 80_000);
    // No item lost or seen twice.
    assert_eq!(pairs(&upper_end), stream.items().len() / 2);
}

#[test]
fn a_cancelled_upper_pass_keeps_what_it_has() {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(noisy(3, 30_000))]);
    let reader = capture.reader();
    let stream = ItemStream::new();
    let _lower = job(
        &LOWER_BYTES,
        testing::setup(&LOWER_BYTES, 1),
        &reader,
        stream.clone(),
        None,
    );
    let mut upper = job(
        &UPPER_WORDS,
        testing::setup_above(&UPPER_WORDS, 2, 1),
        &reader,
        ItemStream::new(),
        Some(stream),
    );
    std::thread::sleep(Duration::from_millis(150));
    upper.cancel();
    let end = wait(&mut upper);
    assert_eq!(end.outcome.unwrap(), Outcome::Cancelled);
    assert!(!end.results.lane.is_empty());
    assert!(end.results.coverage.incomplete);
    capture.close().unwrap();
}

#[test]
fn refusals_name_the_cause() {
    let registry = testing::registry();
    let lower = testing::setup(&LOWER_BYTES, 1);
    let upper = testing::setup_above(&UPPER_WORDS, 2, 1);
    assert!(
        upper
            .validate(&registry, std::slice::from_ref(&lower), 2)
            .is_ok()
    );
    // A source that produces the wrong kind: `upper_words` makes words, not bytes.
    let wrong = testing::setup_above(&UPPER_WORDS, 3, 2);
    let e = wrong
        .validate(&registry, &[lower.clone(), upper.clone()], 2)
        .unwrap_err();
    assert_eq!(e.code(), error::STACK_INCOMPATIBLE);
    // A decoder that reads items cannot read the capture.
    let mut from_capture = upper.clone();
    from_capture.source = Source::Capture;
    assert_eq!(
        from_capture
            .validate(&registry, std::slice::from_ref(&lower), 2)
            .unwrap_err()
            .code(),
        error::STACK_INCOMPATIBLE
    );
    // An instance cannot read itself.
    let mut itself = upper.clone();
    itself.source = Source::Instance(2);
    assert_eq!(
        itself.validate(&registry, &[lower], 2).unwrap_err().code(),
        error::STACK_CYCLE
    );
}

#[test]
fn the_instance_limit_counts_stacked_instances() {
    let registry = testing::registry();
    let mut setups = vec![testing::setup(&LOWER_BYTES, 1)];
    for id in 2..=8 {
        setups.push(testing::setup_above(&UPPER_WORDS, id, 1));
    }
    assert_eq!(setups.len(), 8);
    let ninth = testing::setup_above(&UPPER_WORDS, 9, 1);
    assert_eq!(
        ninth.validate(&registry, &setups, 2).unwrap_err().code(),
        error::TOO_MANY_INSTANCES
    );
}

#[test]
fn the_stack_order_puts_every_instance_after_its_source() {
    let a = testing::setup(&LOWER_BYTES, 5);
    let b = testing::setup_above(&UPPER_WORDS, 3, 5);
    let c = testing::setup_above(&UPPER_WORDS, 9, 3);
    let setups = vec![c, a, b];
    assert_eq!(stack_order(&setups), vec![5, 3, 9]);
    assert_eq!(dependants(&setups, 5), vec![3, 9]);
    assert_eq!(dependants(&setups, 3), vec![9]);
    assert!(dependants(&setups, 9).is_empty());
}
