//! Stable error codes and constructors of the host.
//!
//! Every fallible host operation returns [`la_core::Error`]. The codes below are stable
//! identifiers: the localization layer maps them to the user-facing text (`errors.ftl`), and tests
//! compare against them. `detail` is English text for the log only and never contains signal
//! data or user text.

use la_core::{Error, ErrorKind};

/// The settings file exists but cannot be read; the defaults are used.
pub const SETTINGS_UNREADABLE: &str = "host.settings_unreadable";
/// The settings file is not valid or has an unsupported version; the defaults are used.
pub const SETTINGS_INVALID: &str = "host.settings_invalid";
/// The settings file cannot be saved.
pub const SETTINGS_WRITE: &str = "host.settings_write";
/// The `VIEW` section of an opened file is invalid; the capture opens with the default
/// presentation.
pub const VIEW_INVALID: &str = "host.view_invalid";
/// The `LAYT` section of an opened file is invalid; the capture opens with automatic heights.
pub const LAYOUT_INVALID: &str = "host.layout_invalid";
/// The frame plan could not be built; the view shows the "cannot draw" state.
pub const RENDER_FAILED: &str = "host.render_failed";
/// A command failed unexpectedly (a contained panic or an impossible state).
pub const INTERNAL: &str = "host.internal";
/// A command is not valid in the current state.
pub const INVALID_COMMAND: &str = "host.invalid_command";

/// Every code of this module, for the completeness checks.
pub const ALL_CODES: [&str; 8] = [
    SETTINGS_UNREADABLE,
    SETTINGS_INVALID,
    SETTINGS_WRITE,
    VIEW_INVALID,
    LAYOUT_INVALID,
    RENDER_FAILED,
    INTERNAL,
    INVALID_COMMAND,
];

/// The settings file cannot be read.
#[must_use]
pub fn settings_unreadable(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Io, SETTINGS_UNREADABLE, detail)
}

/// The settings file is damaged, too large or from an unsupported version.
#[must_use]
pub fn settings_invalid(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, SETTINGS_INVALID, detail)
}

/// The settings file cannot be written.
#[must_use]
pub fn settings_write(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Io, SETTINGS_WRITE, detail)
}

/// The `VIEW` section of a session file is invalid.
#[must_use]
pub fn view_invalid(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, VIEW_INVALID, detail)
}

/// The `LAYT` section of a session file is invalid.
#[must_use]
pub fn layout_invalid(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, LAYOUT_INVALID, detail)
}

/// The frame plan could not be built.
#[must_use]
pub fn render_failed(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Internal, RENDER_FAILED, detail)
}

/// A command failed unexpectedly.
#[must_use]
pub fn internal(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Internal, INTERNAL, detail)
}

/// A command is not valid in the current state.
#[must_use]
pub fn invalid_command(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, INVALID_COMMAND, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_and_unique() {
        let mut codes = ALL_CODES.to_vec();
        assert!(codes.iter().all(|c| c.starts_with("host.")));
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 8);
        assert_eq!(SETTINGS_WRITE, "host.settings_write");
        assert_eq!(INVALID_COMMAND, "host.invalid_command");
    }

    #[test]
    fn constructors_use_their_codes_and_kinds() {
        assert_eq!(settings_unreadable("x").code(), SETTINGS_UNREADABLE);
        assert_eq!(settings_unreadable("x").kind(), ErrorKind::Io);
        assert_eq!(settings_invalid("x").kind(), ErrorKind::Corrupt);
        assert_eq!(settings_write("x").kind(), ErrorKind::Io);
        assert_eq!(view_invalid("x").code(), VIEW_INVALID);
        assert_eq!(layout_invalid("x").code(), LAYOUT_INVALID);
        assert_eq!(render_failed("x").kind(), ErrorKind::Internal);
        assert_eq!(internal("x").code(), INTERNAL);
        assert_eq!(invalid_command("x").kind(), ErrorKind::InvalidInput);
    }
}
