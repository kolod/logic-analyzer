# Contract: What Is Said to and Read from the Board

The wire protocol of the WeAct LogicAnalyzer V1 running the `fx2lafw` firmware. It is what the driver
implements and what the replay seam and the recordings carry. Sources: the firmware source and the
libsigrok driver (see research R2, R6); items marked **[verify]** are confirmed on the real board by the
first rig run.

## Identity

| State | VID:PID | Manufacturer / product strings | `bcdDevice` |
|-------|---------|--------------------------------|-------------|
| blank (no firmware) | `1d50:608c` | none (observed on the maintainer's board) | `0x0000` (observed) |
| running the firmware | `1d50:608c` | `sigrok` / `fx2lafw` | `0x0100` |

**Telling the two apart without opening the board**: the product string is `fx2lafw`. The manufacturer
string is `sigrok` where the platform reports it; on Windows `nusb` never reports it (the system does not
cache it), so a missing manufacturer is accepted and a different one is not.

One configuration (1), one interface (0, alternate 0, vendor class `0xFF`), one endpoint: bulk IN
`0x82` (512-byte packets at high speed, 64 at full speed).

## Control requests (vendor type, device recipient)

Direction as the table says. `wValue` and `wIndex` are 0 unless stated. Timeouts in the driver: 100 ms
for the commands, 500 ms for each download chunk.

| Name | `bRequest` | Direction | Data | Meaning |
|------|-----------|-----------|------|---------|
| firmware download | `0xA0` | OUT | up to 4,096 bytes, `wValue` = RAM address **[verify chunk size]** | handled by the chip's own loader; also writes the CPUCS register |
| CPUCS | `0xA0` with `wValue = 0xE600` | OUT | 1 byte: `0x01` holds the 8051 in reset, `0x00` releases it | start / stop the loaded program **[verify]** |
| `CMD_GET_FW_VERSION` | `0xB0` | IN, 2 bytes | `[major, minor]` | the firmware version; the driver requires `major == 1`: stock `fx2lafw` reports 1.4, the image that ships with the application 1.5 (see `firmware/SOURCE.md`) |
| `CMD_START` | `0xB1` | OUT, 3 bytes | `[flags, delay_hi, delay_lo]` | prepare the acquisition; the firmware begins on the first IN request after it |
| `CMD_GET_REVID_VERSION` | `0xB2` | IN, 1 byte | the chip revision | not read by the driver |
| `CMD_STOP` | `0xB3` | OUT, no data | none | **firmware 1.5 only (our patch)**: abort the waveform, empty the FIFO, back to idle |

Stock firmware 1.4 has **no stop request**. Its `CMD_START` handler also waits **for ever** (`while (!(GPIFTRIG & 0x80))` in the main loop) for the previous waveform to end, which it does only when the FIFO is full, that is when the host has stopped reading. A START sent while a capture is being read therefore leaves the board deaf to every request until it is unplugged (seen on the maintainer's board, 2026-11-17). The patched 1.5 aborts the waveform and resets the FIFO instead.

### `CMD_START` payload

- `flags`: bit 6 (`0x40`) = clock 48 MHz (clear = 30 MHz); bit 5 (`0x20`) = 16-bit samples (always 0 on
  this board); bit 4 (`0x10`) = analog line (always 0).
- `delay` (16 bits): `delay_hi` is the high byte, `delay_lo` the low byte. The firmware ignores the
  command silently when `delay_hi >= 6`. The delay is computed from the rate:

| Rate (Hz) | Clock | `delay` | Payload |
|-----------|-------|---------|---------|
| 20,000 | 30 MHz | 1499 | `00 05 DB` |
| 25,000 | 30 MHz | 1199 | `00 04 AF` |
| 50,000 | 48 MHz | 959 | `40 03 BF` |
| 100,000 | 48 MHz | 479 | `40 01 DF` |
| 200,000 | 48 MHz | 239 | `40 00 EF` |
| 250,000 | 48 MHz | 191 | `40 00 BF` |
| 500,000 | 48 MHz | 95 | `40 00 5F` |
| 1,000,000 | 48 MHz | 47 | `40 00 2F` |
| 2,000,000 | 48 MHz | 23 | `40 00 17` |
| 3,000,000 | 48 MHz | 15 | `40 00 0F` |
| 4,000,000 | 48 MHz | 11 | `40 00 0B` |
| 6,000,000 | 48 MHz | 7 | `40 00 07` |
| 8,000,000 | 48 MHz | 5 | `40 00 05` |
| 12,000,000 | 48 MHz | 3 | `40 00 03` |
| 16,000,000 | 48 MHz | 2 | `40 00 02` |
| 24,000,000 | 48 MHz | 1 | `40 00 01` |

Rule: 48 MHz if `48,000,000 % rate == 0` and `48,000,000 / rate - 1 <= 1,536`; else 30 MHz if
`30,000,000 % rate == 0`; any other rate is an error of the driver (never reached: the core validates
against the list). **[verify]** the meaning of the delay at its lowest values (a 1 MS/s capture of a
pattern step of 100 µs must show about 100 samples per step).

## The stream

- Endpoint `0x82`, one byte per sample, no framing, no header. Bit `i` of a byte is input `i`
  (`D0` to `D7`). Disabled channels are masked to zero by the driver.
- Firmware 1.5: the host sends `CMD_START` **with no transfer in flight**, then submits the IN transfers;
  the firmware begins sampling at the first IN request after the start command. Firmware 1.4: the host
  submits the transfers, drains the stale data, then sends `CMD_START` (the older order, kept for a board
  that another program loaded with stock firmware).
- Buffers: size = `10 ms * rate` bytes rounded up to a multiple of 512 (at least 512); count =
  `500 ms * rate / size`, at least 4 and at most 32.
- The firmware has **no lost-sample counter and no end marker**. When the host does not drain the
  endpoint fast enough, the buffer fills and the firmware ends the capture and resets the buffer; the
  host sees transfers that stay empty. A new `CMD_START` is needed for the next capture.
- Stale data: with firmware 1.4, after a cancelled capture the board can hold up to about 4 KB of old
  samples; the driver drains the endpoint before a new start (see `driver-behaviour.md`). Firmware 1.5
  empties the FIFO at every `CMD_START` and `CMD_STOP`.

## Re-enumeration after the download

After the CPUCS release the board leaves the bus (at least 300 ms) and returns with the descriptors of
the firmware at the same port. The driver waits 300 ms, polls the bus every 100 ms for the same
`BoardKey`, and allows 5 s in total.

## What the driver never sends

Nothing other than the requests above, and only to a board with `1d50:608c`.
