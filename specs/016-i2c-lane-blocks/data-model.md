# Data Model: I2C Lane as Blocks

No new stored entity. What changes is how existing annotations are used and two additions to the
annotation model of feature 007.

## Annotation model additions (decoder API 1.2)

| Name | Where | Meaning |
|------|-------|---------|
| `Shape::Part` | `model::Shape` | A range drawn as a solid block in the **upper** band of a lane |
| `Shape::Whole` | `model::Shape` | A range drawn as a solid block in the **lower** band of a lane |
| `Descriptor::lane_rows` | `descriptor::Descriptor` | Number of rows of the decoder's lane, 1 to 3, default 3; the I2C decoder declares 2 |

Rules:

- `Part` and `Whole` are valid only for `Kind::Range`; on a point or an edge the sink drops the record and
  counts it (like any invalid style), so the planner never has to place a point in a band.
- In `lanes::plan` a lane of `lane_rows = n` has `n` rows of spans and marks and one row of dense cells. A
  `Part` is placed in row 0, a `Whole` in row `min(1, n - 1)`; a cell that finds its place taken becomes a
  dense cell. Other shapes are packed into the rows as before.
- Saved results: the shape code of the two values is 5 and 6 (3 bits of the style byte); version 1 of the
  section is kept.

## Blocks of the I2C lane

| Block | Band | Shape | Colour | Text | Error flag |
|-------|------|-------|--------|------|------------|
| start | upper | `Part` | blue | `ann-i2c-start` | no |
| repeated start | upper | `Part` | blue | `ann-i2c-repeated-start` | no |
| address | upper | `Part` | yellow (write) / green (read) | `ann-i2c-address-write/read` | no |
| data | upper | `Part` | yellow / green | `ann-i2c-data` | no |
| ack | upper | `Part` | green | `ann-i2c-ack` | no |
| nak | upper | `Part` | red | `ann-i2c-nak` | yes if the NAK is an error |
| stop | upper | `Part` | blue | `ann-i2c-stop` | no |
| cut byte | upper | `Part` | the colour its data block would have | `ann-i2c-error-byte-cut` | yes |
| lost samples | upper | `Part` | red | `ann-i2c-error-gap` | yes |
| packet | lower | `Whole` | green / red / grey | `ann-i2c-transaction-write/read` | yes if red |
| empty packet | lower | `Whole` | red | `ann-i2c-error-empty` | yes |
| unfinished packet | lower | `Whole` | grey; red if a part of it was already an error (cut byte, lost samples) | `ann-i2c-incomplete` | yes if red |

Positions follow research R2. Blocks of the upper band never overlap (the decoder shortens the earlier);
a packet block is as wide as its parts together. The overlays on SDA (dots) and SCL (arrows) are as in
feature 008, except that the arrow of an acknowledge bit that is an error carries the error text (R5).

## Decoder state (additions, `i2c/bus.rs`)

- the end of the last block written for the current packet (to place the stop and to shorten overlaps);
- the beginning of the packet's first block and whether a part of it was an error (to colour the packet
  block when the stop comes);
- the cell of the current byte (from the first bit's cell start) while its bits arrive.

All bounded by a few integers; nothing grows with the capture.
