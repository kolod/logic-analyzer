//! A decoder that holds work back reports how far its output is final (decoder API 1.1,
//! `specs/008-i2c-decoder-generator/contracts/decoder-api-1.1.md`).
//!
//! The decoders here are written in the test. Coverage is read after every chunk from the results of
//! the sink, which is what the interface and the stack watermark follow.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::capture_from_edges;
use la_decoders::descriptor::{Descriptor, Input, Signal};
use la_decoders::input::{Chunk, SignalReader};
use la_decoders::model::{Item, Payload};
use la_decoders::run::{ChunkLimits, DecodeError, End, Outcome, Pass, Run, Sink, run_pass};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Limits};

static SIGNALS: [Signal; 1] = [Signal {
    role: "clock",
    label: "k",
    required: true,
    may_share: false,
}];

static DESCRIPTOR: Descriptor = Descriptor {
    id: "test.holder",
    version: 1,
    name: "k",
    description: "k",
    signals: &SIGNALS,
    options: &[],
    input: Input::Signals,
    outputs: &["held"],
    generator: false,
};

/// What a test decoder says about its progress.
#[derive(Clone, Copy)]
enum Report {
    /// Nothing: the default of version 1.0.
    Default,
    /// The end of the last chunk minus this many positions.
    Behind(u64),
    /// Alternating between the end of the chunk and far behind (a decoder that goes backwards).
    Jumpy,
    /// Far beyond the end of the chunk (a decoder that claims too much).
    Ahead,
}

struct Holder {
    report: Report,
    end: u64,
    calls: u64,
    /// Emits an item for the span of every chunk.
    emit: bool,
}

impl Run for Holder {
    fn process(
        &mut self,
        chunk: &Chunk,
        _reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        self.end = chunk.end;
        self.calls += 1;
        if self.emit {
            sink.emit(Item {
                kind: "held",
                start: chunk.start,
                end: chunk.end,
                payload: Payload::Integer(self.calls),
                error: false,
            })?;
        }
        Ok(())
    }

    fn finish(
        &mut self,
        _end: End,
        _reader: &SignalReader,
        _sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        Ok(())
    }

    fn final_through(&self) -> Option<u64> {
        match self.report {
            Report::Default => None,
            Report::Behind(n) => Some(self.end.saturating_sub(n)),
            Report::Jumpy => Some(if self.calls % 2 == 0 { self.end } else { 7 }),
            Report::Ahead => Some(u64::MAX),
        }
    }
}

const LENGTH: u64 = 20_000;

fn capture() -> la_storage::Capture {
    // The clock toggles every 10 positions: about 2,000 edges.
    let mut edges = Vec::new();
    let mut high = false;
    let mut at = 10;
    while at < LENGTH {
        high = !high;
        edges.push((at, 0_u8, high));
        at += 10;
    }
    capture_from_edges(1, 1_000, LENGTH, &edges)
}

fn pass(report: Report, emit: bool) -> Pass {
    let mut setup = Setup::new(1, "test.holder");
    setup.channels.insert("clock".into(), 0);
    Pass::new(
        &DESCRIPTOR,
        setup,
        Box::new(Holder {
            report,
            end: 0,
            calls: 0,
            emit,
        }),
        Sink::new(Limits::default(), Budget::new(1 << 30)),
    )
}

/// Runs the pass over the capture in chunks of 100 edges and returns, for every chunk, the end of
/// the chunk and the coverage of the results after it, then the coverage after the end.
fn run(report: Report, emit: bool) -> (Vec<(u64, u64)>, u64, Pass) {
    let capture = capture();
    let mut pass = pass(report, emit);
    let mut seen = Vec::new();
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 100,
            max_span: 1 << 20,
        },
        &AtomicBool::new(false),
        &mut || false,
        &mut |position, sink| {
            seen.push((position, sink.results().coverage.covered));
        },
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Complete);
    let after = pass.sink().results().coverage.covered;
    (seen, after, pass)
}

#[test]
fn a_decoder_that_says_nothing_is_covered_to_the_end_of_every_chunk() {
    let (seen, after, _) = run(Report::Default, false);
    assert!(seen.len() > 10);
    for (end, covered) in &seen {
        assert_eq!(covered, end);
    }
    assert_eq!(after, LENGTH);
}

#[test]
fn a_decoder_that_holds_work_back_is_covered_only_as_far_as_it_says() {
    let (seen, after, _) = run(Report::Behind(1_000), false);
    for (end, covered) in &seen {
        assert_eq!(*covered, end.saturating_sub(1_000), "chunk ending at {end}");
    }
    // The end of the pass covers all of it: `finish` is where a decoder drains.
    assert_eq!(after, LENGTH);
}

#[test]
fn coverage_never_exceeds_the_chunk_and_never_decreases() {
    for report in [Report::Jumpy, Report::Ahead] {
        let (seen, after, _) = run(report, false);
        let mut previous = 0;
        for (end, covered) in &seen {
            assert!(covered <= end, "covered {covered} beyond the chunk {end}");
            assert!(*covered >= previous, "coverage went back to {covered}");
            previous = *covered;
        }
        assert_eq!(after, LENGTH);
    }
}

#[test]
fn a_stacked_decoder_is_never_handed_an_item_behind_a_watermark_that_passed_it() {
    let capture = capture();
    let mut pass = pass(Report::Behind(3_000), true);
    let stream = pass.sink().stream();
    let mut delivered = 0_usize;
    let mut checked = 0_u32;
    run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 100,
            max_span: 1 << 20,
        },
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {
            let read = stream.read(delivered, usize::MAX);
            // Everything handed over so far ends at or below the watermark.
            for item in &read.items {
                assert!(
                    item.end <= read.covered,
                    "item ending at {} behind a watermark of {}",
                    item.end,
                    read.covered
                );
            }
            delivered += read.items.len();
            checked += 1;
        },
    )
    .unwrap();
    assert!(checked > 10);
    // After the pass ended every item can be read, in order and once.
    let rest = stream.read(delivered, usize::MAX);
    delivered += rest.items.len();
    assert_eq!(delivered, stream.len());
    let all = stream.items();
    assert!(all.windows(2).all(|w| w[0].end <= w[1].end));
}

#[test]
fn a_cut_pass_is_incomplete_and_covers_what_was_done() {
    let capture = capture();
    let mut pass = pass(Report::Behind(500), false);
    let cancel = AtomicBool::new(false);
    let mut chunks = 0;
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 100,
            max_span: 1 << 20,
        },
        &cancel,
        &mut || false,
        &mut |_, _| {
            chunks += 1;
            if chunks == 3 {
                cancel.store(true, std::sync::atomic::Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Cancelled);
    let coverage = pass.sink().results().coverage;
    assert!(coverage.incomplete);
    assert!(coverage.covered > 0 && coverage.covered < LENGTH);
}
