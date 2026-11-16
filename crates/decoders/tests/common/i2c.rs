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
//! lane range 68 85 ann-i2c-address-write 0x50
//! lane point 85 ann-i2c-ack
//! ch0 edge 72 rising
//! lane point 85 ann-i2c-nak error
//! ```
//!
//! An expectation line is `<target> <kind> <positions> <key> <arguments...> [error]` where the target
//! is `lane` or `chN` (an overlay on channel N); the kinds are `range <start> <end>`, `point <at>`
//! and `edge <at> <rising|falling>`; the text is a message key and its arguments, or `=text` for a
//! literal text (the bit an arrow shows). SCL is channel 0 and SDA channel
//! 1 (see [`SCL`] and [`SDA`]).
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

    /// A start condition: SDA falls while SCL is high. Returns the position of the SDA edge.
    pub fn start(&mut self) -> u64 {
        let at = self.pos();
        self.raw(false, true, 4);
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
        self.raw(false, true, 4);
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
    let (positions, rising) = match kind.as_str() {
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
        _ => return Err("the kind is range, point or edge".into()),
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
            args.iter().map(|a| (*a).to_owned()).collect(),
        ),
        None => (String::new(), Vec::new()),
    };
    Ok(Expected {
        channel,
        kind,
        positions,
        rising,
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
    Expected {
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

/// The lane parts of a byte: its range and its acknowledge point.
pub fn byte_lane(at: At, key: &str, text: &str, ack: &str, ack_error: bool) -> Vec<Expected> {
    let mut point = lane_point(at.ack, ack);
    point.error = ack_error;
    vec![lane_range(at.first, at.ack + 1, key, &[text]), point]
}

// Whole transactions with their expected decodes (default options).

pub fn hex(value: u8) -> String {
    format!("0x{value:02X}")
}

/// A write of `data` to `address` (7 bits), everything acknowledged, and what decoding it gives.
pub fn write_tx(bus: &mut Bus, address: u8, data: &[u8]) -> Vec<Expected> {
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(lane_point(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let a = bus.byte(address << 1, true);
    expected.extend(byte_lane(
        a,
        "ann-i2c-address-write",
        &hex(address),
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(a, address << 1, "ann-i2c-ack"));
    for value in data {
        let at = bus.byte(*value, true);
        expected.extend(byte_lane(
            at,
            "ann-i2c-data",
            &hex(*value),
            "ann-i2c-ack",
            false,
        ));
        expected.extend(byte_arrows(at, *value, "ann-i2c-ack"));
    }
    let stop = bus.stop();
    expected.push(lane_point(stop, "ann-i2c-stop"));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    expected.push(lane_range(
        start,
        stop + 1,
        "ann-i2c-transaction-write",
        &[&hex(address)],
    ));
    expected
}

/// A read of `data` from `address`: the controller acknowledges every byte but the last.
pub fn read_tx(bus: &mut Bus, address: u8, data: &[u8]) -> Vec<Expected> {
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(lane_point(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let a = bus.byte(address << 1 | 1, true);
    expected.extend(byte_lane(
        a,
        "ann-i2c-address-read",
        &hex(address),
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(a, address << 1 | 1, "ann-i2c-ack"));
    for (index, value) in data.iter().enumerate() {
        let last = index + 1 == data.len();
        let at = bus.byte(*value, !last);
        let ack = if last { "ann-i2c-nak" } else { "ann-i2c-ack" };
        expected.extend(byte_lane(at, "ann-i2c-data", &hex(*value), ack, false));
        expected.extend(byte_arrows(at, *value, ack));
    }
    let stop = bus.stop();
    expected.push(lane_point(stop, "ann-i2c-stop"));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    expected.push(lane_range(
        start,
        stop + 1,
        "ann-i2c-transaction-read",
        &[&hex(address)],
    ));
    expected
}
