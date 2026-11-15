//! Decoders that fail on purpose, for fault injection (feature `test-support`).
//!
//! These are fixtures. They are documented as such and never registered in a production build:
//! each one breaks the contract of `docs/decoder-api.md` in exactly one way, so that a test can show
//! that the framework contains it. They read the `clock` channel and call the chunks they see by
//! number (the first chunk is number 1).

#![allow(
    clippy::panic,
    reason = "the fixtures break the contract on purpose: this module exists to panic and to fail"
)]

use std::sync::atomic::{AtomicBool, Ordering};

use la_core::Error;

use crate::descriptor::{Descriptor, Input, Signal};
use crate::input::{Chunk, ItemChunk, SignalReader};
use crate::model::{Annotation, Item, Kind, Payload, Shape, Style, Target, Text};
use crate::registry::Registry;
use crate::run::{DecodeError, End, Run, Sink};
use crate::setup::Setup;

/// The chunk (counting from 1) at which a fixture misbehaves.
pub const FAULT_AT_CHUNK: u64 = 3;

/// Lets the threads of `never_returns` end (call it when a test is over, so that no thread of a
/// test keeps running).
static RELEASE: AtomicBool = AtomicBool::new(false);

/// Lets the stuck decoders of this process return.
pub fn release_stuck_decoders() {
    RELEASE.store(true, Ordering::Release);
}

/// Makes the stuck decoders stick again (for the next test).
pub fn hold_stuck_decoders() {
    RELEASE.store(false, Ordering::Release);
}

static SIGNALS: [Signal; 1] = [Signal {
    role: "clock",
    label: "decoder-sample-clocked-bits-signal-clock",
    required: true,
    may_share: false,
}];

const fn descriptor(id: &'static str) -> Descriptor {
    Descriptor {
        id,
        version: 1,
        name: "decoder-sample-clocked-bits-name",
        description: "decoder-sample-clocked-bits-description",
        signals: &SIGNALS,
        options: &[],
        input: Input::Signals,
        outputs: &[],
        generator: false,
    }
}

/// Produces one `byte` item per chunk (its payload is the number of the chunk): the lower decoder
/// of the stacking tests.
pub static LOWER_BYTES: Descriptor = Descriptor {
    outputs: &["byte"],
    ..descriptor("test.lower_bytes")
};
/// Reads `byte` items and annotates them in pairs, producing a `word` item for each pair: the upper
/// decoder of the stacking tests. A byte left over at the end is an error mark.
pub static UPPER_WORDS: Descriptor = Descriptor {
    signals: &[],
    input: Input::Items("byte"),
    outputs: &["word"],
    ..descriptor("test.upper_words")
};
/// Counts chunks and emits a range for each: the healthy neighbour of the faulty ones.
pub static HEALTHY: Descriptor = descriptor("test.healthy");
/// Healthy, but takes 20 ms for every chunk.
pub static SLOW: Descriptor = descriptor("test.slow");
/// Returns an error at chunk [`FAULT_AT_CHUNK`].
pub static RETURNS_ERROR: Descriptor = descriptor("test.returns_error");
/// Panics at chunk [`FAULT_AT_CHUNK`].
pub static PANICS: Descriptor = descriptor("test.panics");
/// Loops forever at chunk [`FAULT_AT_CHUNK`], ignoring the cancel flag.
pub static NEVER_RETURNS: Descriptor = descriptor("test.never_returns");
/// Emits long texts until the memory limit stops it.
pub static GROWS_MEMORY: Descriptor = descriptor("test.grows_memory");
/// Emits many points until the annotation limit stops it.
pub static TOO_MANY_ANNOTATIONS: Descriptor = descriptor("test.too_many_annotations");
/// Emits records that are not valid, among valid ones.
pub static INVALID_OUTPUT: Descriptor = descriptor("test.invalid_output");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind0 {
    Healthy,
    Lower,
    Upper,
    Slow,
    ReturnsError,
    Panics,
    NeverReturns,
    GrowsMemory,
    TooMany,
    Invalid,
}

struct Fixture {
    kind: Kind0,
    chunks: u64,
    channel: u8,
    pending: Option<Item>,
}

fn style() -> Style {
    Style {
        colour: 0,
        shape: Shape::Bar,
    }
}

fn annotation(kind: Kind, target: Target, text: Text) -> Annotation {
    Annotation {
        kind,
        target,
        style: style(),
        text,
        error: false,
    }
}

