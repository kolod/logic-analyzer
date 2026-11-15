//! The sample decoder on hand-drawn captures (spec US1, FR-035, SC-001, SC-012).
//!
//! Every expected annotation is worked out by hand from the drawing: a bit lasts 10 samples, data
//! changes at the start of it and the clock is high from the 3rd to the 7th sample, so the rising
//! edge is 3 samples into the bit and the falling edge 8.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::{Part, capture_from_parts};
use la_decoders::decoders::clocked_bits::{BYTE_ITEM, DESCRIPTOR, ID};
use la_decoders::model::{Item, Kind, Payload, Text};
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, Outcome, Pass, Sink, run_pass};
use la_decoders::setup::{Setup, Value};
use la_decoders::store::{Budget, Limits};

const CLOCK: u8 = 0;
const DATA: u8 = 1;
const BYTE: &str = "ann-sample-clocked-bits-byte";
const BIT: &str = "ann-sample-clocked-bits-bit";
const INCOMPLETE: &str = "ann-sample-clocked-bits-incomplete";
const GAP: &str = "ann-sample-clocked-bits-gap";

/// Samples of `bits` starting at `start`, then `tail` quiet samples (the clock low, the data held).
/// Channel 0 is the clock, channel 1 the data.
fn draw(bits: &[bool], start: usize, tail: usize) -> Vec<u32> {
    let mut words = vec![0_u32; start];
    for bit in bits {
        for offset in 0..10 {
            let clock = (3..8).contains(&offset);
            words.push(u32::from(clock) | (u32::from(*bit) << 1));
        }
    }
    let held = bits.last().map_or(0, |b| u32::from(*b) << 1);
    words.extend(std::iter::repeat_n(held, tail));
    words
}

fn bits_of(value: u32, count: usize) -> Vec<bool> {
    (0..count).rev().map(|i| value >> i & 1 != 0).collect()
}

fn setup(options: &[(&str, Value)]) -> Setup {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("clock".into(), CLOCK);
    setup.channels.insert("data".into(), DATA);
    for (id, value) in options {
        setup.options.insert((*id).into(), value.clone());
    }
    setup
}

/// Everything a pass produced.
#[derive(Debug, PartialEq)]
struct Output {
    lane: Vec<(Kind, Text, bool)>,
    clock: Vec<(Kind, Text, bool)>,
    data: Vec<(Kind, Text, bool)>,
    items: Vec<Item>,
}

fn entries(snapshot: &la_decoders::store::Snapshot) -> Vec<(Kind, Text, bool)> {
    snapshot
        .window(0, u64::MAX)
        .iter()
        .map(|e| {
            (
                e.annotation.kind,
                e.annotation.text.clone(),
                e.annotation.error,
            )
        })
        .collect()
}

fn run(parts: &[Part], setup: &Setup, limits: ChunkLimits) -> Output {
    let mut capture = capture_from_parts(2, 1_000_000, parts);
    capture.close().unwrap();
    let registry = Registry::builtin();
    registry.get(ID).expect("registered");
    setup.validate(&registry, &[], 2).unwrap();
    let mut pass = Pass::new(
        &DESCRIPTOR,
        setup.clone(),
        registry.create(setup).unwrap(),
        Sink::new(Limits::default(), Budget::new(1 << 30)),
    );
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &limits,
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Complete);
    let results = pass.sink().results();
    let overlay = |c: u8| {
        results
            .overlays
            .get(&c)
            .map(|s| entries(s))
            .unwrap_or_default()
    };
    Output {
        lane: entries(&results.lane),
        clock: overlay(CLOCK),
        data: overlay(DATA),
        items: pass.sink().items().to_vec(),
    }
}

fn samples(words: Vec<u32>) -> Vec<Part> {
    vec![Part::Samples(words)]
}

