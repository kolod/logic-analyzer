# The I2C decoder

Decoder id `i2c`, version 1. Implemented in `crates/decoders/src/decoders/i2c/`, its generator in
`crates/decoders/src/generators/i2c.rs`. The contract it is written against is
[decoder-api.md](../decoder-api.md); the design is in `specs/008-i2c-decoder-generator`. It was written
from the I2C-bus specification (NXP UM10204); no code of PulseView or libsigrokdecode is used.

## What it reads

Two signals, both required, on different channels:

| Role | Meaning |
|------|---------|
| `scl` | the clock line |
| `sda` | the data line |

It reads edges, so it works at any sample rate that shows the clock pulses as separate edges. It does
not check the bus against a speed class and has no option for the speed: it finds the bus clock
period from the signal (see "Synchronising").

## Synchronising

1. **The bus period.** From the rising edges of SCL the decoder measures the intervals between them and
   keeps the last 32. As soon as three neighbouring quarter-octave bins hold 16 of them, the period is
   the mean of those intervals and stays that for the pass. The decision is made interval by interval,
   so it depends on the signal alone, not on how the capture was cut into chunks or how fast it grew.
2. **Until the period is known** (a growing capture, or too few clock pulses) the decoder holds
   everything back: nothing is shown and, through `Run::final_through`, nothing is reported final, so a
   decoder stacked on it receives nothing yet. When the period is known the decoder decodes **from the
   start of the capture**, a bounded number of edges per call, so the result equals the result for the
   whole capture. If the capture ends without the period being found the result is empty, and that is
   not an error.
3. **The first start.** A start condition counts as the first one only after both lines were high,
   without a break, for at least one bus period (a run that begins at the first sample counts). Until
   then every edge, including a falling edge of SDA while SCL is high, is ignored without a trace: a
   capture that begins in the middle of a byte, or before the MCU has set up its I2C interface (the
   lines low, floating or noisy), shows nothing before the first valid start. After a stop, and inside
   a transaction, no idle is needed. A gap in the capture (lost samples) ends what is in progress and
   the decoder looks for an idle bus again.

## How the bus is read

- **Start** (SDA falls while SCL is high), **repeated start** (the same inside a transaction, between
  bytes) and **stop** (SDA rises while SCL is high).
- **A bit** is a rising edge of SCL whose pulse ends (SCL falls) with no SDA edge in it; the bit is the
  level SDA had at the rising edge. An SDA edge while SCL is high is a start or a stop and the pulse
  before it was its set-up. Edges of one sample are processed SCL first. Eight bits, most significant
  first, and the acknowledge bit make a byte; an acknowledge bit that is low is ACK.
- **The first byte after a start** is the address: seven bits and the direction bit (0 write, 1
  read). A first byte `11110xxW` with `W` = 0 is the first byte of a **10-bit address**; the next byte
  is its low byte and the two are one address of ten bits. After a repeated start, a byte `11110xxR`
  with `R` = 1 whose `xx` is those of the address just written is the read part of the same address.
  Any other byte of that form is an ordinary address.
- **Acknowledgement.** A NAK after an address, or after a byte written to a target, is an error. The
  controller's NAK of the last byte it reads is how a read ends and is never an error.

## What it produces

The lane has two bands of solid blocks (feature 016, `specs/016-i2c-lane-blocks/contracts/i2c-lane.md`).
`T` is the bus period; a bit occupies the period centred on the rising edge of SCL at which it is read
(`T / 2` rounded down before the edge, the rest after it). Blocks of the upper band do not overlap (the
earlier is cut where the later begins).

