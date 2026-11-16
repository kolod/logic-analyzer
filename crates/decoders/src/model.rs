//! The annotation model: what a decoder says about a capture, independent of any drawing
//! technology (constitution Principle IX, `data-model.md`).
//!
//! An [`Annotation`] is what is drawn: a range, a point or an edge marker, placed in the lane of the
//! decoder or as an overlay on a channel, with a style and a text. An [`Item`] is what an upper
//! decoder reads when decoders are stacked. Everything a decoder emits is validated against the
//! capture before it is stored ([`Annotation::validate`], [`Item::validate`]); a refused record is
//! counted and never stored.

use crate::text::sanitize;

/// The number of colours in the fixed palette an annotation can choose from.
pub const COLOURS: u8 = 8;
/// The longest literal text of an annotation, in bytes of UTF-8.
pub const MAX_TEXT_BYTES: usize = 256;
/// The most arguments a message annotation can carry.
pub const MAX_ARGS: usize = 4;
/// The longest argument of a message annotation, in bytes.
pub const MAX_ARG_BYTES: usize = 64;
/// The largest payload of an output item, in bytes.
pub const MAX_PAYLOAD_BYTES: usize = 64;

/// The shape an annotation is drawn with (a range ignores it and is drawn as a bar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A filled dot.
    Dot,
    /// An arrow (the natural shape of an edge marker).
    Arrow,
    /// A bar.
    Bar,
    /// A flag.
    Flag,
    /// A cross.
    Cross,
}

/// How an annotation is drawn: an optional index into the fixed colour palette and a shape.
///
/// A decoder that names no colour (`None`) leaves it to the interface, which draws the mark in the
/// colour of the signal it is on, so that a decoder need not know about the colours of the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Index into the palette, below [`COLOURS`], or `None` for the colour of the signal.
    pub colour: Option<u8>,
    /// The shape.
    pub shape: Shape,
}

/// Where in time an annotation is, and what kind it is. Positions are sample positions of the
/// capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A span of positions, half open: `start` is in it, `end` is not. A zero-length range
    /// (`start == end`) is accepted and drawn as a point.
    Range {
        /// First position.
        start: u64,
        /// One past the last position.
        end: u64,
    },
    /// One position.
    Point {
        /// The position.
        at: u64,
    },
    /// One position of an edge of a signal, with its direction.
    Edge {
        /// The position of the first sample with the new level.
        at: u64,
        /// `true` for a rising edge.
        rising: bool,
    },
}

/// Where an annotation is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The lane of the decoder.
    Lane,
    /// An overlay on this channel of the capture.
    Channel(u8),
}

/// The text of an annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    /// A localizable message of the resources of the application, used by built-in decoders so that
    /// it follows the language of the user; the key is a string literal of the source of the decoder.
    Message {
        /// The message key.
        key: &'static str,
        /// The arguments (numbers and names already formatted as text).
        args: Vec<String>,
    },
    /// Literal text from outside the application: shown as it is, sanitized, never translated.
    Literal(String),
}

impl Text {
    /// The same text with control, bidirectional and invisible characters removed from the
    /// literal or from every argument.
    #[must_use]
    pub fn sanitized(self) -> Self {
        match self {
            Text::Literal(text) => Text::Literal(sanitize(&text)),
            Text::Message { key, args } => Text::Message {
                key,
                args: args.iter().map(|a| sanitize(a)).collect(),
            },
        }
    }
}

/// One result of a decoder, as it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// What and where in time.
    pub kind: Kind,
    /// The lane or a channel.
    pub target: Target,
    /// Colour and shape.
    pub style: Style,
    /// The text.
    pub text: Text,
    /// Whether it reports a problem in the signal (drawn distinctly and counted).
    pub error: bool,
}

/// What an annotation or an item is checked against: the capture it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The number of positions of the capture (of the snapshot the chunk was cut from).
    pub length: u64,
    /// The number of channels of the capture.
    pub channels: u8,
}

