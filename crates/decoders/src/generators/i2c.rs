//! The generator of the I2C decoder (`specs/008-i2c-decoder-generator/contracts/i2c-generator.md`).
//!
//! A pure function of the position, so any way of asking for the samples gives the same signal.
//! Time is divided into **frames** of one fixed length; frame `k` holds one transaction whose content
//! is drawn from the seed and `k`. A frame is
//!
//! ```text
//! lead-in (64 samples, both lines high) | start (4) | bytes of 9 slots of 16 samples each
//!   [ | repeated start (12) | bytes ] | stop (8) | idle
//! ```
//!
//! In a bit slot SCL is low for samples 0..4, high for 4..12 and low for 12..16 and SDA holds the bit
//! for all 16, so the data is stable around the rising edge of SCL, which is where the decoder reads.
//!
//! With `errors` on, about one frame in `error_rate` carries a planted error: the address is not
//! acknowledged, a written byte is not acknowledged, or the transaction is cut short by a stop in the
//! middle of a byte. With `startup` on, a start-up precedes frame 0: both lines low, then noise whose
//! highs are shorter than a bus period, which the decoder must ignore without a trace.

// Everything here is arithmetic on values clamped when the parameters are made (at most 9 bytes of 9
// slots of 16 samples in a frame) and on positions that are saturated or divided by a constant
// frame length greater than zero; `sample` runs for every sample of a capture, so the plain
// operators are kept.
#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use std::sync::Arc;

use la_devices::SignalSource;

use super::clocked_bits::{instance_seed, mix};
use super::composite::Claim;
use crate::decoders::i2c::items::{self, Event};
use crate::decoders::i2c::period::Period;
use crate::model::Item;
use crate::setup::Setup;

/// Samples in one SCL period.
pub const SAMPLES_PER_BIT: u64 = 16;
/// Samples with both lines high at the beginning of a frame (more than any bus period found).
pub const LEAD_IN: u64 = 64;
/// Samples of idle bus after the stop of a frame.
pub const TAIL: u64 = 32;
/// The most data bytes in a transaction.
pub const MAX_BYTES_LIMIT: u64 = 6;
/// The generator option: the most data bytes in a transaction.
pub const OPTION_MAX_BYTES: &str = "max_bytes";
/// The generator option: 1 plants errors (the key the application edits).
pub const OPTION_ERRORS: &str = "errors";
/// The generator option: about one transaction in this many has an error (the key the application
/// edits).
pub const OPTION_ERROR_RATE: &str = "error_rate";
/// The generator option: 1 puts a start-up before the first frame.
pub const OPTION_STARTUP: &str = "startup";
/// The rate of errors when none is given.
pub const DEFAULT_ERROR_RATE: i64 = 8;

const SLOT: u64 = SAMPLES_PER_BIT;
const BYTE: u64 = 9 * SLOT;
const START: u64 = 4;
const RESTART: u64 = 12;
const STOP: u64 = 8;

/// An error the generator planted, with where the decoder must report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantedError {
    /// The address was not acknowledged; `at` is the rising edge of SCL of the acknowledge bit.
    AddressNak {
        /// The position of the acknowledge bit.
        at: u64,
    },
    /// A written byte was not acknowledged; `at` as above.
    DataNak {
        /// The position of the acknowledge bit.
        at: u64,
    },
    /// A byte was cut short by a stop: from its first bit to the stop.
    ByteCut {
        /// The rising edge of SCL of the first bit that was read.
        start: u64,
        /// The position of the stop.
        end: u64,
    },
}

impl PlantedError {
    /// Where the error begins, for sorting.
    #[must_use]
    pub fn position(&self) -> u64 {
        match self {
            Self::AddressNak { at } | Self::DataNak { at } => *at,
            Self::ByteCut { start, .. } => *start,
        }
    }
}

/// The parameters of a generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    scl: u8,
    sda: u8,
    max_bytes: u64,
    errors: bool,
    error_rate: u64,
    startup: bool,
    seed: u64,
}

/// One byte of a transaction as it is put on the bus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Byte {
    /// The eight bits as sent (an address byte includes the direction bit).
    value: u8,
    /// The ninth bit is high (not acknowledged).
    nak: bool,
    /// An address byte.
    address: bool,
    /// The direction of the part the byte belongs to.
    read: bool,
}

/// Which error a frame carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Planted {
    None,
    AddressNak,
    DataNak,
    ByteCut,
}

/// The content of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    bytes: [Byte; 12],
    /// The bytes that are sent whole.
    count: usize,
    /// A repeated start comes before byte `restart_before`.
    restart: bool,
    restart_before: usize,
    /// Bits of byte `count` that are sent before the stop cuts it short (0: none).
    cut_bits: u64,
    planted: Planted,
}

