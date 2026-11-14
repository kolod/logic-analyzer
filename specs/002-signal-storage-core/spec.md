# Feature Specification: Signal Storage Core

**Feature Branch**: `002-signal-storage-core`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "Signal storage core (roadmap item 002): a platform-independent, UI-free component that stores captured digital signals for many channels and answers viewport queries fast. Reading the visible part of a signal costs O(log N + W), never O(N); zoomed-out views use multi-resolution summaries and never hide glitches; memory is bounded and predictable with explicit limits, and exceeding a limit is a handled error, never an out-of-memory crash; data is appendable while a capture is in progress and a reader can query concurrently; incomplete captures are flagged, never silently valid; malformed or hostile input returns typed errors, never panics; rendering performance is covered by benchmarks with a recorded baseline. Out of scope: UI, drawing, devices, decoders, session file format, export."

## Overview

The signal storage core is the foundation for everything that touches captured data:
the device layer writes samples into it, and the signal view, decoders and export read
from it. Its "users" are the developers of those components and, indirectly, end users
who need to explore very large captures smoothly and trust that what they see is
complete and correct.

Terms used in this document:

- **Capture**: one recording session holding the signals of several channels.
- **Position**: an integer sample index counted from the start of the capture; the
  capture records its sample rate so consumers can convert positions to time.
- **Chunk**: a block of consecutive samples for all channels, appended in one step.
- **Transition**: a change of a channel's level between two consecutive samples.
- **Viewport query**: a request for the part of selected channels that is visible
  in a window of the capture (a position range) rendered into a given number of
  pixel columns.
- **Pixel summary**: for one pixel column, enough information to draw it
  faithfully: the level at its start and end and how much activity happened inside it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Store a capture and read it back exactly (Priority: P1)

A producer creates a capture (channel count, sample rate), appends chunks of samples
in order, and consumers read the exact level of any channel at any position and iterate
over the exact transitions in any range. Nothing is altered or lost by storage.

**Why this priority**: every other feature depends on the store returning exactly what
was written.

**Independent Test**: append known sample patterns (constant, alternating, random,
long runs) across several chunks and verify level lookups and transition lists against
an independent reference at many positions, including chunk boundaries.

**Acceptance Scenarios**:

1. **Given** a capture with several chunks appended, **When** a consumer reads the level
   of a channel at any position, **Then** it equals the sample that was appended.
2. **Given** a range of positions, **When** a consumer asks for the transitions of a
   channel in that range, **Then** it receives exactly the positions and directions of
   the level changes, in order, with none missing or invented.
3. **Given** a transition that falls exactly on a chunk boundary, **When** the signal
   is read across the boundary, **Then** the transition appears exactly once.
4. **Given** a capture of constant signals lasting a very long time, **When** it is
   stored, **Then** it occupies memory proportional to the number of changes, not to
   the duration.

---

### User Story 2 - Fast viewport queries on very large captures (Priority: P1)

A consumer asks for what is visible in a window of a large capture, rendered into a
given number of pixel columns. The answer is produced in time that depends on the
logarithm of the capture size and on the window width in pixels, not on the capture
size, so zooming and panning stay smooth even for captures with hundreds of millions
of transitions.

**Why this priority**: interactive exploration of large captures is a core requirement
of the product (constitution Principle VIII).

**Independent Test**: benchmark viewport queries at several capture sizes spanning
three orders of magnitude and verify that query time grows with the logarithm of the
size (and linearly with pixel width), not linearly with the size.

**Acceptance Scenarios**:

1. **Given** captures with 1 thousand, 1 million and 1 billion transitions, **When** the
   same viewport query is run on each, **Then** the time grows by at most a small
   constant factor per tenfold growth and never in proportion to the size.
2. **Given** a viewport over the whole capture or over a tiny part of it, **When** the
   query runs, **Then** the time depends on the number of requested pixels, not on how
   many transitions the window covers.
3. **Given** a recorded benchmark baseline, **When** a change makes the complexity class
   worse (query time starts growing linearly with capture size), **Then** the benchmark
   check fails and the change cannot be merged.

---

### User Story 3 - Zoomed-out views never hide glitches (Priority: P1)

When the window covers more samples than there are pixels, each pixel column covers
many samples. The store reports for each pixel column whether it contains no
transitions, exactly one, or several, so a consumer can draw a single clean edge, a
flat line or a dense-activity mark, and a narrow glitch is never silently dropped.

**Why this priority**: hiding a glitch would make the analyzer misleading in exactly
the situations where it is most needed.

**Independent Test**: build captures with isolated single-sample glitches and bursts
among long flat stretches; at every zoom level, every pixel column containing a glitch
is reported as having activity.

**Acceptance Scenarios**:

1. **Given** a flat signal with one single-sample glitch, **When** viewed at any zoom
   level, **Then** the pixel column containing the glitch reports activity (more than
   one transition) and all other columns report none.
