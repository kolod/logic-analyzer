# Research: Decoder Framework and Annotations

**Feature**: 007-decoder-framework-annotations | **Date**: 2026-11-15

Every decision below answers a question the plan had to settle before design. Nothing is left
open as "to be clarified": where a value is a default, it says so and where it can be changed.
References to constitution principles are to `.specify/memory/constitution.md`; references to
earlier features are to their `specs/` directories and to the code as merged.

What the existing code gives this feature (checked, not assumed):

- **Storage** (`la-storage`): an immutable `Snapshot` with `transitions(channel, start, end)` (an
  ordered iterator of edges), `level_at(channel, position)` (indexed), `gaps()`, `status()`,
  `length()` and `sample_rate_hz()`; a `CaptureReader` that is `Send + Sync` and takes a new snapshot
  of a growing capture at any time. Decoders need nothing else from the capture.
- **Devices** (`la-devices`): the virtual device takes a `SignalSourceFactory` whose `SignalSource`
  fills samples for increasing positions and must be deterministic for any chunking. That is the
  generator interface.
- **Session** (`la-session`): optional sections are opaque `(tag, payload)` pairs, preserved when a
  file is saved again, counted against the memory budget when loaded. `VIEW` (feature 006) is the
  model for a section whose reader lives in the host.
- **Diagnostics** (`la-diagnostics`): `spawn_worker` (a panic is caught at the thread boundary and
  returned as a failure) and `lock_recover`. They are the containment primitives.
- **Host** (`la-host`) and **application** (`la-app`): commands, state and frame plan as plain data;
  a canvas that draws a plan; side panels and menus built from the shortcut table.

## R1. Where the decoder framework lives

**Decision**: a new crate `la-decoders` (directory `crates/decoders`) with no UI dependency. It holds
the annotation model, the decoder interface and registry, the annotation store and its index, the
execution (jobs, supervision, budgets), stacking, the built-in sample decoder with its generator, and
the reader and writer of the persisted setup and results. `la-host` depends on it and orchestrates;
`la-app` only draws plans and sends commands. `la-devices` does not depend on `la-decoders` (the
generator implements the existing `SignalSourceFactory`, so the dependency points the other way:
`la-decoders` depends on `la-devices`).

**Rationale**: Principle IV (domain logic in UI-free crates) and the layout already sketched in the
plan template (`decoders/`). A separate crate keeps decoder faults, tests, fuzzing and benchmarks
apart from the host.

**Alternatives considered**: inside `la-host` (rejected: the host is the glue to the UI and is
already large; decoders must also be usable by the export of feature 009 without the host); inside
`la-storage` (rejected: storage must stay a pure signal store).

## R2. The shape of the decoder interface

**Decision**: a decoder is a **resumable state machine fed in chunks**, defined by this project:

```text
descriptor()                      what it is: id, version, signals, options, input, output, texts
create(setup) -> instance         a fresh state for one pass (validated setup)
process(chunk, sink) -> result    one step over a range of positions
```

The framework drives the decoder over successive position ranges of the capture (a *chunk*, at most
a few ten thousand edges and a bounded span). The decoder keeps its own state between chunks, reads
signals through a read-only `SignalReader` (merged edges of its assigned channels in position order
inside the chunk, `level_at` for a role at a position, and the gaps inside the chunk) and writes
through a `Sink` (annotations and output items). It never sees the capture or the storage.

**Rationale**:

- *Cancellation, progress and responsiveness* (FR-012, FR-013): the framework regains control after
  every chunk, so it can check the cancel flag, publish results, report progress and enforce budgets
  without the decoder's cooperation beyond returning from a chunk.
- *A growing capture* (FR-015): a pass is "continue from the position where the last chunk ended",
  with a new snapshot each time; nothing is recomputed.
- *Stacking* (FR-018): an upper decoder consumes a stream of output items in the same chunked way.
- *Safety*: the decoder interface is narrow and read-only, so a decoder cannot write to the capture;
  panics, errors and invalid output are all visible at the chunk boundary.
