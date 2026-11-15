# Contract: Decoder API (version 1.0)

**Status**: design source for `docs/decoder-api.md`, which becomes the authoritative copy during
implementation (as `docs/host-api.md` did for feature 006). `la_decoders::DECODER_API_VERSION` and the
document's version line must agree; a test fails otherwise.

This is the contract between the framework and a decoder (constitution Principle IX). It is defined by
this project: it is **not** compatible with, and does not depend on, PulseView, sigrok or Logic 2
decoders. It is independent of any user interface toolkit. Decoders are built into the application
(this feature adds no loading of decoders from files).

## Rules

1. **A decoder is untrusted-quality code.** The framework contains every way it can fail (an error, a
   panic, no response, too much memory, too many or invalid annotations) and none of them can affect
   the capture, another decoder or the unsaved-data protection.
2. **A decoder is read-only.** It sees the capture only through the reader it is handed and cannot
   write to it.
3. **A decoder is a resumable state machine.** It is called with one chunk after another and keeps its
   own state between calls; it never blocks waiting for data and never loops without returning.
4. **A decoder is deterministic.** The same setup and the same chunks give the same output, whatever
   the chunking (a test checks this for the built-in decoders).
5. **Documentation is English;** user-visible text is a message key (localized) or literal text from
   outside (shown as it is, sanitized and length-limited).

## What a decoder provides

```text
descriptor() -> Descriptor        // static: see data-model.md "Decoder descriptor"
create(setup: &Setup) -> Box<dyn Run>   // setup already validated by the framework; may still fail
```

`Run` is the state of one pass:

```text
process(&mut self, chunk: &Chunk, reader: &SignalReader, sink: &mut Sink) -> Result<(), DecodeError>
finish(&mut self, end: End, reader: &SignalReader, sink: &mut Sink) -> Result<(), DecodeError>
```

- `process` is called for successive chunks `[start, end)` of positions with `start` equal to the
  previous `end`. A chunk is bounded in edges and span so that one call takes milliseconds.
- `finish` is called when the pass has reached the end of what will ever come (the capture is closed
  and all of it was processed) or the pass was cut (`End::Complete`, `End::Cut` for a cancelled or
  limited pass, `End::Gap` is delivered through the chunk, see below). A decoder uses it to flush a
  pending construct (the sample decoder ends an incomplete byte).
- A growing capture is just more chunks later: `finish` is called only at the real end.

### Chunk

`Chunk { start, end, capture_length, gaps }`: the range of positions to process, the length of the
capture at the snapshot, and the gaps (lost samples) that intersect the range, sorted. A decoder must
not assume that a gap does not exist; it treats positions inside a gap as "no data" and must not guess
across it (FR-016). A decoder that accepts `Items(kind)` receives `ItemChunk { items, covered }`
instead of edges: the items of that kind whose `end` lies in the chunk, in order, and the position up to
which the source has finished (the watermark).

### SignalReader (read-only access to the assigned channels)

```text
edges(range) -> iterator of Edge { position, role, rising }   // merged over the assigned roles, by position
level(role, position) -> Level  // High, Low or Missing (inside a gap or not assigned); O(log N)
role_assigned(role) -> bool
```

It reads through the capture's snapshot of the moment the chunk was cut; positions at or beyond the
snapshot length return `Missing`. It cannot be used to change anything and holds no reference that
outlives the call.

### Sink (what a decoder writes)

```text
annotate(Annotation)      // drawn: see data-model.md "Annotation"
emit(Item)                // for stacking: see data-model.md "Output item"
```

Everything written is validated by the framework (positions, ranges, targets, styles, text limits,
non-decreasing item ends); an invalid record is dropped and counted (`decoder.invalid_output`), never
stored. The sink also enforces the annotation count and memory budgets: past a budget `annotate` and
`emit` return `Err(Limit)` and the decoder is expected to return at once.

### Errors a decoder returns

`DecodeError` is a value with a short technical detail for the log (never shown to the user and never
containing signal data). The framework turns it into `decoder.failed`. A decoder reports a *problem in
the signal* (a framing error, a missing acknowledge) as an **error annotation**, not as a `DecodeError`:
only a decoder that cannot continue returns an error.

## What the framework guarantees a decoder

- `process` is called with increasing, contiguous chunks from the start of the capture, each at most the
  chunk bound, never concurrently for one instance, always on a worker that may be abandoned if the call
  exceeds its deadline (then the decoder instance is dropped and nothing more is read from it).
- The `level` and `edges` answers are consistent with the snapshot of the chunk.
- A stack gives an upper decoder only items that passed validation and that the lower decoder declared.

## What a decoder must not do

- Block, sleep, spawn threads, perform I/O, read the clock or random sources (determinism), or keep a
  reference to the reader or the sink between calls.
- Allocate without bound: its state must be small (bounded by its options); the stored output is the
  framework's, counted and limited.
- Panic on any input. (If it does, the panic is caught and reported, but a decoder that panics is a bug
  that is fixed, not a way to signal errors.)

## Descriptor and message keys

A descriptor refers to texts by **static message keys** (string literals in the decoder's source), which
is what lets the localization parity check find them: `decoder-<id>-name`, `decoder-<id>-description`,
`decoder-<id>-signal-<role>`, `decoder-<id>-option-<option>`, `decoder-<id>-choice-<option>-<value>`, and
the standard annotation texts `ann-<id>-<kind>` with their arguments documented next to the key in the
English resource. Every key exists in all six languages.

## Generators

A decoder that can be used with the virtual device provides a generator (constitution Principle XI):

```text
generator(setup: &Setup, seed: u64, sample_rate_hz: u64) -> Box<dyn SignalSource>   // the device crate's trait
expected(setup: &Setup, seed: u64, length: u64) -> Vec<Item>                        // what a correct decode must yield
```

- `SignalSource::fill` is a deterministic function of the position for any chunking (the existing contract
  of the virtual device).
- `expected` returns the items (content) the generator produced up to `length`, which is what the
  round-trip test compares with the decoder's output. Planted errors are part of the expected result
  (an error item or error annotation at the planted position).
- A generator encodes at its own level and, for a stacked decoder, down through the generators of the
  decoders below it; version 1.0 ships the single-level case and fixes the trait for the stacked one.

## Registering a decoder

Registering a descriptor plus its `create` (and generator) in the registry is all that is needed for it
to appear in the list of decoders, its configuration dialog, its lane and its saved setup: the
application builds all of them from the descriptor (SC-012). Registration validates the descriptor and
refuses an invalid one.

## Versioning

`DECODER_API_VERSION` is `1.0`. Adding an optional capability that existing decoders can ignore is a
minor version; changing the meaning of a call, a chunk guarantee or a limit is a major version and
requires every built-in decoder to be updated in the same merge. Each decoder has its own `version`
(the descriptor field) for its saved options.
