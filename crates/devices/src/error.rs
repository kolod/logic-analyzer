//! Stable error codes and constructors for devices and acquisitions.
//!
//! Every fallible operation returns [`la_core::Error`]. The codes below are stable
//! identifiers: the localization layer maps them to user-facing text later and tests compare
//! against them. The `detail` is English text that says what is wrong and what is allowed;
//! it is meant for the log and for the actionable explanation, never contains signal data,
//! and is not translated.
//!
//! Driver authors build their errors with [`disconnected`], [`transport`] and
//! [`driver_error`].

use la_core::{Error, ErrorKind};

/// A device description is inconsistent (zero channels, no sample rate, ...).
pub const INVALID_CAPABILITIES: &str = "device.invalid_capabilities";
/// A configuration names a channel the device does not have, or none at all.
pub const INVALID_CHANNEL: &str = "device.invalid_channel";
/// The requested sample rate is not supported by the device.
pub const UNSUPPORTED_SAMPLE_RATE: &str = "device.unsupported_sample_rate";
/// The requested trigger is not supported by the device or is misconfigured.
pub const UNSUPPORTED_TRIGGER: &str = "device.unsupported_trigger";
/// The capture length is missing, zero or beyond the device's buffer depth.
pub const INVALID_CAPTURE_LENGTH: &str = "device.invalid_capture_length";
/// The worst-case storage cost of the capture exceeds the memory budget.
pub const MEMORY_BUDGET: &str = "device.memory_budget";
/// No such device is known.
pub const NOT_FOUND: &str = "device.not_found";
/// The device already runs an acquisition.
pub const BUSY: &str = "device.busy";
/// The operation is not allowed in the current state.
pub const INVALID_STATE: &str = "device.invalid_state";
/// The device was unplugged or went away.
pub const DISCONNECTED: &str = "device.disconnected";
/// The transport to the device failed (USB, serial, ...).
pub const TRANSPORT: &str = "device.transport";
/// The driver reported an error.
pub const DRIVER_ERROR: &str = "device.driver_error";
/// The driver panicked.
pub const DRIVER_PANIC: &str = "device.driver_panic";
/// The driver stopped responding.
pub const UNRESPONSIVE: &str = "device.unresponsive";
/// The trigger condition did not occur within the search limit.
pub const TRIGGER_TIMEOUT: &str = "device.trigger_timeout";

fn input(code: &'static str, detail: String) -> Error {
    Error::new(ErrorKind::InvalidInput, code, detail)
}

fn device(code: &'static str, detail: String) -> Error {
    Error::new(ErrorKind::Device, code, detail)
}

/// A device description is inconsistent.
#[must_use]
pub fn invalid_capabilities(what: &str) -> Error {
    input(
        INVALID_CAPABILITIES,
        format!("the device description is invalid: {what}"),
    )
}

/// A configuration names an invalid channel set.
#[must_use]
pub fn invalid_channel(detail: String) -> Error {
    input(INVALID_CHANNEL, detail)
}

/// The sample rate is not supported; `allowed` describes what is.
#[must_use]
pub fn unsupported_sample_rate(requested: u64, allowed: &str, nearest: Option<u64>) -> Error {
    let hint = nearest.map_or_else(String::new, |n| {
        format!("; the nearest supported rate is {n} Hz")
    });
    input(
        UNSUPPORTED_SAMPLE_RATE,
        format!("sample rate {requested} Hz is not supported; supported: {allowed}{hint}"),
    )
}

/// The trigger is not supported or misconfigured.
#[must_use]
pub fn unsupported_trigger(detail: String) -> Error {
    input(UNSUPPORTED_TRIGGER, detail)
}

/// The capture length is invalid for this device.
#[must_use]
pub fn invalid_capture_length(detail: String) -> Error {
    input(INVALID_CAPTURE_LENGTH, detail)
}

/// The worst-case storage cost of the capture exceeds the memory budget.
#[must_use]
pub fn memory_budget(detail: String) -> Error {
    Error::new(ErrorKind::Resource, MEMORY_BUDGET, detail)
}

/// No such device.
#[must_use]
pub fn not_found(id: &str) -> Error {
    input(
        NOT_FOUND,
        format!("no device with id `{id}` is available; refresh the device list"),
    )
}

/// The device already runs an acquisition.
#[must_use]
pub fn busy(id: &str) -> Error {
    input(
        BUSY,
        format!("device `{id}` already runs an acquisition; stop it first"),
    )
}

/// The operation is not allowed in the current state.
#[must_use]
pub fn invalid_state(detail: String) -> Error {
    input(INVALID_STATE, detail)
}

/// The device was unplugged or went away. `detail` says what happened and what the user
/// can do (for example "reconnect the device and start again").
#[must_use]
pub fn disconnected(detail: impl Into<String>) -> Error {
    device(DISCONNECTED, detail.into())
}

/// The transport to the device failed.
#[must_use]
pub fn transport(detail: impl Into<String>) -> Error {
    device(TRANSPORT, detail.into())
}

/// The driver reported an error.
#[must_use]
pub fn driver_error(detail: impl Into<String>) -> Error {
    device(DRIVER_ERROR, detail.into())
}

/// The driver panicked; `detail` carries the panic message and location.
#[must_use]
pub fn driver_panic(detail: String) -> Error {
    device(DRIVER_PANIC, detail)
}

/// The driver stopped responding; `detail` says for how long.
#[must_use]
pub fn unresponsive(detail: String) -> Error {
    device(UNRESPONSIVE, detail)
}

/// The trigger condition did not occur within the given number of searched samples.
#[must_use]
pub fn trigger_timeout(searched: u64) -> Error {
    device(
        TRIGGER_TIMEOUT,
        format!(
            "the trigger condition did not occur within {searched} samples; check the signal and the trigger settings"
        ),
    )
}
