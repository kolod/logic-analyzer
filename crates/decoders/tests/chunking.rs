//! The chunk loop: chunk cutting, resumability, validation, progress, the end of a pass and the
//! limits (spec FR-012 to FR-017, research R6).
//!
//! The decoders here are written in the test: they are small state machines whose correct output
//! is stated by hand (every rising edge of the clock, every third one closing a range).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use common::{Lcg, Part, append_parts, capture_from_parts};
use la_decoders::descriptor::{Descriptor, Input, Signal};
use la_decoders::error;
use la_decoders::input::{Chunk, SignalReader};
use la_decoders::model::{Annotation, Kind, Shape, Style, Target, Text};
use la_decoders::run::{ChunkLimits, DecodeError, End, Outcome, Pass, Run, Sink, run_pass};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Full, Limits};

static SIGNALS: [Signal; 2] = [
    Signal {
        role: "clock",
        label: "k",
        required: true,
        may_share: false,
    },
    Signal {
        role: "data",
        label: "k",
        required: false,
        may_share: false,
    },
];

static DESCRIPTOR: Descriptor = Descriptor {
    id: "test.recorder",
    version: 1,
    name: "k",
    description: "k",
    signals: &SIGNALS,
    options: &[],
    input: Input::Signals,
    outputs: &[],
    generator: false,
    lane_rows: 3,
};

/// What the decoder was called with, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Process { start: u64, end: u64, edges: usize },
    Finish(End),
}

type Log = Arc<Mutex<Vec<Event>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The normal behaviour.
    Plain,
    /// Also emit one invalid annotation (beyond the capture) and one valid one in every chunk.
    Sloppy,
    /// Return an error at the third chunk.
    FailAtThird,
}

/// A mark at every rising clock edge; every third one also ends a range that started at the
/// previous third (a resumable state machine: its state lives between chunks).
struct Recorder {
    log: Log,
    mode: Mode,
    rising: u64,
    range_from: u64,
    chunks: u64,
}

fn style() -> Style {
    Style {
        colour: Some(0),
        shape: Shape::Dot,
    }
}

fn mark(at: u64, text: &str, error: bool) -> Annotation {
    Annotation {
        kind: Kind::Point { at },
        target: Target::Lane,
        style: style(),
        text: Text::Literal(text.into()),
        error,
    }
}

impl Run for Recorder {
    fn process(
        &mut self,
        chunk: &Chunk,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        let edges: Vec<_> = reader.edges(chunk.start, chunk.end).collect();
        self.log.lock().unwrap().push(Event::Process {
            start: chunk.start,
            end: chunk.end,
            edges: edges.len(),
        });
        self.chunks += 1;
        if self.mode == Mode::FailAtThird && self.chunks == 3 {
            return Err(DecodeError::new("planned failure"));
        }
        for edge in edges.iter().filter(|e| e.role == "clock" && e.rising) {
            self.rising += 1;
            sink.annotate(mark(edge.position, &format!("r{}", self.rising), false))?;
            if self.rising % 3 == 0 {
                sink.annotate(Annotation {
                    kind: Kind::Range {
                        start: self.range_from,
                        end: edge.position,
                    },
                    target: Target::Lane,
                    style: style(),
                    text: Text::Literal(format!("g{}", self.rising / 3)),
                    error: false,
                })?;
                self.range_from = edge.position;
            }
        }
        if self.mode == Mode::Sloppy {
            sink.annotate(mark(chunk.capture_length + 5, "beyond", false))?;
            sink.annotate(mark(chunk.start, "inside", false))?;
        }
        Ok(())
    }

    fn finish(
        &mut self,
        end: End,
        _reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        self.log.lock().unwrap().push(Event::Finish(end));
        // Whatever is pending when the pass ends is flushed as an error mark.
        if self.rising % 3 != 0 {
            sink.annotate(mark(self.range_from, "pending", true))?;
        }
        Ok(())
    }
}

fn words(seed: u64, length: usize) -> Vec<u32> {
    let mut rng = Lcg(seed);
    let mut word = 0_u32;
    (0..length)
        .map(|_| {
            if rng.below(7) == 0 {
                word ^= 1;
            }
            if rng.below(11) == 0 {
                word ^= 2;
            }
            word
        })
        .collect()
}

