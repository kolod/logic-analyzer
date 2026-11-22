//! Building I2C bus signals and reading recorded ones, for the tests of feature 008.
//!
//! The **bus builder** turns a list of bus events into samples with 16 samples per SCL period, exactly
//! as `specs/008-i2c-decoder-generator/contracts/i2c-generator.md` describes the bus: in a bit slot
//! SCL is low for samples 0..4, high for 4..12 and low for 12..16 and SDA holds the bit for all 16, so
//! the rising edge of SCL (the sampling edge) is at slot sample 4 and the falling edge at sample 12.
//! It is written independently of the decoder and of the generator; tests state their expectations
//! by hand from the positions it reports.
//!
//! The **fixture format** (a recorded signal and what a decode of it must give) is one text file:
//!
//! ```text
//! # a comment
//! 11 64              <- a run: SDA level, SCL level, number of samples
//! 01 4
//! --- expect         <- what follows is the expectation, one line each
//! lane part 52 180 1 ann-i2c-address-write 0x50
//! lane whole 52 300 2 ann-i2c-transaction-write 0x50
//! ch0 edge 72 rising
//! ch0 edge 180 rising ann-i2c-nak error
//! ```
//!
//! An expectation line is `<target> <kind> <positions> <key> <arguments...> [error]` where the target
//! is `lane` or `chN` (an overlay on channel N); the kinds are `range <start> <end>`, `point <at>`
//! and `edge <at> <rising|falling>`, and for the lane the blocks `part <from> <to> <colour>` (the upper
//! band) and `whole <from> <to> <colour>` (the lower band, the colour is an index of the palette);
//! the text is a message key and its arguments (an underscore in an argument is a space), or `=text`
//! for a literal text (the bit an arrow shows). SCL is channel 0 and SDA channel 1 (see [`SCL`] and [`SDA`]).
#![allow(dead_code)]

/// The channel the helpers put SCL on.
pub const SCL: u8 = 0;
/// The channel the helpers put SDA on.
pub const SDA: u8 = 1;
/// Samples in one SCL period.
pub const SLOT: u64 = 16;

/// Where the parts of a byte are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct At {
    /// The SCL rising edge of the first bit.
    pub first: u64,
    /// The SCL rising edge of the acknowledge bit.
    pub ack: u64,
    /// The SCL falling edge that ends the acknowledge bit.
    pub fall: u64,
}

/// A bus signal under construction: `(sda, scl)` per sample.
#[derive(Debug, Clone, Default)]
pub struct Bus {
    /// The samples.
    pub samples: Vec<(bool, bool)>,
}

impl Bus {
    /// An empty signal.
    pub fn new() -> Self {
        Self::default()
    }

    /// The position the next sample gets.
    pub fn pos(&self) -> u64 {
        self.samples.len() as u64
    }

    /// `n` samples of the given levels.
    pub fn raw(&mut self, sda: bool, scl: bool, n: u64) -> &mut Self {
        for _ in 0..n {
            self.samples.push((sda, scl));
        }
        self
    }

    /// `n` samples with both lines high.
    pub fn idle(&mut self, n: u64) -> &mut Self {
        self.raw(true, true, n)
    }

    /// A start condition: SDA falls while SCL is high. Returns the position of the SDA edge. The
    /// first clock pulse rises 16 samples after it, as in the signal of the generator.
    pub fn start(&mut self) -> u64 {
        let at = self.pos();
        self.raw(false, true, 12);
        at
    }

    /// One clock pulse carrying `bit`. Returns the position of the SCL rising edge.
    pub fn bit(&mut self, bit: bool) -> u64 {
        let at = self.pos() + 4;
        self.raw(bit, false, 4);
        self.raw(bit, true, 8);
        self.raw(bit, false, 4);
        at
    }

