# Contract: the protocol (the original's, extended)

One document for both sides: `la-proto` (the shared crate), the firmware and the driver are checked against
it. What is marked **original** was read from the gusmanb sources and wiki and is kept exactly; what is marked
**new** is this feature's. Numbers are little-endian. Nothing here is code of the original (research R11).

## Link and framing (original)

USB serial port (CDC), no baud rate meaning. A host-to-device message is a frame:

```text
55 AA  <command> <payload…>  AA 55
```

Inside the frame the bytes `AA` and `F0` are escaped: `AA` is sent as `F0 5A` and `F0` as `F0 00` (the
receiver takes the byte after `F0` and XORs it with `F0`). A frame is at most 256 bytes after unescaping;
more is `ERR_MSG_OVERFLOW` (the receiver drops the frame and goes on). While a capture runs, **any byte** from the
host aborts it.

Device-to-host replies are text lines ended by `LF`, and, after a capture, binary data (below).

## Commands

| Cmd | Name | Payload | Reply | |
|-----|------|---------|-------|---|
| 0 | device information | none (frame length 5) | text lines (below) | original |
| 1 | capture request | the original's structure (up to 24 channels) | `CAPTURE_STARTED` then data, or `CAPTURE_ERROR` | original |
| 2 | network settings | — | `ERR_UNSUPPORTED` on this board (no radio) | original |
| 3 | power status | — | `ERR_UNSUPPORTED` | original |
| 4 | restart into the bootloader | none | `RESTARTING_BOOTLOADER`, then the board restarts as a drive | original |
| 5, 6 | indicator on, off | none | none | original |
| 7 | **capture request 2** | below | `CAPTURE_STARTED` then data, or `CAPTURE_ERROR` | **new** |
| 8 | **normal restart** | none | `RESTARTING`, then the board resets and starts the analyzer again | **new** |

Commands 4 and 8 are accepted at any time, also while a capture runs (the capture ends); the reply is sent and
flushed before the board resets. A board that does not know command 8 answers `ERR_UNKNOWN_MSG`.

An unknown command is answered `ERR_UNKNOWN_MSG`. A board that does not know command 7 answers it that way, and
the driver then knows it talks to an older firmware.

## Device information reply (command 0)

Lines, each `NAME:value` except the first. **Original**: the identity line
`LOGIC_ANALYZER_<board>_<version>`, `FREQ:<Hz>` (highest rate), `BLASTFREQ:<Hz>`, `BUFFER:<samples>` (the buffer of
the internal memory), `CHANNELS:<n>`. **New** (a reader that does not know a name ignores the line):

| Line | Meaning |
|------|---------|
| `FW:<major>.<minor>.<patch>` | the firmware's version |
| `PROTO:<n>` | the protocol extension level, 2 for this document |
| `FEATURES:<names>` | a comma list from: `cmd7`, `compact`, `psram`, `mode32` |
| `PSRAM:<bytes>` | the size of the external memory, 0 or absent when there is none |
| `PSRAMRATE:<Hz>` | the highest rate at which a capture of 32 channels can go to the external memory |
| `MAXPRE:<a>,<b>,<c>,<d>` | the most samples before the trigger in modes 0, 1, 2, 3 |
| `MAXPOST:<a>,<b>,<c>,<d>` | the most samples after the trigger in modes 0, 1, 2, 3 |
| `TRIGGERS:<names>` | from `edge`, `pattern:<max channels>`, `fast:<max channels>`, `burst:<max>`, `blast` |

## Capture mode

The mode is set by the highest channel used (original): 0 for channels up to 8, 1 up to 16, 2 up to 24,
**3** (new) up to 32. The original's transfer of a capture is `1`, `2` or `4` bytes per sample by the mode
(modes 2 and 3 are 4 bytes).

## Capture request 2 (command 7, new)

The payload, with explicit offsets (no padding):

| Offset | Size | Field |
|-------:|-----:|-------|
| 0 | 1 | trigger type: 0 edge, 1 pattern, 2 fast pattern, 3 blast (as the original) |
| 1 | 1 | trigger channel (the base channel of a pattern), 0 to 31 |
| 2 | 1 | for an edge: 1 for a falling edge, 0 for a rising one; for a pattern: its length (1 to 16, fast: 1 to 5) |
| 3 | 4 | trigger value: the pattern bits, bit 0 for the base channel |
| 7 | 1 | number of channels captured, 1 to 32 |
| 8 | 32 | the channel numbers captured, in output order, the first `number` used |
| 40 | 1 | capture mode 0 to 3 |
| 41 | 4 | frequency in hertz |
| 45 | 4 | samples before the trigger |
| 49 | 4 | samples after the trigger |
| 53 | 1 | bursts after the trigger, 0 for none |
| 54 | 1 | 1 to return the time of each burst |
| 55 | 1 | role: 0 single board, 1 master, 2 slave |
| 56 | 1 | encoding asked for the samples: 0 raw, 1 compact |
| 57 | 1 | memory: 0 the board chooses, 1 internal, 2 external |

The payload is 58 bytes. A shorter or longer one is `ERR_BAD_REQUEST`. Every field is checked against the limits
of the information reply before the capture starts; the first failure is `CAPTURE_ERROR` followed by the name of
the field.

## The data after a capture (command 7)

After the line `CAPTURE_STARTED` and when the capture ends:

```text
1 byte   encoding used: 0 raw, 1 compact   (the board may fall back from compact to raw)
4 bytes  number of samples
…        the samples (below)
1 byte   number of burst times n (0 when none)
4·n      the burst times (as the original: the counter value of the start of each burst)
```

**Raw**: each sample is `ceil(number of channels / 8)` bytes; bit `i` of the sample (byte 0 first, bit 0 first) is
the `i`-th channel of the request's list. Only the selected channels are sent.

**Compact**: the first sample as in raw; then records until the number of samples is reached:
a variable-length number `d` (7 bits per byte, the high bit set on all bytes but the last) that is the number of
samples to the next change, at least 1, then a mask of the same size as a raw sample, whose set bits are the
channels that change at that sample. The samples after the last record equal the last value. The
decoder rebuilds the raw samples exactly; a stream that does not add up to the number of samples is
`device.protocol` in the driver.

For command 1 (the original's request) the data is the original's: the samples as 1, 2 or 4 bytes by the mode
(the bits are the channels' numbers), the number of burst times and the times.

## Errors

Reply lines that begin with `ERR_` and name the fault: `ERR_UNKNOWN_MSG`, `ERR_MSG_OVERFLOW`, `ERR_UNSUPPORTED`,
and (new) `ERR_BAD_REQUEST`, `ERR_BUSY` (a capture runs), `ERR_NO_MEMORY` (the buffer does not fit). The
firmware answers every malformed input with one of them and goes on.

## What the parser guarantees (`la-proto`)

Fed any bytes in any chunking, the frame parser gives the same frames and errors, never panics, never
allocates more than 256 bytes per frame, and drops a damaged frame without losing the next one. The
compact decoder rejects any stream that is too long, too short or that would produce more than the declared
number of samples, without allocating more than that number.
