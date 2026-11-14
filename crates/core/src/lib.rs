//! Core types shared by all logic analyzer crates.

mod error;

pub use error::{Error, ErrorKind, Result};

/// Deliberate violation.
pub fn bad(v: &[u32]) -> u32 {
    v[0]
}
