//! The sample decoder "Clocked bits" (research R13): bits sampled on the edges of a clock, grouped
//! into bytes.
//!
//! On every chosen clock edge the level of the data channel is sampled and added to the byte in
//! progress. A byte is complete after `bits` bits. It ends early, as an **incomplete byte** error,
//! when the next clock edge comes `idle` or more samples after the last one, when a gap in the
//! capture begins, and when the pass ends. Output:
//!
//! * in the lane: a point for every bit, a range for every byte (its value in hexadecimal), an
//!   error range for an incomplete byte and an error range for a gap;
//! * on the clock channel: an edge arrow at every sampled edge;
//! * on the data channel: a dot at the first sampled edge of every byte;
//! * for stacking: one `byte` item for every complete byte.

use la_core::Error;

use crate::descriptor::{Descriptor, Input, OptionKind, OptionSpec, Signal};
use crate::input::{Chunk, Level, SignalReader};
use crate::model::{Annotation, Item, Kind, Payload, Shape, Style, Target, Text};
use crate::run::{DecodeError, End, Run, Sink};
use crate::setup::{Setup, Value};

/// The decoder id.
pub const ID: &str = "sample.clocked_bits";
/// The kind of the items it produces.
pub const BYTE_ITEM: &str = "byte";

const MSG_BYTE: &str = "ann-sample-clocked-bits-byte";
const MSG_BIT: &str = "ann-sample-clocked-bits-bit";
const MSG_INCOMPLETE: &str = "ann-sample-clocked-bits-incomplete";
const MSG_GAP: &str = "ann-sample-clocked-bits-gap";

/// The colour of bytes, bits and errors in the lane (indexes of the palette). The marks on the
/// signals (the edge arrows and the data dots) name no colour: they take the one of their signal.
const COLOUR_BYTE: u8 = 0;
const COLOUR_BIT: u8 = 1;
const COLOUR_ERROR: u8 = 4;

static SIGNALS: [Signal; 2] = [
    Signal {
        role: "clock",
        label: "decoder-sample-clocked-bits-signal-clock",
        required: true,
        may_share: false,
    },
    Signal {
        role: "data",
        label: "decoder-sample-clocked-bits-signal-data",
        required: true,
        may_share: false,
    },
];

static OPTIONS: [OptionSpec; 4] = [
    OptionSpec {
        id: "clock_edge",
        label: "decoder-sample-clocked-bits-option-clock_edge",
        kind: OptionKind::Choice {
            choices: &[
                (
                    "rising",
                    "decoder-sample-clocked-bits-choice-clock_edge-rising",
                ),
                (
                    "falling",
                    "decoder-sample-clocked-bits-choice-clock_edge-falling",
                ),
            ],
            default: "rising",
        },
    },
    OptionSpec {
        id: "bit_order",
        label: "decoder-sample-clocked-bits-option-bit_order",
        kind: OptionKind::Choice {
            choices: &[
                ("msb", "decoder-sample-clocked-bits-choice-bit_order-msb"),
                ("lsb", "decoder-sample-clocked-bits-choice-bit_order-lsb"),
            ],
            default: "msb",
        },
    },
    OptionSpec {
        id: "bits",
        label: "decoder-sample-clocked-bits-option-bits",
        kind: OptionKind::Integer {
            min: 4,
            max: 16,
            default: 8,
        },
    },
    OptionSpec {
        id: "idle",
        label: "decoder-sample-clocked-bits-option-idle",
        kind: OptionKind::Integer {
            min: 1,
            max: 1_000_000,
            default: 100,
        },
    },
];

/// The descriptor of the decoder.
pub static DESCRIPTOR: Descriptor = Descriptor {
    id: ID,
    version: 1,
    name: "decoder-sample-clocked-bits-name",
    description: "decoder-sample-clocked-bits-description",
    signals: &SIGNALS,
    options: &OPTIONS,
    input: Input::Signals,
    outputs: &[BYTE_ITEM],
    generator: true,
    lane_rows: 3,
};

/// The state of one pass.
struct ClockedBits {
    clock_channel: u8,
    data_channel: u8,
    rising: bool,
    msb_first: bool,
    bits: usize,
    idle: u64,
    /// The bits of the byte in progress, in the order they were sampled.
    sampled: Vec<bool>,
    /// The position of the first and of the last sampled edge of the byte in progress.
    first: u64,
    last: u64,
    /// Gaps that begin before this position have been dealt with.
    gap_done: u64,
}

/// Creates the state of a pass from a validated setup.
///
/// # Errors
///
/// `decoder.invalid_setup` if the setup lacks a channel or an option has an unexpected value (it
/// does not after validation).
pub fn create(setup: &Setup) -> Result<Box<dyn Run>, Error> {
    let channel = |role: &str| {
        setup
            .channels
            .get(role)
            .copied()
            .ok_or_else(|| crate::error::invalid_setup(format!("channels.{role}: missing")))
    };
    let choice = |id: &str| match setup.option_value(&DESCRIPTOR, id) {
        Some(Value::Choice(value)) => Ok(value),
        _ => Err(crate::error::invalid_setup(format!(
            "options.{id}: invalid"
        ))),
    };
    let number = |id: &str| match setup.option_value(&DESCRIPTOR, id) {
        Some(Value::Integer(value)) => Ok(value),
        _ => Err(crate::error::invalid_setup(format!(
            "options.{id}: invalid"
        ))),
    };
    Ok(Box::new(ClockedBits {
        clock_channel: channel("clock")?,
        data_channel: channel("data")?,
        rising: choice("clock_edge")? == "rising",
        msb_first: choice("bit_order")? == "msb",
        bits: usize::try_from(number("bits")?).unwrap_or(8),
        idle: u64::try_from(number("idle")?).unwrap_or(100),
        sampled: Vec::new(),
        first: 0,
        last: 0,
        gap_done: 0,
    }))
}

