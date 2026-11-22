# Contract: the I2C generator

The counterpart of the decoder for the virtual device (constitution Principle XI). Implemented as
`generators::i2c` next to `generators::clocked_bits`, with the same four functions.

```text
source(setup: &Setup, seed: u64) -> Option<I2cSource>          // a SignalSource: a pure function of the position
claim(setup: &Setup) -> Option<Claim>                          // the channels it fills and how to make its source
expected(setup: &Setup, seed: u64, length: u64) -> Vec<Item>   // what a correct decode of `length` samples yields
expected_errors(setup: &Setup, seed: u64, length: u64) -> Vec<PlantedError>
```

`Params::from_setup` returns `None` unless both signals are assigned; the registry then gives no claim
and the channels get the ordinary random signals (FR-018).

## Options (`Setup::generator`, integers)

| Key | Meaning | Default | Edited by |
|-----|---------|---------|-----------|
| `errors` | 1 plants errors, 0 plants none | 0 | the shell (checkbox, unchanged) |
| `error_rate` | about one transaction in this many has a planted error; at least 2 | 8 | the shell (field, unchanged) |
| `max_bytes` | the most data bytes in a transaction, 1 to 6 | 4 | tests |
| `ten_bit` | 1 uses 10-bit addresses | 0 | tests |
| `startup` | 1 puts a start-up before the first frame (R7) | 0 | tests |

Unknown keys and out-of-range values are clamped, never an error (they come from a session file).

## The signal

- One SCL period is 16 samples: SCL is high on samples 4 to 11 of a bit slot; SDA takes the bit in the
  sample where SCL falls (sample 12 of the slot before) and is stable around the rising edge.
- SDA changes in the sample where SCL falls, except in a start, a repeated start and a stop, where its
  edge is 8 samples from an edge of SCL; no clock pulse is narrower than 8 samples (FR-025). A start is
  12 samples: SDA low with SCL high for 8, then SCL falls and SDA takes the first bit (4 samples before
  the clock pulse of that bit). A repeated start is 24: SDA high (it was set when SCL fell) with SCL low
  (4), SCL high (8), SDA low with SCL high (8), SCL falls with the first bit set (4). A stop is 12: SDA
  low with SCL low (4), SCL high with SDA low (8), then SDA rises. Frame length grows by 24 samples with
  respect to the first version (1008 with the defaults).
- A frame is `lead-in (64 samples, both lines high) + transaction + idle`, of one fixed length sized for
  `max_bytes` (and twice the address length for 10-bit), so frame `k` starts at `k × frame length` and
  `sample(position)` needs no state.
- A transaction is: start, address byte(s) with direction, `1..=max_bytes` data bytes, stop; or, when
  drawn as a split, a write part, a repeated start and a read part, then stop. The acknowledge bit is
  low (ACK) unless an error is planted. In a read part the controller acknowledges every byte but the
  last and not-acknowledges the last (normal, not an error).
- **Planted errors**, one per affected transaction: (a) the address is not acknowledged (the acknowledge
  bit is high) and the transaction ends with a stop; (b) a written data byte is not acknowledged and the
  transaction ends with a stop after it; (c) a stop in the middle of a byte, after `b` of its bits.
- **Start-up** (`startup` = 1): before frame 0, `S` samples (a multiple of the bit period, drawn from
  the seed, at most 8 periods) in which both lines are held low and then pulsed with highs of less than
  `T` samples, so the decoder must ignore them silently; the first frame's lead-in follows. Frame 0
  then begins at position `S`; with `startup` = 0, `S` = 0.
- The result is the same for any way of asking for the samples (a test fills the same range with chunk
  sizes 1, 7, 4096 and the whole).
- Different instances (different instance numbers) with the same seed differ.

## What `expected` returns

The items of kind `i2c` (see i2c-items.md) of every construct that completed before `length`, in order,
with the error flag where an acknowledge was required and missing; and `expected_errors` the planted
errors with their positions: `AddressNak { at }`, `DataNak { at }`, `ByteCut { start, end }`. A capture
that ends inside a transaction yields the items completed before the end and no error for the part that
is missing. These are exactly what the round-trip tests compare with the decoder's output (SC-001).