impl Params {
    /// The parameters for a setup and a capture seed; `None` unless both signals are assigned.
    /// Values out of range are clamped, never an error.
    #[must_use]
    pub fn from_setup(setup: &Setup, seed: u64) -> Option<Self> {
        let scl = *setup.channels.get("scl")?;
        let sda = *setup.channels.get("sda")?;
        let option = |key: &str, default: i64| setup.generator.get(key).copied().unwrap_or(default);
        Some(Self {
            scl,
            sda,
            max_bytes: option(OPTION_MAX_BYTES, 4).clamp(1, MAX_BYTES_LIMIT as i64) as u64,
            errors: option(OPTION_ERRORS, 0) != 0,
            error_rate: option(OPTION_ERROR_RATE, DEFAULT_ERROR_RATE).clamp(2, i64::MAX) as u64,
            startup: option(OPTION_STARTUP, 0) != 0,
            seed: instance_seed(seed, setup.instance),
        })
    }

    /// The length of a frame in samples (it does not depend on the content).
    #[must_use]
    pub fn frame_length(&self) -> u64 {
        let bytes = self.max_bytes + 2;
        LEAD_IN + START + bytes * BYTE + RESTART + STOP + TAIL
    }

    /// The length of the start-up before frame 0: 0 without one, else a multiple of 16 from 32 to
    /// 128 drawn from the seed.
    #[must_use]
    pub fn startup_length(&self) -> u64 {
        if self.startup {
            SLOT * (2 + mix(self.seed ^ 0x57) % 7)
        } else {
            0
        }
    }

