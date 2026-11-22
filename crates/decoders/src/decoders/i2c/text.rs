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

/// The colours of the blocks of the lane (indexes of the palette of eight annotation colours, see
/// `specs/016-i2c-lane-blocks/contracts/i2c-lane.md`). The marks on the signals name none: they take
/// the colour of their signal.
const BLUE: u8 = 4;
const YELLOW: u8 = 1;
const GREEN: u8 = 2;
const RED: u8 = 0;
const GREY: u8 = 7;

fn message(key: &'static str, args: Vec<String>) -> Text {
    Text::Message { key, args }
}

/// The style of a block of the upper band (a part of a packet).
fn part(colour: u8) -> Style {
    Style {
        colour: Some(colour),
        shape: Shape::Part,
    }
}

/// The style of a block of the lower band (a whole packet).
fn whole(colour: u8) -> Style {
    Style {
        colour: Some(colour),
        shape: Shape::Whole,
    }
}

/// The colour of an address or data block: yellow for a write, green for a read.
const fn direction_colour(read: bool) -> u8 {
    if read { GREEN } else { YELLOW }
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

/// Start, as a blue block.
pub fn start() -> (Text, Style) {
    (message(MSG_START, Vec::new()), part(BLUE))
}

/// Repeated start, as a blue block.
pub fn repeated_start() -> (Text, Style) {
    (message(MSG_REPEATED_START, Vec::new()), part(BLUE))
}

/// Stop, as a blue block.
pub fn stop() -> (Text, Style) {
    (message(MSG_STOP, Vec::new()), part(BLUE))
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

/// The block of a whole transaction, named after its first part: red when it has an error.
pub fn transaction(read: bool, address: String, bad: bool) -> (Text, Style) {
    let key = if read {
        MSG_TRANSACTION_READ
    } else {
        MSG_TRANSACTION_WRITE
    };
    (
        message(key, vec![address]),
        whole(if bad { RED } else { GREEN }),
    )
}

/// The block of an address byte (eight bits).
pub fn address(read: bool, value: String) -> (Text, Style) {
    let key = if read {
        MSG_ADDRESS_READ
    } else {
        MSG_ADDRESS_WRITE
    };
    (message(key, vec![value]), part(direction_colour(read)))
}

/// The block of a data byte (eight bits).
pub fn data(read: bool, value: String) -> (Text, Style) {
    (message(MSG_DATA, vec![value]), part(direction_colour(read)))
}

/// The block of an acknowledge bit: green for an ACK, red for a NAK.
pub fn acknowledge(ack: bool) -> (Text, Style) {
    let key = if ack { MSG_ACK } else { MSG_NAK };
    (
        message(key, Vec::new()),
        part(if ack { GREEN } else { RED }),
    )
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

/// The block of a transaction that was not finished: red when it already had an error, else grey.
pub fn incomplete(bad: bool) -> (Text, Style) {
    (
        message(MSG_INCOMPLETE, Vec::new()),
        whole(if bad { RED } else { GREY }),
    )
}

/// The error block of a byte that was cut short after `bits` bits, in the colour its byte would have.
pub fn byte_cut(bits: u8, read: bool) -> (Text, Style) {
    (
        message(MSG_BYTE_CUT, vec![bits.to_string()]),
        part(direction_colour(read)),
    )
}

/// The text of an address that was not acknowledged, on the arrow of its acknowledge bit.
pub fn address_nak(address: String) -> (Text, Style) {
    (
        message(MSG_ADDRESS_NAK, vec![address]),
        signal(Shape::Arrow),
    )
}

/// The text of a written byte that was not acknowledged, on the arrow of its acknowledge bit.
pub fn data_nak(value: String) -> (Text, Style) {
    (message(MSG_DATA_NAK, vec![value]), signal(Shape::Arrow))
}

/// The error block of a transaction with no address.
pub fn empty() -> (Text, Style) {
    (message(MSG_EMPTY, Vec::new()), whole(RED))
}

/// The error block of lost samples.
pub fn gap() -> (Text, Style) {
    (message(MSG_GAP, Vec::new()), part(RED))
}
