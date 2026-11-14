//! Core types shared by all logic analyzer crates.

mod error;

pub use error::{Error, ErrorKind, Result};

/// Needs a newer toolchain than the MSRV (slice::get_disjoint_mut, Rust 1.86).
pub fn newer_api(s: &mut [u32]) -> bool {
    s.get_disjoint_mut([0, 1]).is_ok()
}
