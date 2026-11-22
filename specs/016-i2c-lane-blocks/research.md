# Research: I2C Lane as Blocks

Decisions that the plan rests on. Nothing here is left open except the points marked **owner**.

## R1. Sub-bands

**Problem**: the planner of feature 007 packs the spans of a lane into rows by overlap, in the order of
their start. The packet block starts before the parts it covers, so it would be put in the first row and
the parts below it: the opposite of the spec. A row that depends on the order of arrival would also differ
between a growing capture and the finished one.

**Decision**: the band is a property of the annotation's shape. Two new shapes, `Shape::Part` (a solid block
of the upper band) and `Shape::Whole` (a solid block of the lower band), both for ranges only. The planner
puts a `Part` in row 0 and a `Whole` in row 1 (clamped to the last row), without packing; if the place in
its row is taken by another cell the new cell becomes a dense cell, as for any annotation that finds no
room. A descriptor field `lane_rows` (default 3, as before) gives the number of rows of the lane; the I2C
decoder says 2. The host passes it to `lanes::plan`.

**Rationale**: the style byte of the saved results has 3 bits for the shape, of which 5 values are used, so
the file format and the 44 places that build a `Style` stay as they are; the rule is local to the planner;
other decoders are not touched.

**Alternatives**: a `band` field on `Annotation` or `Style` (format change, every construction site);
a second lane for the packets (the owner asked for two bands of one row); ordering the insertions (fragile).

## R2. Where the blocks are

**Decision**: all places come from the bus period `T` that the decoder already measures, `before = T / 2`
(rounded down) and `after = T - before`; positions are half-open `[from, to)`, an edge at `p` lies on the
boundary `p`.

| Block | From | To |
|-------|------|----|
| bit cell of a bit read at the rising edge `r` | `r - before` | `r + after` |
| address / data block (8 bits) | cell start of its first bit | cell end of its eighth bit |
| ack / nak block | cell start of the acknowledge bit | its cell end |
| start / repeated start | `s - before` | `s + after`, `s` = the falling edge of SDA |
| stop | `max(last block end, p - T)` | `p`, `p` = the rising edge of SDA |
| cut byte (stop, start or end inside it) | cell start of its first bit | cell end of its last bit read (`at` of a stop that came sooner) |
| packet | start block `from` | stop block `to` (unfinished: end of the last block; no repeated start splits it) |

For the generated signal (frame of feature 008: SDA falls at `s`, SCL falls at `s + 8`, the first bit's
rising edge is at `s + 16`, `T` = 16) this gives exactly the rule of the spec and the places of the owner's
four marker pairs: the start `[s - 8, s + 8)` ends where the first bit's cell begins; the stop begins where
the last ack cell ends and ends at the SDA rise. For a bus whose SCL falls later after the start, the start
block is still the period around the SDA edge and may leave a gap to the first bit (spec FR-003 is worded
that way). Where a block would overlap the next (a stop and the next start less than a period apart, a stop
inside the ack cell) the earlier one is shortened to the start of the later one (FR-005); a block that
would be empty or inverted is not written.

**Why not wait for the first bit to place the start**: the decoder emits at the time it sees the edge;
placing the start block by the first bit would delay it and need the packet to remember it.

## R3. Colours

**Decision**: roles map to indices of the existing palette of eight annotation colours (the colours the
user already knows from markers; chosen for the three variants of the interface): blue 4, yellow 1, green 2,
red 0, grey 7.

| Block | Colour |
|-------|--------|
| start, repeated start, stop | blue (4) |
| address, data: write | yellow (1) |
| address, data: read | green (2) |
| ack | green (2) |
| nak | red (0) |
| packet: no error | green (2) |
| packet: an error | red (0) |
| packet: unfinished | grey (7) (**owner**: not specified), red if a part of it was already an error |

The `error` flag keeps its other duties (counted in dense cells and by the selection, a heavier outline).
For `Part` and `Whole` shapes the fill is **not** replaced by the danger colour: a cut data block keeps its
colour and is told by its outline, and a NAK that ends a read is red without being an error. A test checks
that the five colours are pairwise distinguishable (by hue or luminance) in the dark, light and
high-contrast variants and that the text colour has contrast against each.

## R4. Drawing

**Decision** (`decoder_view.rs`): a `Part` or `Whole` is a solid rectangle in its palette colour with a 1 px
outline of the text colour at low alpha (2 px when it is an error, a second 2 px outline when selected).
Its text is drawn only if the whole text fits (width estimated as the existing label code does, with the
padding on both sides); otherwise the block is drawn without text; no ellipsis. The text colour is black or
white, whichever has the better contrast against the fill. The two rows share the usable height of the lane
(at most 24 px each). Blocks of a row never reach into the next one: the existing rule that the later cell
wins applies. Dense cells keep their row (the third one), as now.

## R5. Errors and their texts

**Decision**: the texts `Address 0x50 not acknowledged` and `Data 0x12 not acknowledged` were on the lane
point of the acknowledge bit, which the nak block replaces; the block says "NAK" as the owner asked. The
explanation goes to the overlay arrow of that bit on SCL (`Target::Channel(scl)`, the arrow keeps its edge
shape and position) as its text, with the error flag, as the lane point carried it; selecting the nak block
shows "NAK" and the framework's line that it is an error. The packet block carries the text of the
transaction (`Write to 0x43`) and the error flag. The other errors keep their texts: `Byte cut short after
3 bits` is the text of the cut data block (it fits only when wide), `Samples lost here` and `Empty
transaction: no address` are blocks of the upper and of the lower band with the same texts, in their
colours: gap: red upper block; empty: red packet block with a start and a stop block.

**Spec**: FR-013 says the arrows do not change; the one change (the text of the arrow of an unacknowledged
error bit) is written into the spec and into the contract.

## R6. Tests

- **Geometry (hand-built)**: signals with a known period; the expected `[from, to)` of every block written
  by hand from R2 (SC-002): a write of one byte, a read ending in NAK, a repeated start, a 10-bit address,
  a cut byte, a stop soon after the ack, two packets less than a period apart, an unfinished packet.
- **Round trip**: for the seeds of the generator, the expected blocks are derived from `expected()` (the
  items with their positions) and `T = 16`, and compared with the lane of the decoder; the packets that are
  red are exactly those with a planted error (SC-001).
- **Planner**: a `Part` goes to row 0, a `Whole` to row 1, in any order of insertion, in a window of a
  growing and of a finished store; a lane of another decoder is packed as before; dense traffic keeps the
  cost bound.
- **Interface helpers**: text fits whole or not at all (no ellipsis for blocks), contrast of the text
  colour, the five colours distinguishable on every variant.
- **Unchanged**: the I2C round-trip, chunking, sync, options, recorded-signal and stacking suites pass
  without a change in what they compare (the lane assertions that read ranges and points are rewritten to
  blocks; the items, arrows and dots are asserted as before).

## R7. Documents

`docs/decoders/i2c.md` (the table "What it produces" and the colours), `docs/decoder-api.md` (shapes,
`lane_rows`, versioning 1.2, rows of a lane), `docs/user-guide.md` if it describes the lane. All in English.
