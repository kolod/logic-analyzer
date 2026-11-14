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

mod capabilities;
mod config;
mod driver;
pub mod error;
#[cfg(feature = "test-support")]
pub mod harness;
mod registry;
mod virtual_device;

pub use capabilities::{Capabilities, DeviceDescriptor, DeviceId, SampleRates, TriggerKind};
pub use config::{AppliedConfig, DeviceConfig, DeviceOptions, ResolvedConfig, Trigger};
pub use driver::{
    Device, DeviceEvent, DeviceEventSink, DeviceProvider, READ_TIMEOUT, ReadOutcome,
    START_STOP_TIMEOUT, Transport,
};
pub use registry::{DeviceRegistry, RegistryConfig};
pub use virtual_device::{
    FailureKind, PROVIDER_NAME as VIRTUAL_PROVIDER_NAME, Pacing, ScriptedFailure, SignalSource,
    SignalSourceFactory, VirtualDevice, VirtualOptions, VirtualProvider, virtual_descriptor,
};
