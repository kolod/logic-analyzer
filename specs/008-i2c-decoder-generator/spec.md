# Feature Specification: I2C Decoder and Generator

**Feature Branch**: `008-i2c-decoder-generator`

**Created**: 2026-11-16

**Status**: Draft

**Input**: User description: "008-i2c-decoder-generator: I2C decoder with overlays (start/stop on SDA, sampling edge on SCL), generator for the virtual device, round-trip tests, deliberate errors (NAK). Depends on 007 (decoder framework). See roadmap.md and docs/decoder-api.md."

## Overview

Feature 007 built the machinery that reads signals and shows what they mean, and proved it with a
small sample decoder. This feature delivers the first **real protocol decoder**: I2C, the two-wire
bus that connects sensors, memories, displays and many other chips, and the protocol a logic
analyzer is most often pointed at.

What the user gets:

- **An "I2C" decoder** that can be added to any capture like the sample decoder: the user says which
  channel carries SDA (data) and which SCL (clock), and the decoder reads the transactions on the
  bus.
- **A decoder lane** that shows each transaction as it happened on the bus: the address with its
  direction (read or write), every data byte, and whether the receiver acknowledged it (ACK) or not
  (NAK); start, repeated start and stop conditions are visible as events.
- **Overlays on the signal itself**: a coloured dot on SDA where a start or a stop condition happens,
  and an arrow on SCL at the clock edge where each bit is read, so the user can check by eye that the
  decoder looked at the right place.
- **Honest errors**: a bus that misbehaves (a byte cut short by a stop, a clock that stops in the
  middle of a byte, a transaction that begins without an address) is reported where it happens and
  the decoder carries on with the rest of the capture, instead of giving up or guessing silently.
- **A generator for the virtual device**: when an I2C decoder is configured on the channels, the
  virtual device produces random but valid I2C traffic (reproducible from a seed), including
  deliberate NAKs and other errors on request, so the whole feature can be tried and tested without
  hardware (constitution Principle XI).
- **Output ready for stacking**: the decoder hands the transactions on in a form a higher-level
  decoder can consume (feature 010 builds the first one), so nothing has to be reworked later.

Nothing in the application shell changes: the decoder is added by registering it, as feature 007
promised (`docs/decoder-api.md`, "Adding a decoder"). This feature is also the test of that promise.

Out of scope: exporting the decoded log (feature 009), a higher-level decoder on top of I2C
(feature 010), SMBus and PMBus extensions (packet error checking, alert, timeouts), the Hs-mode
protocol and other I2C variants with a different bus signalling, checking bus timing against a speed
class (the decoder measures the bus period only to synchronise, and does not judge speed), glitch filtering of noisy captures, and
decoders written by users.

## Clarifications

### Session 2026-11-16

- Q: What is shown before the first valid start condition (a capture that begins mid-transaction, or before the MCU has set up its I2C interface)? → A: Nothing is drawn and nothing is reported for it; decoding begins at the first start condition.
- Q: How does the decoder decide that a start condition is real when the capture begins before the MCU has set up I2C (lines stuck, floating or noisy)? → A: It synchronises: the first start is accepted only after the bus has been seen idle, meaning both SDA and SCL high for at least one bus period (the bus clock period is determined automatically from the signal, which may take two passes). Everything before that, including falling edges of SDA while SCL is high, is ignored silently.
- Q: What happens while the bus period is not yet known (a growing capture, or too few clock pulses)? → A: The decoder waits: nothing is decoded or shown until enough SCL clock pulses have been seen to determine the period; then it decodes from the start of the capture, giving the same result as for the whole capture. If the capture ends without the period ever being determined, the result is empty and no error is reported.

### Session 2026-11-22

- Q: When does SDA change with respect to SCL in the generated signal? → A: First answer, four samples apart; replaced by the owner on 2026-11-22: SDA changes in the same sample in which SCL falls, and in a start, a repeated start and a stop the edge of SDA is 8 samples from an edge of SCL.
- Q: What does the list of channels for SDA and SCL show when a channel is assigned but not recorded? → A: The list holds only the channels that are recorded; an assigned channel that is not recorded stays selected, marked unavailable, and the decoder decodes nothing and says why.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read the transactions on an I2C bus (Priority: P1)

