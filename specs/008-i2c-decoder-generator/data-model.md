# Data Model: I2C Decoder and Generator

Nothing here is stored in the session file in a new way: the decoder's setup (options) goes through
the `DECO` section and its results through `DANN`, both of feature 007, unchanged. This document
describes the values the decoder and the generator work with.

## Descriptor

| Field | Value |
|-------|-------|
| `id` | `i2c` |
| `version` | 1 |
| `name`, `description` | `decoder-i2c-name`, `decoder-i2c-description` |
| `signals` | `scl` (required, not shareable), `sda` (required, not shareable); in this order, so simultaneous edges arrive SCL first |
| `options` | `address`: choice `addr7` (default), `addr8`; `radix`: choice `hex` (default), `dec`, `bin` |
| `input` | `Input::Signals` |
| `outputs` | `["i2c"]` |
| `generator` | `true` |

## Decoder state (one pass)

Small and bounded by constants, never by the capture (Rule "state must be small").

| Part | Content |
|------|---------|
| Channels | the channel numbers of SCL and SDA, for the overlay targets |
| Options | address notation, radix |
| Levels | the level of SCL and SDA at the position reached; whether it is known (a gap or the start makes it unknown) |
| Calibration | the histogram of intervals (256 bins: count `u32`, sum `u64`), the position of the last SCL rising edge, the number of intervals; then `T` (a `u64`) once found |
| Synchronisation | the start of the current run of both lines high (or none) |
| Transaction | none, or: start position, direction, address (7 or 10 bit) and whether it is complete, number of bytes so far, whether the first part was a 10-bit write header |
| Byte | bits read so far (0 to 9), their values, the position of the first bit's rising edge |
| Progress | the position processed so far (what `final_through` reports), the position scanned by calibration |

## Events and what each produces

| Event (on the bus) | Lane | Overlay | Item (`i2c`) |
|--------------------|------|---------|--------------|
| Start | point "Start" | dot on SDA | type 1 |
| Repeated start | point "Repeated start" | dot on SDA | type 2 |
| Bit (8 data bits and the acknowledge bit) | — | arrow on SCL at the rising edge | — |
| Address byte (7-bit, or the two bytes of 10-bit) | range "Address 0x50 (write)" from the first bit's edge to the acknowledge edge | — | type 3 |
| Acknowledge bit of any byte | point "ACK" or "NAK" at the acknowledge edge (error style where R4 says error) | — | in the byte's item |
| Data byte | range "Data 0x12" | — | type 4 |
| Stop | point "Stop" | dot on SDA | type 5 |
| Whole transaction | range from start to stop "Write to 0x50" / "Read from 0x50" | — | — |
| Unfinished transaction at the end | range to the end "Incomplete transaction" (not an error) | — | — |
| Error (cut short, empty, no acknowledge, samples lost) | error range or point with a message | — | `error` flag on the construct's item, if it has one |

## Item `i2c` (stack output)

See [contracts/i2c-items.md](contracts/i2c-items.md).

## Generator

| Entity | Content |
|--------|---------|
| Parameters | the channels of SCL and SDA, `address_bits` (7 or 10, from `ten_bit`), `max_bytes`, `errors`, `error_rate`, `startup`, the seed mixed with the instance |
| Frame | one transaction slot of fixed length: lead-in idle, transaction, idle; frame `k` is drawn from `mix(seed, k)` |
| Planned transaction | address, direction, bytes, split by a repeated start or not, planted error (none, address NAK, data NAK at byte *i*, cut short at bit *b* of byte *i*) |
| Start-up | a prefix of samples before frame 0 (only when `startup` is set): lines low, then pulses whose highs are shorter than the bus period; it contains no valid start |
| Expected result | `expected()`: the items of everything completed before `length`; `expected_errors()`: the planted errors that fall before `length`, with positions and kinds |

Constants: samples per SCL period 16 (SCL high on samples 4 to 11 of a slot, SDA changes on sample 0),
lead-in 4 periods (64 samples), so the lead-in is always at least `T` (16).

## Validation rules (carried by the framework, listed for the tests)

- Annotations: positions inside the capture, ordered ranges, an existing channel, a colour from the
  fixed set or none. The decoder names no colour for overlays.
- Items: `end` never decreases; payload at most 64 bytes (this one uses at most 6).
- Setup: both signals assigned to different channels (framework check), options one of the listed
  choices.

## State transitions of an instance

Unchanged from feature 007 (waiting, running, finished, cancelled, failed, outdated). The one addition
is internal: while calibrating, a running instance reports a coverage that does not advance
(`final_through`), so the interface shows work in progress and no results.
