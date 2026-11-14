# Contract: Virtual Device, Signal-Source Hook and Test Harness

## Virtual device

- Provider name `virtual`; one device, `DeviceId` `virtual:0`, name "Virtual device",
  `simulated = true`, 32 channels, sample rates `Range { min: 1, max: 500_000_000 }`,
  triggers: level (high/low), rising, falling, any edge, streaming (no buffer depth).
- Always listed, never reported as disappearing, no external requirement, identical on
  Windows 10/11 and Debian 13.

### Determinism rules (tested)

1. **Same seed and configuration give identical samples**, independent of the chunk sizes
   the acquisition uses and of the pacing mode. The generator advances per channel, and all
   arithmetic is integer (SplitMix64 to derive a stream per channel from the seed, xorshift64
   for draws), so results are identical across platforms.
2. **Different seeds give different samples** (for any two seeds, on at least one enabled
   channel within the first 10,000 samples at the default `mean_gap`).
3. Seed `0` and `u64::MAX` are valid.
4. Nothing fails unless scripted: with an empty failure script the device never returns an
   error and never reports a loss.

### Default random generator

Each enabled channel keeps its level and the position of its next flip. After every flip the
distance to the next flip is `1 + draw % (2 * mean_gap - 1)` samples (`mean_gap` in
1..=1,000,000; default 50). Disabled channels are always zero. A chunk is produced by
filling constant stretches and applying one XOR per flip: cost O(samples + flips).

### Failure script

`failures: Vec<ScriptedFailure>`, sorted by position, applied as the device passes each
position (positions count samples produced after the trigger, from 0):

| Failure | Effect |
|---------|--------|
| `Loss { n }` | the next `read` returns `Lost(n)`; the generator skips `n` samples (the stream continues as if they were produced and lost) |
| `Overrun { n }` | same as `Loss`, logged as an overrun |
| `TransportError` | `read` returns `device.transport` |
| `Disconnect` | `read` returns `device.disconnected` |

### Triggers

Level or edge on one enabled channel. Before capturing, the device generates and discards
samples until the condition holds (for an edge: the first sample whose level differs
from the previous one in the requested direction; for a level: the first sample with that
level). Capturing starts at the trigger sample (capture position 0). The count of
discarded samples is `samples_before_trigger` in the capture description. If the condition
does not occur within `trigger_search_limit` samples, `start` fails with
`device.trigger_timeout`.

### Pacing

`Fast`: `read` returns as soon as a chunk is generated. `Realtime`: the device releases
samples at the configured sample rate; waiting is done in slices of at most 10 ms and the
stop flag is honoured between slices, so stop latency stays within one `READ_TIMEOUT`.

## Signal-source hook (for decoders, items 007–008)

```rust
pub trait SignalSource: Send {
    /// Writes the samples for positions `start .. start + out.len()` of the capture
    /// (bit i = channel i, disabled channels zero). Must be deterministic: the same
    /// source created with the same seed produces the same samples for any chunking.
    fn fill(&mut self, start: u64, out: &mut [u32]);
}

pub trait SignalSourceFactory: Send + Sync {
    fn create(&self, enabled_mask: u32, sample_rate_hz: u64, seed: u64) -> Box<dyn SignalSource>;
}
```

`VirtualOptions::source = Some(factory)` replaces the random generator; the acquisition,
storage and failure paths are unchanged, so data from a decoder's generator is delivered
exactly like any other and decodes back to what was generated.

## Test harness (feature `test-support`)

```rust
/// Scriptable replacement for USB/serial access; drivers are written against Transport.
pub struct RecordedTransport { /* script of steps */ }
pub enum TransportStep { Bytes(Vec<u8>), ShortRead(usize), Timeout, Disconnect, Error(String) }
impl RecordedTransport {
    pub fn new(steps: Vec<TransportStep>) -> Self;
    pub fn written(&self) -> Vec<Vec<u8>>;     // what the driver sent, for assertions
}
impl Transport for RecordedTransport {}

/// A device that replays a script; used for conformance and fault-injection tests.
pub struct ScriptedDevice { /* descriptor + steps */ }
pub enum Step { Samples(Vec<u32>), Lost(u64), Idle, Error(la_core::Error), Panic, Hang(Duration), Finish }
impl ScriptedDevice { pub fn new(descriptor: DeviceDescriptor, steps: Vec<Step>) -> Self; }

/// Records every call made to the wrapped device.
pub struct CountingDevice<D: Device> { /* ... */ }
impl<D: Device> CountingDevice<D> { pub fn calls(&self) -> CallCounts; }

/// Provider with scripted appear/disappear sequences, for hot-plug tests.
pub struct TestProvider { /* ... */ }
impl TestProvider { pub fn plug(&self, d: DeviceDescriptor); pub fn unplug(&self, id: &DeviceId); }

pub mod conformance {
    /// Runs the shared checks against any driver: descriptor sanity, capability
    /// enforcement with zero device calls, start/stop, completion, loss accounting,
    /// failure handling and panic containment.
    pub fn run_all(provider: Arc<dyn DeviceProvider>, id: &DeviceId, expectations: Expectations);
}
```

A new driver's own tests need only a `RecordedTransport` script for its protocol and one
call to `conformance::run_all`.