A user opens a capture of an I2C bus (or captures from the virtual device), adds the I2C decoder and
tells it which channel is SDA and which is SCL. The decoder lane shows one entry per transaction:
the target address and whether the controller writes or reads, then each data byte with its ACK or
NAK. On the signals, a dot marks every start and stop condition on SDA, and an arrow marks the SCL
edge at which each bit was read. Zooming in on one byte shows nine arrows (eight bits and the
acknowledge bit) on the exact clock edges.

**Why this priority**: this is the reason the feature exists. A decoder that cannot show correct
addresses, bytes and acknowledgements is of no use, and everything else builds on it.

**Independent Test**: decode a capture of a known write to a known address with two data bytes and
check that the lane shows that address, the write direction, the two bytes and their acknowledgements,
that the start and the stop are marked on SDA, and that the sampling arrows sit on the clock edges.

**Acceptance Scenarios**:

1. **Given** a capture with one write transaction (address 0x50, bytes 0x12 and 0x34, all
   acknowledged), **When** the I2C decoder is added with SDA and SCL assigned, **Then** the lane shows
   address 0x50 with the write direction, the bytes 0x12 and 0x34, each marked as acknowledged, and
   the transaction as one unit from its start to its stop.
2. **Given** the same capture, **When** the user zooms in on the first byte, **Then** nine arrows are
   drawn on SCL, one on each clock edge at which a bit was read, and a dot is drawn on SDA at the start
   and at the stop condition.
3. **Given** a capture with a read transaction, **When** it is decoded, **Then** the direction is shown
   as a read and the data bytes are attributed to the target, and the final byte of the read is shown
   with the controller's NAK that ends a read, without being reported as an error.
4. **Given** a capture with a repeated start (a write of a register number followed, without a stop,
   by a read), **When** it is decoded, **Then** the repeated start is shown as its own event and both
   parts belong to one transaction.
5. **Given** the decoder is added with SDA and SCL swapped by mistake, **When** it runs, **Then** the
   result is clearly wrong or empty but nothing fails, and the user can swap the channels and see the
   correct result without re-capturing.

---

### User Story 2 - Test the decoder against traffic the virtual device made (Priority: P1)

A user without hardware selects the virtual device, adds an I2C decoder on two channels and starts a
capture. The virtual device does not produce noise: it produces valid I2C traffic, with random
addresses, directions and data. The decoder shows exactly what was generated. The same seed gives the
same traffic every time.

**Why this priority**: constitution Principle XI requires it, and it is also how the decoder is
checked: the generator and the decoder are each other's test. It is P1 because the test suite and
the manual evaluation both depend on it.

**Independent Test**: for many seeds, generate traffic, decode it and compare the decoded
transactions with the generated ones; they must be equal.

**Acceptance Scenarios**:

1. **Given** an I2C decoder configured on two channels of the virtual device, **When** a capture is
   started with a seed, **Then** the channels carry valid I2C traffic and the decoder shows the
   transactions that were generated.
2. **Given** two captures with the same seed and configuration, **When** they are compared, **Then** the
   signals are identical sample for sample; with a different seed they differ.
3. **Given** no decoder on a channel, **When** a capture is started, **Then** that channel carries the
   virtual device's ordinary random signal, as before this feature.
4. **Given** the capture is produced in pieces of any size, **When** the signal is compared with the
   signal produced in one piece, **Then** they are identical (the result does not depend on how the
   signal is requested).

---

### User Story 3 - See the errors on purpose (Priority: P2)

A user wants to know how a bus looks, and how the decoder reports it, when something goes wrong: a
target that does not acknowledge, a transfer that is cut short. They ask the generator to include
errors at a chosen rate. The decoder lane marks each acknowledgement that did not come (NAK) and
each protocol violation in the error style, with a message that says what was wrong, at the place it
happened; the bytes before and after still decode.

**Why this priority**: the error paths are where a decoder is most likely to be wrong, and they are
what a user debugging a real bus is looking for. P2 because the decoder is useful without it, but the
feature is not finished without it.

**Independent Test**: generate traffic with errors planted at known places and check that the decoder
reports each one where it was planted and no others, and that the transactions around them still
decode.