impl Run for Fixture {
    fn process(
        &mut self,
        chunk: &Chunk,
        _reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        self.chunks = self.chunks.saturating_add(1);
        let range = Kind::Range {
            start: chunk.start,
            end: chunk.end,
        };
        match self.kind {
            Kind0::Upper => {}
            Kind0::Lower => {
                sink.emit(Item {
                    kind: "byte",
                    start: chunk.start,
                    end: chunk.end,
                    payload: Payload::Integer(self.chunks),
                    error: false,
                })?;
            }
            Kind0::Healthy | Kind0::Slow => {
                if self.kind == Kind0::Slow {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                sink.annotate(annotation(range, Target::Lane, Text::Literal("ok".into())))?;
            }
            Kind0::ReturnsError | Kind0::Panics | Kind0::NeverReturns => {
                if self.chunks == FAULT_AT_CHUNK {
                    match self.kind {
                        Kind0::ReturnsError => return Err(DecodeError::new("planned error")),
                        Kind0::Panics => panic!("planned panic of a test decoder"),
                        _ => {
                            while !RELEASE.load(Ordering::Acquire) {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            return Ok(());
                        }
                    }
                }
                sink.annotate(annotation(
                    range,
                    Target::Lane,
                    Text::Literal("before".into()),
                ))?;
            }
            Kind0::GrowsMemory => loop {
                sink.annotate(annotation(
                    Kind::Point { at: chunk.start },
                    Target::Lane,
                    Text::Literal("x".repeat(200)),
                ))?;
            },
            Kind0::TooMany => loop {
                sink.annotate(annotation(
                    Kind::Point { at: chunk.start },
                    Target::Lane,
                    Text::Literal(String::new()),
                ))?;
            },
            Kind0::Invalid => {
                let beyond = chunk.capture_length.saturating_add(10);
                let bad = [
                    annotation(
                        Kind::Range {
                            start: chunk.end,
                            end: chunk.start,
                        },
                        Target::Lane,
                        Text::Literal("order".into()),
                    ),
                    annotation(
                        Kind::Point { at: beyond },
                        Target::Lane,
                        Text::Literal("position".into()),
                    ),
                    annotation(
                        Kind::Point { at: chunk.start },
                        Target::Channel(200),
                        Text::Literal("channel".into()),
                    ),
                    annotation(
                        Kind::Point { at: chunk.start },
                        Target::Lane,
                        Text::Literal("t".repeat(1_000)),
                    ),
                ];
                for a in bad {
                    sink.annotate(a)?;
                }
                // An item that goes backwards in `end`: the second one is refused.
                for end in [chunk.end, chunk.start] {
                    sink.emit(Item {
                        kind: "x",
                        start: chunk.start,
                        end,
                        payload: Payload::Integer(0),
                        error: false,
                    })?;
                }
                sink.annotate(annotation(
                    range,
                    Target::Lane,
                    Text::Literal("valid".into()),
                ))?;
            }
        }
        let _ = self.channel;
        Ok(())
    }

    fn process_items(&mut self, chunk: &ItemChunk, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.kind != Kind0::Upper {
            return Ok(());
        }
        for item in &chunk.items {
            match self.pending.take() {
                None => self.pending = Some(item.clone()),
                Some(first) => {
                    sink.annotate(annotation(
                        Kind::Range {
                            start: first.start,
                            end: item.end,
                        },
                        Target::Lane,
                        Text::Literal("pair".into()),
                    ))?;
                    sink.emit(Item {
                        kind: "word",
                        start: first.start,
                        end: item.end,
                        payload: Payload::Integer(0),
                        error: false,
                    })?;
                }
            }
        }
        Ok(())
    }

    fn finish(&mut self, _: End, _: &SignalReader, sink: &mut Sink) -> Result<(), DecodeError> {
        if let Some(first) = self.pending.take() {
            let mut mark = annotation(
                Kind::Point { at: first.start },
                Target::Lane,
                Text::Literal("odd".into()),
            );
            mark.error = true;
            sink.annotate(mark)?;
        }
        Ok(())
    }
}

macro_rules! create {
    ($name:ident, $kind:expr) => {
        fn $name(setup: &Setup) -> Result<Box<dyn Run>, Error> {
            Ok(Box::new(Fixture {
                kind: $kind,
                chunks: 0,
                channel: setup.channels.get("clock").copied().unwrap_or(0),
                pending: None,
            }))
        }
    };
}

create!(create_healthy, Kind0::Healthy);
create!(create_slow, Kind0::Slow);
create!(create_lower, Kind0::Lower);
create!(create_upper, Kind0::Upper);
create!(create_returns_error, Kind0::ReturnsError);
create!(create_panics, Kind0::Panics);
create!(create_never_returns, Kind0::NeverReturns);
create!(create_grows_memory, Kind0::GrowsMemory);
create!(create_too_many, Kind0::TooMany);
create!(create_invalid, Kind0::Invalid);

/// Every fixture with the function that creates its passes.
#[must_use]
pub fn fixtures() -> Vec<(&'static Descriptor, crate::registry::Create)> {
    vec![
        (&HEALTHY, create_healthy),
        (&SLOW, create_slow),
        (&LOWER_BYTES, create_lower),
        (&UPPER_WORDS, create_upper),
        (&RETURNS_ERROR, create_returns_error),
        (&PANICS, create_panics),
        (&NEVER_RETURNS, create_never_returns),
        (&GROWS_MEMORY, create_grows_memory),
        (&TOO_MANY_ANNOTATIONS, create_too_many),
        (&INVALID_OUTPUT, create_invalid),
    ]
}

/// A registry holding the fixtures (and nothing else).
#[must_use]
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    for (descriptor, create) in fixtures() {
        let _ = registry.register_decoder(descriptor, create);
    }
    registry
}

/// The setup of a fixture on channel 0 of the capture.
#[must_use]
pub fn setup(descriptor: &Descriptor, instance: u32) -> Setup {
    let mut setup = Setup::new(instance, descriptor.id);
    setup.channels.insert("clock".into(), 0);
    setup
}

/// The setup of an instance of a decoder that reads the items of instance `below`.
#[must_use]
pub fn setup_above(descriptor: &Descriptor, instance: u32, below: u32) -> Setup {
    let mut setup = Setup::new(instance, descriptor.id);
    setup.source = crate::setup::Source::Instance(below);
    setup
}
