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

const SLOT: u64 = SAMPLES_PER_BIT;
const BYTE: u64 = 9 * SLOT;
const START: u64 = 4;
const RESTART: u64 = 12;
const STOP: u64 = 8;

/// The parameters of a generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    scl: u8,
    sda: u8,
    max_bytes: u64,
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

/// The content of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    bytes: [Byte; 12],
    count: usize,
    /// The number of bytes before the repeated start; `count` when there is none.
    restart_before: usize,
}

impl Params {
    /// The parameters for a setup and a capture seed; `None` unless both signals are assigned.
    #[must_use]
    pub fn from_setup(setup: &Setup, seed: u64) -> Option<Self> {
        let scl = *setup.channels.get("scl")?;
        let sda = *setup.channels.get("sda")?;
        let max_bytes = setup
            .generator
            .get(OPTION_MAX_BYTES)
            .copied()
            .unwrap_or(4)
            .clamp(1, MAX_BYTES_LIMIT as i64) as u64;
        Some(Self {
            scl,
            sda,
            max_bytes,
            seed: instance_seed(seed, setup.instance),
        })
    }

    /// The length of a frame in samples (it does not depend on the content).
    #[must_use]
    pub fn frame_length(&self) -> u64 {
        let bytes = self.max_bytes + 2;
        LEAD_IN + START + bytes * BYTE + RESTART + STOP + TAIL
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
            restart_before: 0,
        };
        let push = |frame: &mut Frame, byte: Byte| {
            frame.bytes[frame.count] = byte;
            frame.count += 1;
        };
        let data = |index: u64| (draw(0x100 + index) & 0xFF) as u8;
        if split {
            // A write of d1 bytes, a repeated start and a read of d2 bytes.
            let total = 2 + draw(4) % (self.max_bytes - 1);
            let first = 1 + draw(5) % (total - 1);
            push(
                &mut frame,
                Byte {
                    value: address << 1,
                    nak: false,
                    address: true,
                    read: false,
                },
            );
            for i in 0..first {
                push(
                    &mut frame,
                    Byte {
                        value: data(i),
                        nak: false,
                        address: false,
                        read: false,
                    },
                );
            }
            frame.restart_before = frame.count;
            push(
                &mut frame,
                Byte {
                    value: address << 1 | 1,
                    nak: false,
                    address: true,
                    read: true,
                },
            );
            let second = total - first;
            for i in 0..second {
                push(
                    &mut frame,
                    Byte {
                        value: data(first + i),
                        nak: i + 1 == second,
                        address: false,
                        read: true,
                    },
                );
            }
        } else {
            let total = 1 + draw(4) % self.max_bytes;
            push(
                &mut frame,
                Byte {
                    value: address << 1 | u8::from(read),
                    nak: false,
                    address: true,
                    read,
                },
            );
            for i in 0..total {
                push(
                    &mut frame,
                    Byte {
                        value: data(i),
                        nak: read && i + 1 == total,
                        address: false,
                        read,
                    },
                );
            }
            frame.restart_before = frame.count;
        }
        frame
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
        let restart = frame.restart_before < frame.count;
        let first = frame.restart_before as u64 * BYTE;
        let second = (frame.count - frame.restart_before) as u64 * BYTE;
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
        if restart {
            if t < first {
                return body(t, 0);
            }
            t -= first;
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
        } else {
            if t < first {
                return body(t, 0);
            }
            t -= first;
        }
        match t {
            0..=3 => (false, false),
            4..=7 => (false, true),
            _ => (true, true),
        }
    }

    /// The sample word at a position.
    fn sample(&self, position: u64, cache: &mut Option<(u64, Frame)>) -> u32 {
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
        let (sda, scl) = Self::levels(&frame, r);
        (u32::from(scl) << self.scl) | (u32::from(sda) << self.sda)
    }

    /// The position of the first sample of a byte's first bit slot, relative to its frame.
    fn slot_start(frame: &Frame, byte: usize) -> u64 {
        let before = frame.restart_before.min(byte) as u64;
        let after = byte.saturating_sub(frame.restart_before) as u64;
        let restart = if byte >= frame.restart_before && frame.restart_before < frame.count {
            RESTART
        } else {
            0
        };
        LEAD_IN + START + before * BYTE + restart + after * BYTE
    }
}

/// The signal source of one I2C generator.
pub struct I2cSource {
    params: Params,
    cache: Option<(u64, Frame)>,
}

impl SignalSource for I2cSource {
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        let mut position = start;
        for word in out.iter_mut() {
            *word = self.params.sample(position, &mut self.cache);
            position = position.saturating_add(1);
        }
    }
}

/// The source for a setup and a capture seed, or `None` if a signal is not assigned.
#[must_use]
pub fn source(setup: &Setup, seed: u64) -> Option<I2cSource> {
    Params::from_setup(setup, seed).map(|params| I2cSource {
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

/// The items a correct decode of the first `length` samples must yield, in order.
#[must_use]
pub fn expected(setup: &Setup, seed: u64, length: u64) -> Vec<Item> {
    let Some(params) = Params::from_setup(setup, seed) else {
        return Vec::new();
    };
    let frame_length = params.frame_length();
    let mut result = Vec::new();
    let mut rises = 0_u64;
    let mut k = 0_u64;
    while k * frame_length < length {
        let base = k * frame_length;
        let frame = params.frame(k);
        let restart = frame.restart_before < frame.count;
        let mut direction = items::UNKNOWN;
        // The edge a bit is read on and the edge that ends its pulse.
        let start_edge = base + LEAD_IN;
        if start_edge < length {
            result.push(
                Event::condition(items::START, items::UNKNOWN).item(start_edge, start_edge + 1),
            );
        }
        for index in 0..frame.count {
            if restart && index == frame.restart_before {
                let at = base + Params::slot_start(&frame, index) - RESTART + 8;
                // The rising edge of SCL that sets the repeated start up.
                if at - 4 < length {
                    rises += 1;
                }
                if at < length {
                    result.push(
                        Event::condition(items::REPEATED_START, items::UNKNOWN).item(at, at + 1),
                    );
                }
            }
            let byte = frame.bytes[index];
            let first = base + Params::slot_start(&frame, index) + 4;
            for bit in 0..9 {
                if first + bit * SLOT < length {
                    rises += 1;
                }
            }
            let ack = first + 8 * SLOT;
            // The byte counts once the clock pulse of its acknowledge bit has ended.
            if ack + 8 < length {
                direction = if byte.read { items::READ } else { items::WRITE };
                let event = Event {
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
                    error: byte.nak && (byte.address || !byte.read),
                };
                result.push(event.item(first, ack + 1));
            }
        }
        let body_end =
            base + LEAD_IN + START + frame.count as u64 * BYTE + if restart { RESTART } else { 0 };
        // The rising edge of SCL that sets the stop up, and the stop itself.
        if body_end + 4 < length {
            rises += 1;
        }
        let stop = body_end + 8;
        if stop < length {
            result.push(Event::condition(items::STOP, direction).item(stop, stop + 1));
        }
        k += 1;
    }
    // The period of the bus is found from the clock pulses: with too few the decoder shows nothing.
    if rises < 17 {
        return Vec::new();
    }
    result
}
