# Feature Specification: 32-Channel RP2350B Logic Analyzer (Driver and Rust Firmware)

**Feature Branch**: `017-driver-rp2350b-32ch`

**Created**: 2026-11-22

**Status**: Draft

**Input**: User description: "017-driver-rp2350b-32ch" (roadmap item 017: a driver for a 32-channel logic analyzer, the continuation of the gusmanb/logicanalyzer project on the RP2350B chip, on the WeAct RP2350B Core Board; it needs firmware written in Rust that extends the support from 24 to 32 channels; the firmware must be at least as capable as the original one, at least for 24 channels)

## What the device is (study of the original project and of the chip)

The facts below were read from the original project's README and wiki and from its firmware sources
(<https://github.com/gusmanb/logicanalyzer>). They are inputs to this specification, not requirements; the
exact wire details belong to the plan and the contract that follow. What could not be read is said so.

**The original device (gusmanb).**

- A logic analyzer on a Raspberry Pi Pico (also Pico W and others): **24 channels**, a sample rate from
  **3.1 kHz to 100 MHz**, 3.3 V inputs on the chip, with an input board of level shifters (1.6 to 5 V,
  with a reference that can be set) that is part of the project. The licence shown is **GPL-3.0**.
- **Capture buffer** (samples before and after the trigger, by the highest channel used): up to channel 8,
  at most 98,303 before and 131,069 after; up to channel 16, 49,151 and 65,533; up to channel 24, 24,576 and
  32,765. At least 2 samples before and 512 after.
- **Triggers**: an edge trigger on one channel (rising or falling); a pattern trigger on up to 16 channels
  (the trigger logic runs at about 66 MHz) and a fast pattern trigger on up to 5 channels (100 MHz); a
  **burst** mode that repeats the capture after the trigger several times and records when each burst
  began; a **blast** mode (a capture without trigger logic at the highest rate); aborting a capture that
  runs. The trigger lines of the pattern trigger use two extra pins that are wired together. Up to five
  devices can be chained to make 120 channels (the first one is the master; the edge trigger is not
  available then).
- **Host link**: a USB serial port (CDC); on the Pico W also a network socket carrying the same frames.
  Commands are binary frames (start `55 AA`, end `AA 55`, an escape byte `F0`); the command is the third
  byte: 0 device information, 1 capture request, 2 network settings, 3 power status, 4 restart into the
  bootloader, 5 and 6 indicator on and off; any byte received while a capture runs aborts it. Replies are
  text lines (`CAPTURE_STARTED`, `CAPTURE_ERROR`, error names) and, after a capture, a length, the samples,
  and the burst timestamps. The information reply lists the name and version, the highest frequency, the
  highest blast frequency, the buffer size and the number of channels.
- **The capture request** holds the trigger type and channel, an inverted flag or a pattern length, the
  trigger pattern value, a list of up to **24 channels**, their count, the frequency, the samples before
  and after, the burst count, a flag for burst timing and the **capture mode** (0: 8 channels, 1: 16, 2: 24).
  The samples are returned as one byte, two bytes or a 32-bit word each, by mode.
- **How it works inside**: the programmable I/O blocks sample the pins into a FIFO that two DMA channels
  empty into a ring buffer; a second block runs the pattern trigger. The inputs are GPIO 2 to 22 and 26 to
  28, plus two pins for the trigger. The number **24 is fixed** in the request, in the pin table and in the
  loops that set the pins up.
- **Not read**: the values of the limits in the firmware's headers (the real maximum rates and buffer size
  of each board), the protocol of the blast request, the network sources, and a licence file (the README
  says GPL-3.0).

