//! The generator of the sample decoder "Clocked bits" (research R13, R14).
//!
//! A pure function of the position: from the seed it draws, for every frame `k`, a byte and (when
//! deliberate errors are on) whether the frame is cut short. A frame is `bits` bit slots of
//! [`PERIOD`] samples followed by an idle stretch, so frames have a fixed length and the signal at
//! any position is computed without state. In a bit slot the data holds the bit from its first sample
//! and the clock is high for [`HIGH_FROM`]..[`HIGH_TO`] samples into the slot, so the data is stable
//! on both clock edges.

// `bits` is clamped to 4..=16 and the error rate to at least 2 when the parameters are made, so
// the shifts and the `bits - 1` cannot overflow and no modulo is by zero; `sample` runs for every
// sample of a capture, so the plain operators are kept.
#![allow(clippy::arithmetic_side_effects)]

use std::collections::BTreeMap;
use std::sync::Arc;

use la_devices::SignalSource;

use super::composite::Claim;
use crate::decoders::clocked_bits::BYTE_ITEM;
use crate::model::{Item, Payload};
use crate::setup::{InstanceId, Setup, Value};

/// Samples per bit.
pub const PERIOD: u64 = 10;
/// The clock is high from this many samples into a bit slot ...
pub const HIGH_FROM: u64 = 3;
/// ... until this many.
pub const HIGH_TO: u64 = 8;
/// Option of the generator: whether bytes are deliberately cut short (0 or 1).
pub const OPTION_ERRORS: &str = "errors";
/// Option of the generator: about one frame in this many is cut short.
pub const OPTION_ERROR_RATE: &str = "error_rate";
/// The default rate of deliberate errors.
pub const DEFAULT_ERROR_RATE: i64 = 8;

/// What a generator needs to know, taken from the setup of its instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    clock: u8,
    data: u8,
    bits: u64,
    idle: u64,
    msb_first: bool,
    falling: bool,
    errors: bool,
    error_rate: u64,
    seed: u64,
}

pub(super) fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The seed of one instance: instances with the same seed of the capture still differ.
#[must_use]
pub fn instance_seed(seed: u64, instance: InstanceId) -> u64 {
    mix(seed ^ mix(u64::from(instance)))
}

fn number(setup: &Setup, id: &str, default: i64) -> i64 {
    match setup.option_value(&crate::decoders::clocked_bits::DESCRIPTOR, id) {
        Some(Value::Integer(v)) => v,
        _ => default,
    }
}

/// The channels a generator for this setup fills: the clock and the data channel.
#[must_use]
pub fn claimed_channels(setup: &Setup) -> u32 {
    ["clock", "data"]
        .iter()
        .filter_map(|role| setup.channels.get(*role))
        .filter_map(|c| 1_u32.checked_shl(u32::from(*c)))
        .fold(0, |mask, bit| mask | bit)
}

impl Params {
    /// The parameters of a setup of the decoder, for the capture seed `seed`; `None` when the
    /// clock or the data channel is not assigned.
    #[must_use]
    pub fn from_setup(setup: &Setup, seed: u64) -> Option<Self> {
        let clock = *setup.channels.get("clock")?;
        let data = *setup.channels.get("data")?;
        let choice = |id: &str, wanted: &str| {
            matches!(
                setup.option_value(&crate::decoders::clocked_bits::DESCRIPTOR, id),
                Some(Value::Choice(v)) if v == wanted
            )
        };
        let generator: &BTreeMap<String, i64> = &setup.generator;
        Some(Self {
            clock,
            data,
            bits: u64::try_from(number(setup, "bits", 8))
                .unwrap_or(8)
                .clamp(4, 16),
            idle: u64::try_from(number(setup, "idle", 100))
                .unwrap_or(100)
                .max(1),
            msb_first: choice("bit_order", "msb"),
            falling: choice("clock_edge", "falling"),
            errors: generator.get(OPTION_ERRORS).copied().unwrap_or(0) != 0,
            error_rate: u64::try_from(
                generator
                    .get(OPTION_ERROR_RATE)
                    .copied()
                    .unwrap_or(DEFAULT_ERROR_RATE),
            )
            .unwrap_or(8)
            .max(2),
            seed: instance_seed(seed, setup.instance),
        })
    }

