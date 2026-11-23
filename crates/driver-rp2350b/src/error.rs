//! The error codes of the RP2350B driver and their constructors.
//!
//! Every error is a [`la_core::Error`] of kind Device. The codes are stable identifiers: the localization layer
//! maps each to a message that says what to do (`errors.ftl`, key `error-` plus the code with the dot written
//! as a hyphen). The `detail` is English text for the log: which port, which line, which field; it never
//! contains sample data and is not translated.
//!
//! `device.disconnected` is used for an unplug, as by the other drivers.

use la_core::{Error, ErrorKind};

/// The system does not let this program open the serial port.
pub const ACCESS_DENIED: &str = "rp2350b.access_denied";
/// Another program has the serial port open.
pub const PORT_BUSY: &str = "rp2350b.port_busy";
/// A listed board went away before it could be opened.
pub const NOT_FOUND: &str = "rp2350b.not_found";
/// The firmware or the protocol level of the board is not supported.
pub const FIRMWARE_UNSUPPORTED: &str = "rp2350b.firmware_unsupported";
/// The board answered `CAPTURE_ERROR` or an `ERR_` line.
pub const REQUEST_REFUSED: &str = "rp2350b.request_refused";
/// The capture ended before the samples promised arrived.
pub const STREAM_STOPPED: &str = "rp2350b.stream_stopped";
/// A reply that does not follow the contract.
pub const PROTOCOL: &str = "rp2350b.protocol";
/// No answer within the time allowed.
pub const TIMEOUT: &str = "rp2350b.timeout";
/// The board was unplugged.
pub const DISCONNECTED: &str = "rp2350b.disconnected";

fn device(code: &'static str, detail: String) -> Error {
    Error::new(ErrorKind::Device, code, detail)
}

/// The system refuses to open `port`.
#[must_use]
pub fn access_denied(port: &str, what: &str) -> Error {
    device(
        ACCESS_DENIED,
        format!("the system does not let the program open {port}: {what}"),
    )
}

/// Another program holds `port`.
#[must_use]
pub fn port_busy(port: &str, what: &str) -> Error {
    device(
        PORT_BUSY,
        format!("{port} is held by another program: {what}"),
    )
}

/// The board at `port` is gone.
#[must_use]
pub fn not_found(port: &str) -> Error {
    device(NOT_FOUND, format!("the board at {port} is no longer there"))
}

/// The firmware is not supported; `found` says what it reported.
#[must_use]
pub fn firmware_unsupported(found: &str, supported: &str) -> Error {
    device(
        FIRMWARE_UNSUPPORTED,
        format!("the board reports {found}; this program supports {supported}"),
    )
}

/// The board refused a request; `what` is the line or the field it named.
#[must_use]
pub fn request_refused(what: &str) -> Error {
    device(
        REQUEST_REFUSED,
        format!("the board refused the request: {what}"),
    )
}

/// The capture stopped after `received` of `promised` samples.
#[must_use]
pub fn stream_stopped(received: u64, promised: u64) -> Error {
    device(
        STREAM_STOPPED,
        format!("the capture stopped after {received} of {promised} samples"),
    )
}

/// The board sent something the contract does not allow.
#[must_use]
pub fn protocol(what: &str) -> Error {
    device(
        PROTOCOL,
        format!("the board's reply does not follow the protocol: {what}"),
    )
}

/// The board did not answer within `waited_ms` milliseconds while `doing`.
#[must_use]
pub fn timeout(doing: &str, waited_ms: u64) -> Error {
    device(
        TIMEOUT,
        format!("no answer within {waited_ms} ms while {doing}"),
    )
}

/// The board was unplugged.
#[must_use]
pub fn disconnected(port: &str) -> Error {
    Error::new(
        ErrorKind::Device,
        la_devices::error::DISCONNECTED,
        format!("the board at {port} was unplugged; reconnect it and start again"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&str; 9] = [
        ACCESS_DENIED,
        PORT_BUSY,
        NOT_FOUND,
        FIRMWARE_UNSUPPORTED,
        REQUEST_REFUSED,
        STREAM_STOPPED,
        PROTOCOL,
        TIMEOUT,
        DISCONNECTED,
    ];

    #[test]
    fn the_codes_are_unique_and_in_the_rp2350b_domain() {
        for (i, code) in ALL.iter().enumerate() {
            assert!(code.starts_with("rp2350b."), "{code}");
            assert!(!ALL.iter().skip(i + 1).any(|other| other == code), "{code}");
        }
    }

    #[test]
    fn every_constructor_uses_its_code_and_the_device_kind() {
        let errors = [
            access_denied("COM3", "x"),
            port_busy("COM3", "x"),
            not_found("COM3"),
            firmware_unsupported("FW 9.0.0", "1.x"),
            request_refused("frequency"),
            stream_stopped(1, 2),
            protocol("x"),
            timeout("waiting", 2_000),
        ];
        for (error, code) in errors.iter().zip(ALL) {
            assert_eq!(error.code(), code);
            assert_eq!(error.kind(), ErrorKind::Device);
            assert!(!error.detail().is_empty());
        }
        assert_eq!(disconnected("COM3").code(), la_devices::error::DISCONNECTED);
    }
}
