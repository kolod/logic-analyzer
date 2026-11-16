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

/// The colours of the lane (indexes of the palette). The marks on the signals name none: they take
/// the colour of their signal.
const COLOUR_TRANSACTION: u8 = 5;
const COLOUR_ADDRESS: u8 = 0;
const COLOUR_DATA: u8 = 1;
const COLOUR_EVENT: u8 = 3;
const COLOUR_ACK: u8 = 2;
/// The colour of errors.
#[expect(dead_code, reason = "used by the error paths of the decoder (T025)")]
pub const COLOUR_ERROR: u8 = 4;

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

/// How values are written: addresses and bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display;

impl Display {
    /// A byte or an address: hexadecimal, at least two digits.
    #[must_use]
    pub fn value(self, value: u16) -> String {
        format!("0x{value:02X}")
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