**The new device.** The RP2350B is the RP2350 in the package with 48 general-purpose pins (the RP2040 has
30), three blocks of programmable I/O and 520 KB of memory (from the chip's datasheet; the plan confirms
each). The WeAct RP2350B Core Board carries it
(<https://github.com/WeActStudio/WeActStudio.RP2350BCoreBoard/blob/main/HDK/RP2350B_SCH.pdf>); which pins
are free for the inputs and the trigger is decided from its schematic in the plan. The chip's pins take
3.3 V; the board has no level shifting. The owner's board carries an external memory chip, **ESP-PSRAM64H
(8 MB)**, and was seen in its bootloader as a drive (`UF2 Bootloader v1.1`, `Model: Raspberry Pi RP2350`,
`Board-ID: RP2350`); which variant of the chip it is, and which pin the memory chip's select line uses, come
from the schematic in the plan.

## Overview

The application supports one more capture device: a **32-channel logic analyzer** made of an RP2350B board,
and the **firmware** that turns the board into one. It is the continuation of the gusmanb Pico analyzer:
the same kind of device, with the same kinds of capture, but with **32 channels instead of 24**, because
the chip has the pins. The firmware is written in **Rust** and is part of this repository, so the device
can be built, flashed and tested from it; it does everything the original firmware does with 24 channels
(not less, and not worse), and the same for the extra eight channels where the hardware allows it.

The driver is the application's side of the device: it finds the board, declares what it can do, and
runs captures through the common device abstraction, so that the interface, the decoders and the session
files work with it as with every other device.

Out of scope: a printed circuit board or level-shifting input board for the RP2350B (the board's pins are
3.3 V; the plan says how to connect, and the documentation warns); changing the original Pico driver or its
firmware; the Pico W network link (the board has no radio, see Assumptions); any change of the common
device abstraction other than what a 32-channel device needs to be described by it.

## Clarifications

### Session 2026-11-22

- Q: Which protocol does the firmware speak? -> A: The original gusmanb protocol, extended for 32 channels (a
  capture mode for 25 to 32 channels and a longer request), so that the original software keeps working with the
  device for up to 24 channels and the driver of 011 shares the protocol (FR-020).
- Q: Is the chaining of several devices part of this feature? -> A: Yes. Up to five boards can be chained as in
  the original, the first being the master (FR-022 to FR-024).
- Q: Is the application's limit of 32 channels per capture raised as part of this feature? -> A: Yes, to 160
  (five chained boards of 32): FR-025 and FR-026.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Capture all 32 channels (Priority: P1)

A user plugs in the RP2350B analyzer, starts the application and chooses it in the device list. The
capture settings show 32 channel buttons (four rows of eight); the user turns on channels from the whole
range, starts a capture and sees the signals of all of them, including channels 24 to 31, with the
decoders working on any of them.

**Why this priority**: the reason the device exists. Without 32 working channels it is only a copy of the
original.

**Independent Test**: drive all 32 inputs with a known pattern from a rig, capture, and check that every
channel shows its pattern and no channel shows another's.

**Acceptance Scenarios**:

1. **Given** the board running the firmware, **When** the application lists the devices, **Then** the
   board is there under a name that tells it is the 32-channel analyzer, with 32 channels.
2. **Given** all 32 channels on, **When** a capture runs at a rate the device offers for 32 channels,
   **Then** the capture shows each channel exactly as the rig drove it.
3. **Given** a capture of channels 24 to 31 only, **When** it runs, **Then** it works as one of any other
   channels (the capture mode follows the highest channel in use).
4. **Given** a session saved from such a capture, **When** it is opened later without the device,
   **Then** it shows the 32 channels as captured and is not marked as simulated.

---

### User Story 2 - At least what the original does, for 24 channels (Priority: P1)

A user who knows the gusmanb analyzer finds everything it did with 24 channels: the sample rates from 3.1
kHz up to 100 MHz, the numbers of samples before and after the trigger, the edge trigger, the pattern
trigger on up to 16 channels, the fast pattern trigger on up to 5, the burst mode, the blast mode, and
aborting a capture, with at least the same limits and the same accuracy.

**Why this priority**: the owner's condition for this device ("not worse than the original, at least for 24
channels"); a driver and firmware that lose a function of the original are not a continuation.

**Independent Test**: the list of the original's functions (made from its documentation and behaviour when
this specification is planned) is checked off function by function, each with an automated test or a step
of the manual procedure; the same traffic driven into the original and into this device gives the same
captured data.

