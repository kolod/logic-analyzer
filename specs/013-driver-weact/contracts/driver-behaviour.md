# Contract: The Device Contract Applied to the WeAct Driver

How `la-driver-weact` meets the rules of `specs/004-device-abstraction-virtual-device/contracts/device-contract.md`
for this board. The traits and the core's guarantees are unchanged.

## Provider

| Call | Behaviour | Time |
|------|-----------|------|
| `name()` | `"weact"` | – |
| `scan()` | lists the bus, updates the table of boards, hands blank boards to the preparation worker, returns the descriptors of the boards in the table (every state except `Absent`); never uploads, never waits for a transfer; a bus that cannot be read gives `Ok` with the boards already known and a log line, or a typed `device.transport` error when there is nothing known | under 200 ms (the registry allows 1 s) |
| `open(id)` | `device.not_found` for an identity that is not a board of this driver or not in the table; for a board in `Blank`/`Preparing`: `weact.preparing`; in `Failed`: the stored error (then back to `Blank`); in `Ready`: opens the USB device and claims interface 0 (`weact.access_denied`, `weact.driver_missing`, `weact.in_use`, `device.disconnected` as the open error says), returns a `WeActDevice` | under 500 ms |
| `watch(sink)` | not used: the registry reconciles scans every 500 ms (a hot-plug is seen within one scan interval); a later version may push events | – |

Identity and listing are those of the data model; the registry never sees a second entry for one
board and never sees a board leave during its preparation.

## Device

| Call | Behaviour |
|------|-----------|
| `descriptor()` | the descriptor of the data model |
| `flow()` | default `Flow::Realtime` (the board cannot wait) |
| `start(config)` | within 2 s: drain stale data (≤ 500 ms), create the buffers, **submit the IN transfers**, send `CMD_START` with the payload of the rate; returns `AppliedConfig::new(rate)` (every listed rate is exact). Errors: `device.disconnected`, `device.transport`, `weact.in_use` |
| `read(max, out)` | within 50 ms (waits at most 40 ms): fills `out` with up to `max` words and returns `Samples`; `Idle` when nothing came; `Finished` after the last sample of a set length; on silence beyond the limit: `Lost(requested - received)` once (set length), then the error of the next call |
| `stop()` | cancels the transfers, waits at most 500 ms for them to return, releases the interface and closes the link; sends no command; called once, also after a failed `read` |

## Silence limits

| What | Limit | Result |
|------|-------|--------|
| no data at all after `CMD_START` | 2 s | `weact.no_data` |
| data came, then silence | the larger of 2 s and four transfer periods | `weact.stream_stopped` |

A transfer period is `buffer size / rate`. At the lowest rate (20 kHz, 512-byte buffers) it is 25.6 ms,
so the 2 s floor governs; at 24 MS/s it is about 10 ms, so the floor governs again. The floor exists
because the host's wait can be late on a busy machine; the limits are constants of the driver and are
tested through the seam with a controllable clock.

## Accounting

The core's guarantee holds because the driver reports exactly what it saw: samples as `Samples`, the
positions never captured (set length only) as one `Lost`, in order. It never invents a gap in the middle
(the firmware does not give one) and never returns `Finished` for a capture that did not reach its length.

## Failure matrix of this driver

| What happens | Driver result | Core outcome (from the contract) |
|--------------|---------------|-----------------------------------|
| board unplugged during a capture | `Err(device.disconnected)` from `read` | closed, interrupted, device gone |
| a transfer fails with an error status | `Err(device.transport)` | closed, interrupted |
| the stream stops (the board's firmware ended the capture) | `Lost(rest)` then `Err(weact.stream_stopped)` | gap recorded, closed, interrupted |
| nothing at all arrives | `Err(weact.no_data)` | closed, interrupted |
| the interface is claimed elsewhere | `Err(weact.in_use)` from `open` or `start` | no capture |
| permission refused / no WinUSB | `Err(weact.access_denied)` / `Err(weact.driver_missing)` from `open` | no capture |
| the board does not return after the download | stored, handed to `open`: `weact.not_renumerated` | no capture |
| firmware answers another major version | stored: `weact.firmware_rejected` | no capture |
| a panic or a hang in the driver | contained by the supervisor (`device.driver_panic`, `device.unresponsive`) | closed, interrupted |
| the user stops | `stop()` returns, the capture closes complete if nothing was lost | closed, complete |
| the set length is reached | `Finished` | closed, complete |

## Logging

At `info`: a board appearing and disappearing (its key), each state change of preparation with the
elapsed time, the firmware version read, a capture start (rate, enabled mask), a capture end with the
counts received and lost. At `warn`/`error`: every failure with its code and the USB error kind. Never
sample data. The driver writes through `tracing`; nothing is printed.

## Safety

No `unsafe`; no `unwrap`, `expect`, indexing or unchecked arithmetic (workspace lints). Every length read
from the device is checked before use: a control answer of the wrong length is `device.transport` with
the lengths in the detail; a bulk completion longer than its buffer cannot occur and is treated as an
error. Memory: the buffers are allocated once per acquisition (at most 32 of at most about 240 KB at
24 MS/s, about 8 MB) and reused.
