//! Session files: atomic save and safe load of a capture with its description.
//!
//! A session file is a versioned container of individually checksummed sections (see
//! `specs/003-session-file-format`). Saving never damages an existing file; loading treats
//! every byte as untrusted and returns a typed error for anything that is not a correct session.
//!
//! **Never log signal data or the content of text fields.** Log only sizes, counts, codes and
//! durations.

#![deny(missing_docs)]
// The building blocks are used by the writer, reader and operations that later tasks add; remove
// this attribute when the public API is complete (the last task of the feature).
#![allow(
    dead_code,
    reason = "used by the writer, reader and operations added later"
)]

mod crc;
pub mod error;
mod format;
mod fs;
mod limits;
mod meta;
mod varint;
