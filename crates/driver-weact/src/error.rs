//! The error codes of the WeAct driver and their constructors.
//!
//! Every error is a [`la_core::Error`] of kind Device. The codes are stable identifiers: the
//! localization layer maps each to a message that says what to do (`errors.ftl`, key `error-` plus
//! the code with the dot written as a hyphen). The `detail` is English text for the log: what
//! happened, which board, which USB error; it never contains sample data and is not translated.
//!
//! Where an existing code means the same thing the driver uses it instead of adding one:
//! `device.disconnected` for an unplug, `device.transport` for a failed transfer,
//! `device.not_found` for an identity that is not a board of this driver.

use la_core::{Error, ErrorKind};

/// The system does not let this program open the board (on Linux: no permission).
pub const ACCESS_DENIED: &str = "weact.access_denied";
/// Windows: the board is not bound to the WinUSB driver.
pub const DRIVER_MISSING: &str = "weact.driver_missing";
/// Another program holds the board.
pub const IN_USE: &str = "weact.in_use";
/// The board refused the firmware, or reports a firmware version the driver does not know.
pub const FIRMWARE_REJECTED: &str = "weact.firmware_rejected";
/// The board did not come back after the firmware was loaded.
pub const NOT_RENUMERATED: &str = "weact.not_renumerated";
/// The board is still being prepared.
pub const PREPARING: &str = "weact.preparing";
/// No data arrived after the capture was started.
pub const NO_DATA: &str = "weact.no_data";
/// The stream of samples stopped before the capture was complete.
pub const STREAM_STOPPED: &str = "weact.stream_stopped";
/// The firmware image that ships with the application is unusable.
pub const FIRMWARE_INVALID: &str = "weact.firmware_invalid";

fn device(code: &'static str, detail: String) -> Error {
    Error::new(ErrorKind::Device, code, detail)
}

/// The system refuses to open the board; `detail` says which board and what the system said.
#[must_use]
pub fn access_denied(detail: impl Into<String>) -> Error {
    device(ACCESS_DENIED, detail.into())
}

/// The board is not on WinUSB; `driver` names the driver that holds it, when known.
#[must_use]
pub fn driver_missing(driver: Option<&str>) -> Error {
    let found = driver.map_or_else(
        || "the driver could not be determined".to_owned(),
        |name| format!("the driver in use is `{name}`"),
    );
    device(
        DRIVER_MISSING,
        format!("the board is not bound to the WinUSB driver ({found}); install WinUSB with Zadig"),
    )
}

/// Another program holds the board.
#[must_use]
pub fn in_use(detail: impl Into<String>) -> Error {
    device(IN_USE, detail.into())
}

/// The board refused the firmware or reports an unknown version; `detail` says which.
#[must_use]
pub fn firmware_rejected(detail: impl Into<String>) -> Error {
    device(FIRMWARE_REJECTED, detail.into())
}

/// The board did not return within `waited_ms` milliseconds of the firmware download.
#[must_use]
pub fn not_renumerated(waited_ms: u64) -> Error {
    device(
        NOT_RENUMERATED,
        format!("the board did not come back within {waited_ms} ms of the firmware download"),
    )
}

/// The board is being prepared and cannot be opened yet.
#[must_use]
pub fn preparing() -> Error {
    device(
        PREPARING,
        "the board is being prepared; try again in a moment".to_owned(),
    )
}

/// Nothing arrived within `waited_ms` milliseconds of the start of the capture.
#[must_use]
pub fn no_data(waited_ms: u64) -> Error {
    device(
        NO_DATA,
        format!("no data arrived within {waited_ms} ms of the start of the capture"),
    )
}

/// The stream stopped after `received` samples of `requested` (when the capture has a length).
#[must_use]
pub fn stream_stopped(received: u64, requested: Option<u64>) -> Error {
    let wanted = requested.map_or_else(
        || "an open-ended capture".to_owned(),
        |n| format!("{n} requested"),
    );
    device(
        STREAM_STOPPED,
        format!(
            "the stream of samples stopped after {received} samples ({wanted}); the board stops the capture when the computer cannot keep up"
        ),
    )
}

/// The firmware image is unusable (empty or larger than the chip's program memory).
#[must_use]
pub fn firmware_invalid(detail: impl Into<String>) -> Error {
    device(FIRMWARE_INVALID, detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&str; 9] = [
        ACCESS_DENIED,
        DRIVER_MISSING,
        IN_USE,
        FIRMWARE_REJECTED,
        NOT_RENUMERATED,
        PREPARING,
        NO_DATA,
        STREAM_STOPPED,
        FIRMWARE_INVALID,
    ];

    #[test]
    fn the_codes_are_unique_and_in_the_weact_domain() {
        for (i, code) in ALL.iter().enumerate() {
            assert!(code.starts_with("weact."), "{code}");
            assert!(!ALL.iter().skip(i + 1).any(|other| other == code), "{code}");
        }
    }

    #[test]
    fn every_constructor_uses_its_code_and_the_device_kind() {
        let errors = [
            access_denied("x"),
            driver_missing(Some("libusb0")),
            in_use("x"),
            firmware_rejected("x"),
            not_renumerated(5_000),
            preparing(),
            no_data(2_000),
            stream_stopped(10, Some(100)),
            firmware_invalid("x"),
        ];
        for (error, code) in errors.iter().zip(ALL) {
            assert_eq!(error.code(), code);
            assert_eq!(error.kind(), ErrorKind::Device);
            assert!(!error.detail().is_empty());
        }
    }

    #[test]
    fn the_driver_name_is_in_the_detail_when_it_is_known() {
        assert!(
            driver_missing(Some("libusb0"))
                .detail()
                .contains("`libusb0`")
        );
        assert!(
            driver_missing(None)
                .detail()
                .contains("could not be determined")
        );
    }
}
