# Contract change: decoder API 1.0 → 1.1

The single change this feature makes to the framework contract (research R3; FR-022). It is an optional
capability that existing decoders can ignore, so it is a **minor** version under the rules of
`docs/decoder-api.md`.

## The addition

```text
trait Run {
    // … existing methods …

    /// The position up to which this decoder's output is final: nothing it will still produce
    /// begins or ends before it. `None` (the default) means the end of the last chunk, which is what
    /// every decoder of version 1.0 means. A decoder that holds work back (it is waiting for
    /// information that arrives later in the capture, or is catching up on a part of the capture it
    /// has already been given) returns the position it has actually processed.
    fn final_through(&self) -> Option<u64> { None }
}
```

## What the framework does with it

- After every chunk, `Pass::advance` sets the coverage of the results to
  `min(chunk end, final_through)`; with `None` this is the chunk end, as before.
- The watermark of a stack (`ItemStream::set_covered`) follows the coverage, so a decoder above never
  receives an item behind a watermark that has passed it.
- `finish` is called as before; a decoder that held work back drains it there. After `finish` the
  coverage is the end of the pass (or the position reached, for a pass that ended cut).
- The value is never trusted blindly: it is clamped to the chunk end, and a value that goes backwards
  is ignored (coverage never decreases).

## What a decoder must do to use it

Return the position up to which it has processed and emitted everything, no further than the end of the
chunk it was last given. It must still return from `process` in the usual time; a decoder cannot use
this to delay work without limit, because `finish` has the same deadline as a chunk.

## Documentation and tests

- `docs/decoder-api.md`: the version line, the method under "What a decoder provides", a sentence under
  "What the framework guarantees" and the generator-option keys `errors` and `error_rate` as common
  generator options; `DECODER_API_VERSION` becomes `1.1` in the same merge.
- A framework test with a test-only decoder that holds work back: coverage follows `final_through`,
  never exceeds the chunk end, never decreases, and a stacked decoder never sees an item behind its
  watermark.
- All tests of feature 007 pass unchanged (the sample decoder returns `None`).
