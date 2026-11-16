//! Format constants and the limits a loader enforces.

use la_core::Result;
use la_storage::Limits;

use crate::error;

/// The major version this crate reads and writes.
pub const FORMAT_MAJOR: u16 = 1;
/// The minor version this crate writes.
pub const FORMAT_MINOR: u16 = 2;
/// Size of the file header in bytes.
pub const HEADER_LEN: usize = 32;
/// Size of the file footer in bytes.
pub const FOOTER_LEN: usize = 16;
/// Size of a section's frame head (tag, flags, payload length) in bytes.
pub const SECTION_HEAD_LEN: usize = 16;
/// Size of a section's checksum in bytes.
pub const SECTION_CRC_LEN: usize = 4;
/// The longest text (device id, device name) in bytes.
pub const MAX_TEXT_BYTES: usize = 4_096;
/// The largest number of sections in a file.
pub const MAX_SECTIONS: u32 = 64;
/// The size of the I/O buffer; cancellation is checked at least once per buffer.
pub const IO_BUFFER_BYTES: usize = 64 * 1024;
/// The largest `META` payload a reader accepts (two texts plus fixed fields).
pub const MAX_META_BYTES: u64 = 16 * 1024;

/// The limits of one load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadLimits {
    /// Memory budget and chunk limits of the loaded capture.
    pub storage: Limits,
    /// The largest file a load accepts, in bytes.
    pub max_file_bytes: u64,
}

impl Default for LoadLimits {
    fn default() -> Self {
        Self {
            storage: Limits::default(),
            max_file_bytes: 64 * 1024 * 1024 * 1024,
        }
    }
}

impl LoadLimits {
    /// Checks the limits.
    ///
    /// # Errors
    ///
    /// `session.invalid_input` for invalid storage limits or a zero file size limit.
    pub fn validate(&self) -> Result<()> {
        self.storage
            .validate()
            .map_err(|e| error::invalid_input(format!("invalid load limits: {}", e.detail())))?;
        if self.max_file_bytes == 0 {
            return Err(error::invalid_input(
                "max_file_bytes must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_the_text_limit_matches_the_device_layer() {
        let limits = LoadLimits::default();
        assert!(limits.validate().is_ok());
        assert_eq!(limits.max_file_bytes, 64 * 1024 * 1024 * 1024);
        assert_eq!(limits.storage, Limits::default());
        assert_eq!(MAX_TEXT_BYTES, la_devices::MAX_TEXT_BYTES);
        assert_eq!(FORMAT_MAJOR, 1);
    }

    #[test]
    fn invalid_limits_are_rejected() {
        let limits = LoadLimits {
            max_file_bytes: 0,
            ..LoadLimits::default()
        };
        assert_eq!(limits.validate().unwrap_err().code(), error::INVALID_INPUT);
        let limits = LoadLimits {
            storage: Limits {
                memory_budget_bytes: 1,
                ..Limits::default()
            },
            ..LoadLimits::default()
        };
        assert_eq!(limits.validate().unwrap_err().code(), error::INVALID_INPUT);
    }
}
