//! The texts and styles of the annotations of the I2C decoder (research R5, R9).
//!
//! Message keys are string literals in this file (the localization parity check finds them here);
//! the values shown (addresses, bytes) are arguments and are not translated.

use crate::model::{Shape, Style, Text};

const MSG_START: &str = "ann-i2c-start";
const MSG_REPEATED_START: &str = "ann-i2c-repeated-start";
const MSG_STOP: &str = "ann-i2c-stop";
const MSG_TRANSACTION_WRITE: &str = "ann-i2c-transaction-write";
const MSG_TRANSACTION_READ: &str = "ann-i2c-transaction-read";
const MSG_ADDRESS_WRITE: &str = "ann-i2c-address-write";
const MSG_ADDRESS_READ: &str = "ann-i2c-address-read";
const MSG_DATA: &str = "ann-i2c-data";
const MSG_ACK: &str = "ann-i2c-ack";
const MSG_NAK: &str = "ann-i2c-nak";
const MSG_INCOMPLETE: &str = "ann-i2c-incomplete";
const MSG_BYTE_CUT: &str = "ann-i2c-error-byte-cut";
const MSG_ADDRESS_NAK: &str = "ann-i2c-error-address-nak";
const MSG_DATA_NAK: &str = "ann-i2c-error-data-nak";
const MSG_EMPTY: &str = "ann-i2c-error-empty";
const MSG_GAP: &str = "ann-i2c-error-gap";

/// The colours of the lane (indexes of the palette). The marks on the signals name none: they take
/// the colour of their signal.
const COLOUR_TRANSACTION: u8 = 5;
const COLOUR_ADDRESS: u8 = 0;
const COLOUR_DATA: u8 = 1;
const COLOUR_EVENT: u8 = 3;
const COLOUR_ACK: u8 = 2;
/// The colour of errors.
const COLOUR_ERROR: u8 = 4;

fn message(key: &'static str, args: Vec<String>) -> Text {
    Text::Message { key, args }
}

fn lane(colour: u8, shape: Shape) -> Style {
    Style {
        colour: Some(colour),
        shape,
    }
}

/// A style that leaves the colour to the signal the mark is on.
fn signal(shape: Shape) -> Style {
    Style {
        colour: None,
        shape,
    }
}

/// The number base of the values shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radix {
    /// Hexadecimal, `0x5A`.
    Hex,
    /// Decimal, `90`.
    Dec,
    /// Binary, `0b01011010`.
    Bin,
}

/// How values are written: addresses and bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    /// Addresses are shown as the 8-bit value with the direction bit (7-bit addresses only).
    pub address8: bool,
    /// The number base.
    pub radix: Radix,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            address8: false,
            radix: Radix::Hex,
        }
    }
}

impl Display {
    fn number(self, value: u16, bits: u32) -> String {
        match self.radix {
            Radix::Hex => format!("0x{value:02X}"),
            Radix::Dec => value.to_string(),
            Radix::Bin => format!(
                "0b{value:0width$b}",
                width = usize::try_from(bits).unwrap_or(8)
            ),
        }
    }

    /// A data byte.
    #[must_use]
    pub fn byte(self, value: u16) -> String {
        self.number(value, 8)
    }

    /// An address: 7 or 10 bits, or, with the 8-bit notation, a 7-bit address with the direction
    /// bit. A 10-bit address is always shown as it is.
    #[must_use]
    pub fn address(self, address: u16, read: bool, ten_bit: bool) -> String {
        if ten_bit {
            self.number(address, 10)
        } else if self.address8 {
            self.number(address << 1 | u16::from(read), 8)
        } else {
            self.number(address, 7)
        }
    }
}

/// Start, as a lane point.
pub fn start() -> (Text, Style) {
    (
        message(MSG_START, Vec::new()),
        lane(COLOUR_EVENT, Shape::Flag),
    )
}

/// Repeated start, as a lane point.
pub fn repeated_start() -> (Text, Style) {
    (
        message(MSG_REPEATED_START, Vec::new()),
        lane(COLOUR_EVENT, Shape::Flag),
    )
}

