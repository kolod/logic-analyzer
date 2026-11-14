# Data Model: Device Abstraction and Virtual Device

All types live in the new crate `la-devices` unless noted. Nothing is persisted (session
file format is roadmap item 003). Field names are indicative; the contracts fix the API.

## Identity and description

### DeviceId

An opaque, hashable string `"<provider>:<unique>"` (for example `virtual:0`,
`usb:0925:3881:SN123`). Equal across listings and reconnects of the same device.

### Capabilities

| Field | Type | Validation (checked when a device is listed) |
|-------|------|----------------------------------------------|
| `channels` | `u8` | `1..=32` |
| `sample_rates` | `SampleRates` | non-empty list, or a range with `min >= 1` and `min <= max` |
| `triggers` | set of `TriggerKind` | may be empty (no trigger support) |
| `buffer_depth` | `Option<u64>` | `None` = streaming; `Some(n)` requires `n >= 1` |

`SampleRates` is `List(Vec<u64>)` (strictly increasing, each > 0) or
`Range { min: u64, max: u64 }`. A device that violates this is not listed
(`device.invalid_capabilities`, logged).

`TriggerKind`: `Level(high|low)`, `RisingEdge`, `FallingEdge`, `AnyEdge`.

### DeviceDescriptor

`id: DeviceId`, `name: String` (non-empty), `simulated: bool`, `capabilities: Capabilities`.

## Configuration

### DeviceConfig (what the user chooses)

| Field | Type | Meaning |
|-------|------|---------|
| `enabled_channels` | `u32` mask | at least one bit set, every bit below `capabilities.channels` |
| `sample_rate_hz` | `u64` | must be in the declared rates; never rounded silently |
| `trigger` | `Option<Trigger>` | `Trigger { channel: u8, kind: TriggerKind }`; channel enabled, kind supported |
| `capture_length` | `Option<u64>` | samples to capture; `None` = until stopped; required for buffered devices; `<= buffer_depth` |
| `options` | `DeviceOptions` | `None` or `Virtual(VirtualOptions)`; `#[non_exhaustive]` for driver-specific options |
| `accept_budget_risk` | `bool` | allow a finite length whose worst-case storage cost exceeds the budget |

### ResolvedConfig

The validated configuration the device will use: the fields above after checking, plus
the capture's channel count (`capabilities.channels`). Created only by `validate`; its
fields are private, so a device never receives an unchecked configuration.

### AppliedConfig

What the device returns from `start`: the actual `sample_rate_hz` (must equal the
resolved one for devices that do not round, otherwise start fails with
`device.driver_error`) and, for triggered captures, nothing yet (the trigger position
arrives with the outcome).

## Acquisition

### AcquisitionState

```text
Starting --driver start ok--> Running --stop requested--> Stopping --> Finished
Starting --failure---------> Failed       Running --failure--------> Failed
Running --length reached---> Finished     Stopping --failure-------> Failed
```

`Idle`/`Configured` exist at the registry level: a device is `Idle` when it has no
acquisition and is not busy; a validated config is just a `ResolvedConfig` value.
Wrong-state operations return `device.invalid_state`; a second start on a busy device
returns `device.busy`.

### Messages (driver thread to ingest thread, bounded channel)

`Samples(Vec<u32>)`, `Lost(u64)`, `Finished`, `Failed(Error)`. The driver thread also
updates a heartbeat atomic (a timestamp) after every loop iteration.

### CaptureDescription (immutable, private fields)

| Field | Meaning |
|-------|---------|
| `device_id`, `device_name` | where the data came from |
| `simulated` | `true` only for the virtual device; no setter |
| `seed` | virtual device seed, `None` for real devices |
| `requested_sample_rate_hz`, `sample_rate_hz` | requested and actual (equal unless a driver rounds) |
| `channels`, `enabled_mask` | channels in the capture and which ones are real |
| `trigger` | the configured trigger, if any |
| `samples_before_trigger` | samples the device skipped waiting for the trigger |
| `started_at` | wall-clock start (for the log and later the session file) |

### AcquisitionOutcome (set once, never changes)

| Field | Meaning |
|-------|---------|
| `end` | `Completed` (length reached), `Stopped` (user), `Failed` |
| `error` | `Some(Error)` for `Failed` (stable code, actionable detail) |
| `capture_status` | final `la_storage::CaptureStatus` |
| `positions` | `capture.length()` at the end (samples stored plus lost) |
| `delivered_samples`, `lost_positions` | accounting split used by the invariant |

Invariant (tested): `delivered_samples + lost_positions == positions`, and
`lost_positions == capture_status.lost_positions`.

### AcquisitionEvent (best effort, bounded)

`Started { applied }`, `GapRecorded { position, length }`, `Finished`, `Failed { code }`.

### Progress

`state: AcquisitionState`, `samples_captured: u64`, `positions: u64`, read from atomics
without locks.

## Discovery

### DeviceEvent

`Appeared(DeviceDescriptor)` or `Disappeared(DeviceId)`.

### Registry state

`known: map DeviceId -> DeviceDescriptor` (the cached list), `busy: set DeviceId`,
`unresponsive_providers: set`. The virtual device is always in `known` and never
disappears.

## Virtual device settings

### VirtualOptions

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `seed` | `u64` | 1 | any value including 0 and `u64::MAX` is valid |
| `source` | `Option<Arc<dyn SignalSourceFactory>>` | `None` | hook for decoder generators; `None` = random generator |
| `mean_gap` | `u32` | 50 | mean samples between flips per channel (1..=1,000,000) |
| `failures` | `Vec<ScriptedFailure>` | empty | `(position, Loss{n} \| Overrun{n} \| TransportError \| Disconnect)` |
| `pacing` | `Pacing` | `Fast` | `Fast` or `Realtime` |
| `trigger_search_limit` | `u64` | 10,000,000 | samples to search before `device.trigger_timeout` |

## Relationships

```text
DeviceRegistry (1) --has--> DeviceProvider (many) --lists--> DeviceDescriptor (many)
DeviceRegistry --events--> DeviceEvent
DeviceDescriptor.capabilities --validate(DeviceConfig)--> ResolvedConfig
ResolvedConfig --start--> Acquisition (1 per busy device)
Acquisition = driver thread (Device) --Messages--> ingest thread (la_storage::Capture)
Acquisition --exposes--> Progress, events, CaptureReader, CaptureDescription, Outcome
```
