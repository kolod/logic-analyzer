//! The tables the board reports and the mode of a channel set.

use la_proto::DeviceInfo;
use la_proto::info::{Features, TriggerSupport};

/// What the board knows about itself: its limits, as numbers. The values that depend on the hardware (the
/// highest rates, the buffers, the samples before and after the trigger by mode) are measured on the board
/// (task T048); until then they are the values the board crate puts here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tables {
    /// The name in the identity line.
    pub board: &'static str,
    /// The firmware version.
    pub version: (u8, u8, u8),
    /// The highest sample rate in hertz.
    pub frequency: u32,
    /// The highest rate of blast in hertz.
    pub blast_frequency: u32,
    /// The buffer of the internal memory, in samples.
    pub buffer: u32,
    /// The size of the external memory in bytes, 0 for none.
    pub psram_bytes: u64,
    /// The highest rate at which 32 channels can go to the external memory.
    pub psram_rate: u32,
    /// The most samples before the trigger, by mode.
    pub max_before: [u32; 4],
    /// The most samples after the trigger, by mode.
    pub max_after: [u32; 4],
    /// The triggers.
    pub triggers: TriggerSupport,
    /// Whether the compact coding is available.
    pub compact: bool,
}

/// The information reply of a board with these tables: 32 channels, capture request 2 and mode 3.
#[must_use]
pub fn device_info(tables: &Tables) -> DeviceInfo {
    DeviceInfo {
        frequency: tables.frequency,
        blast_frequency: tables.blast_frequency,
        buffer: tables.buffer,
        channels: 32,
        firmware: Some(tables.version),
        proto: 2,
        features: Features {
            cmd7: true,
            compact: tables.compact,
            psram: tables.psram_bytes > 0,
            mode32: true,
            restart: true,
        },
        psram_bytes: tables.psram_bytes,
        psram_rate: tables.psram_rate,
        max_before: tables.max_before,
        max_after: tables.max_after,
        triggers: tables.triggers,
    }
}

/// The capture mode of a set of channels (bit `n` is channel `n`): 0 up to channel 7, 1 up to 15, 2 up to 23,
/// 3 above. An empty set is mode 0.
#[must_use]
pub fn mode_for_mask(mask: u32) -> u8 {
    let highest = 31_u32.saturating_sub(mask.leading_zeros());
    if mask == 0 {
        return 0;
    }
    u8::try_from(highest.checked_div(8).unwrap_or(0)).unwrap_or(3)
}
