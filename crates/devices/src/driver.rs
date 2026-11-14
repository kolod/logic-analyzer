//! The device contract: what a driver implements.
//!
//! Everything else (validation, threading, storage, failure handling) is done by the core.
//! See `specs/004-device-abstraction-virtual-device/contracts/device-contract.md`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use la_core::Result;

use crate::capabilities::{DeviceDescriptor, DeviceId};
use crate::config::{AppliedConfig, ResolvedConfig};

/// The longest a [`Device::read`] call may take. Return [`ReadOutcome::Idle`] when nothing is
/// available yet.
pub const READ_TIMEOUT: Duration = Duration::from_millis(50);

/// The longest [`Device::start`] and [`Device::stop`] should take.
pub const START_STOP_TIMEOUT: Duration = Duration::from_secs(2);

/// What a [`Device::read`] call found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    /// The output vector holds the next samples (at least one).
    Samples,
    /// This many samples were lost at this point (for example a buffer overrun); the output
    /// vector is empty.
    Lost(u64),
    /// Nothing was available within the timeout; the device is alive.
    Idle,
    /// A finite capture is complete.
    Finished,
}

/// One open device, used by exactly one acquisition on its own thread.
///
/// Rules a driver follows: bit `i` of a sample word is channel `i` and bits of disabled
/// channels are zero; [`read`](Self::read) returns within [`READ_TIMEOUT`]; losses are
/// reported with [`ReadOutcome::Lost`], never hidden; errors are typed (`device.disconnected`,
/// `device.transport`, `device.driver_error`) with an actionable English `detail`; the driver
/// never panics on device data.
pub trait Device: Send {
    /// The description of this device.
    fn descriptor(&self) -> &DeviceDescriptor;

    /// Starts capturing with an already validated configuration and returns the configuration
    /// actually in effect.
    ///
    /// # Errors
    ///
    /// A typed `device.*` error if the device cannot start.
    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig>;

    /// Fills `out` (empty on entry) with up to `max_samples` words and reports what happened.
    ///
    /// # Errors
    ///
    /// A typed `device.*` error; it ends the acquisition.
    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome>;

    /// Stops capturing and releases the hardware.
    ///
    /// # Errors
    ///
    /// A typed `device.*` error; it is logged and does not change the outcome of an
    /// acquisition that already ended.
    fn stop(&mut self) -> Result<()>;
}

/// A hot-plug event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// A device was connected.
    Appeared(DeviceDescriptor),
    /// A device was disconnected.
    Disappeared(DeviceId),
}

/// Where a provider pushes hot-plug events. Cloneable; sending never panics or blocks.
#[derive(Debug, Clone)]
pub struct DeviceEventSink {
    sender: Sender<DeviceEvent>,
}

impl DeviceEventSink {
    /// Creates a sink and the receiver of its events.
    #[must_use]
    pub fn channel() -> (Self, Receiver<DeviceEvent>) {
        let (sender, receiver) = channel();
        (Self { sender }, receiver)
    }

    /// Sends an event; returns `false` if nobody listens any more.
    pub fn send(&self, event: DeviceEvent) -> bool {
        self.sender.send(event).is_ok()
    }
}

/// A family of devices (for example "the Logic16 clone over USB").
pub trait DeviceProvider: Send + Sync {
    /// A short unique name, used as the prefix of the device ids this provider creates.
    fn name(&self) -> &str;

    /// Lists the devices that are connected now. Must return promptly; the registry runs it
    /// on a contained worker with a timeout and omits the provider if it is late.
    ///
    /// # Errors
    ///
    /// A typed `device.*` error; the provider is omitted from the list until it recovers.
    fn scan(&self) -> Result<Vec<DeviceDescriptor>>;

    /// Creates a fresh driver instance for one acquisition. Does not touch the hardware
    /// beyond what is needed to claim it.
    ///
    /// # Errors
    ///
    /// `device.not_found` or another typed `device.*` error.
    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>>;

    /// Optional: push hot-plug events through `sink`. The default does nothing; the registry
    /// reconciles periodic scans instead.
    fn watch(&self, sink: DeviceEventSink) {
        let _ = sink;
    }
}

/// A seam for USB or serial access, scriptable by the test harness. Drivers are written
/// against this trait so their protocol code can be tested without hardware.
pub trait Transport: Send {
    /// Reads up to `buf.len()` bytes, waiting at most `timeout`. Returns the number of bytes
    /// read (zero means nothing arrived in time).
    ///
    /// # Errors
    ///
    /// `device.disconnected` or `device.transport`.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize>;

    /// Writes `data`, waiting at most `timeout`. Returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// `device.disconnected` or `device.transport`.
    fn write(&mut self, data: &[u8], timeout: Duration) -> Result<usize>;
}
