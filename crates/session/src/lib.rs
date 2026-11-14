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

mod capture_codec;
mod control;
mod crc;
pub mod error;
#[cfg(any(test, feature = "test-support"))]
mod fault;
mod format;
mod fs;
mod limits;
mod meta;
mod read;
mod session;
mod varint;
mod write;

pub use control::{Control, OperationState, Progress};
pub use format::SectionTag;
pub use limits::LoadLimits;
pub use read::load_blocking;
pub use session::{Section, Session, SessionData};
pub use write::{SaveReport, save_blocking};

/// Test support (feature `test-support`; not part of the stable API): the file-system seam, a
/// fault-injecting implementation and the entry points that take a file system.
#[cfg(feature = "test-support")]
pub mod harness {
    pub use crate::fault::{FaultFs, FaultPlan, Op};
    pub use crate::fs::{DirEntryInfo, FileSystem, PathKind, RealFileSystem, WriteSink};
    pub use crate::read::load_with;
    pub use crate::write::save_with;
}
