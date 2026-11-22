# Data Model: 32-Channel RP2350B Logic Analyzer

The entities of the spec as the plan makes them. Contracts: [protocol](contracts/protocol.md),
[device abstraction](contracts/device-abstraction.md), [firmware](contracts/firmware.md).

## ChannelSet

A set of channel numbers below `MAX_CHANNELS` (160), used only through its methods; inside, a fixed bitset
whose words are computed from the constant (word `k`, bit `i` is channel `32k + i`). Value type, `Copy`. Invariants: no bit at or above the capture's channel count after validation. Replaces the `u32` masks of
the configuration, the description of a capture and the presentation. Written to a session file as the
first word where it was a `u32` (and the following words only when the capture has more than 32 channels).

## Sample

A sample is `words_per_sample` words (R1). The driver and the supervisor move `n × words_per_sample` words per
`n` samples; storage turns them into per-channel transitions. For a capture of one word per sample it is the
present `u32`.

## Capture mode

`0..=3`: the number of bytes (1, 2, 4, 4) each sample takes on the link in the original's transfer, chosen by
the highest channel used. Only the original's command 1 depends on it for the layout; request 2 sends only the
selected channels.

## Capture request (command 7)

The 58 bytes of `contracts/protocol.md`: trigger (type, channel, flag or length, value), the channel list (at
most 32), the mode, the rate, the samples before and after, bursts, the burst-time flag, the role in a chain,
the encoding and the memory. Validated twice: by the core against `Capabilities` before the driver sees it, and
by the firmware against the limits it reports. A field that fails names itself.

## Device information

The lines of `contracts/protocol.md`: identity, `FW`, `PROTO`, `FEATURES`, the rates, the buffer of the internal
and of the external memory, the limits before and after by mode, the triggers. The driver turns it into
`Capabilities` (channels 32, `max_rates`, `trigger_limits`, `pre_post`, `buffer_depths`, `bursts`).

## Firmware version

`major.minor.patch` and the protocol level. The driver holds the list it supports; a version outside it is
`rp2350b.firmware_unsupported` with the version found.

## Board and chain

A **board** is one device on one serial port, identified by the chip's serial number. A **chain** is an ordered
list of boards (the first is the master) and a shift in samples for each board after the first, defined by the
user and saved in the settings; it is a device while all its boards are present. Its channels are those of its
boards in order (board 0's 32, then board 1's, …), `words_per_sample` is the number of boards (each board
gives one word per sample), and a missing board makes the chain unlisted with the board named.

## Memory choice

`auto`, `internal` or `external`. `auto` takes the external memory only when the rate times the bytes per
sample of the mode is below `PSRAMRATE` (R6) and the length does not fit the internal one.

## Errors (typed, with stable codes; texts in six languages)

| Code | When |
|------|------|
| `rp2350b.access_denied` | the system does not let the application open the serial port |
| `rp2350b.port_busy` | another program has the port open |
| `rp2350b.not_found` | a listed board went away before it could be opened |
| `rp2350b.firmware_unsupported` | the board's firmware or protocol level is not supported (the version found is named) |
| `rp2350b.request_refused` | the board answered `CAPTURE_ERROR` or an `ERR_` line (the name of the field or the error is in the detail) |
| `rp2350b.stream_stopped` | the capture ended before the samples promised arrived |
| `rp2350b.protocol` | a reply that does not follow the contract (a compact stream that does not add up, a wrong length) |
| `rp2350b.timeout` | no answer within the time allowed |
| `rp2350b.disconnected` | the board was unplugged |
| `rp2350b.chain_incomplete` | a board of the chain is missing (named) or does not answer |
| `rp2350b.chain_mismatch` | the boards of a chain run different firmware versions |

## State of a capture on the board

`idle → armed (CAPTURE_STARTED) → triggered → filling → done → sending → idle`; `abort` (any byte) from
`armed`, `triggered` or `filling` goes to `sending` with what was captured (a capture that was aborted before the
trigger returns no samples and says so). A request in another state than `idle` is `ERR_BUSY`.