    fn frame(&self, k: u64) -> Frame {
        let h = mix(self.seed ^ k.wrapping_mul(0xD6E8_FEB8_6659_FD93));
        let draw = |n: u64| mix(h ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let address = 0x08 + (draw(1) % 0x68) as u8;
        let read = draw(2) & 1 != 0;
        let split = self.max_bytes >= 2 && draw(3) % 4 == 0;
        let mut frame = Frame {
            bytes: [Byte::default(); 12],
            count: 0,
            restart: false,
            restart_before: 0,
            cut_bits: 0,
            planted: Planted::None,
        };
        let push = |frame: &mut Frame, byte: Byte| {
            frame.bytes[frame.count] = byte;
            frame.count += 1;
        };
        let data = |index: u64| (draw(0x100 + index) & 0xFF) as u8;
        let byte = |value: u8, nak: bool, address: bool, read: bool| Byte {
            value,
            nak,
            address,
            read,
        };
        if split {
            // A write of `first` bytes, a repeated start and a read of the rest.
            let total = 2 + draw(4) % (self.max_bytes - 1);
            let first = 1 + draw(5) % (total - 1);
            push(&mut frame, byte(address << 1, false, true, false));
            for i in 0..first {
                push(&mut frame, byte(data(i), false, false, false));
            }
            frame.restart = true;
            frame.restart_before = frame.count;
            push(&mut frame, byte(address << 1 | 1, false, true, true));
            let second = total - first;
            for i in 0..second {
                push(
                    &mut frame,
                    byte(data(first + i), i + 1 == second, false, true),
                );
            }
        } else {
            let total = 1 + draw(4) % self.max_bytes;
            push(
                &mut frame,
                byte(address << 1 | u8::from(read), false, true, read),
            );
            for i in 0..total {
                push(
                    &mut frame,
                    byte(data(i), read && i + 1 == total, false, read),
                );
            }
            frame.restart_before = frame.count;
        }
        if self.errors && draw(7) % self.error_rate == 0 {
            self.plant(&mut frame, &draw);
        }
        frame
    }

    /// Puts an error into a frame: the transaction ends early.
    fn plant(&self, frame: &mut Frame, draw: &dyn Fn(u64) -> u64) {
        // A data byte written to the target, in the first part, that can go unacknowledged.
        let writable: Vec<usize> = (1..frame.restart_before.min(frame.count))
            .filter(|i| !frame.bytes[*i].read && !frame.bytes[*i].address)
            .collect();
        let mut kind = draw(8) % 3;
        if kind == 1 && (writable.is_empty() || frame.bytes[0].read) {
            kind = 0;
        }
        match kind {
            0 => {
                frame.count = 1;
                frame.restart = false;
                frame.restart_before = 1;
                frame.bytes[0].nak = true;
                frame.planted = Planted::AddressNak;
            }
            1 => {
                let index = writable[(draw(9) % writable.len() as u64) as usize];
                frame.count = index + 1;
                frame.restart = false;
                frame.restart_before = frame.count;
                frame.bytes[index].nak = true;
                frame.planted = Planted::DataNak;
            }
            _ => {
                let index = (draw(9) % frame.count as u64) as usize;
                frame.cut_bits = 1 + draw(10) % 8;
                frame.restart = frame.restart && index >= frame.restart_before;
                frame.count = index;
                frame.restart_before = frame.restart_before.min(index);
                frame.planted = Planted::ByteCut;
            }
        }
    }

    /// The `(sda, scl)` levels at offset `r` of a frame.
    fn levels(frame: &Frame, r: u64) -> (bool, bool) {
        if r < LEAD_IN {
            return (true, true);
        }
        let mut t = r - LEAD_IN;
        if t < START {
            return (false, true);
        }
        t -= START;
        let cut = frame.cut_bits * SLOT;
        let (first, second) = if frame.restart {
            (
                frame.restart_before as u64 * BYTE,
                (frame.count - frame.restart_before) as u64 * BYTE + cut,
            )
        } else {
            (frame.count as u64 * BYTE + cut, 0)
        };
        let body = |t: u64, from: usize| {
            let slot = t / SLOT;
            let byte = from + (slot / 9) as usize;
            let bit = slot % 9;
            let off = t % SLOT;
            let b = frame.bytes[byte];
            let sda = if bit < 8 {
                b.value >> (7 - bit) & 1 != 0
            } else {
                b.nak
            };
            (sda, (4..12).contains(&off))
        };
        if t < first {
            return body(t, 0);
        }
        t -= first;
        if frame.restart {
            if t < RESTART {
                return match t {
                    0..=3 => (true, false),
                    4..=7 => (true, true),
                    _ => (false, true),
                };
            }
            t -= RESTART;
            if t < second {
                return body(t, frame.restart_before);
            }
            t -= second;
        }
        match t {
            0..=3 => (false, false),
            4..=7 => (false, true),
            _ => (true, true),
        }
    }

    /// The levels at a position of the start-up (which is `length` samples long).
    fn startup_levels(position: u64, length: u64) -> (bool, bool) {
        let half = length / 2;
        if position < half {
            return (false, false);
        }
        // Eight samples a round: both high for two, SDA falls while SCL is still high, both low.
        match (position - half) % 8 {
            0 | 1 => (true, true),
            2 => (false, true),
            _ => (false, false),
        }
    }

    /// The sample word at a position.
    fn sample(&self, position: u64, startup: u64, cache: &mut Option<(u64, Frame)>) -> u32 {
        let (sda, scl) = if position < startup {
            Self::startup_levels(position, startup)
        } else {
            let position = position - startup;
            let length = self.frame_length();
            let (k, r) = (position / length, position % length);
            let frame = match cache {
                Some((cached, frame)) if *cached == k => *frame,
                _ => {
                    let frame = self.frame(k);
                    *cache = Some((k, frame));
                    frame
                }
            };
            Self::levels(&frame, r)
        };
        (u32::from(scl) << self.scl) | (u32::from(sda) << self.sda)
    }

    /// The offset in a frame of the first sample of a byte's first bit slot.
    fn slot_start(frame: &Frame, byte: usize) -> u64 {
        let body = if frame.restart && byte >= frame.restart_before {
            frame.restart_before as u64 * BYTE
                + RESTART
                + (byte - frame.restart_before) as u64 * BYTE
        } else {
            byte as u64 * BYTE
        };
        LEAD_IN + START + body
    }

    /// The offset in a frame at which the body (everything between the start and the stop) ends.
    fn body_end(frame: &Frame) -> u64 {
        let cut = frame.cut_bits * SLOT;
        let restart = if frame.restart { RESTART } else { 0 };
        LEAD_IN + START + frame.count as u64 * BYTE + restart + cut
    }
}

/// The signal source of one I2C generator.
pub struct I2cSource {
    params: Params,
    startup: u64,
    cache: Option<(u64, Frame)>,
}

impl SignalSource for I2cSource {
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        let mut position = start;
        for word in out.iter_mut() {
            *word = self.params.sample(position, self.startup, &mut self.cache);
            position = position.saturating_add(1);
        }
    }
}

/// The source for a setup and a capture seed, or `None` if a signal is not assigned.
#[must_use]
pub fn source(setup: &Setup, seed: u64) -> Option<I2cSource> {
    Params::from_setup(setup, seed).map(|params| I2cSource {
        startup: params.startup_length(),
        params,
        cache: None,
    })
}

/// The channels the generator of a setup fills (a bit mask).
#[must_use]
pub fn claimed_channels(setup: &Setup) -> u32 {
    ["scl", "sda"]
        .iter()
        .filter_map(|role| setup.channels.get(*role))
        .filter_map(|c| 1_u32.checked_shl(u32::from(*c)))
        .fold(0, |mask, bit| mask | bit)
}