**Acceptance Scenarios**:

1. **Given** a rig that drives a known signal on 24 channels, **When** it is captured at 100 MHz, **Then**
   no sample is lost and the data equals what was driven.
2. **Given** an edge trigger, a pattern trigger on 16 channels and a fast pattern trigger on 5, **When** each
   is set and the rig produces the pattern, **Then** the capture begins at the pattern with the accuracy
   the original has (to within one sample) and the requested samples before and after it.
3. **Given** the burst mode with several bursts, **When** it runs, **Then** every burst is returned with the
   time at which it began.
4. **Given** a capture that runs, **When** the user stops it, **Then** it ends within the time every device
   must keep, and what was captured is kept.
5. **Given** a setting outside what the device can do (a rate above its maximum, more samples than its
   buffer holds, a pattern wider than allowed), **When** the user applies it, **Then** it is refused with
   the reason before it reaches the board, and the limits shown are the real ones.

---

### User Story 3 - The firmware is in the repository, in Rust, and can be put on the board (Priority: P2)

A maintainer builds the firmware from the repository with documented commands, puts it on the board
through the board's bootloader, and the application recognizes the new firmware by its version. A board
with another or an older firmware is told apart and the user is told what to do, not left with a device
that does not answer.

**Why this priority**: the device cannot be used without its firmware, and the owner wants it in Rust and
in this repository.

**Independent Test**: build the firmware from a clean checkout following the documentation, flash it to the
board, and see the application list the device with the firmware's version; flash a build with another
version and see the application refuse or warn as specified.

**Acceptance Scenarios**:

1. **Given** a clean checkout, **When** the documented commands are run, **Then** a firmware image for the
   board is produced, and the commands and the tools they need are named.
2. **Given** the board in its bootloader, **When** the image is copied to it, **Then** it restarts as the
   analyzer, with no other tool needed.
3. **Given** a board that runs a firmware this version of the application does not know, **When** the
   application finds it, **Then** it says so (the version it found and the ones it supports) and does not
   start a capture.
4. **Given** the application asks the device to restart into its bootloader, **When** it accepts, **Then**
   the board appears as the bootloader drive for the next update.

---

### User Story 4 - Failures never lose data or stop the application (Priority: P2)

When the cable is pulled, the device answers with an error, the samples cannot be taken fast enough, or the
firmware meets a condition it cannot handle, the capture ends with a typed error that tells the user what
happened, whatever was captured is kept and marked incomplete where it is, and neither the application nor
the firmware stops answering.

**Why this priority**: the constitution's stability rule; a logic analyzer that loses data silently is
worse than none.

**Independent Test**: with the recorded-transport tests, replay a device that stops answering, sends
garbage, an error, a short capture and a disconnect; with the device, pull the cable during a capture.

**Acceptance Scenarios**:

1. **Given** a capture that runs, **When** the cable is unplugged, **Then** it ends, the captured samples
   are kept and marked incomplete, the user is told, and the device list updates.
2. **Given** a request the firmware cannot serve (an unknown command, a damaged frame, a frame that is too
   long), **When** it arrives, **Then** the firmware answers with an error and goes on; it does not reset or
   hang.
3. **Given** the firmware meets an internal fault, **When** it does, **Then** it restarts into a state in
   which the application finds it again, and a capture that was running is reported as ended by an error.

---

### User Story 5 - Documentation and a verification procedure (Priority: P3)

A user or a maintainer finds, in English, how to connect the inputs (which pins are channels 1 to 32 and
which are the trigger pins), the warning that the pins take 3.3 V only, how to build and flash the firmware,
what the device can and cannot do, and a manual procedure with a rig that verifies the device on a real
board; and how to get access to it on Windows 10 and 11 and on Debian 13.