fn style(colour: u8, shape: Shape) -> Style {
    Style {
        colour: Some(colour),
        shape,
    }
}

/// A style that leaves the colour to the signal the mark is on.
fn signal_style(shape: Shape) -> Style {
    Style {
        colour: None,
        shape,
    }
}

fn message(key: &'static str, args: Vec<String>) -> Text {
    Text::Message { key, args }
}

impl ClockedBits {
    fn value(&self) -> u64 {
        let mut value = 0_u64;
        for (index, bit) in self.sampled.iter().enumerate() {
            let bit = u64::from(*bit);
            if self.msb_first {
                value = (value << 1) | bit;
            } else {
                value |= bit << index;
            }
        }
        value
    }

    /// The byte in progress ends early: an error range over what was sampled.
    fn flush_incomplete(&mut self, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.sampled.is_empty() {
            return Ok(());
        }
        let count = self.sampled.len();
        self.sampled.clear();
        sink.annotate(Annotation {
            kind: Kind::Range {
                start: self.first,
                end: self.last.saturating_add(1),
            },
            target: Target::Lane,
            style: style(COLOUR_ERROR, Shape::Bar),
            text: message(MSG_INCOMPLETE, vec![count.to_string()]),
            error: true,
        })?;
        Ok(())
    }

    /// Deals with the gaps that begin before `limit`.
    fn gaps_before(
        &mut self,
        chunk: &Chunk,
        limit: u64,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        for gap in &chunk.gaps {
            if gap.start < self.gap_done || gap.start >= limit {
                continue;
            }
            self.flush_incomplete(sink)?;
            let end = gap
                .end()
                .unwrap_or(chunk.capture_length)
                .min(chunk.capture_length);
            sink.annotate(Annotation {
                kind: Kind::Range {
                    start: gap.start,
                    end,
                },
                target: Target::Lane,
                style: style(COLOUR_ERROR, Shape::Bar),
                text: message(MSG_GAP, Vec::new()),
                error: true,
            })?;
            self.gap_done = end;
        }
        Ok(())
    }

    fn sample(&mut self, position: u64, data: Level, sink: &mut Sink) -> Result<(), DecodeError> {
        let bit = data == Level::High;
        if self.sampled.is_empty() {
            self.first = position;
            sink.annotate(Annotation {
                kind: Kind::Point { at: position },
                target: Target::Channel(self.data_channel),
                style: signal_style(Shape::Dot),
                text: message(MSG_BIT, vec![u8::from(bit).to_string()]),
                error: false,
            })?;
        }
        self.last = position;
        self.sampled.push(bit);
        sink.annotate(Annotation {
            kind: Kind::Point { at: position },
            target: Target::Lane,
            style: style(COLOUR_BIT, Shape::Dot),
            text: message(MSG_BIT, vec![u8::from(bit).to_string()]),
            error: false,
        })?;
        sink.annotate(Annotation {
            kind: Kind::Edge {
                at: position,
                rising: self.rising,
            },
            target: Target::Channel(self.clock_channel),
            style: signal_style(Shape::Arrow),
            text: message(MSG_BIT, vec![u8::from(bit).to_string()]),
            error: false,
        })?;
        if self.sampled.len() == self.bits {
            let value = self.value();
            let width = self.bits.div_ceil(4);
            let (start, end) = (self.first, self.last.saturating_add(1));
            self.sampled.clear();
            sink.annotate(Annotation {
                kind: Kind::Range { start, end },
                target: Target::Lane,
                style: style(COLOUR_BYTE, Shape::Bar),
                text: message(MSG_BYTE, vec![format!("0x{value:0width$X}")]),
                error: false,
            })?;
            sink.emit(Item {
                kind: BYTE_ITEM,
                start,
                end,
                payload: Payload::Integer(value),
                error: false,
            })?;
        }
        Ok(())
    }
}

impl Run for ClockedBits {
    fn process(
        &mut self,
        chunk: &Chunk,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        for edge in reader.edges(chunk.start, chunk.end) {
            if edge.role != "clock" || edge.rising != self.rising {
                continue;
            }
            let position = edge.position;
            self.gaps_before(chunk, position, sink)?;
            // An edge next to a gap is not an edge: one side of it is unknown.
            if reader.level("clock", position.saturating_sub(1)) == Level::Missing
                || reader.level("clock", position) == Level::Missing
            {
                continue;
            }
            if !self.sampled.is_empty() && position.saturating_sub(self.last) >= self.idle {
                self.flush_incomplete(sink)?;
            }
            self.sample(position, reader.level("data", position), sink)?;
        }
        self.gaps_before(chunk, chunk.end, sink)
    }

    fn finish(
        &mut self,
        _end: End,
        _reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        self.flush_incomplete(sink)
    }
}