2. **Given** a pixel column with exactly one transition, **When** viewed, **Then** the
   column reports one transition together with its exact position and direction.
3. **Given** a pixel column with many transitions, **When** viewed, **Then** the column
   reports dense activity and the number of transitions (saturating at a documented
   maximum), plus the level at its start and end.
4. **Given** any viewport, **When** the per-pixel results are combined, **Then** the
   levels at column boundaries are consistent: the end level of one column equals the
   start level of the next.

---

### User Story 4 - Bounded, predictable memory (Priority: P1)

The capture has an explicit memory limit. Appending data that would exceed it does not
crash or corrupt anything: the append is rejected with a typed error, data already
stored remains fully readable, and the capture reports that it stopped because of the
limit.

**Why this priority**: an out-of-memory crash during a capture would lose the data the
user has already recorded (constitution Principle I and IV).

**Independent Test**: set a small limit, append until the limit is reached, and verify
the typed error, that all earlier data reads back exactly, and that memory use never
exceeds the limit by more than a documented margin.

**Acceptance Scenarios**:

1. **Given** a configured memory limit, **When** appends would exceed it, **Then** the
   append is rejected with a typed "limit reached" error and nothing is partially
   applied (the chunk is accepted completely or not at all).
2. **Given** a capture that reached its limit, **When** consumers query it, **Then**
   all data stored before the limit reads back exactly and queries keep their speed.
3. **Given** a capture that reached its limit, **When** its status is inspected,
   **Then** it states that it stopped because the memory limit was reached.
4. **Given** the worst case for storage (a level change at every sample), **When** the
   limit is set, **Then** the limit is still honoured; the memory required per sample
   in the worst case is documented.

---

### User Story 5 - Read while a capture is in progress (Priority: P2)

While a producer keeps appending chunks, consumers (for example a live signal view)
issue viewport queries at the same time. Readers see a consistent snapshot, the
producer is never blocked for long, and readers never observe half-appended chunks.

**Why this priority**: live display during capture is expected by users; it builds on
the P1 stories but is not needed for offline analysis.

**Independent Test**: run a producer appending continuously at a high rate while
several readers query repeatedly; verify every reader result matches a consistent
prefix of the data, and measure the producer's worst-case append delay.

**Acceptance Scenarios**:

1. **Given** a producer appending continuously, **When** readers query, **Then** every
   result reflects all chunks up to some complete chunk boundary and none beyond.
2. **Given** readers querying continuously, **When** the producer appends, **Then** no
   single append is delayed beyond a documented maximum by reader activity.
3. **Given** a reader holding a result, **When** more data is appended, **Then** the
   earlier result remains valid and unchanged.
4. **Given** a capture in progress, **When** a reader asks for its length, **Then** it
   receives the number of positions that are fully available.

---

### User Story 6 - Incomplete captures are flagged (Priority: P2)

When the producer reports that samples were lost (for example a device buffer overrun),
the store records where data is missing and flags the capture as incomplete. Consumers
can see the gaps, and a capture with gaps is never presented as complete.

**Why this priority**: silently treating a capture with lost samples as valid would
produce wrong analysis results; the constitution requires such captures to be flagged.

**Independent Test**: append chunks with a reported gap between them; verify the
capture is flagged incomplete, the gap is reported at its exact position and length, and
queries across the gap say so.

**Acceptance Scenarios**:

1. **Given** the producer reports that N positions were lost, **When** the gap is
   recorded, **Then** the capture is flagged incomplete and the gap's position and
   length are available.
2. **Given** a viewport that covers a gap, **When** it is queried, **Then** the affected
   pixel columns are marked as containing missing data instead of showing invented
   levels.
3. **Given** a capture marked complete, **When** inspected, **Then** it reports no gaps;
   a capture cannot be marked complete if gaps were recorded or the limit stopped it.
4. **Given** an incomplete capture, **When** its status is read, **Then** it states the
   reasons (lost samples, memory limit) and the total number of lost positions.

---

### User Story 7 - Hostile or malformed input never crashes the store (Priority: P1)

Every way of calling the store with unacceptable input — chunks out of order or
overlapping, chunk sizes or channel counts outside limits, zero sample rate, positions
that would overflow, queries outside the capture — results in a typed error or a
defined empty/clamped answer, never a panic, and never changes data already stored.

**Why this priority**: the stability and data-integrity goal of the project applies
most strongly to the component that holds the user's data.

**Independent Test**: feed a large set of generated invalid and extreme inputs
(including random bytes where applicable) and confirm that every call returns a typed
error or a defined result, that no call panics, and that the capture's data is
unchanged afterwards.

**Acceptance Scenarios**:

1. **Given** a chunk whose start position does not continue the capture, **When** it is
   appended, **Then** it is rejected with a typed error and the capture is unchanged.
