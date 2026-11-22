# Contract: what the I2C decoder writes into its lane

Replaces the lane rows of the table "What it produces" of `docs/decoders/i2c.md` (feature 008). The
overlays on SDA and SCL and the items for stacking are as in feature 008, with the single exception in
"Errors".

## Places

`T` is the bus period in samples; `before = T / 2`, `after = T - before`. All ranges are half open.

```text
bit cell of a bit read at the rising edge r        [r - before, r + after)
address or data block                              [cell(bit 1).from, cell(bit 8).to)
ack or nak block                                   cell(bit 9)
start, repeated start (SDA falls at s)             [s - before, s + after)
stop (SDA rises at p)                              [max(last block end, p - T), p)
packet                                             [start.from, stop.to)
```

Where two blocks of the upper band would overlap the earlier is cut at the start of the later; a block
with `from >= to` is not written. For the generated signal (`T` = 16) the blocks touch: start, address,
ack, data, ack, stop follow each other with no gap.

## Bands, shapes, colours

Upper band: shape `Part`; lower band: shape `Whole`. Colour indices of the palette: blue 4, orange 6, yellow 1, green
2, red 0, grey 7.

| Block | Colour | Text |
|-------|--------|------|
| start / repeated start / stop | blue | Start / Repeated start / Stop |
| address, data | orange when writing, yellow when reading | as in 008 |
| ack | green | ACK |
| nak | red | NAK |
| packet without error | green | Write to 0x43 / Read from 0x43 |
| packet with an error | red | the same |
| packet not finished | grey, or red if a part of it was already an error (a cut byte, lost samples) | Incomplete transaction |

A NAK that is an error is an address not acknowledged, a written byte not acknowledged; the controller's
NAK of the last byte it reads is red but is not an error. A packet is red when any of its parts is an error
(including a cut byte, lost samples, an empty transaction).

## Errors

The error flag of a block is set for the nak block of an error, a cut byte, lost samples and for a red
packet. The text of the error that the lane point used to carry (`Address 0x50 not acknowledged`, `Data
0x12 not acknowledged`) is now the text of the SCL arrow of the acknowledge bit, with the error flag: this
is the one change to the overlays of feature 008. The other error texts are the texts of their blocks.

## Unchanged

Items (kinds, spans, payloads, flags) and their order, the dots on SDA, the arrows on SCL (positions and
shapes), the texts and keys, the options, the synchronisation, the limits.
