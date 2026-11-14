//! Device abstraction, acquisition engine and virtual device.
//!
//! Every capture device looks the same to the application: it declares its capabilities,
//! is configured through a validated configuration, and delivers samples into a
//! `la_storage::Capture` from a background acquisition. Failures of a device never cost
//! the user data that was already captured. The always-available virtual device lets every
//! feature be tried without hardware (see `specs/004-device-abstraction-virtual-device`).
//!
//! **Never log signal data.** Log only metadata such as device ids, counts and codes.

#![deny(missing_docs)]