/// Stop, as a lane point.
pub fn stop() -> (Text, Style) {
    (
        message(MSG_STOP, Vec::new()),
        lane(COLOUR_EVENT, Shape::Flag),
    )
}

/// The dot on SDA for a start, a repeated start or a stop: the text of the event, the colour of SDA.
pub fn event_dot(text: Text) -> (Text, Style) {
    (text, signal(Shape::Dot))
}

/// The text of the start, for the dot.
pub fn start_text() -> Text {
    message(MSG_START, Vec::new())
}

/// The text of the repeated start, for the dot.
pub fn repeated_start_text() -> Text {
    message(MSG_REPEATED_START, Vec::new())
}

/// The text of the stop, for the dot.
pub fn stop_text() -> Text {
    message(MSG_STOP, Vec::new())
}

/// The range of a whole transaction, named after its first part.
pub fn transaction(read: bool, address: String) -> (Text, Style) {
    let key = if read {
        MSG_TRANSACTION_READ
    } else {
        MSG_TRANSACTION_WRITE
    };
    (
        message(key, vec![address]),
        lane(COLOUR_TRANSACTION, Shape::Bar),
    )
}

/// The range of an address byte.
pub fn address(read: bool, value: String) -> (Text, Style) {
    let key = if read {
        MSG_ADDRESS_READ
    } else {
        MSG_ADDRESS_WRITE
    };
    (message(key, vec![value]), lane(COLOUR_ADDRESS, Shape::Bar))
}

/// The range of a data byte.
pub fn data(value: String) -> (Text, Style) {
    (
        message(MSG_DATA, vec![value]),
        lane(COLOUR_DATA, Shape::Bar),
    )
}

/// The point of an acknowledge bit.
pub fn acknowledge(ack: bool) -> (Text, Style) {
    let key = if ack { MSG_ACK } else { MSG_NAK };
    (message(key, Vec::new()), lane(COLOUR_ACK, Shape::Dot))
}

/// The arrow on SCL at a sampled data bit: the bit, as literal text.
pub fn bit_arrow(bit: bool) -> (Text, Style) {
    (
        Text::Literal(if bit { "1" } else { "0" }.to_owned()),
        signal(Shape::Arrow),
    )
}

/// The arrow on SCL at the acknowledge bit.
pub fn ack_arrow(ack: bool) -> (Text, Style) {
    let key = if ack { MSG_ACK } else { MSG_NAK };
    (message(key, Vec::new()), signal(Shape::Arrow))
}

/// The range of a transaction that was not finished (not an error).
pub fn incomplete() -> (Text, Style) {
    (
        message(MSG_INCOMPLETE, Vec::new()),
        lane(COLOUR_TRANSACTION, Shape::Bar),
    )
}

/// The error range of a byte that was cut short after `bits` bits.
pub fn byte_cut(bits: u8) -> (Text, Style) {
    (
        message(MSG_BYTE_CUT, vec![bits.to_string()]),
        lane(COLOUR_ERROR, Shape::Bar),
    )
}

/// The error point of an address that was not acknowledged.
pub fn address_nak(address: String) -> (Text, Style) {
    (
        message(MSG_ADDRESS_NAK, vec![address]),
        lane(COLOUR_ERROR, Shape::Cross),
    )
}

/// The error point of a written byte that was not acknowledged.
pub fn data_nak(value: String) -> (Text, Style) {
    (
        message(MSG_DATA_NAK, vec![value]),
        lane(COLOUR_ERROR, Shape::Cross),
    )
}

/// The error range of a transaction with no address.
pub fn empty() -> (Text, Style) {
    (
        message(MSG_EMPTY, Vec::new()),
        lane(COLOUR_ERROR, Shape::Bar),
    )
}

/// The error range of lost samples.
pub fn gap() -> (Text, Style) {
    (message(MSG_GAP, Vec::new()), lane(COLOUR_ERROR, Shape::Bar))
}
