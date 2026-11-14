//! The shared typed error model.
//!
//! Every fallible operation in the workspace returns [`Result`]. An [`Error`] never
//! carries text meant for the end user: the user-facing message is derived from
//! [`Error::kind`] and [`Error::code`] by the UI layer so it can be localized. The
//! [`Error::detail`] is an English technical description meant for the log only.

use std::error::Error as StdError;

/// Category of a failure. Used for coarse handling decisions and user-message lookup.
///
/// New variants are added as later features need them, so matches must have a
/// wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Operating system I/O failed (files, ports).
    Io,
    /// Input supplied from outside (user, script, caller) is not acceptable.
    InvalidInput,
    /// Stored or received data is malformed, truncated or inconsistent.
    Corrupt,
    /// A capture device failed or misbehaved.
    Device,
    /// The operation was cancelled by the user or the application.
    Cancelled,
    /// A resource limit (memory, disk, handles) was reached.
    Resource,
    /// A bug or a state that should be impossible; never caused by external input alone.
    Internal,
}

/// A typed failure with a stable code, an English technical detail and an optional cause.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct Error {
    kind: ErrorKind,
    code: &'static str,
    detail: String,
    #[source]
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    /// Creates an error.
    ///
    /// `code` is a stable dotted identifier such as `io.read` that the localization
    /// layer maps to user-facing text. `detail` is English text for the log.
    #[must_use]
    pub fn new(kind: ErrorKind, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            detail: detail.into(),
            source: None,
        }
    }

    /// Attaches the underlying cause, preserving it for the log.
    #[must_use]
    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// The failure category.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The stable identifier used to look up the user-facing message.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// English technical detail for the log; not for end users.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Result alias used throughout the workspace.
pub type Result<T> = core::result::Result<T, Error>;
