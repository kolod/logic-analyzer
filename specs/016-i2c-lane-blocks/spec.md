# Feature Specification: I2C Lane as Blocks

**Feature Branch**: `016-i2c-lane-blocks`

**Created**: 2026-11-22

**Status**: Draft

**Input**: User description: "016-i2c-lane-blocks: change how the I2C decoder row is drawn. Two sub-bands instead of three: the upper one shows the separate parts of a packet (start, the eight data bits as one data byte, ack/nak, stop), the lower one the whole packet. Everything is drawn as rectangular blocks. A start block is blue with the text Start, a stop block blue with the text Stop, a data block yellow for a write and green for a read with the value as text, an ack block green, a nak block red; the block of the whole packet is red if the packet has errors and green if all is well (a NAK on the last byte of a read is normal). Text disappears completely when it does not fit in the block. The position of each block is shown by the owner with four pairs of markers on a screenshot (1: start, 2: stop, 3 and 4: the acknowledge bits)."

## Overview

Feature 008 draws the I2C decoder in a lane of three sub-bands in which a transaction, its address and
its data bytes overlap as ranges, and each acknowledge bit is a single point. In use it is hard to
read: the ranges start and end at places that do not match what one sees on the signals, the
acknowledge is a point that is easy to miss, and nothing says at a glance whether a transaction went
well.

This feature changes only how the I2C lane is drawn. The lane has **two sub-bands** and everything in
it is a **rectangular block**:

- The **upper sub-band** shows the parts of a packet one after another, without gaps and without
  overlap: **Start**, the **address** (eight bits), its **ack** or **nak**, **data** bytes (eight bits
  each) each followed by its **ack** or **nak**, a **repeated start** where there is one, and **Stop**.
- The **lower sub-band** shows the **whole packet** as one block from its start to its stop, in green
  when the packet is good and in red when it has an error.

Each block lies on the signals exactly where the bits it stands for are: a data block covers the eight
clock pulses of its bits, an ack block the clock pulse of the acknowledge bit, the start block the
period before the first bit, the stop block the period after the last one. The owner marked the places
with four pairs of markers on a capture (pair 1 around the start, pair 2 around the stop, pairs 3 and 4
around two acknowledge bits); the blocks of this feature are drawn there.

Out of scope: any change to what the decoder reads or to the errors it reports (feature 008), to the
items it gives to a stacked decoder, to the generator of the virtual device, to the arrows and dots the
decoder draws on the SDA and SCL signals, to the lanes of other decoders, and colours chosen by the
user.

## Clarifications

### Session 2026-11-22