- *Simplicity* for decoder authors: a pull-style "wait for the next edge" interface (as in
  libsigrokdecode) needs a coroutine or a thread per decoder and cannot be resumed after growth
  without extra machinery; a state machine over edges is the natural form for protocol decoders and is
  easy to test with a recorded edge list.

**Alternatives considered**: a pull interface with a blocking `wait(condition)` (rejected: needs a
thread per instance with channels between the framework and the decoder, makes cancellation depend on
the decoder, and complicates resuming); a full-capture `decode(&Snapshot) -> Vec<Annotation>` function
(rejected: no progress, no cancellation inside it, no growth, no bounded memory); a scripting language
for decoders (rejected: out of scope, and a different isolation problem, see R9).

## R3. Annotations, output items and their separation

**Decision**: two kinds of records leave a decoder, both through the sink:

- **Annotation**: what is drawn. Fields: `kind` (`Range { start, end }`, `Point { at }`,
  `Edge { at, rising }`), `target` (`Lane` or `Channel(channel)`), `style` (a colour index from a
  fixed set of 8 and a shape from a fixed set: `Dot`, `Arrow`, `Bar`, `Flag`, `Cross`),
  `text` (`Message { key, args }` with a static key and up to 4 arguments, or `Literal(String)`),
  `error` (a flag), and `row_hint` (optional nesting hint). Positions are sample positions.
- **Item**: what an upper decoder reads. Fields: a `kind` (a static identifier such as `"byte"`), a
  `start`/`end` span, and a small `payload` (an integer, a short byte string or a short text). A
  decoder declares the item kinds it produces and the kinds it accepts; the framework checks that a
  stack matches.

An item does not draw anything and an annotation carries no meaning for a stack; a decoder usually
emits both for the same event. The store keeps each in its own indexed structure per instance.

**Rationale**: the constitution's annotation model (Principle IX: range, point, edge marker, target,
style, text, independent of any toolkit) is a drawing model; stacking needs typed data. Mixing them
would force upper decoders to parse display text. Messages with static keys make the standard
annotation texts of built-in decoders localizable (FR-008, FR-030) while literal text stays as typed.

**Alternatives considered**: items as annotations with a payload (rejected: bloats every annotation by
the payload and makes the stacking contract implicit); decoders emitting display strings only
(rejected: not localizable and not stackable).

## R4. The annotation store and its index

**Decision**: per instance, an append-only store of annotations with a **time index**:

- Annotations are kept in blocks in emission order. A decoder emits ranges after the points inside
  them, so emission order is not start order.
- The index is a small number of immutable **sorted runs** (sorted by start, each with an implicit
  segment tree of maximum end) built by merging newly emitted annotations geometrically (the
  standard logarithmic method: a run of size n is merged with the next of the same level). A query for
  a window binary-searches each run for the last start before the window end and walks the tree for
  entries whose end reaches the window start: `O(log n + m)` per run.
- While a pass is running there are at most `R = 16` runs plus a small unsorted tail of at most 4,096
  annotations that is scanned linearly. When a pass finishes, a background step compacts to a single run.
  A finished store therefore answers a window query in `O(log n + m)`; a growing store in
  `O(R·log n + m + 4,096)` with `R` bounded by a constant. The plan states this honestly and a
  probe-count test (like `crates/storage/tests/complexity.rs`) pins it.
- For zoomed-out drawing the number of annotations per pixel column is the difference of two rank
  queries on the sorted starts (`O(log n)` per column boundary, `W + 1` boundaries), so a column
  with many annotations costs the same as one with few. Long ranges that span many columns are fetched
  by the interval query; the number of rows they need is capped (R5).
- Memory is accounted per block; the store refuses to grow past its budget and tells the job (R7).

**Rationale**: Principle IX (overlay lookup stays indexed) and VIII (cost independent of the capture
length): the index must answer "what intersects this window" without scanning, with ranges that
overlap and nest. The logarithmic method gives amortised `O(log n)` insertion, supports progressive
results and lets finished stores be compact and immutable (which also makes the snapshot handed to
the interface trivial: an `Arc` of runs).