/// The claim of the generator of a setup for the virtual device.
#[must_use]
pub fn claim(setup: &Setup) -> Option<Claim> {
    Params::from_setup(setup, 0)?;
    let setup = setup.clone();
    Some(Claim {
        channels: claimed_channels(&setup),
        make: Arc::new(move |seed| {
            source(&setup, seed).map(|s| Box::new(s) as Box<dyn SignalSource>)
        }),
    })
}

/// Everything a correct decode of the first `length` samples must show.
struct Analysis {
    items: Vec<Item>,
    errors: Vec<PlantedError>,
    /// Whether the bus period is found from the clock pulses (without it nothing is decoded).
    calibrated: bool,
}

fn analyse(params: &Params, length: u64) -> Analysis {
    let mut analysis = Analysis {
        items: Vec::new(),
        errors: Vec::new(),
        calibrated: false,
    };
    let mut period = Period::new();
    let mut rise = |position: u64, analysis: &mut Analysis| {
        if position < length && period.edge(position).is_some() {
            analysis.calibrated = true;
        }
    };
    let startup = params.startup_length();
    if startup > 0 {
        let half = startup / 2;
        for position in (half..startup).step_by(8) {
            rise(position, &mut analysis);
        }
        // The lead-in of the first frame raises both lines.
        rise(startup, &mut analysis);
    }
    let frame_length = params.frame_length();
    let mut k = 0_u64;
    while startup + k * frame_length < length {
        let base = startup + k * frame_length;
        let frame = params.frame(k);
        let mut direction = items::UNKNOWN;
        let start_edge = base + LEAD_IN;
        if start_edge < length {
            analysis.items.push(
                Event::condition(items::START, items::UNKNOWN).item(start_edge, start_edge + 1),
            );
        }
        for index in 0..=frame.count {
            if frame.restart && index == frame.restart_before {
                let setup = base + Params::slot_start(&frame, index) - RESTART + 4;
                rise(setup, &mut analysis);
                let at = setup + 4;
                if at < length {
                    analysis.items.push(
                        Event::condition(items::REPEATED_START, items::UNKNOWN).item(at, at + 1),
                    );
                }
            }
            let first = base + Params::slot_start(&frame, index) + 4;
            if index == frame.count {
                // The byte that is cut short, if there is one: its pulses and nothing else.
                for bit in 0..frame.cut_bits {
                    rise(first + bit * SLOT, &mut analysis);
                }
                continue;
            }
            let byte = frame.bytes[index];
            for bit in 0..9 {
                rise(first + bit * SLOT, &mut analysis);
            }
            let ack = first + 8 * SLOT;
            // The byte counts once the clock pulse of its acknowledge bit has ended.
            if ack + 8 < length {
                direction = if byte.read { items::READ } else { items::WRITE };
                let error = byte.nak && (byte.address || !byte.read);
                analysis.items.push(
                    Event {
                        kind: if byte.address {
                            items::ADDRESS
                        } else {
                            items::DATA
                        },
                        direction,
                        ack: if byte.nak { items::NAK } else { items::ACK },
                        ten_bit: false,
                        value: if byte.address {
                            u16::from(byte.value >> 1)
                        } else {
                            u16::from(byte.value)
                        },
                        error,
                    }
                    .item(first, ack + 1),
                );
                if error {
                    analysis.errors.push(if byte.address {
                        PlantedError::AddressNak { at: ack }
                    } else {
                        PlantedError::DataNak { at: ack }
                    });
                }
            }
        }
        let body_end = base + Params::body_end(&frame);
        // The rising edge of SCL that sets the stop up, and the stop itself.
        rise(body_end + 4, &mut analysis);
        let stop = body_end + 8;
        if stop < length {
            analysis
                .items
                .push(Event::condition(items::STOP, direction).item(stop, stop + 1));
            if frame.planted == Planted::ByteCut {
                let start = base + Params::slot_start(&frame, frame.count) + 4;
                analysis
                    .errors
                    .push(PlantedError::ByteCut { start, end: stop });
            }
        }
        k += 1;
    }
    if !analysis.calibrated {
        analysis.items.clear();
        analysis.errors.clear();
    }
    analysis
}

/// The items a correct decode of the first `length` samples must yield, in order. A capture with too
/// few clock pulses for the bus period to be found yields none.
#[must_use]
pub fn expected(setup: &Setup, seed: u64, length: u64) -> Vec<Item> {
    Params::from_setup(setup, seed).map_or_else(Vec::new, |p| analyse(&p, length).items)
}

/// The errors planted in the first `length` samples that a correct decode reports, in order.
#[must_use]
pub fn expected_errors(setup: &Setup, seed: u64, length: u64) -> Vec<PlantedError> {
    Params::from_setup(setup, seed).map_or_else(Vec::new, |p| analyse(&p, length).errors)
}