- Q: What is written on the stop block? → A: "Stop" (the description said "Start" by a slip of the pen).
- Q: Where is each block placed? → A: By the bus period `T` of the decoder (feature 008): a bit occupies
  the period centred on the rising edge of SCL at which it is read, so a data block spans eight such
  periods and an ack block one; the start block is the period centred on the fall of SDA, the
  stop block the period that ends at the rise of SDA (FR-002 to FR-005, the plan's research R2 gives the exact rule).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read a transaction at a glance (Priority: P1)

A user looks at an I2C capture decoded by the I2C decoder. The row of the decoder shows, in its upper
sub-band, a blue **Start**, a yellow **Address 0x43 (write)**, a green **ACK**, a yellow **Data 0x75**,
a green **ACK** and a blue **Stop**, side by side; in its lower sub-band one green block, **Write to
0x43**, across all of them.

**Why this priority**: it is the whole point of the change: to tell the parts of a packet and its
result by shape and colour, without reading the text.

**Independent Test**: decode a good write of one data byte (hand-built or from the virtual device) and
check the blocks, their text, their colours and their places against the clock pulses.

**Acceptance Scenarios**:

1. **Given** a good write of one byte, **When** the lane is shown with enough room, **Then** the upper
   sub-band holds six blocks in order — start (blue, "Start"), address (yellow, text as before), ack
   (green), data (yellow, text as before), ack (green), stop (blue, "Stop") — and the lower sub-band one
   green block with the transaction's text.
2. **Given** a read, **When** the lane is shown, **Then** the address and data blocks are green, not
   yellow.
3. **Given** a read whose last byte is not acknowledged by the controller, **When** the lane is shown,
   **Then** the nak block is red, but the packet block below it is green.
4. **Given** blocks too narrow for their text, **When** the lane is shown, **Then** a block shows its
   text only if the whole text fits inside it, otherwise no text at all (never a cut or overflowing
   text), and the block keeps its colour.

---

### User Story 2 - See at once that a packet went wrong (Priority: P1)

A user decodes a capture with errors (a missing acknowledge, a byte cut short). The packet block in
the lower sub-band is red; in the upper sub-band the nak block that caused it is red, and the user
finds the fault without opening anything.

**Why this priority**: finding faults is what an analyser is for; the colour of the packet is the
quickest way to find the packet that needs a look.

**Independent Test**: decode traffic with planted errors (the generator of feature 008) and check that
exactly the packets that have an error are red.

**Acceptance Scenarios**:

1. **Given** a write whose address is not acknowledged, **When** the lane is shown, **Then** the
   address's nak block is red and the packet block is red.
2. **Given** a write whose data byte is not acknowledged, **Then** that nak block and the packet block
   are red.
3. **Given** a byte cut short by a stop, **Then** the data block of the bits that were read keeps its
   colour, there is no ack block for it, the stop block follows, and the packet block is red.
4. **Given** a packet with no error, **Then** its packet block is green, whatever the direction.

---

### User Story 3 - Blocks that line up with the signals (Priority: P2)

A user zooms to one byte. Each block starts and ends at the same places as the clock pulses of its bits
and the edges of its start and stop, so a marker placed on a block's edge sits on an edge of the signals.

**Why this priority**: blocks that do not match the signals are worse than none; this is what makes the
picture trustworthy and what the owner's four marker pairs ask for.

**Independent Test**: for hand-built signals with a known bus period, compute the expected edges of
every block and compare them with the drawn blocks.

**Acceptance Scenarios**:

1. **Given** a decoded byte, **When** the lane is shown, **Then** its data block begins half a period
   before the rising edge of SCL of its first bit and ends half a period after that of its eighth bit,
   and the ack block covers the next period, so the blocks touch without gap or overlap.
2. **Given** a start, **Then** its block is the period that ends where the first data block begins, and
   contains the falling edge of SDA; **Given** a stop, **Then** its block is the period that begins where
   the last ack block (or data block, for a cut byte) ends, and ends at or after the rising edge of SDA.
3. **Given** a repeated start, **Then** it is drawn like a start, between the blocks of the two parts
   of the packet, and the packet block covers both parts.

---

### Edge Cases

- A packet that is not finished when the capture ends, or was cut by lost samples: the upper blocks
  that are complete are drawn, the packet block runs to the end of the last block, and it is not red
  (as in feature 008, an incomplete packet is not an error in itself) but has its own, neutral colour
  (see Assumptions).
- A start followed at once by a stop (no clock pulse between): a start block, a stop block, and a red
  packet block for the empty transaction.
- A 10-bit address: the two address bytes are two address blocks, each with its ack block, and the text
  of the first one names the address as feature 008 does.
- Two packets close together (a stop and the next start less than a period apart): the blocks are
  shortened so that none overlaps the next, never drawn over one another.
- A zoom so far out that blocks are a pixel or less wide: the row is drawn as feature 007 draws any dense
  lane (marks only when legible, bounded drawing cost); colours still tell good from bad where drawn.
- A bus period that is not known yet (a growing capture): nothing is drawn, as in feature 008.
- Language: the words "Start" and "Stop" and the texts of the blocks are translated in the six
  languages; a longer translation just hides sooner.

## Requirements *(mandatory)*

### Functional Requirements

**Structure**

- **FR-001**: The lane of the I2C decoder MUST have two sub-bands (not three): the upper one for the
  parts of a packet, the lower one for whole packets. Every item in it MUST be drawn as a rectangle.

**Places**

- **FR-002**: A bit MUST occupy the bus period centred on the rising edge of SCL at which it is read. A
  **data block** (or **address block**) MUST span the eight bits of its byte, from half a period before
  the rising edge of its first bit to half a period after that of its eighth; an **ack** or **nak
  block** MUST span the one period of the acknowledge bit that follows.
- **FR-003**: A **start block** and a **repeated start block** MUST be one bus period, centred on the
  falling edge of SDA that makes the start; on a bus whose clock behaves as the generated one (the
  clock falls half a period after the edge) it ends where the first block of the byte after it begins.
- **FR-004**: A **stop block** MUST end at the rising edge of SDA that makes the stop and MUST begin one
  period before it, but not before the end of the last block before it (so on the generated signal it
  begins where the last ack block ends).
- **FR-005**: Blocks of the upper sub-band MUST NOT overlap; where two would, the earlier is shortened to
  the start of the later.
- **FR-006**: The **packet block** of the lower sub-band MUST run from the beginning of its start block
  to the end of its stop block (or, for an unfinished packet, to the end of its last block); a packet
  with a repeated start is one block.

**Colours and texts**

- **FR-007**: A start, a repeated start and a stop block MUST be blue; their text is "Start", "Repeated
  start" and "Stop".
- **FR-008**: An address or data block MUST be yellow when its direction is write and green when it is
  read; its text is what feature 008 gives it now (the address, with its direction, or the value in the
  chosen notation).
- **FR-009**: An ack block MUST be green with the text "ACK"; a nak block MUST be red with the text
  "NAK".
- **FR-010**: A packet block MUST be red if the packet contains an error as defined by feature 008 (an
  address or a written byte not acknowledged, a byte cut short, an empty transaction, lost samples) and
  green otherwise; the controller's NAK of the last byte of a read is not an error. Its text is what
  feature 008 gives a transaction now (`Write to 0x43`, `Read from 0x43`).
- **FR-011**: The text of a block MUST be drawn only if all of it fits inside the block, with a small
  margin; otherwise the block MUST have no text at all. The text MUST stay readable on every colour of
  every theme.
- **FR-012**: The colours MUST be distinguishable on every theme the application has (light and dark) and
  MUST NOT depend on the colour of the channel they sit on.

**Unchanged**

- **FR-013**: What the decoder reads, the errors it reports and their texts, the items it gives to a
  stacked decoder (their kinds, spans and flags) and the arrows and dots on the signals MUST NOT change.
  The error texts of feature 008 (`Address 0x50 not acknowledged` and the others) MUST still be
  available: the one change to the arrows is that the arrow of an acknowledge bit that is an error carries
  its error text, since the nak block says only "NAK".
- **FR-014**: Selecting a block MUST work as selecting any decoder annotation does (feature 007), and
  show its details; the drawing cost MUST NOT grow with the length of the capture.
- **FR-015**: All new and changed text MUST be available in the six languages of the application, and
  the checks of feature 005 (parity, budget, fonts) MUST pass.

### Key Entities

- **Part block**: a rectangle in the upper sub-band: start, repeated start, address, data, ack, nak or
  stop; has a place (from, to), a colour decided by its kind and direction and a text.
- **Packet block**: a rectangle in the lower sub-band: one transaction from its start to its stop, red
  or green.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For 1,000 generated transactions, with and without planted errors, the packet block is red
  for exactly those that have an error and green for all others, and every nak that is not an error (the
  end of a read) leaves its packet green.
- **SC-002**: For hand-built signals with a known bus period, the edges of every block equal the
  expected positions to the sample (FR-002 to FR-006), and no two blocks of the upper sub-band overlap.
- **SC-003**: At any zoom, no text is ever drawn partly outside its block; a block either shows its whole
  text or none.
- **SC-004**: A user who has never seen the decoder can say from a screenshot, within ten seconds,
  which packets in a capture failed and which byte was not acknowledged.
- **SC-005**: The I2C round-trip, chunking and stacking tests of feature 008 pass without a change in
  what they compare; drawing a row of a ten-million-sample capture costs no more than before.
- **SC-006**: All new text exists in the six languages and the key-parity check passes.

## Assumptions

- Each block's place follows from the bus period that the decoder already measures; no new option is
  added. The owner's four marker pairs agree with this rule on the reference capture (start: the period
  around the SDA fall, ending where the first bit's period begins; ack: one period centred on the clock
  pulse; stop: one period, ending at the rise of SDA).
- An unfinished packet is drawn with a neutral colour (the colour of an ordinary decoder annotation),
  not red or green, since it is neither good nor failed yet; the owner did not specify this and may
  change it.
- The lane framework of feature 007 may need to let a decoder name the colour of a block in a way that
  follows the theme (blue, yellow, green, red as roles, not as fixed values); this is decided in the plan.
- The details of a block (what the user sees on selecting it) are those of feature 008, now attached to
  the block that carries them (an error on the nak block it belongs to).
- The text "Repeated start" is used for a repeated start block; the owner named only Start and Stop.
