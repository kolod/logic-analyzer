//! Driver for the WeAct LogicAnalyzer V1.
//!
//! The board is an 8-channel, 24 MHz, streaming logic analyzer built on a Cypress FX2 USB
//! controller that holds no firmware of its own. This crate implements the device contract of
//! `la-devices` for it: it lists boards by their position on the USB bus, prepares a blank board
//! by loading an open-source firmware image on a background worker, and streams samples from one
//! bulk endpoint into the acquisition engine. All USB access goes through a small seam so that
//! the driver is tested without hardware.
//!
//! The contracts are in `specs/013-driver-weact/contracts/`: the wire protocol, the behaviour
//! against the device contract, and the test rig.

#![deny(missing_docs)]

mod boards;
mod device;
pub mod error;

/// The name of the provider and the prefix of every device identity of this driver.
pub const PROVIDER_NAME: &str = "weact";

mod firmware;
mod nusb_backend;
pub mod protocol;
#[cfg(feature = "test-support")]
pub mod testing;
pub mod usb;