**Acceptance Scenarios**:

1. **Given** traffic with a NAK on the address byte (no target answered), **When** it is decoded,
   **Then** the NAK is marked as an error on the address, with a message that the address was not
   acknowledged, and the transaction ends at its stop.
2. **Given** traffic with a NAK on a data byte written to a target, **When** it is decoded, **Then** the
   NAK is marked as an error on that byte; a NAK on the last byte of a read is not.
3. **Given** a transaction cut short by a stop in the middle of a byte, **When** it is decoded, **Then**
   an error is reported for the incomplete byte, the stop is still marked, and the next transaction
   decodes normally.
4. **Given** a capture that ends in the middle of a transaction, **When** it is decoded, **Then** the
   unfinished transaction is shown as incomplete (not dropped, not an error of the decoder), and when
   more samples arrive during a running capture the transaction continues from where it was.
5. **Given** a chosen error rate and seed, **When** traffic is generated twice, **Then** the same errors
   are planted at the same places.

---

### User Story 4 - Choose how the bus is read (Priority: P2)

The user can adjust how the decoder reads the bus: whether addresses are shown as 7-bit addresses
(the usual way, the address without the direction bit) or as the 8-bit value with the direction bit
included (as many data sheets and some tools show them), and the number base in which addresses and
data are displayed in the lane. The setting is saved with the session like any other decoder option.

**Why this priority**: address notation is the most common source of confusion with I2C (the same
device is "0x50" or "0xA0" depending on the convention), so a user comparing the display with a data
sheet needs the choice. P2 because the default already serves most users.

**Independent Test**: decode the same capture with each setting and check that the displayed address
changes between 0x50 and 0xA0/0xA1 and nothing else about the decoding changes.

**Acceptance Scenarios**:

1. **Given** a write to the device at 7-bit address 0x50, **When** the notation is the 7-bit address,
   **Then** the lane shows 0x50 and a write; **When** the notation is the 8-bit value, **Then** the lane
   shows 0xA0 for the write (and 0xA1 for a read).
2. **Given** the user changes the notation, **When** the change is made, **Then** the decoder runs again
   under the usual rules of feature 007 (the old results stay, flagged as outdated, until the new ones
   replace them) and the choice is restored when the session is reopened.
3. **Given** an address with the 10-bit addressing header (the two-byte form of the protocol),
   **When** it is decoded, **Then** the two bytes are shown as one 10-bit address with the direction,
   not as an address followed by a stray data byte.

---

### User Story 5 - Build on the result (Priority: P3)

A developer writing the next decoder (feature 010) wants to read I2C transactions rather than
signals. The I2C decoder offers its results to a stacked decoder as a sequence of items: for each
address, data byte, start, repeated start and stop, with the direction, the acknowledgement and the
exact position. A stacked decoder added on top of an I2C decoder receives these items and nothing
else.

**Why this priority**: it is the only part that no user sees in this feature, and it is cheap to get
right now and expensive to change after feature 010 depends on it.

**Independent Test**: stack the sample test decoder of feature 007 (or a test-only decoder) on the
I2C decoder and check that it receives exactly the transactions the I2C lane shows, in order.

**Acceptance Scenarios**:

1. **Given** an I2C decoder with a decoder stacked on it, **When** a capture is decoded, **Then** the
   stacked decoder receives one item per address, data byte, start, repeated start and stop, in time
   order, with the fields it needs (direction, acknowledgement, position) and no field it cannot rely
   on.
2. **Given** the I2C decoder is removed or its setup changes, **When** the change is applied, **Then**
   the stacked decoder behaves as feature 007 specifies (it is stopped or re-run, and the user is
   told).

---

### Edge Cases

- What happens when SDA or SCL is held low for the whole capture, or never changes? The decoder shows
  nothing and reports nothing: a bus without a start condition has no transactions, and a bus
  without enough clock pulses has no determinable period (FR-024).
- What happens in a growing capture before the bus period is known? The decoder waits and shows
  nothing; when the period is known it decodes from the start and the result equals that of the
  whole capture.