2. **Given** a chunk larger than the maximum allowed chunk size, **When** it is
   appended, **Then** it is rejected with a typed error before any large allocation.
3. **Given** a channel count of zero or above the limit, or a sample rate of zero,
   **When** a capture is created, **Then** creation fails with a typed error.
4. **Given** a viewport with a zero width, a reversed range, a range beyond the end of
   the capture, or an enormous pixel count, **When** queried, **Then** the result is a
   typed error or a defined empty/clamped answer, never a panic or an unbounded
   allocation.
5. **Given** positions near the largest representable value, **When** used in appends
   or queries, **Then** no arithmetic overflow occurs.

---

### Edge Cases

- A capture with no samples: queries return a defined empty result.
- A capture with a single sample.
- A viewport that starts before position 0 or ends after the last position: it is
  clamped or reported as outside the capture, consistently documented.
- A viewport narrower than one sample per pixel (zoomed in beyond sample resolution):
  each sample spans several pixels; results remain exact.
- Constant signal for a very long time followed by a single transition.
- Alternating signal at every sample (the worst case for memory and for query cost).
- Many channels changing at the same position: each channel is independent.
- A chunk that exactly fills the remaining memory budget, and one that exceeds it by a
  single sample.
- A reader holding results while the producer appends or reaches the limit.
- A gap at the very start, the very end, or adjacent to another gap of the capture.
- Appending after the capture was marked complete or closed: rejected with a typed
  error.
- A panic in a consumer thread while it holds read access does not block the producer
  or corrupt the store (see the worker containment from roadmap item 001).

## Requirements *(mandatory)*

### Functional Requirements

**Storage and exact readback**

- **FR-001**: The component MUST create a capture from a channel count, a sample rate
  and resource limits, and MUST reject invalid parameters with a typed error.
- **FR-002**: The component MUST accept sample data in chunks whose start position
  continues the capture, for all channels at once.
- **FR-003**: The component MUST return, for any channel and position, exactly the
  level that was appended.
- **FR-004**: The component MUST return, for any channel and position range, exactly
  the transitions (position and direction) within it, in order, with no omissions or
  duplicates, including across chunk boundaries.
- **FR-005**: Storage cost for a signal MUST depend on the number of transitions, not
  on its duration.
- **FR-006**: The component MUST be independent of any UI, drawing, device, decoder
  or file-format code and MUST be usable and testable headlessly.

**Viewport queries**

- **FR-007**: A viewport query MUST take a channel set, a position range and a pixel
  count, and return one pixel summary per requested pixel column and channel.
- **FR-008**: The time cost of a viewport query for one channel MUST be
  O(log N + W) where N is the number of stored transitions and W the number of
  requested pixels, and MUST NOT depend on the number of transitions inside the
  window.
- **FR-009**: Each pixel summary MUST include: the level at the start and at the end
  of the column, the number of transitions in the column (saturating at a documented
  maximum), and a flag for missing data.
- **FR-010**: A pixel column containing exactly one transition MUST report its exact
  position and direction.
- **FR-011**: A pixel column containing more than one transition MUST be reported as
  having multiple transitions (dense activity) and MUST never be indistinguishable from
  a column without transitions.
- **FR-012**: The end level of each pixel column MUST equal the start level of the
  next column of the same channel.
- **FR-013**: When the window spans fewer samples than pixels, results MUST remain
  exact (no information is invented or lost).
- **FR-014**: Viewport results MUST NOT be larger than the requested pixel count times
  the requested channels, and the pixel count MUST be bounded by a documented maximum.

**Memory and limits**

- **FR-015**: The capture MUST enforce an explicit, configurable limit on memory used
  for stored signal data and its indexes, and MUST document the worst-case memory per
  sample.
- **FR-016**: An append that would exceed the limit MUST be rejected with a typed
  error, atomically: a chunk is accepted completely or not at all.
- **FR-017**: After the limit is reached, all stored data MUST remain readable and
  correct, and the capture MUST report that it stopped because of the limit.
- **FR-018**: The component MUST NOT allocate memory proportional to untrusted size
  fields (chunk length, pixel count, channel count) before validating them against
  limits.

**Concurrency**

- **FR-019**: Queries MUST be possible while a producer appends. Each query MUST
  observe a consistent snapshot that contains only complete chunks.
- **FR-020**: Reader activity MUST NOT delay an append beyond a documented maximum, and
  a slow or failed reader MUST NOT prevent appends or corrupt data.
- **FR-021**: Results already returned MUST remain valid and unchanged when more data
  is appended.

**Completeness**

- **FR-022**: The producer MUST be able to record lost positions (a gap) at a given
  position and length; the capture MUST then be flagged incomplete and report each gap
  and the total lost positions.
- **FR-023**: Pixel columns overlapping a gap MUST be flagged as containing missing
  data; levels MUST NOT be invented for lost positions.