**Why this priority**: the constitution requires a documented hardware verification procedure for every
driver; the pin map is what a user needs most.

**Independent Test**: someone who has not worked on it follows the documents with a board and ends with a
verified device.

**Acceptance Scenarios**:

1. **Given** the documents, **When** a user wires a signal to channel 29, **Then** it appears as channel 29
   in the application.
2. **Given** the verification procedure and a rig, **When** a maintainer follows it, **Then** every function
   of the list of US2 is checked, with the expected result stated for each step.

---

### User Story 6 - Chain several boards to capture more channels (Priority: P2)

A user who needs more than 32 channels wires several boards together as the documents describe, plugs the
first (the master) into the computer, and the application shows the chain as one device with the channels of
all the boards. A capture starts on all boards together and the signals of the boards line up in time.

**Why this priority**: the owner asked for it (the original does it, and the device must not be worse); it
extends the device beyond the 32 channels of one board.

**Independent Test**: wire two boards to the same test signal, capture with the chain, and check that the
signal appears on a channel of each board at the same time to within one sample.

**Acceptance Scenarios**:

1. **Given** two boards wired as the documents say, **When** the application lists the devices, **Then** the
   chain is one device whose channels are those of both boards, in order.
2. **Given** one signal wired to both boards, **When** a capture runs on the chain, **Then** the signal is
   at the same time on both within one sample, or the user can bring it there with the correction.
3. **Given** a chain, **When** the user sets the edge trigger, **Then** it is refused with the reason, and the
   pattern and fast pattern triggers on the master are offered.
4. **Given** a chain with a board unplugged, **When** the application looks for it, **Then** the missing board
   is named and no capture is started as if the chain were whole.

---

### Edge Cases

- A capture of all 32 channels at a rate the board cannot keep up with (the memory, the DMA or the link):
  the capture ends with the error of US4 or is refused beforehand because the rate is above what the
  device declares for 32 channels; the highest rate that works with 32 channels is measured and documented.