- What happens when a capture begins in the middle of a transaction (no start condition seen)? The
  bits before the first start are not decoded, not drawn and not reported (no lane entry, no overlay,
  no error, no notice); decoding begins at the first start condition. This holds equally when the
  capture begins before the MCU has set up its I2C interface and the lines are idle, stuck or noisy:
  the first start counts only after the bus has been idle (both lines high) for at least one bus
  period (FR-023).
- What happens at a start condition in the middle of a byte? It is a protocol violation: the
  incomplete byte is reported and the decoder treats the condition as a new start.
- What happens when SDA changes while SCL is high but not as a clean start or stop (a glitch)? An edge
  on SDA while SCL is high is a start or a stop by definition of the protocol; the decoder reports
  what the signal says and does not filter. Filtering noisy captures is out of scope.
- What happens when a clock pulse is stretched by the target (SCL held low longer)? The stretch is
  normal on the bus and invisible to the decoder, which reads edges and not durations; the transaction
  decodes as usual.
- What happens when there are very many transactions (millions)? The lane and the overlays obey the
  rules of feature 007 (marks only when legible, bounded drawing cost); decoding runs in the
  background, can be cancelled and stays within the memory limit of feature 007.
- What happens when SDA and SCL are assigned the same channel, or one of them is missing? The decoder
  is not started; the user is told which signal is missing, as for any decoder.
- What happens when the capture has fewer samples per bit than the decoder can read (a clock sampled
  too coarsely, so edges are missing)? The decoder reports the byte that cannot be completed and
  carries on; it never reads beyond the data it has and never panics.
- What happens when the channel assigned to SDA or SCL is not recorded (turned off after the assignment)?
  The assignment is kept and shown as unavailable; the decoder does not start and the user is told
  that the channel is not recorded.
- What happens with a generator seed and a capture length too short for even one transaction? The
  capture holds idle bus (both lines high) and decodes to nothing; this is not an error.

## Requirements *(mandatory)*

### Functional Requirements

**Decoder**

- **FR-001**: The application MUST offer an "I2C" decoder, selectable in the same way as the sample
  decoder, with two required signals: SDA and SCL. Each is assigned to one channel of the capture.
- **FR-002**: The decoder MUST recognise start, repeated start and stop conditions, a bit being read
  at the rising edge of SCL, and the acknowledgement bit that follows each byte; the data bits are
  most significant first, as the protocol defines.
- **FR-003**: For each transaction the decoder MUST show, in its lane, the address, the direction
  (read or write), each data byte and the acknowledgement of each, as ranges with text; start,
  repeated start and stop MUST be visible as events (points) in the lane.
- **FR-004**: The decoder MUST draw overlays on the signal channels: a point on the SDA channel at
  every start, repeated start and stop condition, and an edge marker on the SCL channel at every
  clock edge at which a bit (including the acknowledgement bit) is read. Overlays MUST be placed
  exactly on the sample of the event.
- **FR-005**: The decoder MUST classify acknowledgement correctly: a NAK is an error when it answers an
  address byte or a byte written to a target, and is NOT an error when the controller ends a read by
  not acknowledging the last byte it reads.
- **FR-006**: The decoder MUST support 7-bit addressing and the two-byte 10-bit addressing form, and
  MUST show a 10-bit address as one address.
- **FR-007**: The decoder MUST offer an option for the address notation (the 7-bit address, or the
  8-bit value with the direction bit), and an option for the number base of the displayed values; the
  defaults MUST be the 7-bit address and hexadecimal. Options follow the rules for options of feature
  007 (validated, saved with the session, a change re-runs the decoder).
- **FR-008**: The decoder MUST report protocol violations (a byte cut short by a stop or a start, a
  transaction that begins without an address byte, a capture that ends inside a byte) as errors at the
  place they occur, with a message that says what was wrong, and MUST continue decoding after them.
  A capture that ends inside a transaction MUST show the transaction as incomplete, not as an error of
  the decoder.
- **FR-023**: The decoder MUST synchronise before it decodes: a start condition is accepted as the
  first one only after the bus has been seen idle, with both SDA and SCL high continuously for at
  least one bus clock period. Until then every edge (including a falling edge of SDA while SCL is
  high) is ignored without any annotation, mark or notice. After synchronisation, violations are
  reported as errors under FR-008.
