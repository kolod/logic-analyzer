//! The embedded firmware image and its download into the board.
//!
//! The board has no program memory of its own: the image is written into the RAM of the 8051 core
//! of the FX2 chip through the vendor request `0xA0` (the "A0 loader" every FX2 has in ROM), with
//! the core held in reset, and the core is then released. The board leaves the bus and comes back
//! running the firmware. See `specs/013-driver-weact/contracts/usb-protocol.md`.

use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;

use crate::error;
use crate::protocol::{CPUCS, DOWNLOAD_CHUNK, DOWNLOAD_TIMEOUT, REQUEST_LOADER};
use crate::usb::UsbLink;

/// The firmware image that ships with the application (see `firmware/SOURCE.md`).
pub const EMBEDDED: &[u8] = include_bytes!("../firmware/fx2lafw-sigrok-fx2-8ch.fw");

/// The largest image the program memory of the chip holds, in bytes.
pub const MAX_IMAGE: usize = 16_384;

/// A firmware image: the bytes written from address 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    bytes: Vec<u8>,
}

impl Image {
    /// An image of these bytes (tests use it for images of other sizes).
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// The image that ships with the application.
    #[must_use]
    pub fn embedded() -> Self {
        Self::new(EMBEDDED.to_vec())
    }

    /// The bytes of the image.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The image if it can be used.
    ///
    /// # Errors
    ///
    /// `weact.firmware_invalid` for an empty image and for one larger than the chip holds.
    pub fn checked(&self) -> Result<&Self, Error> {
        if self.bytes.is_empty() {
            return Err(error::firmware_invalid("the firmware image is empty"));
        }
        if self.bytes.len() > MAX_IMAGE {
            return Err(error::firmware_invalid(format!(
                "the firmware image is {} bytes; the chip holds at most {MAX_IMAGE}",
                self.bytes.len()
            )));
        }
        Ok(self)
    }
}

/// The `(address, bytes)` of every chunk of the download, in order: at most [`DOWNLOAD_CHUNK`]
/// bytes each, from address 0, without gaps.
pub(crate) fn chunks(image: &Image) -> impl Iterator<Item = (u16, &[u8])> {
    image
        .bytes
        .chunks(DOWNLOAD_CHUNK)
        .enumerate()
        .map(|(index, part)| {
            let address = index
                .saturating_mul(DOWNLOAD_CHUNK)
                .try_into()
                .unwrap_or(u16::MAX);
            (address, part)
        })
}

/// Writes `image` into the board: hold the core in reset, write the chunks, release the core.
///
/// The release makes the board leave the bus, so an error of that last request that says the
/// board is gone is the expected end of the download, not a failure.
///
/// # Errors
///
/// `weact.firmware_invalid` before anything is sent, or the typed error of a request.
pub(crate) fn upload(link: &mut dyn UsbLink, image: &Image) -> Result<(), Error> {
    image.checked()?;
    let timeout: Duration = DOWNLOAD_TIMEOUT;
    link.control_out(REQUEST_LOADER, CPUCS, 0, &[0x01], timeout)?;
    for (address, part) in chunks(image) {
        link.control_out(REQUEST_LOADER, address, 0, part, timeout)?;
    }
    match link.control_out(REQUEST_LOADER, CPUCS, 0, &[0x00], timeout) {
        Err(error) if error.code() == device_error::DISCONNECTED => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(length: usize) -> Image {
        Image::new(vec![0xAA; length])
    }

    #[test]
    fn the_size_check_accepts_one_to_the_size_of_the_chip() {
        for length in [1, 4_096, 4_097, 8_120, 16_384] {
            assert!(image(length).checked().is_ok(), "{length}");
        }
        for length in [0, 16_385, 100_000] {
            assert_eq!(
                image(length).checked().unwrap_err().code(),
                "weact.firmware_invalid",
                "{length}"
            );
        }
    }

    #[test]
    fn the_chunks_cover_the_image_from_zero_without_gaps() {
        for length in [1_usize, 4_095, 4_096, 4_097, 8_120, 16_384] {
            let image = Image::new((0..length).map(|i| (i % 251) as u8).collect());
            let mut next = 0_usize;
            let mut joined = Vec::new();
            for (address, part) in chunks(&image) {
                assert_eq!(usize::from(address), next, "length {length}");
                assert!(!part.is_empty() && part.len() <= DOWNLOAD_CHUNK);
                next += part.len();
                joined.extend_from_slice(part);
            }
            assert_eq!(joined, image.bytes(), "length {length}");
        }
        assert_eq!(chunks(&image(0)).count(), 0);
    }
}