fn message(key: &'static str, args: &[&str]) -> Text {
    Text::Message {
        key,
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn point(at: u64) -> Kind {
    Kind::Point { at }
}

fn range(start: u64, end: u64) -> Kind {
    Kind::Range { start, end }
}

fn edge(at: u64, rising: bool) -> Kind {
    Kind::Edge { at, rising }
}

fn byte_item(start: u64, end: u64, value: u64) -> Item {
    Item {
        kind: BYTE_ITEM,
        start,
        end,
        payload: Payload::Integer(value),
        error: false,
    }
}

fn default_limits() -> ChunkLimits {
    ChunkLimits::default()
}

#[test]
fn one_byte_gives_a_range_a_point_per_bit_clock_arrows_and_a_data_dot() {
    // 0xB1 = 1011 0001, most significant bit first; the rising edges are at 8, 18, ... 78.
    let out = run(
        &samples(draw(&bits_of(0xB1, 8), 5, 300)),
        &setup(&[]),
        default_limits(),
    );
    let sampled = [8_u64, 18, 28, 38, 48, 58, 68, 78];
    let levels = ["1", "0", "1", "1", "0", "0", "0", "1"];
    let mut lane = Vec::new();
    for (at, level) in sampled.iter().zip(levels) {
        lane.push((point(*at), message(BIT, &[level]), false));
    }
    // The byte ends at the last sampled edge, included.
    lane.push((range(8, 79), message(BYTE, &["0xB1"]), false));
    // The store orders by start, then by emission: the byte comes after the bit it ends with.
    let key =
        |(k, _, _): &(Kind, Text, bool)| (k.start_for_test(), matches!(k, Kind::Range { .. }));
    let mut got = out.lane.clone();
    got.sort_by_key(key);
    lane.sort_by_key(key);
    assert_eq!(got, lane);
    // An arrow on the clock channel at every sampled edge.
    let arrows: Vec<(Kind, Text, bool)> = sampled
        .iter()
        .zip(levels)
        .map(|(at, level)| (edge(*at, true), message(BIT, &[level]), false))
        .collect();
    assert_eq!(out.clock, arrows);
    // One dot on the data channel, at the first sampled edge.
    assert_eq!(out.data, vec![(point(8), message(BIT, &["1"]), false)]);
    assert_eq!(out.items, vec![byte_item(8, 79, 0xB1)]);
}

#[test]
fn the_bit_order_decides_the_value() {
    let signal = samples(draw(&bits_of(0xB1, 8), 5, 300));
    let lsb = run(
        &signal,
        &setup(&[("bit_order", Value::Choice("lsb".into()))]),
        default_limits(),
    );
    // The same bits read the other way round: 1000 1101.
    assert_eq!(lsb.items, vec![byte_item(8, 79, 0x8D)]);
    assert!(
        lsb.lane
            .iter()
            .any(|(k, t, _)| *k == range(8, 79) && *t == message(BYTE, &["0x8D"]))
    );
}

#[test]
fn the_falling_edge_samples_on_the_falling_edges() {
    // Falling edges are at 13, 23, ... 83.
    let signal = samples(draw(&bits_of(0xB1, 8), 5, 300));
    let out = run(
        &signal,
        &setup(&[("clock_edge", Value::Choice("falling".into()))]),
        default_limits(),
    );
    assert_eq!(out.items, vec![byte_item(13, 84, 0xB1)]);
    let at: Vec<u64> = out
        .clock
        .iter()
        .map(|(k, _, _)| match k {
            Kind::Edge { at, rising } => {
                assert!(!rising, "the arrow shows the edge that was used");
                *at
            }
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(at, vec![13, 23, 33, 43, 53, 63, 73, 83]);
}

#[test]
fn the_number_of_bits_is_an_option_from_4_to_16() {
    // 0xA55A as 16 bits, then as two bytes of 8, then as four nibbles.
    let bits = bits_of(0xA55A, 16);
    let sixteen = run(
        &samples(draw(&bits, 5, 300)),
        &setup(&[("bits", Value::Integer(16))]),
        default_limits(),
    );
    assert_eq!(sixteen.items, vec![byte_item(8, 159, 0xA55A)]);
    assert!(
        sixteen
            .lane
            .iter()
            .any(|(_, t, _)| *t == message(BYTE, &["0xA55A"]))
    );
    let nibbles = run(
        &samples(draw(&bits, 5, 300)),
        &setup(&[("bits", Value::Integer(4))]),
        default_limits(),
    );
    let values: Vec<Payload> = nibbles.items.iter().map(|i| i.payload.clone()).collect();
    assert_eq!(values, [0xA, 0x5, 0x5, 0xA].map(Payload::Integer).to_vec());
    // Items never go backwards and never overlap here.
    assert!(nibbles.items.windows(2).all(|w| w[0].end <= w[1].start));
}

#[test]
fn a_byte_cut_by_a_long_pause_is_an_incomplete_byte_error() {
    // Five bits, 200 quiet samples (more than the idle limit), then a whole byte.
    let mut words = draw(&bits_of(0b10110, 5), 5, 200);
    let second_start = words.len();
    words.extend(draw(&bits_of(0x3C, 8), 0, 100));
    let out = run(&samples(words), &setup(&[]), default_limits());
    // The first five bits were sampled at 8, 18, 28, 38, 48: an error range over them.
    let incomplete = (range(8, 49), message(INCOMPLETE, &["5"]), true);
    assert!(out.lane.contains(&incomplete), "{:?}", out.lane);
    // The whole byte after the pause is decoded normally: its first edge is 3 samples in.
    let first = second_start as u64 + 3;
    assert_eq!(out.items, vec![byte_item(first, first + 71, 0x3C)]);
    assert_eq!(out.lane.iter().filter(|(_, _, e)| *e).count(), 1);
    // Both bytes get a data dot.
    assert_eq!(out.data.len(), 2);
}

#[test]
fn a_byte_cut_by_the_end_of_the_capture_is_flushed_as_an_error() {
    let out = run(
        &samples(draw(&bits_of(0b101, 3), 5, 4)),
        &setup(&[]),
        default_limits(),
    );
    assert!(out.items.is_empty());
    assert!(
        out.lane
            .contains(&(range(8, 29), message(INCOMPLETE, &["3"]), true))
    );
}

#[test]
fn a_byte_cut_by_a_gap_is_an_error_and_the_gap_is_reported() {
    // Five bits (the last edge at 48), then 50 lost positions starting at 55, then a whole byte.
    let first = draw(&bits_of(0b10110, 5), 5, 0);
    assert_eq!(first.len(), 55);
    let second = draw(&bits_of(0xC3, 8), 5, 200);
    let parts = vec![Part::Samples(first), Part::Gap(50), Part::Samples(second)];
    let out = run(&parts, &setup(&[]), default_limits());
    assert!(
        out.lane
            .contains(&(range(8, 49), message(INCOMPLETE, &["5"]), true))
    );
    assert!(
        out.lane
            .contains(&(range(55, 105), message(GAP, &[]), true))
    );
    // The byte after the gap is whole: its edges are 105 + 8 ... 105 + 78.
    assert_eq!(out.items, vec![byte_item(113, 184, 0xC3)]);
    assert_eq!(out.lane.iter().filter(|(_, _, e)| *e).count(), 2);
}

#[test]
fn a_one_sample_glitch_on_the_clock_is_one_edge() {
    // No data edges, the clock high for exactly one sample at 20.
    let mut words = vec![0_u32; 100];
    words[20] = 1;
    let out = run(&samples(words), &setup(&[]), default_limits());
    assert_eq!(
        out.clock,
        vec![(edge(20, true), message(BIT, &["0"]), false)]
    );
    assert_eq!(
        out.lane.len(),
        2,
        "one bit and the incomplete byte it ends up in"
    );
    assert!(
        out.lane
            .contains(&(range(20, 21), message(INCOMPLETE, &["1"]), true))
    );
}

#[test]
fn nothing_happens_without_clock_edges() {
    // Data toggling, the clock never moving, and a capture of nothing at all.
    let words: Vec<u32> = (0..500).map(|i| u32::from(i / 7 % 2 == 0) << 1).collect();
    let out = run(&samples(words), &setup(&[]), default_limits());
    assert_eq!(
        out,
        Output {
            lane: vec![],
            clock: vec![],
            data: vec![],
            items: vec![]
        }
    );
    let empty = run(&samples(vec![]), &setup(&[]), default_limits());
    assert!(empty.lane.is_empty() && empty.items.is_empty());
}

#[test]
fn data_stuck_high_or_low_gives_all_ones_or_all_zeros() {
    for (data, value) in [(true, 0xFF_u64), (false, 0x00)] {
        let out = run(
            &samples(draw(&[data; 8], 5, 100)),
            &setup(&[]),
            default_limits(),
        );
        assert_eq!(out.items, vec![byte_item(8, 79, value)], "{data}");
    }
}

#[test]
fn an_idle_limit_of_one_makes_every_bit_an_incomplete_byte() {
    let out = run(
        &samples(draw(&bits_of(0xB1, 8), 5, 100)),
        &setup(&[("idle", Value::Integer(1))]),
        default_limits(),
    );
    assert!(out.items.is_empty());
    let errors = out.lane.iter().filter(|(_, _, e)| *e).count();
    assert_eq!(errors, 8, "each of the eight bits ends alone");
}

#[test]
fn the_output_does_not_depend_on_the_chunk_size() {
    // Two bytes, a pause, a cut byte, a gap, another byte, a glitch: everything at once.
    let mut a = draw(&bits_of(0xB1, 8), 5, 20);
    a.extend(draw(&bits_of(0x4E, 8), 0, 200));
    a.extend(draw(&bits_of(0b110, 3), 0, 150));
    let mut b = draw(&bits_of(0b01, 2), 0, 0);
    let c = draw(&bits_of(0x99, 8), 3, 40);
    b.push(0);
    let parts = vec![
        Part::Samples(a),
        Part::Gap(37),
        Part::Samples(b),
        Part::Samples(c),
    ];
    let reference = run(&parts, &setup(&[]), default_limits());
    assert!(reference.items.len() >= 3);
    assert!(reference.lane.iter().any(|(_, _, e)| *e));
    for (max_edges, max_span) in [
        (1, 1),
        (1, 5),
        (2, 100),
        (3, 1_000),
        (7, u64::MAX),
        (65_536, 1 << 22),
    ] {
        let limits = ChunkLimits {
            max_edges,
            max_span,
        };
        assert_eq!(
            run(&parts, &setup(&[]), limits),
            reference,
            "{max_edges}/{max_span}"
        );
    }
}

#[test]
fn the_setup_is_validated_against_the_descriptor() {
    let registry = Registry::builtin();
    let mut missing = Setup::new(1, ID);
    missing.channels.insert("clock".into(), 0);
    assert!(
        missing.validate(&registry, &[], 2).is_err(),
        "the data channel is required"
    );
    let mut shared = setup(&[]);
    shared.channels.insert("data".into(), 0);
    assert!(
        shared.validate(&registry, &[], 2).is_err(),
        "one channel for two roles"
    );
    for (id, value) in [
        ("bits", Value::Integer(3)),
        ("bits", Value::Integer(17)),
        ("idle", Value::Integer(0)),
        ("idle", Value::Integer(1_000_001)),
        ("bit_order", Value::Choice("middle".into())),
    ] {
        assert!(setup(&[(id, value)]).validate(&registry, &[], 2).is_err());
    }
    assert_eq!(DESCRIPTOR.id, ID);
    assert_eq!(DESCRIPTOR.options.len(), 4);
}

trait StartForTest {
    fn start_for_test(&self) -> u64;
}

impl StartForTest for Kind {
    fn start_for_test(&self) -> u64 {
        match self {
            Kind::Range { start, .. } => *start,
            Kind::Point { at } | Kind::Edge { at, .. } => *at,
        }
    }
}
