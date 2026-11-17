//! The wire protocol of the board: constants, the start payload and the stream decoding.
//!
//! Everything here is pure (no I/O) and written from `specs/013-driver-weact/contracts/usb-protocol.md`.
//! The functions are total: they return typed errors for any input and never panic.

use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;

/// USB vendor ID of the board (before and after the firmware is loaded).
pub const VENDOR_ID: u16 = 0x1d50;
/// USB product ID of the board.
pub const PRODUCT_ID: u16 = 0x608c;
/// The manufacturer string of a board that runs the firmware.
pub const MANUFACTURER: &str = "sigrok";
/// The product string of a board that runs the firmware.
pub const PRODUCT: &str = "fx2lafw";

/// The interface that carries the stream.
pub const INTERFACE: u8 = 0;
/// The bulk IN endpoint of the samples.
pub const ENDPOINT_IN: u8 = 0x82;

/// Vendor request of the chip's own loader: writes memory (the firmware) and the CPUCS register.
pub const REQUEST_LOADER: u8 = 0xA0;
/// The CPUCS register: bit 0 holds the 8051 in reset.
pub const CPUCS: u16 = 0xE600;
/// Vendor request: read the firmware version (2 bytes).
pub const REQUEST_GET_FW_VERSION: u8 = 0xB0;
/// Vendor request: start an acquisition (3 bytes).
pub const REQUEST_START: u8 = 0xB1;
/// Vendor request: read the chip revision (1 byte).
pub const REQUEST_GET_REVID: u8 = 0xB2;
/// Vendor request: stop the acquisition and empty the FIFO (firmware 1.5 and later, no data).
pub const REQUEST_STOP: u8 = 0xB3;
/// The firmware major version the driver works with.
pub const REQUIRED_MAJOR: u8 = 1;
/// The first minor version that has [`REQUEST_STOP`] and a START that cannot hang the board.
pub const MINOR_WITH_STOP: u8 = 5;

/// `CMD_START` flag: the 48 MHz clock (clear: 30 MHz).
const FLAG_CLOCK_48_MHZ: u8 = 0x40;
/// The firmware ignores a start command whose delay is 6 * 256 or more.
const MAX_SAMPLE_DELAY: u64 = 6 * 256;
/// The two clocks the firmware can sample with.
const CLOCK_48_HZ: u64 = 48_000_000;
const CLOCK_30_HZ: u64 = 30_000_000;

/// Timeout of the small control requests.
pub const COMMAND_TIMEOUT: Duration = Duration::from_millis(100);
/// Timeout of each chunk of the firmware download.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_millis(500);
/// The most bytes of one firmware download request.
pub const DOWNLOAD_CHUNK: usize = 4096;
/// How long one `read` waits for a completed transfer (the contract allows 50 ms).
pub const READ_WAIT: Duration = Duration::from_millis(40);
/// How long `stop` waits for the cancelled transfers.
pub const STOP_WAIT: Duration = Duration::from_millis(500);
/// The most time `start` spends draining stale data.
pub const DRAIN_LIMIT: Duration = Duration::from_millis(500);
/// No data at all within this time of the start: `weact.no_data`.
pub const FIRST_DATA_TIMEOUT: Duration = Duration::from_secs(2);
/// The least silence that ends a stream that had started: `weact.stream_stopped`.
pub const SILENCE_FLOOR: Duration = Duration::from_secs(2);
/// After the download the board needs at least this long to leave the bus.
pub const RENUMERATION_HEAD_START: Duration = Duration::from_millis(300);
/// How often the bus is polled for the board to come back.
pub const RENUMERATION_POLL: Duration = Duration::from_millis(100);
/// How long the board may take to come back before `weact.not_renumerated`.
pub const RENUMERATION_LIMIT: Duration = Duration::from_secs(5);
/// A failed board stays listed this long after it left the bus during its preparation.
pub const FAILED_LISTING: Duration = Duration::from_secs(30);

/// The sample rates the board can run, in hertz, exactly (no rounding): the rates the firmware
/// produces from its two clocks with a delay in range, up to the 24 MHz the vendor states.
pub const SAMPLE_RATES: [u64; 16] = [
    20_000, 25_000, 50_000, 100_000, 200_000, 250_000, 500_000, 1_000_000, 2_000_000, 3_000_000,
    4_000_000, 6_000_000, 8_000_000, 12_000_000, 16_000_000, 24_000_000,
];

/// The clock the firmware samples with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// 30 MHz.
    Mhz30,
    /// 48 MHz.
    Mhz48,
}