- **FR-024**: A capture MUST be reported as complete only if it has no gaps, was not
  stopped by a limit, and was explicitly closed by the producer; appends after closing
  MUST be rejected.

**Robustness**

- **FR-025**: Every public operation MUST return a typed error (using the shared error
  model) or a defined result for invalid input; no public operation may panic on
  external input (out-of-order or overlapping chunks, sizes or counts outside limits,
  zero sample rate, reversed or out-of-range windows, arithmetic overflow).
- **FR-026**: A rejected operation MUST NOT change data already stored.
- **FR-027**: Significant events (capture created, limit reached, gap recorded, capture
  closed) MUST be logged with metadata only (sizes, counts, positions), never signal
  data.

**Performance verification**

- **FR-028**: The repository MUST contain benchmarks for viewport query cost across
  capture sizes spanning at least three orders of magnitude and for append
  throughput, with a recorded baseline, and a check that fails when the complexity
  class of viewport queries worsens.

### Key Entities

- **Capture**: one recording session: channel count, sample rate, limits, status
  (in progress, complete, incomplete, stopped by limit), length, gaps.
- **Channel signal**: the sequence of levels of one channel over positions, stored by
  its transitions.
- **Chunk**: a block of consecutive samples for all channels, with a start position.
- **Transition**: a position and a direction (rising or falling) on one channel.
- **Gap**: a range of positions with no data because samples were lost.
- **Viewport query**: channel set, position range, pixel count.
- **Pixel summary**: start level, end level, transition count (saturating), single
  transition detail, missing-data flag.
- **Limits**: maximum channel count, maximum chunk size, maximum pixel count, memory
  budget.
- **Capture status**: complete / incomplete with reasons (lost positions, limit).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Level and transition lookups match an independent reference for 100% of
  generated test captures, including all chunk boundaries.
- **SC-002**: A viewport query for 2,000 pixels and 16 channels on a capture with
  1 billion transitions completes in under 20 milliseconds on the reference machine
  defined in the plan, so that interactive zoom and pan stay smooth.
- **SC-003**: Growing a capture from 1 million to 1 billion transitions (1,000 times)
  increases viewport query time by less than 3 times.
- **SC-004**: Across all generated glitch tests, 100% of pixel columns that contain a
  transition are reported as containing at least one, and 100% of columns with more
  than one transition are distinguishable from columns with at most one, at every
  zoom level tested.
- **SC-005**: Memory used for stored data never exceeds the configured limit; when the
  limit is reached, 100% of previously stored data reads back exactly and the typed
  error is returned for the rejected append.
- **SC-006**: The store sustains ingestion at a 100 MHz sample rate on 32 channels
  (the highest rates among the supported devices) without rejecting or delaying
  appends beyond the documented maximum, on the reference machine, for signals with
  up to one level change per 50 samples per channel (for example a 2 MHz signal on
  every channel at 100 MS/s). Denser signals are measured and reported but not held
  to this target: at one change per 20 samples per channel the rate is lower, and the
  absolute worst case (a change on every channel at every sample) would need about
  13 GB of memory writes per second at 100 MHz and 32 channels, which no in-memory
  store can sustain; its rate is reported for information.
- **SC-007**: While a producer appends continuously, readers' results are consistent
  with a complete-chunk prefix in 100% of 1,000,000 sampled queries, and the
  producer's worst-case append delay stays within the documented maximum.
- **SC-008**: Of a corpus of at least 10,000 generated invalid or extreme inputs, 0
  cause a panic and 100% leave previously stored data unchanged.
- **SC-009**: A capture with lost samples is never reported as complete in any
  tested scenario.
- **SC-010**: A change that makes viewport query time grow linearly with capture size
  fails the benchmark check in the merge gate.

## Assumptions

- Positions are 64-bit sample indices counted from the capture start; time is derived
  from the stored sample rate by consumers.
- Channels are digital (two levels). Analog data is out of scope.
- The default maximum channel count is 32 (above the highest count of the supported
  devices); it is a limit set at capture creation and can be raised deliberately.
- The default memory limit, maximum chunk size, maximum pixel count and the saturating
  transition count are chosen in the plan and are documented.
- When the memory limit is reached, appending stops with a typed error and stored data
  remains available. Spilling to disk or other controlled continuation is out of scope
  for this feature and may be added by a later feature (for example together with the
  session file format).
- The reference machine for the performance targets is a typical current desktop
  computer and is defined in the plan, together with how benchmark baselines are
  recorded and compared.
- Consumers of this component are the device layer (producer), the signal view,
  decoders and export (readers); none of them exist yet, so this feature is verified
  with test producers and readers only.
- Persisting captures to disk is out of scope (session file format, roadmap item 003).
- Uses the shared typed error model and the logging of roadmap item 001.