    /// Eight bits, most significant first, and an acknowledge bit (`ack` true is a low SDA).
    pub fn byte(&mut self, value: u8, ack: bool) -> At {
        let mut first = 0;
        for i in 0..8 {
            let at = self.bit(value >> (7 - i) & 1 != 0);
            if i == 0 {
                first = at;
            }
        }
        let ack_at = self.bit(!ack);
        At {
            first,
            ack: ack_at,
            fall: ack_at + 8,
        }
    }

    /// The first `bits` bits of a byte and nothing else (SCL left low).
    pub fn bits(&mut self, value: u8, bits: u32) -> &mut Self {
        for i in 0..bits {
            self.bit(value >> (7 - i) & 1 != 0);
        }
        self
    }

    /// A repeated start: SDA is raised while SCL is low, SCL rises, SDA falls while SCL is high.
    /// Returns the position of the SDA falling edge.
    pub fn repeated_start(&mut self) -> u64 {
        self.raw(true, false, 4);
        self.raw(true, true, 4);
        let at = self.pos();
        self.raw(false, true, 12);
        at
    }

    /// A stop condition: SDA is lowered while SCL is low, SCL rises, SDA rises while SCL is high.
    /// Returns the position of the SDA rising edge (the first sample of the idle that follows).
    pub fn stop(&mut self) -> u64 {
        self.raw(false, false, 4);
        self.raw(false, true, 4);
        let at = self.pos();
        self.raw(true, true, 1);
        at
    }

    /// The sample words: SCL on [`SCL`], SDA on [`SDA`].
    pub fn words(&self) -> Vec<u32> {
        self.samples
            .iter()
            .map(|(sda, scl)| (u32::from(*scl) << SCL) | (u32::from(*sda) << SDA))
            .collect()
    }
}

/// A line of the expected result of a recorded signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// `None` for the lane, the channel of an overlay otherwise.
    pub channel: Option<u8>,
    /// `range`, `point` or `edge`.
    pub kind: String,
    /// The positions of the annotation.
    pub positions: Vec<u64>,
    /// `Some(true)` for an edge mark that rises.
    pub rising: Option<bool>,
    /// For a block of the lane: its shape (`part` or `whole`) and the colour index it names.
    pub block: Option<(String, u8)>,
    /// The message key, empty for marks without text.
    pub key: String,
    /// The arguments of the message.
    pub args: Vec<String>,
    /// The error flag.
    pub error: bool,
}

/// A recorded signal and what a decode of it must give.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The samples (SCL on channel 0, SDA on channel 1).
    pub words: Vec<u32>,
    /// The expected annotations.
    pub expected: Vec<Expected>,
}

/// Why a fixture was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

/// The most samples a fixture may hold, so that a damaged file cannot ask for gigabytes.
pub const MAX_SAMPLES: u64 = 50_000_000;

/// Reads a fixture from its text.
pub fn parse_fixture(text: &str) -> Result<Fixture, Malformed> {
    let bad = |line: usize, why: &str| Malformed(format!("line {}: {why}", line + 1));
    let mut words: Vec<u32> = Vec::new();
    let mut expected = Vec::new();
    let mut in_expect = false;
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "--- expect" {
            in_expect = true;
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if in_expect {
            expected.push(parse_expected(&parts).map_err(|why| bad(number, &why))?);
            continue;
        }
        let [levels, count] = parts.as_slice() else {
            return Err(bad(number, "a run is `<sda><scl> <count>`"));
        };
        let word = match *levels {
            "00" => 0,
            "01" => 1 << SCL,
            "10" => 1 << SDA,
            "11" => (1 << SCL) | (1 << SDA),
            _ => return Err(bad(number, "levels are 00, 01, 10 or 11")),
        };
        let count: u64 = count.parse().map_err(|_| bad(number, "a run length"))?;
        if count == 0 || count > MAX_SAMPLES || words.len() as u64 + count > MAX_SAMPLES {
            return Err(bad(number, "a run length out of range"));
        }
        words.extend(std::iter::repeat_n(word, count as usize));
    }
    if !in_expect {
        return Err(Malformed("the `--- expect` section is missing".into()));
    }
    if words.is_empty() {
        return Err(Malformed("no samples".into()));
    }
    Ok(Fixture { words, expected })
}