**Alternatives considered**: a balanced interval tree with mutation (rejected: mutable shared
structure, awkward snapshots for the interface thread, more code with more ways to fail); sorting the
whole store after every chunk (rejected: `O(n)` per publish, breaks progressive display on big
results); a fixed time grid of buckets (rejected: ranges that are long or nested defeat a grid, and
one-sample annotations among millions need exact answers, SC-003).

## R5. Turning annotations into drawing without losing any (the lane and overlay plan)

**Decision**: the frame plan of feature 006 gains, per decoder instance, a **lane plan** and, per
channel, an **overlay plan**, both built from one window query per instance:

- *Columns.* The window is divided into the same `W` columns as the waveforms. An annotation that
  alone occupies a column (point, edge, or a range that ends in it) becomes a **cell** with its exact
  sub-pixel position. Where `k > 1` point-like annotations share a column the cell is a **dense
  cell** with the count `k` and how many of them are errors (errors stay visibly distinct). Ranges
  wider than a column are drawn as spans with their text and an ellipsis when the text does not fit
  (R10); ranges narrower than a column are counted like points.
- *Rows.* Overlapping ranges in a lane are assigned to rows by a greedy interval colouring computed
  for the window (not stored), with at most 4 rows per lane; a range that does not fit a row is
  merged into the dense cell of its columns. This keeps the plan `O(W)` and never drops information
  silently: the dense cell says "there is more here" and zooming in separates it.
- *Cost.* One query per instance, `O(log n + W log n)` for the rank counts plus the number of
  annotations actually drawn (bounded by `4W` per lane by construction); drawing then costs `O(W)`.
- *Overlays* follow the channel they target: filtered from the same query by target, on top of the
  waveform, hidden when the channel's flag is off.

**Rationale**: Principle VIII's rule that a pixel with several items is visibly distinguished from one
with none or one, applied to annotations (FR-027, SC-003); a bounded plan keeps frame time independent
of the number of annotations.

**Alternatives considered**: drawing every annotation in the window (rejected: `O(N_w)`, breaks the
bound on dense results); levels of detail precomputed per zoom (rejected: memory and invalidation on
growth, and a query-time summary is cheap enough).

## R6. Execution: jobs, chunks, budgets and the supervisor

**Decision**: each decoder instance has at most one **job** at a time. A job runs on a supervised
worker (`spawn_worker`, so a panic is caught at the thread boundary) and loops:

1. take a snapshot of the capture (a new one each round, so growth is followed);
2. cut the next chunk (at most 65,536 edges and a bounded position span, so one chunk takes a few
   milliseconds), run the decoder on it, and validate and store what it emitted;
3. publish the new annotations to the store, update the progress (an atomic), and check the cancel
   flag, the memory and annotation budgets and the deadline;
4. yield to the capture (R8) and repeat until the end of the capture, or until it is waiting for a
   growing capture to produce more.

Cancelling sets the flag; the job stops at the next chunk boundary (well within one second).
A **supervisor** inside the host's `poll` watches each job's heartbeat. If a chunk takes longer than
the deadline (default 5 s) or the job does not stop within 1 s of a cancel, the worker is
**abandoned**: its generation number is retired so anything it still emits is ignored, the instance is
marked failed with `decoder.not_responding`, and the interface never waited. A thread that cannot be
stopped is leaked until it returns; the number of abandoned threads is counted and, past a small
limit (4), new passes are refused with a message that names the cause (a restart is then advised),
so a faulty decoder cannot exhaust the process.

Budgets (defaults, settable where noted): annotations per instance 5,000,000 (counted) and bytes per
instance within the decoder results memory limit (default 256 MiB in total, 16 MiB to 4 GiB, a
visible setting like the capture memory limit); text per annotation 256 bytes; chunk deadline 5 s.

**Rationale**: FR-012 to FR-017, FR-020 to FR-022 and the constitution's "a decoder error, panic, hang
or excessive memory use MUST be contained". Chunking gives the framework control without trusting the
decoder; abandoning gives a bound on the wait for a decoder that never returns.

**Alternatives considered**: a separate process per decoder (rejected for this feature: strong
isolation but a new IPC protocol, serialization of every chunk and a packaging burden; the design keeps
the interface narrow so a process boundary can be added later, see R9); killing the thread (not
possible safely in Rust); running decoders on the interface's executor (rejected: blocks).

