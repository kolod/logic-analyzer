# Decoder API

Version: 1.1

This is the authoritative description of the contract between the decoder framework (`la-decoders`)
and a decoder (constitution Principle IX). Its constant `la_decoders::DECODER_API_VERSION` and the
version line above must agree (a test checks it, together with the sections named below). The design
that led here is `specs/007-decoder-framework-annotations/contracts/decoder-api.md`; this document
replaces it as the source of truth.

The API is defined by this project: it is **not** compatible with, and does not depend on, PulseView,
sigrok or Logic 2 decoders. It is independent of any user interface toolkit. Decoders are built into
the application (version 1.0 adds no loading of decoders from files).

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

A **descriptor** (`descriptor::Descriptor`, static data) and a way to create the state of one pass.

```text
Descriptor { id, version, name, description, signals, options, input, outputs, generator }
create(setup: &Setup) -> Box<dyn Run>      // the setup was validated by the framework
```

`run::Run` is the state of one pass:

```text
process(&mut self, chunk: &Chunk, reader: &SignalReader, sink: &mut Sink) -> Result<(), DecodeError>
finish(&mut self, end: End, reader: &SignalReader, sink: &mut Sink) -> Result<(), DecodeError>
```

- `process` is called for successive chunks `[start, end)` of positions with `start` equal to the
  previous `end`. A chunk is bounded in edges (at most 65,536) and in span so that one call takes
  milliseconds.
- `finish` is called once, when the pass ends: `End::Complete` when the capture is closed and all of it
  was processed, `End::Cut` when the pass was cancelled or stopped by a limit. A decoder uses it to
  flush a pending construct (a sample decoder ends an incomplete byte).
- A growing capture is just more chunks later: `finish` is called only at the real end.
- `final_through` (optional, version 1.1) tells the framework how far the decoder's output is final:

  ```text
  final_through(&self) -> Option<u64>      // default None: the end of the last chunk
  ```

  A decoder that holds work back (it waits for something that arrives later in the capture, or is
  catching up on a part it has already been given) returns the position it has actually processed. The
  framework reports `min(end of the chunk, final_through)` as the coverage of the results, never lets
  the coverage go back, and uses it as the watermark of a stack, so a decoder above never receives an
  item behind a watermark that passed it. A decoder drains what it holds in `finish`, which has the same
  deadline as a chunk, so holding work back is bounded by the time one call may take.

### Descriptor

| Field | Rules |
|-------|-------|
| `id` | ASCII lowercase letters, digits, `.` and `_`, at most 48 characters, unique in the registry |
| `version` | `u16`, starts at 1, bumped when saved options or behaviour change incompatibly |
| `name`, `description` | static message keys |
| `signals` | at most 16 `Signal { role, label, required, may_share }`; roles are identifiers, unique |
| `options` | at most 16 `OptionSpec { id, label, kind }`; `Choice` (non-empty list, default one of them), `Integer` (`min`, `max`, `default` inside), `Boolean` |
| `input` | `Input::Signals` (reads channels) or `Input::Items(kind)` (reads the output of another instance; has no signals) |
| `outputs` | the item kinds it produces |
| `generator` | whether a generator exists for it |

`Descriptor::validate` checks all of this; registering an invalid descriptor is refused and logged.

### Chunk

`Chunk { start, end, capture_length, gaps }`: the range of positions to process, the length of the
capture at the snapshot, and the gaps (lost samples) that intersect the range, sorted. A decoder must
not assume that a gap does not exist; it treats positions inside a gap as "no data" and must not guess
across it. A decoder with `Input::Items(kind)` receives `ItemChunk { items, covered }` instead of
edges: the items of that kind whose `end` lies in the chunk, in order, and the position up to which the
source has finished (the watermark).

### SignalReader (read-only access to the assigned channels)

```text
edges(start, end) -> iterator of Edge { position, role, rising }   // merged over the assigned roles, by position
level(role, position) -> Level                                    // High, Low or Missing; O(log N)
role_assigned(role) -> bool
length() -> u64
```

An edge is the change between `position - 1` and `position`; it belongs to exactly one range of any
partition of the positions (position 0 has none). Simultaneous edges come in the order of the roles of
the descriptor. The reader reads the capture's snapshot of the moment the chunk was cut: positions at or
beyond its length, positions inside a gap and unassigned roles return `Missing`. It cannot be used to
change anything.

### Sink (what a decoder writes)

```text
annotate(Annotation) -> Result<(), Limit>     // what is drawn
emit(Item)           -> Result<(), Limit>     // what a decoder above it in a stack reads
```

