//! Core types shared by all logic analyzer crates.

mod error;

pub use error::{Error, ErrorKind, Result};

#[cfg(test)]
mod verify {
    #[test]
    fn fails() {
        assert_eq!(1, 2);
    }
}