## R7. Faults, stable error codes and what is kept

**Decision**: every fault is mapped at the job boundary to a stable code in a new domain
`decoder`, defined in the decoders crate's `error.rs` (so the existing sweep that injects every
code finds them) with a message in each of the six languages:

| Code | Cause | What is kept |
|------|-------|--------------|
| `decoder.failed` | the decoder returned an error | annotations before it, marked incomplete |
| `decoder.panicked` | the decoder panicked | annotations before it, marked incomplete |
| `decoder.not_responding` | a chunk or a cancel exceeded its deadline | annotations published before, marked incomplete |
| `decoder.memory_limit` | the store reached the decoder results memory limit | everything stored, marked incomplete |
| `decoder.annotation_limit` | the annotation count budget was reached | everything stored, marked incomplete |
| `decoder.invalid_output` | refused annotations or items (counted, reported once) | the valid ones |
| `decoder.invalid_setup` | a refused channel or option choice (before any run) | nothing changes |
| `decoder.stack_cycle`, `decoder.stack_incompatible`, `decoder.stack_too_deep` | a refused stacking choice | nothing changes |
| `decoder.too_many_instances`, `decoder.too_many_stuck` | a limit | nothing changes |
| `decoder.section_invalid`, `decoder.unknown_decoder`, `decoder.options_skipped` | the saved setup (R11) | the valid part opens |

A message names the instance (its name is user text, shown as typed) and the code; the technical
detail goes to the log (code, decoder id, position, never annotation text or signal data). Repeated
faults are grouped as in feature 006. A fault in a lower decoder marks its upper decoders incomplete
and says so in their state; it never makes them fail.

**Rationale**: FR-020 to FR-023, SC-005, Principle VI (actionable messages, details only in the log).

## R8. Priority of the capture over decoding

**Decision**: a job yields after every chunk (`yield_now`, and a short sleep of 1 ms per chunk while a
capture is running at a high rate) and runs on at most `max(1, cores / 2)` decode threads at a time;
instances queue. The capture's ingestion is never touched by a decoder: it reads through snapshots of
immutable published blocks and holds no lock the ingestion needs. A test compares the number of samples
lost by the virtual device with and without a decoder at its highest rate (SC-009).

**Rationale**: FR-015 (the capture's priority is absolute). Thread priorities are not portable in the
standard library, so yielding and a bounded thread count are the portable controls; the measured
loss is the check.

**Alternatives considered**: lowering thread priorities with platform calls (rejected: `unsafe`, two
implementations, not needed if the measurement passes; revisit only if the measurement fails).

## R9. Isolation level and the case for later process isolation

**Decision**: contain in the process (R6, R7): a watched worker, budgets, a narrow read-only
interface. The decoder interface is defined so that a decoder could later run in another process: it
takes plain values in and gives plain values out, with no shared references. Decoders are built into
the application in this feature (FR-040), so the only code that runs is code reviewed with the
product.

**Rationale**: Principle I asks that a decoder fault never take down the application or lose the
session; for reviewed built-in decoders, in-process containment meets it, and the sweep of injected
faults (SC-005) is the proof. User-written decoders would change the threat model (arbitrary code) and
need their own feature and a process or WebAssembly boundary; that decision is deferred explicitly.

## R10. Showing annotation text, selection and navigation

**Decision**: a range's text is drawn clipped to its width with an ellipsis; the full text and the
details (kind, value, start, end, length in time formatted for the language, error flag) appear in the
decoder panel for the selected annotation and in a hover readout drawn on the canvas like feature 006's
time readout. Clicking selects the annotation under the pointer (found by a point query); the view
jumps to it with a zoom that shows it whole. `Next annotation` and `Previous annotation` (menu and
keys) move the selection through one instance's annotations in start order using the index (a rank
query), so they cost `O(log n)` however large the results are. Literal text and the text of messages are
sanitized and limited as every outside text (the sanitizer of feature 006).

**Rationale**: FR-027, FR-028, SC-010; the cost bound for navigation follows Principle VIII's spirit.