fn parse_expected(parts: &[&str]) -> Result<Expected, String> {
    let mut it = parts.iter().copied();
    let target = it.next().ok_or("empty line")?;
    let channel = match target {
        "lane" => None,
        t => Some(
            t.strip_prefix("ch")
                .and_then(|n| n.parse::<u8>().ok())
                .ok_or("the target is `lane` or `chN`")?,
        ),
    };
    let kind = it.next().ok_or("a kind")?.to_owned();
    let mut number = |what: &str| -> Result<u64, String> {
        it.next()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| format!("a position for {what}"))
    };
    let mut block = None;
    let (positions, rising) = match kind.as_str() {
        "part" | "whole" => {
            let from = number("from")?;
            let to = number("to")?;
            let colour = u8::try_from(number("colour")?).map_err(|_| "a colour index")?;
            block = Some((kind.clone(), colour));
            (vec![from, to], None)
        }
        "range" => (vec![number("start")?, number("end")?], None),
        "point" => (vec![number("at")?], None),
        "edge" => {
            let at = number("at")?;
            let rising = match it.next() {
                Some("rising") => true,
                Some("falling") => false,
                _ => return Err("`rising` or `falling`".into()),
            };
            (vec![at], Some(rising))
        }
        _ => return Err("the kind is part, whole, range, point or edge".into()),
    };
    let kind = if block.is_some() {
        "range".to_owned()
    } else {
        kind
    };
    let rest: Vec<&str> = it.collect();
    let (error, rest) = match rest.split_last() {
        Some((&"error", head)) => (true, head.to_vec()),
        _ => (false, rest),
    };
    let (key, args) = match rest.split_first() {
        // `=1` is a literal text (no message key): the text of a sampled bit.
        Some((literal, _)) if literal.starts_with('=') => {
            (String::new(), vec![literal[1..].to_owned()])
        }
        Some((key, args)) => (
            (*key).to_owned(),
            // An underscore stands for a space in an argument (the data of a packet).
            args.iter().map(|a| a.replace('_', " ")).collect(),
        ),
        None => (String::new(), Vec::new()),
    };
    Ok(Expected {
        channel,
        kind,
        positions,
        rising,
        block,
        key,
        args,
        error,
    })
}

/// Reads `tests/fixtures/i2c/<name>.txt`.
pub fn load_fixture(name: &str) -> Fixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/i2c")
        .join(format!("{name}.txt"));
    let text = std::fs::read_to_string(&path).unwrap();
    parse_fixture(&text).unwrap_or_else(|e| panic!("{name}: {}", e.0))
}

// ---------------------------------------------------------------------------------------------
// Decoding with the I2C decoder and putting the result into the form of an expectation.
// ---------------------------------------------------------------------------------------------

use std::sync::atomic::AtomicBool;

use la_decoders::decoders::i2c::{DESCRIPTOR, ID, create};
use la_decoders::model::{Annotation, Item, Kind, Shape, Target, Text};
use la_decoders::run::{ChunkLimits, Outcome, Pass, Sink, run_pass};
use la_decoders::setup::{Setup, Value};
use la_decoders::store::{Budget, Limits};

/// The setup of an I2C instance with SCL on [`SCL`] and SDA on [`SDA`] and these options.
pub fn setup(options: &[(&str, Value)]) -> Setup {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("scl".into(), SCL);
    setup.channels.insert("sda".into(), SDA);
    for (id, value) in options {
        setup.options.insert((*id).into(), value.clone());
    }
    setup
}

/// A fresh pass of the decoder over a setup.
pub fn new_pass(setup: &Setup) -> Pass {
    Pass::new(
        &DESCRIPTOR,
        setup.clone(),
        create(setup).unwrap(),
        Sink::new(Limits::default(), Budget::new(1 << 30)),
    )
}