- A pattern trigger that includes channels above 16 or a fast pattern of more than 5 channels: refused with
  the reason (these are the original's limits; the plan says whether the new board can do more).
- More samples before the trigger than the buffer allows at the capture mode chosen: refused, with the
  limit for that mode.
- A trigger on a channel that is not among the channels captured: refused.
- A board of the RP2350 family with fewer pins than the RP2350B (the RP2350A has 30): not accepted by the
  firmware as a 32-channel device; it is told what it can offer, or not offered at all.
- Two such devices connected at once and not chained: each is listed with its own identity and used on its own.
- A chain with a board missing, wired wrongly or with one of another firmware version: the chain is refused with
  the reason, or the boards found are named; nothing is captured as if the chain were whole.
- A chain whose boards are one sample apart or more: the correction of FR-023 shows it; a capture is never
  silently misaligned.
- A device that answers to the information request with a count of channels the application does not
  accept (zero, or more than 32): refused.
- The firmware is flashed while a capture runs, or the board loses power during it: the capture ends with
  the disconnect error; nothing in the application hangs.
- A session file written with this device is opened by a version of the application that does not know the
  device: it shows the capture (the data is the same as for any other device).

## Requirements *(mandatory)*

### Functional Requirements

**The device and the driver**

- **FR-001**: The system MUST list the 32-channel RP2350B analyzer, when it runs the firmware of this
  repository, among the capture devices, with a name that says what it is and with a stable identity per
  board while it stays plugged in.
- **FR-002**: Device listing MUST NOT block on a board that does not answer, and MUST NOT list a serial
  port that is not this device.
- **FR-003**: The driver MUST declare the capabilities of the device: 32 channels, the sample rates, the
  trigger types and their limits, the buffer depth for each capture mode, and the core MUST enforce them: a
  configuration outside them MUST be rejected before it reaches the device, with the reason.
- **FR-004**: The driver MUST be the only code that knows the device; the interface, the decoders, the
  session files and the other drivers MUST work with it through the common device abstraction, with no
  change of their behaviour (Principle II).
- **FR-005**: A capture from this device MUST NOT be marked as simulated, and a session saved from it MUST
  hold all the channels captured, whatever their number up to 32.

**What the device does (at least what the original does with 24 channels)**

- **FR-006**: The device MUST capture any set of the 32 channels, and the driver MUST present them to the
  application as channels 0 to 31.
- **FR-007**: The sample rate MUST be selectable up to 100 MHz and down to 3.1 kHz for the channel counts
  the original supports (up to 24 channels), and the driver MUST report the rate the device will actually
  use when it differs from the one asked for. For more than 24 channels the highest rate the hardware
  sustains MUST be declared (the plan and the hardware verification find it; the aim is 100 MHz).
- **FR-008**: The number of samples before and after the trigger MUST be selectable, with limits that are
  not smaller, for each capture mode (8, 16 and 24 channels), than the original's, and with limits declared
  for the mode that covers 25 to 32 channels. The board's external memory (8 MB) MAY hold the capture where its
  speed allows (at the lower rates); the depth the driver declares MUST follow the rate, and a capture the
  memory cannot take at the rate asked for MUST be refused beforehand.
- **FR-009**: The device MUST offer the edge trigger (rising or falling, on any channel), the pattern trigger
  on at least 16 channels and the fast pattern trigger on at least 5, with the accuracy of the original (to
  within one sample), and a capture without a trigger.
- **FR-010**: The device MUST offer the burst mode (several captures after the trigger, each returned with
  the time at which it began) and the blast mode, with the limits of the original.
- **FR-011**: A capture in progress MUST be abortable; the abort MUST take effect within the time that the
  device contract sets for every device, and the samples already captured MUST be kept.
- **FR-012**: Starting a capture MUST return at once; the transfers MUST run off the interface thread; at
  most one capture runs per device; an operation in the wrong state MUST give a typed error, not undefined
  behaviour.
- **FR-013**: The samples stored plus the positions recorded as lost MUST equal the positions the capture
  covered; where the device cannot deliver samples, the capture MUST end or the loss MUST be recorded, and
  the capture MUST be marked incomplete; it MUST never be presented as a clean measurement.

**The firmware**

- **FR-014**: The firmware MUST be written in Rust, MUST live in this repository, and MUST be built from it
  with documented commands that need no tool other than those the documents name.
- **FR-015**: The firmware MUST provide every function listed in FR-006 to FR-011 and MUST NOT provide
  fewer or worse of the functions of the original firmware for 24 channels; a list of those functions,
  made from the original's documentation and behaviour, MUST be part of the repository, and each item MUST
  have an automated test or a step of the verification procedure.
- **FR-016**: The firmware MUST answer the device information request with its name, its version and its
  limits (the highest rates, the buffer size, the number of channels), and the driver MUST refuse a
  firmware whose version it does not support, telling the user the version it found.
- **FR-017**: The firmware MUST NOT panic, reset or hang on any input from the host: an unknown command, a
  damaged or oversized frame, a request outside the limits, or a request in the wrong state MUST get an
  error reply and leave the device ready for the next request. If the firmware nevertheless fails
  (a fault that should not happen), it MAY restart itself, and the driver MUST find the board again; such a
  restart is a defect to be reported, never an accepted way of handling input.
- **FR-018**: The firmware MUST be able to restart into the board's bootloader on request, so that it can
  be updated without touching the board.
- **FR-019**: The parts of the firmware that are logic (the parsing of requests, the checking of limits, the
  choice of the capture mode, the packing of samples, the computing of timestamps) MUST be testable on the
  computer, without the board.

**Compatibility with the original, and what is added to it**

- **FR-020**: The device and the driver MUST communicate by the original gusmanb protocol, extended for 32
  channels: the framing, the commands and the replies of the original stay as they are, for up to 24 channels
  a request is the original's and the device behaves as the original would for it, and the extension (a capture
  mode for 25 to 32 channels, a longer request) is announced by the device in its information reply and used
  by the driver only when announced. A request or a reply the other side does not know MUST be refused with a
  typed error and never be misread.