fn setup() -> Setup {
    let mut setup = Setup::new(1, "test.recorder");
    setup.channels.insert("clock".into(), 0);
    setup.channels.insert("data".into(), 1);
    setup
}

fn pass_with(mode: Mode, limits: Limits, log: &Log) -> Pass {
    let run = Recorder {
        log: Arc::clone(log),
        mode,
        rising: 0,
        range_from: 0,
        chunks: 0,
    };
    Pass::new(
        &DESCRIPTOR,
        setup(),
        Box::new(run),
        Sink::new(limits, Budget::new(1 << 30)),
    )
}

/// The annotations of the lane as `(kind, text, error)`, in store order of start.
fn output(pass: &mut Pass) -> Vec<(Kind, String, bool)> {
    let results = pass.sink().results();
    results
        .lane
        .window(0, u64::MAX)
        .iter()
        .map(|e| {
            let Text::Literal(text) = &e.annotation.text else {
                panic!("literal expected")
            };
            (e.annotation.kind, text.clone(), e.annotation.error)
        })
        .collect()
}

/// A closed capture made of `words`.
fn closed(words: &[u32]) -> la_storage::Capture {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(words.to_vec())]);
    capture.close().unwrap();
    capture
}

fn run_all(words: &[u32], limits: ChunkLimits, mode: Mode) -> (Pass, Log, Outcome, Vec<u64>) {
    let capture = closed(words);
    let log: Log = Arc::default();
    let mut pass = pass_with(mode, Limits::default(), &log);
    let mut positions = Vec::new();
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &limits,
        &AtomicBool::new(false),
        &mut || false,
        &mut |p, _| positions.push(p),
    )
    .unwrap();
    (pass, log, outcome, positions)
}

/// The number of edges (of both channels) in `[start, end)` by brute force.
fn brute_edge_count(words: &[u32], start: u64, end: u64) -> usize {
    (start.max(1)..end)
        .map(|p| {
            let diff = words[p as usize] ^ words[p as usize - 1];
            (diff & 3).count_ones() as usize
        })
        .sum()
}

#[test]
fn chunks_are_contiguous_from_zero_and_bounded() {
    let words = words(1, 20_000);
    let limits = ChunkLimits {
        max_edges: 50,
        max_span: 1_000,
    };
    let (_, log, outcome, positions) = run_all(&words, limits, Mode::Plain);
    assert_eq!(outcome, Outcome::Complete);
    let events = log.lock().unwrap().clone();
    let mut expected_start = 0;
    let mut chunks = 0;
    for event in &events {
        if let Event::Process { start, end, edges } = event {
            assert_eq!(*start, expected_start, "contiguous");
            assert!(end > start);
            assert!(end - start <= 1_000, "span {}", end - start);
            assert!(*edges <= 50, "{edges} edges");
            assert_eq!(*edges, brute_edge_count(&words, *start, *end));
            expected_start = *end;
            chunks += 1;
        }
    }
    assert_eq!(
        expected_start, 20_000,
        "the last chunk ends where the capture ends"
    );
    assert!(chunks > 20);
    // Progress was reported after every chunk, increasing, ending at the length.
    assert_eq!(positions.len(), chunks);
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(positions.last(), Some(&20_000));
}

