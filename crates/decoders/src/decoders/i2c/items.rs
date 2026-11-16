//! The items of the I2C decoder for a decoder stacked on it (`contracts/i2c-items.md`).
//!
//! One kind, [`ITEM`], whose payload is six bytes: the type of the event, the direction, the
//! acknowledgement, a flags byte and the value as two bytes, big endian.

use crate::model::{Item, Payload};

use super::ITEM;

/// A start condition.
pub const START: u8 = 1;
/// A repeated start condition.
pub const REPEATED_START: u8 = 2;
/// An address (one byte, or the two bytes of a 10-bit address).
pub const ADDRESS: u8 = 3;
/// A data byte.
pub const DATA: u8 = 4;
/// A stop condition.
pub const STOP: u8 = 5;

/// The direction of a write.
pub const WRITE: u8 = 0;
/// The direction of a read.
pub const READ: u8 = 1;
/// No direction known (before an address).
pub const UNKNOWN: u8 = 2;

/// The byte was acknowledged.
pub const ACK: u8 = 0;
/// The byte was not acknowledged.
pub const NAK: u8 = 1;
/// There is no acknowledgement (an event that is not a byte).
pub const NONE: u8 = 2;

/// The flag of a 10-bit address.
pub const TEN_BIT: u8 = 1;

/// What happened on the bus, as an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// [`START`], [`REPEATED_START`], [`ADDRESS`], [`DATA`] or [`STOP`].
    pub kind: u8,
    /// [`WRITE`], [`READ`] or [`UNKNOWN`].
    pub direction: u8,
    /// [`ACK`], [`NAK`] or [`NONE`].
    pub ack: u8,
    /// Whether the address is a 10-bit address.
    pub ten_bit: bool,
    /// The address (7 or 10 bits) or the byte.
    pub value: u16,
    /// An acknowledge was required and did not come.
    pub error: bool,
}

impl Event {
    /// An event that is not a byte: a start, a repeated start or a stop.
    #[must_use]
    pub fn condition(kind: u8, direction: u8) -> Self {
        Self {
            kind,
            direction,
            ack: NONE,
            ten_bit: false,
            value: 0,
            error: false,
        }
    }

    /// The item for the span `[start, end)`.
    #[must_use]
    pub fn item(self, start: u64, end: u64) -> Item {
        let [high, low] = self.value.to_be_bytes();
        Item {
            kind: ITEM,
            start,
            end,
            payload: Payload::Bytes(vec![
                self.kind,
                self.direction,
                self.ack,
                if self.ten_bit { TEN_BIT } else { 0 },
                high,
                low,
            ]),
            error: self.error,
        }
    }
}
