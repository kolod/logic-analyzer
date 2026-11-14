# Contract: The Device Contract (for driver authors)

This is what a hardware driver (roadmap items 011–013) implements. Everything else —
validation, threading, storage, failure handling — is done by the core.

## Traits (signatures only)

```rust
/// A family of devices (for example "the Logic16 clone over USB").
pub trait DeviceProvider: Send + Sync {
    /// Short unique prefix of the DeviceIds this provider creates, e.g. "usb-logic16".
    fn name(&self) -> &str;
    /// Lists the devices that are connected now. Must return promptly; the registry runs
    /// it on a contained worker with a timeout and omits the provider if it is late.
    fn scan(&self) -> la_core::Result<Vec<DeviceDescriptor>>;
    /// Creates a fresh driver instance for one acquisition. Does not touch the hardware
    /// beyond what is needed to claim it.
    fn open(&self, id: &DeviceId) -> la_core::Result<Box<dyn Device>>;
    /// Optional: push hot-plug events. The default does nothing (scans are reconciled).
    fn watch(&self, sink: DeviceEventSink) {}
}

/// One open device, used by exactly one acquisition on its driver thread.
pub trait Device: Send {
    fn descriptor(&self) -> &DeviceDescriptor;
    /// Starts capturing with the validated configuration. Returns the configuration
    /// actually in effect.
    fn start(&mut self, config: &ResolvedConfig) -> la_core::Result<AppliedConfig>;
    /// Fills `out` (cleared by the caller) with up to `max_samples` words and says what
    /// happened. MUST return within `READ_TIMEOUT` (50 ms): return `Idle` when there is
    /// nothing yet.
    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> la_core::Result<ReadOutcome>;
    /// Stops capturing and releases the hardware. Called once, also after a failure of
    /// `read` where possible.
    fn stop(&mut self) -> la_core::Result<()>;
}

pub enum ReadOutcome {
    /// `out` holds the next samples (at least one).
    Samples,
    /// `n` samples were lost at this point (buffer overrun). `out` is empty.
    Lost(u64),
    /// Nothing available within the timeout; the device is alive.
    Idle,
    /// A finite capture is complete.
    Finished,
}

/// Seam for USB/serial access, scriptable by the test harness.
pub trait Transport: Send {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> la_core::Result<usize>;
    fn write(&mut self, data: &[u8], timeout: Duration) -> la_core::Result<usize>;
}
```

## Rules a driver must follow

1. **Words**: bit `i` of a sample word is channel `i`; bits of disabled channels are zero.
2. **Bounded calls**: `read` returns within 50 ms. `start` and `stop` should return within
   2 s. A call that takes longer is treated as a hang (see below).
3. **Losses are reported, never hidden**: if the device lost `n` samples, return
   `Lost(n)` at the position where they were lost. A short read that is not a loss is
   not an error: return the samples you have.
4. **Errors are typed**: return `la_core::Error` with kind `Device` and one of the codes
   `device.disconnected`, `device.transport`, `device.driver_error`; put what happened
   and what the user can do (replug, check cable, close the other program using the
   device) in `detail` (English); never put signal data in an error.
5. **No panics on device data**: the workspace lint policy applies. A panic is contained,
   but it ends the acquisition.
6. **No global state**: instances are independent; two devices of the same family must be
   usable at once.

## What the core guarantees

- `start` is only called with a `ResolvedConfig`, i.e. a configuration that has already
  been checked against `descriptor().capabilities`; the driver does not re-validate the
  basics (channels, rates, trigger kind, length).
- All driver calls happen on one dedicated thread, one at a time.
- A driver panic, error or hang cannot affect the application, other devices or the
  stored data: the capture keeps everything delivered before the failure and is closed
  as interrupted (incomplete).
- Order and accounting: samples and `Lost(n)` are stored in the order they were returned;
  positions stored plus lost equal what the driver accounted for.

## Failure handling matrix

| What happens | Core behaviour | Capture status | Error code |
|--------------|----------------|----------------|------------|
| `read` returns `Lost(n)` | gap of `n` recorded, acquisition continues | incomplete (`lost_positions += n`) | none (event `GapRecorded`) |
| `Err(device.disconnected)` | acquisition ends, device marked gone | closed, interrupted | `device.disconnected` |
| `Err(device.transport)` / `driver_error` | acquisition ends | closed, interrupted | as returned |
| driver panics | contained, acquisition ends | closed, interrupted | `device.driver_panic` |
| no heartbeat for the stall timeout | driver thread abandoned, acquisition ends | closed, interrupted | `device.unresponsive` |
| storage memory budget reached | stop requested, acquisition ends | closed, stopped by limit | `storage.memory_limit` |
| user stop | `Device::stop` called, acquisition ends | closed, complete if nothing was lost | none |
| finite length reached | `Finished`, acquisition ends | closed, complete if nothing was lost | none |
| store slower than the device | excess recorded as `Lost`, never blocks the device | incomplete | none (event `GapRecorded`) |

## Timing guarantees (defaults; configurable per acquisition)

| Quantity | Default | Meaning |
|----------|---------|---------|
| `READ_TIMEOUT` | 50 ms | maximum time of one `read` |
| stop latency | `READ_TIMEOUT` + time of `Device::stop` | from `stop()` to a closed capture, for a well-behaved driver |
| stall timeout | 2 s | no heartbeat for this long = `device.unresponsive` |
| UI-facing calls | under 10 ms | list, configure, start, stop, progress never wait for a device |
