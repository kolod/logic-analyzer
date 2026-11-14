//! Stable error codes and constructors for the session crate.
//!
//! Every fallible operation returns [`la_core::Error`]. The codes below are stable identifiers:
//! the localization layer maps them to user-facing text later, and tests compare against them.
//! `detail` is English text for the log only and never contains signal data.

use la_core::{Error, ErrorKind};

/// The file is not a session file (wrong or missing magic).
pub const NOT_A_SESSION: &str = "session.not_a_session";
/// The file was written by a newer, incompatible major version.
pub const UNSUPPORTED_VERSION: &str = "session.unsupported_version";
/// The file is shorter than it claims to be.
pub const TRUNCATED: &str = "session.truncated";
/// A checksum, framing or encoding rule is violated: the file is damaged.
pub const DAMAGED: &str = "session.damaged";
/// The structure is intact but the content is inconsistent.
pub const INCONSISTENT: &str = "session.inconsistent";
/// The file contains a critical section this version does not understand.
pub const UNKNOWN_CRITICAL_SECTION: &str = "session.unknown_critical_section";
/// A size, count, text or section limit is exceeded.
pub const TOO_LARGE: &str = "session.too_large";
/// The capture would not fit the memory budget.
pub const MEMORY_BUDGET: &str = "session.memory_budget";
/// The caller supplied something unusable (path, section tag, text, time).
pub const INVALID_INPUT: &str = "session.invalid_input";
/// Reading the file failed.
pub const IO_READ: &str = "session.io_read";
/// Writing the file failed.
pub const IO_WRITE: &str = "session.io_write";
/// The operation was cancelled.
pub const CANCELLED: &str = "session.cancelled";
/// A worker failed (a panic was contained).
pub const WORKER_FAILED: &str = "session.worker_failed";
/// A state that cannot be reached from external input; indicates a bug.
pub const INTERNAL: &str = "session.internal";

pub(crate) fn not_a_session(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, NOT_A_SESSION, detail)
}

pub(crate) fn unsupported_version(found: u16, supported: u16) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        UNSUPPORTED_VERSION,
        format!(
            "the file has format version {found}.x but this program supports version {supported}.x; use a newer version of the program"
        ),
    )
}

pub(crate) fn truncated(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, TRUNCATED, detail)
}

pub(crate) fn damaged(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, DAMAGED, detail)
}

pub(crate) fn inconsistent(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Corrupt, INCONSISTENT, detail)
}

pub(crate) fn unknown_critical_section(tag: &str) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        UNKNOWN_CRITICAL_SECTION,
        format!(
            "the file needs the section `{tag}`, which this version does not understand; use a newer version of the program"
        ),
    )
}

pub(crate) fn too_large(what: &str, value: u64, limit: u64) -> Error {
    Error::new(
        ErrorKind::Resource,
        TOO_LARGE,
        format!("{what} of {value} exceeds the limit of {limit}"),
    )
}

pub(crate) fn memory_budget(detail: &str) -> Error {
    Error::new(
        ErrorKind::Resource,
        MEMORY_BUDGET,
        format!(
            "the capture does not fit the memory budget ({detail}); raise the memory budget or open a smaller file"
        ),
    )
}

pub(crate) fn invalid_input(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, INVALID_INPUT, detail)
}

pub(crate) fn io_read(detail: impl Into<String>, source: std::io::Error) -> Error {
    Error::new(ErrorKind::Io, IO_READ, detail).with_source(source)
}

pub(crate) fn io_write(detail: impl Into<String>, source: std::io::Error) -> Error {
    Error::new(ErrorKind::Io, IO_WRITE, detail).with_source(source)
}

pub(crate) fn cancelled() -> Error {
    Error::new(
        ErrorKind::Cancelled,
        CANCELLED,
        "the operation was cancelled",
    )
}

pub(crate) fn worker_failed(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::Internal, WORKER_FAILED, detail)
}

pub(crate) fn internal(what: &str) -> Error {
    Error::new(
        ErrorKind::Internal,
        INTERNAL,
        format!("internal invariant violated: {what}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Error as IoError;

    #[test]
    fn every_constructor_produces_its_own_stable_code() {
        let io = || IoError::other("x");
        let cases: Vec<(Error, &str)> = vec![
            (not_a_session("x"), NOT_A_SESSION),
            (unsupported_version(2, 1), UNSUPPORTED_VERSION),
            (truncated("x"), TRUNCATED),
            (damaged("x"), DAMAGED),
            (inconsistent("x"), INCONSISTENT),
            (unknown_critical_section("ABCD"), UNKNOWN_CRITICAL_SECTION),
            (too_large("a size", 5, 4), TOO_LARGE),
            (memory_budget("needs 10, budget 5"), MEMORY_BUDGET),
            (invalid_input("x"), INVALID_INPUT),
            (io_read("x", io()), IO_READ),
            (io_write("x", io()), IO_WRITE),
            (cancelled(), CANCELLED),
            (worker_failed("x"), WORKER_FAILED),
            (internal("x"), INTERNAL),
        ];
        let mut seen = HashSet::new();
        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert!(code.starts_with("session."), "{code}");
            assert!(seen.insert(code), "{code} is not unique");
        }
    }

    #[test]
    fn details_are_actionable_and_name_the_numbers() {
        assert!(unsupported_version(2, 1).detail().contains("2.x"));
        assert!(memory_budget("needs 10").detail().contains("memory budget"));
        assert!(too_large("a size", 5, 4).detail().contains("limit of 4"));
        assert!(unknown_critical_section("ABCD").detail().contains("ABCD"));
    }
}
