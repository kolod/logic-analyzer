//! The I2C decoder (feature 008; `specs/008-i2c-decoder-generator`).
//!
//! It reads two channels, SCL and SDA, and produces, for every transaction on the bus: in its lane
//! the address with its direction, every data byte with its acknowledgement and the start, repeated
//! start and stop as events; on SDA a dot at every start, repeated start and stop; on SCL an arrow
//! at every clock edge at which a bit was read.
//!
//! Before it reads anything it finds the bus clock period from the signal (module [`period`]) and
//! synchronises on the bus: the first start condition counts only after both lines have been high
//! for one period. Until the period is known the decoder holds everything back (it reports through
//! `Run::final_through` that nothing is final yet), then it decodes from the start of the capture, a
//! bounded number of edges per call (research R2).

mod bus;
pub mod items;
pub mod period;
mod text;

use la_core::Error;

use self::bus::{Bus, WORK_EDGES};
use self::period::Period;
use self::text::{Display, Radix};
use crate::descriptor::{Descriptor, Input, OptionKind, OptionSpec, Signal};
use crate::input::{Chunk, SignalReader};
use crate::run::{DecodeError, End, Run, Sink};
use crate::setup::{Setup, Value};

/// The decoder id.
pub const ID: &str = "i2c";
/// The kind of the items it produces for a decoder stacked on it.
pub const ITEM: &str = "i2c";

static SIGNALS: [Signal; 2] = [
    Signal {
        role: "scl",
        label: "decoder-i2c-signal-scl",
        required: true,
        may_share: false,
    },
    Signal {
        role: "sda",
        label: "decoder-i2c-signal-sda",
        required: true,
        may_share: false,
    },
];

static OPTIONS: [OptionSpec; 2] = [
    OptionSpec {
        id: "address",
        label: "decoder-i2c-option-address",
        kind: OptionKind::Choice {
            choices: &[
                ("addr7", "decoder-i2c-choice-address-addr7"),
                ("addr8", "decoder-i2c-choice-address-addr8"),
            ],
            default: "addr7",
        },
    },
    OptionSpec {
        id: "radix",
        label: "decoder-i2c-option-radix",
        kind: OptionKind::Choice {
            choices: &[
                ("hex", "decoder-i2c-choice-radix-hex"),
                ("dec", "decoder-i2c-choice-radix-dec"),
                ("bin", "decoder-i2c-choice-radix-bin"),
            ],
            default: "hex",
        },
    },
];

/// The descriptor of the decoder.
pub static DESCRIPTOR: Descriptor = Descriptor {
    id: ID,
    version: 1,
    name: "decoder-i2c-name",
    description: "decoder-i2c-description",
    signals: &SIGNALS,
    options: &OPTIONS,
    input: Input::Signals,
    outputs: &[ITEM],
    generator: true,
    lane_rows: 2,
};

/// Finding the period: the part of the capture read so far, and the estimator.
struct Calibration {
    period: Period,
    /// Rising edges of SCL before this position have been given to the estimator.
    scan: u64,
}

impl Calibration {
    /// Gives the estimator the rising edges of SCL from where it stopped, up to the end of the
    /// snapshot but at most `budget` edges (of both lines) at a time. Returns the period once known.
    fn scan(&mut self, reader: &SignalReader, budget: usize) -> Option<u64> {
        if let Some(found) = self.period.found() {
            return Some(found);
        }
        let length = reader.length();
        if self.scan >= length {
            return None;
        }
        let gaps = reader.gaps_in(self.scan, length);
        let mut work = 0_usize;
        let mut last = self.scan;
        let mut exhausted = true;
        for edge in reader.edges(self.scan, length) {
            if work >= budget {
                exhausted = false;
                break;
            }
            work = work.saturating_add(1);
            last = edge.position;
            if edge.role != "scl" || !edge.rising {
                continue;
            }
            if let Some(previous) = self.period.previous() {
                if gap_between(&gaps, previous, edge.position) {
                    self.period.break_at_gap();
                }
            }
            if let Some(found) = self.period.edge(edge.position) {
                self.scan = edge.position.saturating_add(1);
                return Some(found);
            }
        }
        self.scan = if exhausted {
            length
        } else {
            last.saturating_add(1)
        };
        None
    }
}

/// Whether a gap lies between the positions `a` and `b`.
fn gap_between(gaps: &[la_storage::Gap], a: u64, b: u64) -> bool {
    let first = gaps.partition_point(|g| g.end().unwrap_or(u64::MAX) <= a);
    gaps.get(first).is_some_and(|g| g.start < b)
}

enum State {
    Calibrating(Box<Calibration>),
    Running(Box<Bus>),
}

/// The state of one pass.
struct I2c {
    scl_channel: u8,
    sda_channel: u8,
    display: Display,
    state: State,
}

/// Creates the state of a pass from a validated setup.
///
/// # Errors
///
/// `decoder.invalid_setup` if the setup lacks a channel (it does not after validation).
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
    let display = Display {
        address8: choice("address")? == "addr8",
        radix: match choice("radix")?.as_str() {
            "dec" => Radix::Dec,
            "bin" => Radix::Bin,
            _ => Radix::Hex,
        },
    };
    Ok(Box::new(I2c {
        scl_channel: channel("scl")?,
        sda_channel: channel("sda")?,
        display,
        state: State::Calibrating(Box::new(Calibration {
            period: Period::new(),
            scan: 0,
        })),
    }))
}

impl I2c {
    /// Moves to the running state if the period is known.
    fn calibrate(&mut self, reader: &SignalReader, budget: usize) {
        if let State::Calibrating(calibration) = &mut self.state {
            if let Some(period) = calibration.scan(reader, budget) {
                self.state = State::Running(Box::new(Bus::new(
                    self.scl_channel,
                    self.sda_channel,
                    period,
                    self.display,
                )));
            }
        }
    }
}

impl Run for I2c {
    fn process(
        &mut self,
        chunk: &Chunk,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        self.calibrate(reader, WORK_EDGES);
        if let State::Running(bus) = &mut self.state {
            bus.advance(reader, chunk.end, sink)?;
        }
        Ok(())
    }

    fn final_through(&self) -> Option<u64> {
        Some(match &self.state {
            State::Calibrating(_) => 0,
            State::Running(bus) => bus.cursor(),
        })
    }

    fn finish(
        &mut self,
        end: End,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        if end == End::Complete {
            // Everything is there: finish finding the period and drain what was held back.
            self.calibrate(reader, usize::MAX);
            if let State::Running(bus) = &mut self.state {
                while bus.cursor() < reader.length() {
                    bus.advance(reader, reader.length(), sink)?;
                }
            }
        }
        // What is in progress is shown as it is: at the end of the capture, or where a cut pass
        // had got to.
        if let State::Running(bus) = &mut self.state {
            let at = if end == End::Complete {
                reader.length()
            } else {
                bus.cursor()
            };
            bus.flush(at, sink)?;
        }
        Ok(())
    }
}
