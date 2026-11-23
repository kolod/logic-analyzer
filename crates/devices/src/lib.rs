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

mod acquisition;
mod capabilities;
mod channel_set;
mod config;
mod description;
mod driver;
pub mod error;
#[cfg(feature = "test-support")]
pub mod harness;
mod registry;
mod supervisor;
mod virtual_device;

pub use acquisition::{
    Acquisition, AcquisitionEvent, AcquisitionOptions, AcquisitionOutcome, AcquisitionState,
    EndReason, Progress,
};
pub use capabilities::{
    Capabilities, DepthBand, DeviceDescriptor, DeviceId, ModeLimit, SampleRates, TriggerKind,
    TriggerLimits,
};
pub use channel_set::ChannelSet;
pub use config::{
    AppliedConfig, DeviceConfig, DeviceOptions, Pattern, ResolvedConfig, Trigger, validate,
};
pub use description::{CaptureDescription, MAX_TEXT_BYTES, SavedDescription};
pub use driver::{
    Device, DeviceEvent, DeviceEventSink, DeviceProvider, Flow, READ_TIMEOUT, ReadOutcome,
    START_STOP_TIMEOUT, Transport,
};
pub use registry::{DeviceRegistry, RegistryConfig};
pub use virtual_device::{
    FailureKind, PROVIDER_NAME as VIRTUAL_PROVIDER_NAME, Pacing, RandomSource, ScriptedFailure,
    SignalSource, SignalSourceFactory, VirtualDevice, VirtualOptions, VirtualProvider,
    WIDE_PROVIDER_NAME as VIRTUAL_WIDE_PROVIDER_NAME, WideVirtualProvider, virtual_descriptor,
    virtual_descriptor_with_channels,
};