- **FR-021**: The firmware and the driver MAY offer a compact transfer of the captured samples (a coding of the
  changes of level, and only the channels that were selected) that the device announces in its information
  reply and the driver turns on; without the announcement the samples are sent as the original sends them.
  The data the application gets MUST be the same with or without it.

**Several devices (chaining)**

- **FR-022**: Up to five boards MUST be able to be chained as in the original, the first being the master; the
  application MUST present the chain as one capture device whose channels are those of all its boards, one
  after the other, and the documentation MUST say how the boards are wired.
- **FR-023**: The samples of the boards of a chain MUST be aligned in time to within one sample, as the original
  does, with a correction that the user can set for a board whose timing differs.
- **FR-024**: The triggers available in a chain MUST be those the original allows in one (the pattern and the fast
  pattern trigger on the master; not the edge trigger), and the others MUST be refused with the reason.
- **FR-025**: The application MUST handle captures of up to 160 channels (five chained boards of 32), not 32
  as now: the signal storage, the capture description, the session files, the presentation of the channels
  (names, colours, heights, order), the markers and the decoders' channel choices, and the interface (the
  channel buttons in rows of eight, the legend of the signals that scrolls) MUST all work with that many
  channels. Sessions of 32 channels or fewer MUST be read and written as before, and a file written by an
  earlier version MUST open unchanged.
- **FR-026**: Drawing and the other costs of a capture MUST NOT grow with the number of channels that are not
  shown or not captured, and a capture of 160 channels MUST stay as usable (zoom, pan, markers, decoders) as
  one of 32; the limits that the constitution sets on rendering (Principle VIII) hold for it.

**Failures**

- **FR-027**: On unplugging, a transfer error, a stall, or any driver error, the capture MUST end, what was
  captured MUST be kept and marked incomplete, and the application MUST go on working; a driver error,
  panic or transfer that never returns MUST be contained (Principle I).
- **FR-028**: Every failure MUST reach the user as a typed error with a stable code and a message that says
  what happened and what to do; the errors MUST distinguish at least: no permission to open the device,
  no driver or device not found, the device disconnected, a firmware that is not supported, a request the
  device refused, a capture that ended early, and a timeout.

**Tests, documents, the rest**

- **FR-029**: The driver MUST be testable without hardware: recorded exchanges with the device (the
  information request, captures of 8, 16, 24 and 32 channels, each trigger, a burst, an abort, errors,
  a disconnect) are replayed through the transport, and the tests run in the usual checks on Windows and
  Debian; a test of the 32-channel packing MUST fail if the eight extra channels are lost or mixed.
- **FR-030**: The repository MUST contain a manual hardware verification procedure for this device, with
  a rig that drives all 32 inputs, giving for each step the expected result; automated hardware tests MUST
  be opt-in.
- **FR-031**: The documentation, in English, MUST give the pin map (which pin is which channel and which
  are the trigger pins), the warning that the pins take 3.3 V only, how to build and flash the firmware,
  what the device can and cannot do, and how to get access to the device on Windows 10 and 11 and on
  Debian 13.
- **FR-032**: Every new user-facing text (the device name, error messages, notices) MUST exist in the six
  languages of the application, and the checks of feature 005 MUST pass.
- **FR-033**: Whatever is taken from another project MUST be recorded with its licence (Principle XII): the
  firmware is written from the behaviour of the original, and no code of it is copied; the facts of the
  protocol that interoperability needs are described in the contract.

### Key Entities

- **32-channel analyzer**: the device, an RP2350B board with the firmware of this repository; has 32
  channels, a set of rates, trigger types and a buffer per capture mode.