An **annotation** has a kind (`Range { start, end }` with `start <= end`, `Point { at }` or
`Edge { at, rising }`), a target (`Lane` or an overlay on `Channel(c)`), a style (an optional colour 0 to 7
and a shape), a text (`Message { key, args }` with at most 4 arguments of at most 64 bytes, or `Literal` of at
most 256 bytes) and an error flag. A mark on a signal (an overlay) is drawn centred in the lane of its
channel; the arrow of an edge is always in the colour of that signal, and any other mark is too unless
the decoder names a colour, so a decoder need not know about the colours the user chose. An **item** has a kind the decoder declared, a span, a payload (an
integer, at most 64 bytes of bytes or text) and an error flag; the `end` of items never decreases.

Everything written is sanitized (control, bidirectional and invisible characters removed) and validated
against the capture (positions inside it, ordered ranges, an existing channel, a style from the fixed
sets, the limits above). An invalid record is **dropped and counted** (reported once per pass as
`decoder.invalid_output`), never stored, and is not an error of the decoder. The sink also enforces the
annotation count and memory budgets: past a budget `annotate` and `emit` return `Err(Limit)` and the
decoder must return at once (`Limit` converts into a `DecodeError` with `?`).

### Errors a decoder returns

`DecodeError` is a value with a short technical detail for the log (never shown to the user and never
containing signal data). The framework turns it into `decoder.failed`. A decoder reports a *problem in
the signal* (a framing error, a missing acknowledge) as an **error annotation**, not as a `DecodeError`:
only a decoder that cannot continue returns an error.

## What the framework guarantees a decoder

- `process` is called with increasing, contiguous chunks from position 0, each within the chunk bounds,
  never concurrently for one instance, always on a worker that may be abandoned if a call exceeds its
  deadline (then the instance is dropped and nothing more is read from it).
- The `level` and `edges` answers are consistent with the snapshot of the chunk.
- A stack gives an upper decoder only items that passed validation and that the lower decoder declared.
- What is stored before a failure, a cancel or a limit is kept and marked incomplete.
- The coverage of the results is the end of the last chunk, or less if the decoder says so with
  `final_through`; it never exceeds a chunk and never decreases.

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

- `SignalSource::fill` is a deterministic function of the position for any chunking (the existing
  contract of the virtual device).
- `expected` returns the items (content) the generator produced up to `length`, which is what the
  round-trip test compares with the decoder's output. Planted errors are part of the expected result.
- A generator encodes at its own level; version 1.0 ships the single-level case and fixes the trait for
  the stacked one.

The I2C decoder (`decoders::i2c`, [decoders/i2c.md](decoders/i2c.md)) is the reference for a decoder
that uses `final_through`, items, overlays and a generator with planted errors.

Implemented as `generators::clocked_bits` (`source(setup, seed)`, `expected(setup, seed, length)`,
`expected_errors(setup, seed, length)` and `claim(setup)`) and combined with the random signals of the
device by `generators::composite::CompositeFactory`; the host builds the factory from the instances when
a capture starts on a simulated device. The generator options of an instance are kept in
`Setup::generator` (`errors`: 0 or 1, `error_rate`: about one byte in this many is cut short). The keys
`errors` and `error_rate` are the ones the application edits for every decoder that has a generator:
a generator reads them with the meaning "plant errors" and "about one unit of its own content in this
many", and ignores any other key it does not know.

## Registering a decoder

Registering a descriptor (and its `create` and generator) in the registry (`registry::Registry`) is all
that is needed for it to appear in the list of decoders, its configuration dialog, its lane and its saved
setup: the application builds all of them from the descriptor. Registration validates the descriptor and
refuses an invalid one or a repeated id. A setup (`setup::Setup`) is validated as a whole against the
descriptor, the capture and the other instances: unknown decoder, channels that do not exist or are
missing or shared, options of the wrong type or range, a source that does not exist, does not produce
what the decoder reads, closes a cycle or makes a chain deeper than 4, and the limit of 8 instances. The
error code names the cause (`decoder.invalid_setup` names the setting; `decoder.stack_cycle`,
`decoder.stack_incompatible`, `decoder.stack_too_deep`, `decoder.too_many_instances`).

## Results, lanes and overlays

The results of an instance are an annotation store with a time index (`store::Store`, readable through
immutable snapshots): window, rank (how many start before a position, and how many of those are errors)
and neighbour queries. The interface draws from a **plan** (`lanes::plan`) built per frame: spans (ranges
at least one column wide), marks (exact position) and dense cells (a count and an error count for what
shares a column or found no room), in at most 3 rows for a lane and 1 for the overlay of a channel plus
one row of dense cells. Every annotation intersecting the window is represented exactly once, in the
column it starts in; an error is never hidden in a dense cell without the error count; the cost does not
depend on how many annotations the store holds.

## Versioning

`DECODER_API_VERSION` is `1.1` (1.1 added the optional `final_through`). Adding an optional capability that existing decoders can ignore is a
minor version; changing the meaning of a call, a chunk guarantee or a limit is a major version and
requires every built-in decoder to be updated in the same merge. Each decoder has its own `version`
(the descriptor field) for its saved options.