/// The clock and the sample delay for `rate`, by the rule of the firmware's driver: the 48 MHz
/// clock when it divides cleanly and the delay `48 MHz / rate - 1` is at most 1,536, else the
/// 30 MHz clock under the same two conditions; `None` for a rate no clock reaches (also 0).
#[must_use]
pub fn clock_and_delay(rate: u64) -> Option<(Clock, u16)> {
    let delay_for = |clock_hz: u64| -> Option<u16> {
        if clock_hz.checked_rem(rate)? != 0 {
            return None;
        }
        let delay = clock_hz.checked_div(rate)?.checked_sub(1)?;
        if delay > MAX_SAMPLE_DELAY {
            return None;
        }
        u16::try_from(delay).ok()
    };
    if let Some(delay) = delay_for(CLOCK_48_HZ) {
        return Some((Clock::Mhz48, delay));
    }
    delay_for(CLOCK_30_HZ).map(|delay| (Clock::Mhz30, delay))
}

/// The three bytes of `CMD_START` for `rate`: `[flags, delay_hi, delay_lo]`.
///
/// # Errors
///
/// `device.unsupported_sample_rate` for a rate that is not in [`SAMPLE_RATES`].
pub fn start_payload(rate: u64) -> Result<[u8; 3], Error> {
    if !SAMPLE_RATES.contains(&rate) {
        return Err(device_error::unsupported_sample_rate(
            rate,
            "the rates in the capabilities of the board",
            None,
        ));
    }
    let Some((clock, delay)) = clock_and_delay(rate) else {
        return Err(device_error::unsupported_sample_rate(
            rate,
            "the rates in the capabilities of the board",
            None,
        ));
    };
    let flags = match clock {
        Clock::Mhz48 => FLAG_CLOCK_48_MHZ,
        Clock::Mhz30 => 0,
    };
    let [high, low] = delay.to_be_bytes();
    Ok([flags, high, low])
}

/// The firmware version the board reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// Changes with an incompatible change; the driver requires [`REQUIRED_MAJOR`].
    pub major: u8,
    /// Changes with a compatible change; from [`MINOR_WITH_STOP`] the board has a stop command.
    pub minor: u8,
}

impl Version {
    /// Whether the firmware has the stop command and a START that resets the FIFO (the image that
    /// ships with the application does; a stock `fx2lafw` 1.4 loaded by another program does not).
    #[must_use]
    pub fn has_stop(&self) -> bool {
        self.major == REQUIRED_MAJOR && self.minor >= MINOR_WITH_STOP
    }
}

/// Reads the answer of `CMD_GET_FW_VERSION`: exactly two bytes, major then minor.
///
/// # Errors
///
/// `device.transport` for any other length.
pub fn parse_version(bytes: &[u8]) -> Result<Version, Error> {
    match bytes {
        [major, minor] => Ok(Version {
            major: *major,
            minor: *minor,
        }),
        other => Err(device_error::transport(format!(
            "the firmware version answer has {} bytes, expected 2",
            other.len()
        ))),
    }
}

/// The transfers of one capture: the size of each buffer and how many are in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferPlan {
    /// Bytes of one buffer: 10 ms of samples rounded up to a multiple of 512 (at least 512).
    pub size: usize,
    /// Buffers in flight: 500 ms of samples divided by the size, from 4 to 32.
    pub count: usize,
}

/// The buffer plan for `rate`.
///
/// # Errors
///
/// `device.unsupported_sample_rate` for a rate that is not in [`SAMPLE_RATES`].
pub fn buffer_plan(rate: u64) -> Result<BufferPlan, Error> {
    if !SAMPLE_RATES.contains(&rate) {
        return Err(device_error::unsupported_sample_rate(
            rate,
            "the rates in the capabilities of the board",
            None,
        ));
    }
    // One byte per sample: 10 ms is rate / 100 bytes and 500 ms is rate / 2 bytes.
    let ten_ms = rate.checked_div(100).unwrap_or(0);
    let size = ten_ms.div_ceil(512).max(1).checked_mul(512).unwrap_or(512);
    let count = rate
        .checked_div(2)
        .and_then(|bytes| bytes.checked_div(size))
        .unwrap_or(0)
        .clamp(4, 32);
    Ok(BufferPlan {
        size: usize::try_from(size).unwrap_or(512),
        count: usize::try_from(count).unwrap_or(4),
    })
}

/// Turns the bytes of the stream into sample words: bit `i` of a byte is channel `i`, and the
/// bits of channels that are not enabled are zero. Only the eight channels of the board exist,
/// so bits of `enabled_mask` above the eighth are ignored. The words are appended to `out`.
pub fn samples_from_bytes(bytes: &[u8], enabled_mask: u32, out: &mut Vec<u32>) {
    let mask = enabled_mask & 0xFF;
    out.extend(bytes.iter().map(|byte| u32::from(*byte) & mask));
}