/// What a decode produced, in the form of the expectations.
#[derive(Debug, Clone)]
pub struct Decoded {
    /// Every annotation, sorted.
    pub all: Vec<Expected>,
    /// The shapes of the overlay marks, by channel.
    pub shapes: Vec<(u8, Shape, Option<u8>)>,
    /// The items.
    pub items: Vec<Item>,
    /// How many records the sink refused.
    pub refused: u64,
}

impl Decoded {
    /// The annotations of the lane.
    pub fn lane(&self) -> Vec<Expected> {
        self.all
            .iter()
            .filter(|e| e.channel.is_none())
            .cloned()
            .collect()
    }

    /// The annotations of the overlay on a channel.
    pub fn overlay(&self, channel: u8) -> Vec<Expected> {
        self.all
            .iter()
            .filter(|e| e.channel == Some(channel))
            .cloned()
            .collect()
    }
}

fn expected_of(annotation: &Annotation) -> Expected {
    let (kind, positions, rising) = match annotation.kind {
        Kind::Range { start, end } => ("range", vec![start, end], None),
        Kind::Point { at } => ("point", vec![at], None),
        Kind::Edge { at, rising } => ("edge", vec![at], Some(rising)),
    };
    let (key, args) = match &annotation.text {
        Text::Message { key, args } => ((*key).to_owned(), args.clone()),
        Text::Literal(text) => (String::new(), vec![text.clone()]),
    };
    let block = match annotation.style.shape {
        Shape::Part => Some(("part".to_owned(), annotation.style.colour.unwrap_or(255))),
        Shape::Whole => Some(("whole".to_owned(), annotation.style.colour.unwrap_or(255))),
        _ => None,
    };
    Expected {
        block,
        channel: match annotation.target {
            Target::Lane => None,
            Target::Channel(c) => Some(c),
        },
        kind: kind.to_owned(),
        positions,
        rising,
        key,
        args,
        error: annotation.error,
    }
}

fn sort_key(e: &Expected) -> (Option<u8>, Vec<u64>, String, String) {
    (
        e.channel,
        e.positions.clone(),
        e.kind.clone(),
        e.key.clone(),
    )
}

/// Sorts expectations the way [`Decoded::all`] is sorted.
pub fn sorted(mut list: Vec<Expected>) -> Vec<Expected> {
    list.sort_by_key(sort_key);
    list
}

/// Reads the results of a pass.
pub fn collect(pass: &mut Pass) -> Decoded {
    let results = pass.sink().results();
    let mut all = Vec::new();
    let mut shapes = Vec::new();
    for entry in results.lane.window(0, u64::MAX) {
        all.push(expected_of(&entry.annotation));
    }
    for (channel, snapshot) in &results.overlays {
        for entry in snapshot.window(0, u64::MAX) {
            all.push(expected_of(&entry.annotation));
            shapes.push((
                *channel,
                entry.annotation.style.shape,
                entry.annotation.style.colour,
            ));
        }
    }
    Decoded {
        all: sorted(all),
        shapes,
        items: pass.sink().items().to_vec(),
        refused: pass.sink().refused().total(),
    }
}

/// Decodes a closed capture of `words` (SCL on channel 0, SDA on channel 1) in chunks of `limits`.
pub fn decode_with(words: &[u32], options: &[(&str, Value)], limits: &ChunkLimits) -> Decoded {
    let mut capture = crate::common::capture_from_parts(
        2,
        1_000_000,
        &[crate::common::Part::Samples(words.to_vec())],
    );
    capture.close().unwrap();
    decode_capture(&capture, options, limits)
}

/// Decodes a closed capture.
pub fn decode_capture(
    capture: &la_storage::Capture,
    options: &[(&str, Value)],
    limits: &ChunkLimits,
) -> Decoded {
    let setup = setup(options);
    let mut pass = new_pass(&setup);
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        limits,
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Complete);
    collect(&mut pass)
}