#[test]
fn the_output_is_the_same_for_every_chunk_bound() {
    let words = words(2, 30_000);
    let reference = {
        let (mut pass, _, _, _) = run_all(&words, ChunkLimits::default(), Mode::Plain);
        output(&mut pass)
    };
    assert!(reference.len() > 100);
    // The marks are exactly the rising edges of the clock by brute force.
    let rising: Vec<u64> = (1..30_000_usize)
        .filter(|p| words[*p] & 1 != 0 && words[*p - 1] & 1 == 0)
        .map(|p| p as u64)
        .collect();
    let marks: Vec<u64> = reference
        .iter()
        .filter_map(|(kind, text, _)| match kind {
            Kind::Point { at } if text.starts_with('r') => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(marks, rising);
    for (max_edges, max_span) in [
        (1, 1),
        (1, 7),
        (3, 64),
        (50, 1_000),
        (65_536, 1 << 22),
        (1, u64::MAX),
    ] {
        let limits = ChunkLimits {
            max_edges,
            max_span,
        };
        let (mut pass, _, outcome, _) = run_all(&words, limits, Mode::Plain);
        assert_eq!(outcome, Outcome::Complete);
        let mut got = output(&mut pass);
        let mut want = reference.clone();
        got.sort_by_key(|(k, t, _)| (format!("{k:?}"), t.clone()));
        want.sort_by_key(|(k, t, _)| (format!("{k:?}"), t.clone()));
        assert_eq!(
            got, want,
            "chunks of {max_edges} edges and {max_span} positions"
        );
    }
}

#[test]
fn finish_is_called_once_at_the_real_end_and_flushes_what_is_pending() {
    let words = words(3, 5_000);
    let (mut pass, log, outcome, _) = run_all(&words, ChunkLimits::default(), Mode::Plain);
    assert_eq!(outcome, Outcome::Complete);
    let events = log.lock().unwrap().clone();
    let finishes: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::Finish(_)))
        .collect();
    assert_eq!(finishes, vec![&Event::Finish(End::Complete)]);
    assert_eq!(
        events.last(),
        Some(&Event::Finish(End::Complete)),
        "after every chunk"
    );
    // The pending construct (the rising edges since the last range) was flushed as an error.
    let out = output(&mut pass);
    let rising = out.iter().filter(|(_, t, _)| t.starts_with('r')).count();
    assert_eq!(
        out.iter().any(|(_, t, e)| t == "pending" && *e),
        rising % 3 != 0
    );
    let results = pass.sink().results();
    assert_eq!(results.coverage.covered, 5_000);
    assert!(!results.coverage.incomplete);
}

#[test]
fn what_the_decoder_emits_is_validated_and_the_refused_are_counted_not_stored() {
    let words = words(4, 4_000);
    let limits = ChunkLimits {
        max_edges: 100,
        max_span: 500,
    };
    let (mut pass, log, _, positions) = run_all(&words, limits, Mode::Sloppy);
    let out = output(&mut pass);
    let chunks = positions.len() as u64;
    assert_eq!(
        out.iter().filter(|(_, t, _)| t == "inside").count() as u64,
        chunks
    );
    assert!(out.iter().all(|(_, t, _)| t != "beyond"));
    assert_eq!(pass.sink().refused().total(), chunks);
    assert_eq!(pass.sink().refused().by("position"), chunks);
    assert_eq!(pass.sink().results().coverage.refused, chunks);
    assert!(log.lock().unwrap().len() as u64 > chunks);
}

#[test]
fn a_growing_capture_is_followed_and_finish_waits_for_its_end() {
    let all = words(5, 12_000);
    let reference = {
        let (mut pass, _, _, _) = run_all(&all, ChunkLimits::default(), Mode::Plain);
        output(&mut pass)
    };
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(all[..3_000].to_vec())]);
    let reader = capture.reader();
    let log: Log = Arc::default();
    let mut pass = pass_with(Mode::Plain, Limits::default(), &log);
    let mut rounds = 0;
    let mut capture_cell = Some(&mut capture);
    let outcome = run_pass(
        &mut pass,
        &reader,
        &ChunkLimits {
            max_edges: 40,
            max_span: 700,
        },
        &AtomicBool::new(false),
        &mut || {
            // Each time the pass has caught up, the capture grows; the last time it ends.
            rounds += 1;
            let capture = capture_cell.as_mut().unwrap();
            let from = capture.length() as usize;
            let to = (from + 3_000).min(12_000);
            append_parts(capture, &[Part::Samples(all[from..to].to_vec())]);
            if to == 12_000 {
                capture.close().unwrap();
            }
            true
        },
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Complete);
    assert_eq!(rounds, 3);
    let events = log.lock().unwrap().clone();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Finish(_)))
            .count(),
        1
    );
    // Following gives the same result as decoding the finished capture at once.
    assert_eq!(output(&mut pass), reference);
}

#[test]
fn giving_up_on_a_capture_that_stops_growing_cuts_the_pass() {
    let all = words(6, 2_000);
    let capture = capture_from_parts(2, 1_000_000, &[Part::Samples(all)]);
    let log: Log = Arc::default();
    let mut pass = pass_with(Mode::Plain, Limits::default(), &log);
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(log.lock().unwrap().last(), Some(&Event::Finish(End::Cut)));
    let results = pass.sink().results();
    assert!(results.coverage.incomplete);
    assert_eq!(results.coverage.covered, 2_000);
}

