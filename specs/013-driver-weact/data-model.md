# Data Model: WeAct LogicAnalyzer Driver

Nothing here is stored on disk. It is the state the driver keeps in memory, the rules that govern it,
and the vocabulary of the contracts. Names are those of the Rust types to come, in prose.

## BoardKey

Where a board is on the USB bus: the bus identifier **as text** (on Windows the path of the host
controller such as `PCIROOT(0)#PCI(1400)#USBROOT(0)`, on Linux the bus number) and the chain of port
numbers from the root hub, as `nusb` reports them. It is the identity of a board: `DeviceId` is
`weact:<bus>-<p1>.<p2>…`; the text is built and compared as a whole and never split to recover the parts.

- Stable while the board stays plugged into the same port, **including across re-enumeration** (the
  device address changes, the position does not).
- Distinct for two boards at once (two ports).
- A board moved to another port is another identity.
- Built only from what the bus reports; never from the device's strings or serial number (the board
  has none).

## UsbInfo

What a scan reads without opening the board: `BoardKey`, vendor ID, product ID, manufacturer string
and product string (when the OS has them), the device address, and the bound driver's name where
`nusb` provides it.

- The board is *ours* when the IDs are `1d50:608c`; everything else on the bus is ignored and never
  opened (FR-004).
- It is *running the firmware* when the product string is `fx2lafw` and the manufacturer is `sigrok`
  or not reported (Windows never reports it); otherwise it is *blank*.

## Board (an entry of the table)

| Field | Meaning |
|-------|---------|
| `key` | the `BoardKey` |
| `state` | see below |
| `since` | when the state was entered (for the allowed times) |
| `last_seen` | when a scan last saw it on the bus |
| `failure` | the typed error to hand to `open`, if the last preparation failed |

### States and transitions

```text
          scan sees a blank board
Absent ─────────────────────────────▶ Blank ──(worker picks it up)──▶ Preparing
  ▲                                                                      │
  │ gone and nothing to report                          upload ok, board returned,
  │ (or 30 s after a failure,                           version major = 1
  │  when it also left the bus)                                          ▼
  └────────────◀────────────────────── Failed ◀──(any step fails)─── Ready ◀─── scan sees a
                                          ▲                            │       running board
                                          └──── open() reads and ──────┘
                                                clears the failure → Blank again (retry)
```

- `Preparing` lasts at most 5 s of waiting for the return (R6) plus the upload; the board is **listed**
  throughout, also while it is off the bus.
- `Ready` is the only state in which `open` succeeds. In `Blank` and `Preparing` it returns
  `weact.preparing`; in `Failed` the stored failure (then the board goes back to `Blank`).
- A board that is `Ready` and disappears from the bus is `Absent` at the next scan (a normal unplug).
- Transitions happen on the provider's scans and on the worker; the table is behind one lock that is
  never held while the bus is read or a transfer waits.

## Descriptor of a listed board

`DeviceDescriptor`: `id` = `weact:…`; `name` = "WeAct LogicAnalyzer V1" (a proper name; with the
position when two boards are present, e.g. "WeAct LogicAnalyzer V1 (2)"); `simulated` = false;
`capabilities`:

| Field | Value |
|-------|-------|
| `channels` | 8 |
| `sample_rates` | `List`: 20 k, 25 k, 50 k, 100 k, 200 k, 250 k, 500 k, 1 M, 2 M, 3 M, 4 M, 6 M, 8 M, 12 M, 16 M, 24 M Hz |
| `triggers` | none |
| `buffer_depth` | none (streaming) |

## Start payload

Three bytes `[flags, delay_hi, delay_lo]` computed from the rate by the rule of research R2:
`flags` = `0x40` for 48 MHz or `0x00` for 30 MHz (width bit and analog bit are zero), `delay` split into
high and low byte. The function is total over the 16 rates and returns an error (never a panic) for
any other value.

## Stream accounting (one acquisition)

| Quantity | Meaning |
|----------|---------|
| `requested` | the set length (`capture_length`), or none |
| `received` | samples returned by `read` so far |
| `lost_reported` | positions already returned as `Lost` |
| `last_data` | when data last arrived (for the silence limits) |
| `state` | `Idle` (before start), `Streaming`, `Finishing`, `Ended` |

Rules (each is a test):

1. `received` never exceeds `requested`; the last chunk is truncated and `Finished` follows.
2. The books balance: at the end `received + lost_reported` equals `requested`, or, without a set
   length, `received` is what the capture claims (no loss is invented).
3. A stream that stops: for a set length, one `Lost(requested - received)` then the error; the sum is
   `requested`. Without a set length, only the error.
4. Disabled channels are zero in every word; enabled channel `i` is bit `i` of the byte.
5. A completed transfer of length 0, a timed-out wait, and a transfer that ends with a recoverable
   status are *silence*, counted by time, not by number.

## Preparation worker

One per provider, a contained worker that takes `Blank` boards one at a time (a second board waits its
turn; the worker never blocks `scan`). Per board: open → read strings (confirm blank) → hold reset →
write image in chunks → release reset → wait 300 ms → poll for the board by `BoardKey` every 100 ms up to
5 s → open → read the version (`0xB0`) → require major 1 → `Ready`. Any failure sets `Failed` with the
typed error and closes the link.

## The firmware image

Embedded raw bytes (8,120 for the image of release 0.1.7), at most 16,384 bytes: the 16 KB of program memory of the chip. Run-time sanity: not empty and within that size. The checksum is
asserted by a test against `firmware/SOURCE.md`.

## Errors

The nine codes of research R10, each with a message in six languages whose text says what to do, never
containing sample data. Technical detail (the key of the board, the USB error kind, the driver name
found) goes to the log and to the `detail` in English.