| What | Band | Where | Colour | Text |
|------|------|-------|--------|------|
| start | upper | `T` centred on the falling edge of SDA | blue | `Start` |
| repeated start | upper | from the end of the block before it (the acknowledge) to half a period after the falling edge of SDA | blue | `Repeated start` |
| an address | upper | the eight bits (for 10 bits, a block for each byte, both naming the address) | orange (write), yellow (read) | `Address 0x50 (write)` |
| a data byte | upper | the eight bits | orange (write), yellow (read) | `Data 0x12` |
| an acknowledge bit | upper | the one period of the bit | green (ACK), red (NAK) | `ACK`, `NAK` |
| stop | upper | from the end of the last block (at most `T`) to the rising edge of SDA | blue | `Stop` |
| a transaction | lower | from the start of its start block to the end of its stop block | green, red if it has an error | `Write to 0x50: 0x12 0x34` or `Read from 0x50: 0x01`, named after its first part, with the data bytes that went over the bus (a bar between the parts at a repeated start, a cut sign when they are too many for the text); the interface shows the shorter `Write to 0x50` when the long text does not fit |

A block shows its text only if all of it fits. On the signals: a dot on SDA at every start and stop and an
arrow on SCL at the rising edge of every bit (the text is the bit, or ACK/NAK); marks on the signals name
no colour: they take the colour of their signal. The controller's NAK of the last byte it reads is a red
NAK block but is not an error, and the packet stays green.

Errors (a block with a heavy outline; the packet is red when any of them is in it):

| Error | Where |
|-------|-------|
| `Address 0x50 not acknowledged`, `Data 0x12 not acknowledged` | the text of the SCL arrow of the acknowledge bit; the NAK block is red |
| `Byte cut short after 3 bits` | an upper block over the bits that were read, in the colour of its byte |
| `Empty transaction: no address` | the packet block of a start followed by a stop with no clock pulse between them |
| `Samples lost here` | a red upper block over the gap, when a transaction was in progress |

A transaction that is not finished when the capture ends (or when samples were lost, or a start cut a
byte) is a packet block to the end of its last block, `Incomplete transaction`: grey while nothing in it
was an error, red if something was.

## Options

| Option | Choices | Default |
|--------|---------|---------|
| `address` | `addr7`: the 7-bit address (`0x50`); `addr8`: the 8-bit value with the direction bit (`0xA0` write, `0xA1` read) | `addr7` |
| `radix` | `hex`, `dec`, `bin` | `hex` |

They change only the text. A 10-bit address is always shown as it is, and the items carry the address
as a number whatever is shown.

## Items for a stacked decoder

One kind, `i2c`, a payload of six bytes: the type (1 start, 2 repeated start, 3 address, 4 data, 5
stop), the direction (0 write, 1 read, 2 not known), the acknowledgement (0 ACK, 1 NAK, 2 none), the
flags (bit 0: a 10-bit address), and the value, two bytes, big endian (the address, the byte, or 0).
A start and a repeated start have the direction *not known*; a stop has the direction of the last part.
The span is the exact positions: an edge for a start, a repeated start and a stop (`[p, p + 1)`), the
first sampling edge to the acknowledge edge (+ 1) for a byte (for a 10-bit address, from its header to
the end of its low byte). The `error` flag is set when an acknowledge was required and did not come.
Items are emitted when the construct is complete (a bit counts when its clock pulse has ended), in
order of their end; a byte that is cut short gives no item.

## The generator

`generators::i2c` makes valid I2C traffic for the virtual device: one transaction in every frame of a
fixed length (a lead-in of 64 samples with both lines high, the transaction, an idle), 16 samples to a
clock period, random address, direction and data from the seed. It is a pure function of the position.
The options are in `Setup::generator`:

| Key | Meaning | Default |
|-----|---------|---------|
| `errors` | 1 plants errors | 0 |
| `error_rate` | about one transaction in this many has an error (at least 2) | 8 |
| `max_bytes` | the most data bytes, 1 to 6 | 4 |
| `ten_bit` | 1 uses 10-bit addresses (a read is then a write of the address, a repeated start and the read) | 0 |
| `startup` | 1 puts a start-up of 32 to 128 samples before the first frame (the lines low, then pulses whose highs are shorter than the bus period) | 0 |

Planted errors: the address is not acknowledged, a written byte is not acknowledged, or a stop cuts a
byte short. `expected()` returns the items a correct decode of a given length must give and
`expected_errors()` the planted errors; the round-trip tests compare them with the decoder's output for
a thousand seeds.