/// Why a record was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A position lies outside the capture.
    Position,
    /// A range ends before it starts.
    Order,
    /// An overlay targets a channel the capture does not have.
    Target,
    /// The colour is outside the palette.
    Style,
    /// The literal text is too long.
    TextLength,
    /// A message has too many arguments.
    Arguments,
    /// A message argument is too long.
    ArgumentLength,
    /// An item payload is too large.
    Payload,
    /// An item ends before the item that came before it.
    ItemOrder,
}

impl Refusal {
    /// A short, stable name for the log (the log never contains the text of an annotation).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Refusal::Position => "position",
            Refusal::Order => "order",
            Refusal::Target => "target",
            Refusal::Style => "style",
            Refusal::TextLength => "text_length",
            Refusal::Arguments => "arguments",
            Refusal::ArgumentLength => "argument_length",
            Refusal::Payload => "payload",
            Refusal::ItemOrder => "item_order",
        }
    }
}

impl Annotation {
    /// The first position and the end of the annotation (`(at, at)` for a point or an edge).
    #[must_use]
    pub fn span(&self) -> (u64, u64) {
        match self.kind {
            Kind::Range { start, end } => (start, end),
            Kind::Point { at } | Kind::Edge { at, .. } => (at, at),
        }
    }

    /// The same annotation with its text sanitized.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.text = self.text.sanitized();
        self
    }

    /// Checks the annotation against the capture.
    ///
    /// # Errors
    ///
    /// The first rule that is broken: positions inside the capture, a range not ending before it
    /// starts, an existing target channel, a colour of the palette, and the limits of the text.
    pub fn validate(&self, bounds: &Bounds) -> Result<(), Refusal> {
        match self.kind {
            Kind::Range { start, end } => {
                if end < start {
                    return Err(Refusal::Order);
                }
                if end > bounds.length {
                    return Err(Refusal::Position);
                }
            }
            Kind::Point { at } | Kind::Edge { at, .. } => {
                if at >= bounds.length {
                    return Err(Refusal::Position);
                }
            }
        }
        if let Target::Channel(channel) = self.target {
            if channel >= bounds.channels {
                return Err(Refusal::Target);
            }
        }
        if self.style.colour.is_some_and(|c| c >= COLOURS) {
            return Err(Refusal::Style);
        }
        match &self.text {
            Text::Literal(text) => {
                if text.len() > MAX_TEXT_BYTES {
                    return Err(Refusal::TextLength);
                }
            }
            Text::Message { args, .. } => {
                if args.len() > MAX_ARGS {
                    return Err(Refusal::Arguments);
                }
                if args.iter().any(|a| a.len() > MAX_ARG_BYTES) {
                    return Err(Refusal::ArgumentLength);
                }
            }
        }
        Ok(())
    }
}

/// The data of an output item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// An integer.
    Integer(u64),
    /// A short byte string.
    Bytes(Vec<u8>),
    /// A short text.
    Text(String),
}

/// One result of a decoder as data for the decoder above it in a stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The kind, an identifier the producing decoder declared.
    pub kind: &'static str,
    /// First position of the span.
    pub start: u64,
    /// One past the last position of the span.
    pub end: u64,
    /// The data.
    pub payload: Payload,
    /// Whether the item stands for an erroneous event.
    pub error: bool,
}

impl Item {
    /// Checks the item against the capture and against the end of the item before it.
    ///
    /// # Errors
    ///
    /// The span must be ordered and inside the capture, its end must not be before
    /// `previous_end` (which gives an upper decoder its watermark), and the payload must be small.
    pub fn validate(&self, bounds: &Bounds, previous_end: u64) -> Result<(), Refusal> {
        if self.end < self.start {
            return Err(Refusal::Order);
        }
        if self.end > bounds.length {
            return Err(Refusal::Position);
        }
        if self.end < previous_end {
            return Err(Refusal::ItemOrder);
        }
        let size = match &self.payload {
            Payload::Integer(_) => 0,
            Payload::Bytes(bytes) => bytes.len(),
            Payload::Text(text) => text.len(),
        };
        if size > MAX_PAYLOAD_BYTES {
            return Err(Refusal::Payload);
        }
        Ok(())
    }
}