## R11. The saved setup and results (the `DECO` and `DANN` sections)

**Decision**: two optional, non-critical sections of the session format, specified in
`contracts/deco-section.md`:

- **`DECO`** (the setup): written only when there are instances. One record per instance (length
  prefixed so a reader that does not know it can keep the bytes): instance id, decoder id and version,
  name, source instance, channel assignment per signal, option values by option identifier, lane
  visibility, flags. Small by construction (bounded by 8 instances).
- **`DANN`** (the results): written only when the user chose to save them; one block per instance with
  its covered range, incomplete flag and annotations and items in a compact varint encoding with a
  string table for message keys. Counted against the memory budget on load like every optional
  section, so a hostile file cannot exhaust memory.

Reading follows the pattern proven by `VIEW`: every byte untrusted, canonical varints, bounded counts,
an invalid section drops with a warning and the capture opens; a newer section version is kept
untouched with an information message; an instance of an unknown decoder, or with options this version
does not know, keeps its record untouched for write-back. Writing is deterministic. The session format
minor version becomes 2 (the writer's rule 1: new optional sections add a minor version) with a new
fixture directory; version 1.0 and 1.1 files still load.

**Rationale**: FR-031 to FR-034, SC-008; Principle I (validation, atomic files, nothing lost). Two
sections keep the small setup readable and droppable independently of the large results.

**Alternatives considered**: one section with both (rejected: results can be large and are optional;
a damaged result must not cost the setup); storing results in a side file (rejected: breaks "one file"
and atomicity).

## R12. Stacking: the graph, propagation and outdated results

**Decision**: instances form a directed acyclic graph (each has at most one source: another instance
or the capture). A decoder declares what it accepts (`Signals`, or `Items(kind)`) and what it produces
(`Items(kind)`s); the framework checks the match at configuration time, refuses cycles (`decoder.stack_cycle`)
and depth beyond 4. Every instance carries a **setup version** and records the **source version** it
consumed. A change (new setup, new pass, removal) bumps the version and marks every instance above it
outdated until it re-runs; the old results stay visible, marked outdated, until replaced (FR-017,
FR-019). An upper job consumes the lower store's items up to the lower job's *covered position* (the
watermark), so a growing capture flows through a stack incrementally. Removing a lower instance with
instances above asks first (a host decision like the unsaved-data question).

**Rationale**: FR-018, FR-019, Principle IX. A single source per instance matches the roadmap's
stacking (010) and keeps the graph a forest; joining several sources is not needed yet.

## R13. The sample decoder ("Clocked bits") and its generator

**Decision**: the decoder id is `sample.clocked_bits`, version 1. Signals: `clock` (required) and
`data` (required). Options: `clock_edge` (rising or falling, default rising), `bit_order` (most
significant first or least significant first, default most significant first), `bits` (4 to 16, default
8), `idle` (idle limit in samples, 1 to 1,000,000, default 100). Behaviour: on every chosen clock edge it
samples the data level at that position (`level_at`), collecting bits; when no clock edge arrives for
`idle` samples, or at a gap or the end of the capture, the byte ends: with all `bits` bits it is a byte,
with fewer it is an **incomplete byte** error. Output: a range per byte in the lane (message
`"byte {value}"` in hexadecimal), a point per bit in the lane, an edge arrow on the clock channel at each
sampled edge, a dot on the data channel at the byte's first sampled edge, error annotations for an
incomplete byte and for a bit that would fall into a gap, and one item per byte (`"byte"`) for stacking
tests.

The generator (`SignalSource` created through `SignalSourceFactory`) is a pure state machine over the
position: from the seed it draws bytes, encodes each as `bits` clock periods (a fixed period, default 10
samples) on the clock and data channels, separates bytes by an idle of at least `idle + period` samples,
and, when deliberate errors are on, truncates some bytes to fewer bits. The expected content for a seed
and length is available as a function, which is what the round-trip test compares against (R14). The
virtual device combines it with random signals on all other channels.

**Rationale**: FR-035 to FR-038, SC-006. The decoder is simple on purpose (see the spec's assumptions)
but touches every part of the framework: lane ranges and points, overlay arrows and dots, error
annotations, gaps, stacking items, options of two types, a generator with planted errors.

**Alternatives considered**: a UART decoder (rejected: a real protocol that later features own and that
would invite scope creep into baud-rate handling); a pulse-width decoder (rejected: no clock edge, so no
edge-marker overlay to prove).

## R14. How decoders reach the virtual device (Principle XI)

**Decision**: the host asks every instance that has a generator for a `SignalSourceFactory` when a
capture starts on a simulated device and passes, to the virtual device, a composite factory: the
channels claimed by generator instances are filled by their generators, all others by the random source
(with the seed the user gave). A generator therefore follows the decoder setup, which is what the
principle asks ("if a decoder is configured, the generated signals MUST be valid for it"). For a stack,
a generator at the top produces items and encodes them down through the generators of the lower
decoders; this feature defines the trait for it (`Generator::encode`) and ships the single-level case,
because one decoder ships; feature 008 and 010 use it. The generator's own options (deliberate errors on
or off and their rate) belong to the instance and are saved with it.

**Rationale**: Principle XI, FR-036. Deriving the generator from the decoder setup, instead of asking
the user to configure both, means they cannot disagree.

## R15. The place in the application (FR-024 to FR-029)

**Decision**: the application learns about decoders from **data** that the host provides, not from
code: a catalogue of descriptors (id, localized name and description, signals with roles, options with
types, ranges and choices, input and output kinds). One generic "add decoder" dialog and one generic
configuration form are built from a descriptor, so a registered decoder appears in the list, the dialog
and the lane without a change in `la-app` (this is what closes FR-037 of feature 006 for decoders, and
SC-012 checks it with a test decoder). The decoder lanes are drawn by the signal canvas below the channel
lanes (the region reserved in the layout contract, taking no space when empty); the decoder panel is a
third panel in the right-hand column; menu entries and keys come from the shortcut table like every
command. Option and signal labels, decoder names and the standard annotation texts are messages with
static keys declared in the descriptor, so the localization parity check sees them.

**Rationale**: FR-029, FR-030; also Principle VII (keys are literals the parity check can find).

## R16. Dependencies

**Decision**: none new. The crate uses the standard library and the existing workspace crates
(`la-core`, `la-storage`, `la-devices`, `la-diagnostics`, `la-i18n`). The persisted format uses its own
small canonical varint code (as `view_section` does), no serialization crate.

**Rationale**: Principle V. The MSRV (1.85) holds: no `let` chains, nothing newer.

## R17. Testing strategy (Principle III, IX, XI)

**Decision**, all runnable without hardware:

- *Model and index*: unit and property tests of validation, of the store against a brute-force
  reference over random annotations (overlap, nesting, equal positions), probe-count test of the
  window query and of the rank counts (cost independent of the capture and result size), a glitch-like
  test (one one-sample annotation among millions is found at every zoom, SC-003).
- *Fault injection*: test decoders behind a `test-support` feature fail in every way of the table in R7
  next to a healthy one; assertions on capture bytes, the healthy decoder's results, the message code
  and the log (SC-005).
- *Execution*: cancel latency, progress monotonicity, growth (a decoder that follows a capture appended
  to while it runs), gaps, outdated propagation in a two-decoder stack.
- *Persistence*: hand-written byte vectors for `DECO` and `DANN`, round trips, every validation rule
  with a failing fixture, a mutation fuzzer (like `view_section`), the fixtures of the new format minor
  version, old files still load.
- *Sample decoder*: recorded edge lists including malformed and truncated input; round trip over 10,000
  seeds in release mode (1,000 in the normal run) with planted errors reported at the right places.
- *Host*: commands, state, plan; the random walk of feature 006 extended with decoder actions (100,000
  actions in the tests, two million nightly); the shortcut coverage and host API document tests.
- *Application*: headless renders of the lane, overlays, dialog and panel in the six languages at the
  minimum window size; the string budget test covers the new messages.
- *Benchmarks* with a recorded baseline: annotation plan at 1 and 10 million annotations on 100 and
  1,000 million-sample captures (cost must not grow), decode throughput, store insertion; added to the
  bench workflow.