- **Capture mode**: how many bytes each sample takes on the link (8, 16, 24 or 32 channels' worth),
  chosen by the highest channel in use.
- **Capture request**: what the application asks of the device: the channels, the rate, the samples before
  and after, the trigger, the burst count.
- **Firmware version**: what the device reports; what the driver compares with the versions it supports.
- **Verification rig**: what drives the 32 inputs with known patterns during the manual procedure.
- **Chain**: up to five boards wired together, the first the master, that the application treats as one
  device with the channels of all of them; has a time correction for each board after the first.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With a rig that drives all 32 inputs, a capture of all channels shows each channel's pattern
  and no other, in 100% of 20 consecutive captures.
- **SC-002**: Every function of the original firmware's list for 24 channels has a passing automated test or
  a verification step: 100% of the list.
- **SC-003**: At 100 MHz with up to 24 channels, no sample is lost over a capture of the largest size the
  device declares, in 20 consecutive captures; the highest rate that works with 32 channels is measured and
  written in the documentation.
- **SC-004**: For each trigger type, the capture begins within one sample of the pattern the rig produces,
  in 100% of 20 captures.
- **SC-005**: A maintainer builds the firmware from a clean checkout and has it running on a board in under
  15 minutes following the documents.
- **SC-006**: A user lists the device and starts a capture of any channels in under one minute from plugging
  it in (after the firmware is on the board).
- **SC-007**: The recorded-transport tests run, and pass, on Windows and Debian in the usual checks, with
  no hardware; the existing tests of every other driver pass unchanged.
- **SC-008**: Unplugging the cable during a capture ends it within one second of the event with the
  captured data kept and marked incomplete, in 100% of 20 trials.
- **SC-009**: All new text exists in the six languages and the key-parity check passes.
- **SC-010**: With two boards chained and one test signal wired to both, the signal is on both within one
  sample (or within one sample after the user's correction) in 100% of 20 captures.
- **SC-011**: The compact transfer of FR-021 returns exactly the data of the raw transfer in 100% of the tests
  (including a worst case that does not compress), and for a signal with few changes the transfer is at
  least ten times shorter than the raw one.
- **SC-012**: A capture of 160 channels (a chain of five boards, or a recorded one) is saved and reopened with
  every channel as captured, and a session of 32 channels or fewer written by an earlier version opens
  unchanged, in 100% of the tests; the interface keeps zoom, pan and the other controls usable with 160
  channels.

## Assumptions

- The board has no radio, so the network link of the Pico W is not part of this feature.
- The inputs are the chip's pins, which take 3.3 V; a level-shifting input board is not part of this
  feature, and the documents say so clearly.
- The link to the computer is the USB serial port (CDC), as the original's, because the speed of capture does
  not depend on it and the speed of the transfer is helped far more by the compact transfer of FR-021 than by
  another kind of USB interface (which would gain about a tenth or a fifth, and would cost the compatibility
  with the original software); a faster interface of its own can be added later, next to the serial one.
- The aim for the highest rate is the original's 100 MHz for every channel count; whether the hardware
  sustains it for 32 channels is found when the device is built, and what is found is declared.
- The firmware is written from the behaviour of the original (its documentation and the facts of its
  protocol that a driver needs), not from its code (Principle XII); the project's licence and the
  original's (GPL-3.0) are compatible, and the plan still records what was used.
- The roadmap's dependency on the gusmanb Pico driver (011) is kept: if the protocol is the original's, the
  two drivers share it; if 011 is not done first, this feature carries the shared part and 011 reuses it.
- Which pins are the 32 inputs and the trigger pins is decided from the board's schematic in the plan; the
  limits of the original (16 channels for a pattern, 5 for the fast one) are the least the device offers.
- The common device abstraction already describes a device by its channels, rates, triggers and buffer; a
  channel count of 32 fits it (the interface lays out four rows of eight channel buttons).