- **FR-024**: The bus clock period used by FR-023 MUST be determined automatically from the signal;
  the user does not enter it. The determination MAY take more than one pass over the data. It MUST
  give the same result however the capture is divided into chunks or grows (FR-009). Until enough
  SCL clock pulses have been seen to determine the period, the decoder MUST wait and show nothing;
  once it is known the decoder MUST decode from the start of the capture, so that the result equals
  the result for the whole capture. If the capture ends without the period being determined, the
  result MUST be empty and no error MUST be reported. The number of pulses that suffices is defined
  in the plan.
- **FR-009**: The decoder MUST NOT depend on how the capture is divided into chunks or on how fast the
  capture grows: the result for a given signal MUST be the same whether it was decoded in one pass,
  in pieces, or while the capture was still growing (the resumable behaviour of feature 007).
- **FR-010**: The decoder MUST NOT panic, index out of range or overflow on any input; a capture of
  any content, length and number of transactions MUST decode or be rejected with a typed error
  (constitution Principle I).
- **FR-011**: All text the decoder shows (names, option labels, messages, ACK/NAK wording) MUST be
  localised in the six languages through the keys of the application; the values shown (addresses,
  bytes) are not translated.

**Stacking**

- **FR-012**: The decoder MUST provide its results to a stacked decoder as an ordered sequence of
  items (address, data byte, start, repeated start, stop), each with its position, its direction and
  its acknowledgement where they apply, in the item model of feature 007. The description of these
  items MUST be part of the decoder's documentation.

**Generator**

- **FR-013**: The I2C decoder MUST have a generator for the virtual device: when the decoder is
  configured on channels, the virtual device MUST produce on those channels random, protocol-valid I2C
  traffic (addresses, directions, data, repeated starts), at a bit rate that suits the capture's
  sample rate, with idle time between transactions.
- **FR-025**: In the generated signal SDA MUST change in the same sample in which SCL falls (the data
  bits, the acknowledge bit and the level before a repeated start or a stop); in a start, a repeated
  start and a stop the edge of SDA MUST be 8 samples from an edge of SCL: SDA falls 8 samples before SCL
  falls (start, repeated start), SDA rises 8 samples after SCL rose (stop). No clock pulse is narrower
  than a data pulse (eight samples).
- **FR-014**: The generator MUST be able to plant errors on request: a NAK on the address, a NAK on a
  written data byte, and a transaction cut short; the rate of errors MUST be a generator option, and
  with the rate at zero no error is produced.
- **FR-015**: Generation MUST be reproducible: the same seed and configuration produce identical
  signals sample for sample, independent of how the signal is requested in pieces; two instances of
  the decoder on different channels of one capture MUST NOT produce identical traffic.
- **FR-016**: The generator MUST be able to tell, for a seed, configuration and length, which
  transactions and which errors it produced, so that tests can compare them with what the decoder read
  (the round trip).
- **FR-017**: A decoder stacked on the I2C decoder MUST find valid input in the generated traffic
  (the generator is the lowest layer of the generation chain of feature 007).
- **FR-018**: A channel with no I2C decoder configured MUST be generated exactly as before this
  feature.

**Tests and documentation**

- **FR-019**: A round-trip test MUST generate traffic for many seeds and option combinations, decode
  it, and require the decoded transactions to equal the generated ones, and the reported errors to
  equal the planted ones (constitution Principle XI).
- **FR-020**: The decoder MUST be tested with recorded signals, including malformed and truncated
  ones, and with a randomised test that feeds it arbitrary signals to show it never panics
  (constitution Principles I, III and IX).
- **FR-021**: The decoder, its options, its overlays, its stacking output and its generator MUST be
  documented in English; the user guide MUST explain I2C decoding, the address notation and how to
  read the overlays.
- **FR-026**: The lists from which a signal (SDA, SCL) is assigned to a channel MUST offer only the
  channels that are recorded in the capture, not every channel of the selected device.
- **FR-027**: The drop-down lists of a decoder's settings MUST be as wide as the Decoders panel, like
  the other fields of the panel.
