# Contract: Application-Facing API of `la-devices` (signatures only)

What the UI and other application code use. A non-final sketch: bodies, private helpers
and exact lifetimes are decided during implementation; changes follow semver. Every
fallible function returns `la_core::Result<T>`; no function panics on external input and
none blocks on a device.

```rust
// ---- discovery -----------------------------------------------------------------
pub struct DeviceRegistry { /* owns the monitor thread; Send + Sync */ }
impl DeviceRegistry {
    /// Built-in virtual provider is always registered.
    pub fn new(config: RegistryConfig) -> DeviceRegistry;
    pub fn add_provider(&self, provider: Arc<dyn DeviceProvider>);
    /// The cached device list: instant, never waits for a device.
    pub fn list(&self) -> Vec<DeviceDescriptor>;
    /// A new subscriber: receives every hot-plug event from now on, in order, none lost or
    /// duplicated (every call returns its own receiver, so several consumers each see all).
    pub fn events(&self) -> std::sync::mpsc::Receiver<DeviceEvent>;
    pub fn describe(&self, id: &DeviceId) -> Option<DeviceDescriptor>;
}

pub struct RegistryConfig { pub scan_interval: Duration, pub scan_timeout: Duration }
impl Default for RegistryConfig {}   // 500 ms, 1 s

// ---- configuration ---------------------------------------------------------------
pub fn validate(caps: &Capabilities, config: &DeviceConfig, limits: &la_storage::Limits)
    -> la_core::Result<ResolvedConfig>;   // pure, no device call

// ---- acquisition -----------------------------------------------------------------
pub struct AcquisitionOptions {
    pub limits: la_storage::Limits,      // the capture's memory budget etc.
    pub stall_timeout: Duration,         // default 2 s; also bounds start/stop of the driver
    pub channel_capacity: usize,         // default 8; chunks buffered between the two threads
    pub chunk_samples: usize,            // default 16_384; read size, <= limits.max_chunk_samples
}
impl Default for AcquisitionOptions {}

impl DeviceRegistry {
    /// Validates, claims the device (typed error if busy or unknown) and starts the
    /// acquisition in the background. Returns immediately.
    pub fn start(&self, id: &DeviceId, config: DeviceConfig, options: AcquisitionOptions)
        -> la_core::Result<Acquisition>;
}

pub struct Acquisition { /* handle; Send + Sync */ }
impl Acquisition {
    pub fn progress(&self) -> Progress;                       // lock-free
    pub fn description(&self) -> CaptureDescription;          // owned copy; cannot be altered
    pub fn capture(&self) -> la_storage::CaptureReader;       // read while it runs
    /// Events since the last call (best effort; the outcome is authoritative).
    pub fn drain_events(&self) -> Vec<AcquisitionEvent>;
    /// Requests a stop and returns at once.
    pub fn stop(&self);
    /// The authoritative result, once available.
    pub fn outcome(&self) -> Option<AcquisitionOutcome>;
    /// Waits at most `timeout` for the outcome (never forever).
    pub fn wait_timeout(&self, timeout: Duration) -> Option<AcquisitionOutcome>;
}

// ---- values (see data-model.md) ---------------------------------------------------
pub struct Progress { pub state: AcquisitionState, pub samples_captured: u64, pub positions: u64 }
pub enum AcquisitionState { Starting, Running, Stopping, Finished, Failed }
pub struct AcquisitionOutcome { pub end: EndReason, pub error: Option<Arc<la_core::Error>>,
    pub capture_status: la_storage::CaptureStatus, pub positions: u64,
    pub delivered_samples: u64, pub lost_positions: u64 }
pub enum EndReason { Completed, Stopped, Failed }
pub enum AcquisitionEvent { Started { applied: AppliedConfig },
    GapRecorded { position: u64, length: u64 }, Finished, Failed { code: &'static str } }
```

## Error codes

`device.invalid_capabilities`, `device.invalid_channel`, `device.unsupported_sample_rate`,
`device.unsupported_trigger`, `device.invalid_capture_length`, `device.memory_budget`,
`device.not_found`, `device.busy`, `device.invalid_state`, `device.disconnected`,
`device.transport`, `device.driver_error`, `device.driver_panic`, `device.unresponsive`,
`device.trigger_timeout`; storage errors (`storage.*`) pass through unchanged.

## Notes

- `capture()` returns the storage reader, so the same snapshot machinery as item 002 gives
  live viewing during an acquisition.
- The `CaptureDescription` is built by the acquisition from the device descriptor; there
  is no way for a consumer to create or alter one, so `simulated` cannot be cleared.
- `AppliedConfig` carries `sample_rate_hz` and `samples_before_trigger` (samples a device skipped
  waiting for its trigger); `CaptureDescription` repeats both.
- After the outcome is set, the device is released (not busy) and can be started again;
  a hung driver instance is abandoned, the next start opens a fresh instance.

## Saved descriptions (added by feature 003)

```rust
/// Every field of a CaptureDescription as plain values (for session files).
pub struct SavedDescription { pub device_id: String, pub device_name: String, pub simulated: bool,
    pub seed: Option<u64>, pub requested_sample_rate_hz: u64, pub sample_rate_hz: u64, pub channels: u8,
    pub enabled_mask: u32, pub trigger: Option<Trigger>, pub samples_before_trigger: u64,
    pub started_at: SystemTime }
impl CaptureDescription {
    pub fn to_saved(&self) -> SavedDescription;
    /// The loading path: validates every field; there is still no setter.
    pub fn restore(saved: SavedDescription) -> la_core::Result<CaptureDescription>;
}
pub const MAX_TEXT_BYTES: usize = 4_096;   // longest device id or name
```