/// Decodes a closed capture of `words` with the default chunk limits and options.
pub fn decode(words: &[u32]) -> Decoded {
    decode_with(words, &[], &ChunkLimits::default())
}

// Constructors of expectations.

/// A lane range.
pub fn lane_range(start: u64, end: u64, key: &str, args: &[&str]) -> Expected {
    Expected {
        channel: None,
        kind: "range".into(),
        positions: vec![start, end],
        rising: None,
        block: None,
        key: key.into(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        error: false,
    }
}

/// A lane point.
pub fn lane_point(at: u64, key: &str) -> Expected {
    Expected {
        channel: None,
        kind: "point".into(),
        positions: vec![at],
        rising: None,
        block: None,
        key: key.into(),
        args: Vec::new(),
        error: false,
    }
}

/// A point on a channel.
pub fn dot(channel: u8, at: u64, key: &str) -> Expected {
    Expected {
        channel: Some(channel),
        ..lane_point(at, key)
    }
}

/// A sampling arrow on SCL: the bit as text, or a message key for the acknowledge bit.
pub fn arrow(at: u64, key: &str, args: &[&str]) -> Expected {
    Expected {
        channel: Some(SCL),
        kind: "edge".into(),
        positions: vec![at],
        rising: Some(true),
        block: None,
        key: key.into(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        error: false,
    }
}

/// The same with the error flag.
pub fn erroneous(mut e: Expected) -> Expected {
    e.error = true;
    e
}

/// The nine arrows of a byte: the eight bits (most significant first) and the acknowledge bit.
pub fn byte_arrows(at: At, value: u8, ack: &str) -> Vec<Expected> {
    let mut list = Vec::new();
    for i in 0..8_u64 {
        let bit = if value >> (7 - i) & 1 != 0 { "1" } else { "0" };
        list.push(arrow(at.first + 16 * i, "", &[bit]));
    }
    list.push(arrow(at.ack, ack, &[]));
    list
}

// The blocks of the lane, by hand from the contract (`specs/016-i2c-lane-blocks/contracts/i2c-lane.md`)
// for the bus period of 16 samples: a bit's cell is 8 samples either side of its rising edge.

/// The palette indices of the colours of the blocks.
pub const BLUE: u8 = 4;
pub const YELLOW: u8 = 1;
pub const ORANGE: u8 = 6;
pub const GREEN: u8 = 2;
pub const RED: u8 = 0;
pub const GREY: u8 = 7;
/// Half the bus period: how far a cell reaches before and after the rising edge.
pub const HALF: u64 = 8;

/// A block of the upper band.
pub fn part(from: u64, to: u64, colour: u8, key: &str, args: &[&str]) -> Expected {
    Expected {
        block: Some(("part".into(), colour)),
        ..lane_range(from, to, key, args)
    }
}

/// A block of the lower band.
pub fn whole(from: u64, to: u64, colour: u8, key: &str, args: &[&str]) -> Expected {
    Expected {
        block: Some(("whole".into(), colour)),
        ..lane_range(from, to, key, args)
    }
}

/// The block of a start whose SDA edge is at `at`.
pub fn start_block(at: u64, key: &str) -> Expected {
    part(at - HALF, at + HALF, BLUE, key, &[])
}

/// The block of a repeated start whose SDA edge is at `at`: it begins where the block before it, the
/// acknowledge, ended (`previous_end`), as there is no stop before it.
pub fn restart_block(previous_end: u64, at: u64) -> Expected {
    part(
        previous_end.min(at - HALF),
        at + HALF,
        BLUE,
        "ann-i2c-repeated-start",
        &[],
    )
}

/// The block of a whole transaction: named after its first part, `read` or not, with the data that
/// went over the bus as the text says it (`data` empty: the short text).
pub fn packet(from: u64, to: u64, colour: u8, read: bool, address: &str, data: &str) -> Expected {
    let (key, args): (&str, Vec<&str>) = match (read, data.is_empty()) {
        (true, true) => ("ann-i2c-transaction-read", vec![address]),
        (false, true) => ("ann-i2c-transaction-write", vec![address]),
        (true, false) => ("ann-i2c-transaction-read-long", vec![address, data]),
        (false, false) => ("ann-i2c-transaction-write-long", vec![address, data]),
    };
    whole(from, to, colour, key, &args)
}

/// The block of a stop whose SDA edge is at `at`, the block before it ending at `last_end`.
pub fn stop_block(last_end: u64, at: u64) -> Expected {
    part((at - 2 * HALF).max(last_end), at, BLUE, "ann-i2c-stop", &[])
}

/// The end of the acknowledge block of a byte.
pub fn ack_end(at: At) -> u64 {
    at.ack + HALF
}

/// The blocks of a byte: its eight bits (in `colour`) and its acknowledge bit (green for an ACK, red
/// for a NAK, which is an error only if `ack_error`).
pub fn byte_lane(
    at: At,
    key: &str,
    text: &str,
    colour: u8,
    ack: &str,
    ack_error: bool,
) -> Vec<Expected> {
    let ack_colour = if ack == "ann-i2c-ack" { GREEN } else { RED };
    let mut cell = part(at.ack - HALF, at.ack + HALF, ack_colour, ack, &[]);
    cell.error = ack_error;
    vec![
        part(at.first - HALF, at.ack - HALF, colour, key, &[text]),
        cell,
    ]
}

// Whole transactions with their expected decodes (default options).

pub fn hex(value: u8) -> String {
    format!("0x{value:02X}")
}

/// A write of `data` to `address` (7 bits), everything acknowledged, and what decoding it gives.
pub fn write_tx(bus: &mut Bus, address: u8, data: &[u8]) -> Vec<Expected> {
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(start_block(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let a = bus.byte(address << 1, true);
    let mut last_ack_end = ack_end(a);
    expected.extend(byte_lane(
        a,
        "ann-i2c-address-write",
        &hex(address),
        ORANGE,
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(a, address << 1, "ann-i2c-ack"));
    for value in data {
        let at = bus.byte(*value, true);
        last_ack_end = ack_end(at);
        expected.extend(byte_lane(
            at,
            "ann-i2c-data",
            &hex(*value),
            ORANGE,
            "ann-i2c-ack",
            false,
        ));
        expected.extend(byte_arrows(at, *value, "ann-i2c-ack"));
    }
    let stop = bus.stop();
    expected.push(stop_block(last_ack_end, stop));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    let shown: Vec<String> = data.iter().map(|v| hex(*v)).collect();
    expected.push(packet(
        start - HALF,
        stop,
        GREEN,
        false,
        &hex(address),
        &shown.join(" "),
    ));
    expected
}

/// A read of `data` from `address`: the controller acknowledges every byte but the last.
pub fn read_tx(bus: &mut Bus, address: u8, data: &[u8]) -> Vec<Expected> {
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(start_block(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let a = bus.byte(address << 1 | 1, true);
    let mut last_ack_end = ack_end(a);
    expected.extend(byte_lane(
        a,
        "ann-i2c-address-read",
        &hex(address),
        YELLOW,
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(a, address << 1 | 1, "ann-i2c-ack"));
    for (index, value) in data.iter().enumerate() {
        let last = index + 1 == data.len();
        let at = bus.byte(*value, !last);
        last_ack_end = ack_end(at);
        let ack = if last { "ann-i2c-nak" } else { "ann-i2c-ack" };
        expected.extend(byte_lane(
            at,
            "ann-i2c-data",
            &hex(*value),
            YELLOW,
            ack,
            false,
        ));
        expected.extend(byte_arrows(at, *value, ack));
    }
    let stop = bus.stop();
    expected.push(stop_block(last_ack_end, stop));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    let shown: Vec<String> = data.iter().map(|v| hex(*v)).collect();
    expected.push(packet(
        start - HALF,
        stop,
        GREEN,
        true,
        &hex(address),
        &shown.join(" "),
    ));
    expected
}
