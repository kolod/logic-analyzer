//! The protocol of the RP2350B logic analyzer, shared by the driver and the firmware.
//!
//! The crate is `no_std` (the `std` feature only adds `std::error::Error`) and never allocates: every
//! function works on buffers the caller gives. It implements
//! `specs/017-driver-rp2350b-32ch/contracts/protocol.md`: the framing of host-to-device messages
//! ([`frame`]), capture request 2 ([`request`]), the device information reply ([`info`]) and the coding of
//! the captured samples, raw and compact ([`samples`]).
//!
//! Nothing here panics on input: the parser, the decoders and the request checks return errors.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

pub mod data;
pub mod frame;
pub mod info;
pub mod request;
pub mod samples;

pub use data::{
    DataError, DataHeader, HEADER_LEN, LossMark, MARK_LEN, MAX_LOSS_MARKS, check_marks,
};
pub use frame::{Event, FrameError, FrameParser, MAX_BODY_LEN, MAX_FRAME_LEN, encode_frame};
pub use info::{DeviceInfo, Features, InfoError};
pub use request::{
    CaptureRequest2, Encoding, Memory, REQUEST2_LEN, RequestError, RequestField, Role, TriggerType,
};
pub use samples::{
    CompactError, bytes_per_sample, decode_compact, encode_compact, pack_raw, unpack_raw,
};

/// The commands of the protocol (the first byte of a frame body).
pub mod command {
    /// Device information.
    pub const INFO: u8 = 0;
    /// The original's capture request.
    pub const CAPTURE: u8 = 1;
    /// Network settings (unsupported on this board).
    pub const NETWORK: u8 = 2;
    /// Power status (unsupported on this board).
    pub const POWER: u8 = 3;
    /// Restart into the bootloader.
    pub const BOOTLOADER: u8 = 4;
    /// Indicator on.
    pub const INDICATOR_ON: u8 = 5;
    /// Indicator off.
    pub const INDICATOR_OFF: u8 = 6;
    /// Capture request 2.
    pub const CAPTURE2: u8 = 7;
    /// Normal restart.
    pub const RESTART: u8 = 8;
}
