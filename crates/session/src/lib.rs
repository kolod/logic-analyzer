//! Session files: atomic save and safe load of a capture with its description.
//!
//! A session file is a versioned container of individually checksummed sections (see
//! `specs/003-session-file-format`). Saving never damages an existing file; loading treats
//! every byte as untrusted and returns a typed error for anything that is not a correct session.
//!
//! **Never log signal data or the content of text fields.** Log only sizes, counts, codes and
//! durations.

#![deny(missing_docs)]

pub mod error;
