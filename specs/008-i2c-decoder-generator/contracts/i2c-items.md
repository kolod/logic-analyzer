# Contract: the items of the I2C decoder (`i2c`)

This is what a decoder stacked on I2C reads (`Input::Items("i2c")`). It becomes part of
`docs/decoders/i2c.md` when the feature is implemented; a test builds items from the generator and
checks them against this layout.

## One kind, a payload of bytes

`Item.kind` is `"i2c"`. `Item.payload` is `Payload::Bytes` of exactly 6 bytes:

| Byte | Field | Values |
|------|-------|--------|
| 0 | type | 1 start, 2 repeated start, 3 address, 4 data, 5 stop |
| 1 | direction | 0 write, 1 read, 2 not known (a stop or data before any address) |
| 2 | acknowledgement | 0 ACK, 1 NAK, 2 none (start, repeated start, stop; or the acknowledge bit was never reached) |
| 3 | flags | bit 0: the address is a 10-bit address (type 3 only) |
| 4–5 | value | big endian: the address (7-bit value 0–127, or 10-bit 0–1023) for type 3; the data byte in byte 5 (byte 4 is 0) for type 4; 0 otherwise |

The address in the item is always the number without the direction bit, whatever the `address` option
shows. Direction is also given for data (the direction of the transaction it belongs to).

## Span

| Type | `start` | `end` |
|------|---------|-------|
| start, repeated start | the position of the SDA falling edge | the same position + 1 |
| address, data | the position of the first bit's SCL rising edge | the position of the acknowledge bit's SCL rising edge + 1 (for a 10-bit address, the end of the second byte) |
| stop | the position of the SDA rising edge | the same position + 1 |

`end` never decreases from one item to the next (the sink refuses an item that breaks this). The `error`
flag of the item is set when an error annotation was made for the same construct (an address or a data
byte that was not acknowledged where an acknowledge is required).

## When an item is emitted

At the moment the construct is complete: a start, a repeated start and a stop at their edge; an
address or data byte at its acknowledge edge. A byte that is cut short produces **no item**, only its
error annotation. A transaction that is unfinished when the capture ends produces the items of
everything that completed. The decoder's `final_through` (decoder API 1.1) tells the framework how far
the items are final, so a stacked decoder's watermark is true.

## What a stacked decoder may rely on

- Items arrive in time order, never twice.
- Between a start (or repeated start) and the next stop there is at most one address item before the
  first data item, except for a 10-bit address, which is still one item.
- It must not rely on a stop ever coming (the capture may end first), nor on a start before the first
  address (a decoder that begins mid-bus is silent until the first valid start, so in practice it
  does).