    /// Samples in a frame: the bits, then an idle stretch longer than the idle limit.
    #[must_use]
    pub fn frame_length(&self) -> u64 {
        self.bits
            .saturating_mul(PERIOD)
            .saturating_add(self.idle)
            .saturating_add(PERIOD)
    }

    /// The byte of frame `k` and how many of its bits are sent (fewer than `bits` when it is cut).
    fn frame(&self, k: u64) -> (u64, u64) {
        let h = mix(self.seed ^ k.wrapping_mul(0xD6E8_FEB8_6659_FD93));
        let value = h & ((1_u64 << self.bits) - 1);
        let cut = self.errors && mix(h) % self.error_rate == 0;
        let sent = if cut {
            1 + mix(h ^ 0xA5A5) % (self.bits - 1)
        } else {
            self.bits
        };
        (value, sent)
    }

    /// The bit of a byte sent in slot `slot`.
    fn bit(&self, value: u64, slot: u64) -> bool {
        let index = if self.msb_first {
            self.bits - 1 - slot.min(self.bits - 1)
        } else {
            slot.min(self.bits - 1)
        };
        value >> index & 1 != 0
    }

    /// The sample at `position`: the clock and data bits in their channels.
    fn sample(&self, position: u64) -> u32 {
        let frame = self.frame_length();
        let (k, r) = (position / frame, position % frame);
        let (value, sent) = self.frame(k);
        let slot = r / PERIOD;
        let offset = r % PERIOD;
        let data_slot = slot.min(sent - 1);
        let data = self.bit(value, data_slot);
        let clock = slot < sent && (HIGH_FROM..HIGH_TO).contains(&offset);
        (u32::from(clock) << self.clock) | (u32::from(data) << self.data)
    }

    /// The position of the sampled edge in slot `slot` of frame `k`.
    fn edge_at(&self, k: u64, slot: u64) -> u64 {
        let offset = if self.falling { HIGH_TO } else { HIGH_FROM };
        k.saturating_mul(self.frame_length())
            .saturating_add(slot.saturating_mul(PERIOD))
            .saturating_add(offset)
    }
}

/// The signal of one instance, a function of the position.
pub struct ClockedBitsSource {
    params: Params,
}

/// A source for the setup (the clock and the data channel must be assigned).
#[must_use]
pub fn source(setup: &Setup, seed: u64) -> Option<ClockedBitsSource> {
    Params::from_setup(setup, seed).map(|params| ClockedBitsSource { params })
}

impl SignalSource for ClockedBitsSource {
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        let mut position = start;
        for word in out.iter_mut() {
            *word = self.params.sample(position);
            position = position.saturating_add(1);
        }
    }
}

/// The claim of a setup for the composite source: the channels and how to make the source.
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

/// What a correct decode of the first `length` positions must yield: one item per complete byte
/// whose last sampled edge is inside the capture, with its value, start and end exactly as the
/// decoder reports them.
#[must_use]
pub fn expected(setup: &Setup, seed: u64, length: u64) -> Vec<Item> {
    let Some(params) = Params::from_setup(setup, seed) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut k = 0_u64;
    loop {
        if params.edge_at(k, 0) >= length {
            break;
        }
        let (value, sent) = params.frame(k);
        let last = params.edge_at(k, params.bits - 1);
        if sent == params.bits && last < length {
            items.push(Item {
                kind: BYTE_ITEM,
                start: params.edge_at(k, 0),
                end: last.saturating_add(1),
                payload: Payload::Integer(value),
                error: false,
            });
        }
        k = k.saturating_add(1);
    }
    items
}

/// The ranges `(start, end)` of the bytes that were deliberately cut short and whose sent edges
/// are all inside the first `length` positions: the places where an incomplete-byte error must be
/// reported.
#[must_use]
pub fn expected_errors(setup: &Setup, seed: u64, length: u64) -> Vec<(u64, u64)> {
    let Some(params) = Params::from_setup(setup, seed) else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    let mut k = 0_u64;
    while params.edge_at(k, 0) < length {
        let (_, sent) = params.frame(k);
        let last = params.edge_at(k, sent - 1);
        if sent < params.bits && last < length {
            errors.push((params.edge_at(k, 0), last.saturating_add(1)));
        }
        k = k.saturating_add(1);
    }
    errors
}
