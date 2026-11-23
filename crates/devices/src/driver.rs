//! The device contract: what a driver implements.
//!
//! Everything else (validation, threading, storage, failure handling) is done by the core.
//! See `specs/004-device-abstraction-virtual-device/contracts/device-contract.md`.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use la_core::Result;

use crate::capabilities::{DeviceAction, DeviceDescriptor, DeviceId};
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

/// Whether a device can be made to wait for the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    /// The data arrives at the device's own pace and cannot wait (real hardware): if the store
    /// is slower, the excess positions are recorded as lost, never silently dropped.
    #[default]
    Realtime,
    /// The device produces data on demand (for example the virtual device running as fast as
    /// possible) and waits for the store, so a slow store never causes a loss.
    Throttleable,
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

    /// Whether the device can wait for the store. Asked once, after a successful
    /// [`start`](Self::start). The default is [`Flow::Realtime`]: real hardware cannot wait.
    fn flow(&self) -> Flow {
        Flow::Realtime
    }

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
#[allow(
    clippy::large_enum_variant,
    reason = "events are rare and short-lived; boxing the descriptor would change every provider for no gain"
)]
pub enum DeviceEvent {
    /// A device was connected.
    Appeared(DeviceDescriptor),
    /// A device was disconnected.
    Disappeared(DeviceId),
}

/// The function a sink calls for every event; it returns `false` if nobody listens any more.
type SinkFn = dyn Fn(DeviceEvent) -> bool + Send + Sync;

/// Where a provider pushes hot-plug events. Cloneable; sending never panics or blocks.
#[derive(Clone)]
pub struct DeviceEventSink {
    send: Arc<SinkFn>,
}

impl std::fmt::Debug for DeviceEventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceEventSink").finish_non_exhaustive()
    }
}

impl DeviceEventSink {
    /// Creates a sink that hands every event to `send` (which returns `false` once nobody
    /// listens any more). The registry builds its sinks this way.
    #[must_use]
    pub fn new(send: impl Fn(DeviceEvent) -> bool + Send + Sync + 'static) -> Self {
        Self {
            send: Arc::new(send),
        }
    }

    /// Creates a sink and the receiver of its events.
    #[must_use]
    pub fn channel() -> (Self, Receiver<DeviceEvent>) {
        let (sender, receiver) = channel();
        (Self::new(move |event| sender.send(event).is_ok()), receiver)
    }

    /// Sends an event; returns `false` if nobody listens any more.
    pub fn send(&self, event: DeviceEvent) -> bool {
        (self.send)(event)
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

    /// Does `action` to the device `id`, which is not capturing. Only called for an action the device declares
    /// in its capabilities. The default refuses: a provider that declares an action implements this.
    ///
    /// # Errors
    ///
    /// A typed `device.*` error.
    fn perform(&self, id: &DeviceId, action: DeviceAction) -> Result<()> {
        let _ = action;
        Err(crate::error::invalid_state(format!(
            "the device `{id}` has no such action"
        )))
    }

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