#[test]
fn a_cancel_stops_at_the_next_chunk_boundary_and_keeps_what_was_found() {
    let words = words(7, 20_000);
    let capture = closed(&words);
    let log: Log = Arc::default();
    let mut pass = pass_with(Mode::Plain, Limits::default(), &log);
    let cancel = AtomicBool::new(false);
    let mut seen = 0;
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 30,
            max_span: 500,
        },
        &cancel,
        &mut || false,
        &mut |_, _| {
            seen += 1;
            if seen == 4 {
                cancel.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Cancelled);
    let events = log.lock().unwrap().clone();
    let processed = events
        .iter()
        .filter(|e| matches!(e, Event::Process { .. }))
        .count();
    assert_eq!(processed, 4, "no chunk after the cancel was seen");
    assert_eq!(events.last(), Some(&Event::Finish(End::Cut)));
    let results = pass.sink().results();
    assert!(results.coverage.incomplete);
    assert!(results.coverage.covered < 20_000 && results.coverage.covered > 0);
    assert!(!output(&mut pass).is_empty());
}

#[test]
fn the_annotation_limit_ends_the_pass_with_what_fits() {
    let words = words(8, 20_000);
    let capture = closed(&words);
    let log: Log = Arc::default();
    let mut pass = pass_with(
        Mode::Plain,
        Limits {
            max_annotations: 25,
        },
        &log,
    );
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Limit(Full::Count));
    assert_eq!(pass.sink().annotation_count(), 25);
    assert!(pass.sink().results().coverage.incomplete);
    assert_eq!(log.lock().unwrap().last(), Some(&Event::Finish(End::Cut)));
}

#[test]
fn the_memory_limit_ends_the_pass_and_gives_the_memory_back() {
    let words = words(9, 20_000);
    let capture = closed(&words);
    let log: Log = Arc::default();
    let budget = Budget::new(2_000);
    let run = Recorder {
        log: Arc::clone(&log),
        mode: Mode::Plain,
        rising: 0,
        range_from: 0,
        chunks: 0,
    };
    let mut pass = Pass::new(
        &DESCRIPTOR,
        setup(),
        Box::new(run),
        Sink::new(Limits::default(), Arc::clone(&budget)),
    );
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Limit(Full::Memory));
    assert!(budget.used() <= 2_000 && budget.used() > 0);
    drop(pass);
    assert_eq!(budget.used(), 0);
}

#[test]
fn a_decoder_error_is_reported_with_its_code_and_keeps_the_earlier_output() {
    let words = words(10, 20_000);
    let capture = closed(&words);
    let log: Log = Arc::default();
    let mut pass = pass_with(Mode::FailAtThird, Limits::default(), &log);
    let err = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 20,
            max_span: 400,
        },
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap_err();
    assert_eq!(err.code(), error::FAILED);
    assert!(err.detail().contains("planned failure"));
    // No chunk after the failing one, and the output of the first two is kept, incomplete.
    let processed = log
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, Event::Process { .. }))
        .count();
    assert_eq!(processed, 3);
    assert!(!output(&mut pass).is_empty());
    assert!(pass.sink().results().coverage.incomplete);
}

#[test]
fn a_gap_is_listed_in_the_chunks_it_touches() {
    struct Gaps(Arc<Mutex<Vec<(u64, u64, usize)>>>);
    impl Run for Gaps {
        fn process(
            &mut self,
            c: &Chunk,
            _: &SignalReader,
            _: &mut Sink,
        ) -> Result<(), DecodeError> {
            self.0.lock().unwrap().push((c.start, c.end, c.gaps.len()));
            Ok(())
        }
        fn finish(&mut self, _: End, _: &SignalReader, _: &mut Sink) -> Result<(), DecodeError> {
            Ok(())
        }
    }
    let a = words(11, 100);
    let b = words(12, 100);
    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[Part::Samples(a), Part::Gap(100), Part::Samples(b)],
    );
    capture.close().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut pass = Pass::new(
        &DESCRIPTOR,
        setup(),
        Box::new(Gaps(Arc::clone(&seen))),
        Sink::new(Limits::default(), Budget::new(1 << 20)),
    );
    run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits {
            max_edges: 1_000,
            max_span: 60,
        },
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    for (start, end, gaps) in seen.lock().unwrap().iter() {
        let touches = *start < 200 && *end > 100;
        assert_eq!(*gaps, usize::from(touches), "chunk [{start},{end})");
    }
}