- **FR-028**: The Decoders panel MUST NOT have a frame around the whole panel; each decoder has its own
  frame, as each marker has in the Markers panel. (FR-026 to FR-028 change the panel of the shell,
  an exception to FR-022 for this feature.)
- **FR-022**: Adding this decoder MUST NOT require a change to the application shell or to the
  decoder framework contract beyond registering it (feature 007 promise); if a change is found
  necessary it MUST be recorded and justified in the plan.

### Key Entities

- **I2C decoder**: reads two channels (SDA, SCL) and produces annotations and stackable items;
  options: address notation, number base.
- **Transaction**: one start (or repeated start) to the next stop on the bus: an address with a
  direction, followed by zero or more data bytes, each with an acknowledgement.
- **I2C event**: a start, a repeated start, a stop; shown as a point in the lane and on SDA.
- **Bit sample**: the SCL edge at which a bit is read; shown as an arrow on SCL.
- **I2C item (stack output)**: what a stacked decoder receives for an address, a data byte or an
  event.
- **I2C generator**: produces traffic (and, on request, errors) for the virtual device, from a seed
  and options; the counterpart of the decoder.
- **Generated transaction record**: what the generator says it produced (transactions and planted
  errors), used by the round-trip tests.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For at least one thousand different seeds and option combinations, the transactions the
  decoder reads from generated traffic are exactly the transactions that were generated, and the
  errors it reports are exactly the errors that were planted (zero differences).
- **SC-002**: A user can go from an empty application to a decoded I2C transaction on screen using
  only the virtual device in under one minute, without reading the documentation.
- **SC-003**: Every start, repeated start and stop condition and every sampled bit of a decoded
  transaction has a mark on the exact sample on the signal (zero marks off by one sample in the test
  signals).
- **SC-004**: The decoder never panics and never fails the application on any of the malformed and
  random signals of the test suite (at least one million random signal chunks, zero panics), and a
  decoder fault, were one to occur, stays contained as in feature 007.
- **SC-005**: The decoded result of a capture is identical whether it was decoded in one pass,
  in different chunk sizes, or while the capture was growing (zero differences over the test
  seeds).
- **SC-006**: A capture of ten million samples of dense I2C traffic decodes in the background without
  freezing the window (no interface pause longer than 100 ms), and the drawing cost of the lane and
  overlays does not grow with the length of the capture.
- **SC-007**: All new text is available in the six languages and the key-parity check of the
  application passes with no missing or unused keys.
- **SC-008**: Adding the decoder changes no file of the application shell (the UI crate and the shell
  of the host) other than the place where decoders are registered; the panel changes of FR-026 to FR-028
  (made after the review of the running application) are the only other edits, confined to the Decoders
  panel.

## Assumptions

- Feature 007 is merged and its decoder contract (`docs/decoder-api.md`) is the one this decoder is
  written against; the sample decoder `sample.clocked_bits` stays as the framework's own test
  decoder.
- Bits are read at edges, but the decoder does measure one duration: the bus clock period, determined
  automatically from the signal (the user does not enter it), used only to decide that the bus has
  been idle long enough to synchronise on (FR-023). It does not check the bus against a speed class
  (standard, fast, fast-plus). The generator chooses a bit rate that is readable at the capture's
  sample rate; the user does not set it.
- Both bus lines are open-drain with pull-ups, so idle is high on both; the decoder assumes the
  captured levels are the bus levels (a logic analyzer's inputs), with no inversion option.
- 7-bit addressing and the two-byte 10-bit form are in scope; the reserved addresses (general call,
  start byte, CBUS, Hs-mode) are shown as ordinary addresses with their value, not interpreted.
- Clock stretching is not detected or reported; the decoder is correct in its presence because it
  reads at SCL edges.
- The generator produces traffic where the target answers every address; the "no target answered"
  case is an explicitly planted error, not random behaviour.
- The default bit rate and idle time used by the generator, the set of errors it can plant, and the
  mapping of an error rate to a probability are defined in the plan; they do not change what a user
  sees of the decoder.
- Exporting the decoded log (feature 009) and the first stacked decoder (feature 010) read the items
  defined here but are separate features; this feature's stacking test uses a test-only decoder.
- Translations are machine drafts reviewed by the owner before release, as for earlier features.
