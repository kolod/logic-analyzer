# Feature Specification: WeAct LogicAnalyzer Driver

**Feature Branch**: `013-driver-weact`

**Created**: 2026-11-17

**Status**: Draft

**Input**: User description: "013-driver-weact" (roadmap item 013: "WeAct LogicAnalyzer driver, same requirements" as the other hardware drivers; roadmap risk: "the WeAct device protocol must be studied from its repository before the specification is written")

## What the device is (study of its repository)

The facts below were read from the vendor repository
(<https://github.com/WeActStudio/LogicAnalyzerV1>, README) and from the sigrok project that
the vendor's own instructions rely on (the `fx2lafw` firmware and driver pages and sources).
They are inputs to this specification, not requirements; the exact wire details belong to the
plan and the contract that follow.

- The WeAct LogicAnalyzer V1 is a small USB board built on a Cypress CY7C68013A (or its clone
  Corebai CBM9002A) USB controller. It has **8 channels**, a **maximum sample rate of 24 MHz** per
  channel, inputs of **0 to 5.5 V** (high above 2 V, low below 0.8 V; the threshold is fixed), and
  a USB 2.0 connection. The vendor warns that a USB docking station can keep it from reaching
  24 MHz.
- The board has **no memory for samples**: it streams every sample to the computer while the
  capture runs. Reading the firmware source during planning showed what happens when the computer
  cannot keep up: the board's small buffer fills, and **the firmware stops the capture** (it resets
  the buffer and waits for a new start command). It never skips ahead, it has no counter of lost
  samples and it sends no sign that it stopped; the computer sees only that no more data comes.
  So this board has **no gap in the middle of a capture**; its one failure of this kind is a
  capture that ends early (Amendment of 2026-11-17, found while planning).
- The controller **holds no firmware of its own**: the computer must load a firmware image into
  it each time the board is plugged in, after which the board disconnects from the bus and
  reappears under another identity ("renumeration"). The vendor's instructions use the open-source
  `fx2lafw` firmware (GPL version 2 or later) of the sigrok project; sigrok's hardware list
  confirms the board under the USB identity `1d50:608c`, 8 channels.
- The repository has no description of the wire protocol of its own; it points to sigrok. The
  protocol of the `fx2lafw` firmware is small: a few vendor requests (read the firmware version,
  start an acquisition with a sample-width/clock flag byte and a two-byte sample delay, read
  the revision) and a stream of samples on a bulk endpoint.
- The vendor repository contains no licence file (its licence is unknown); nothing from it other
  than the facts above is to be reused.

## Clarifications

### Session 2026-11-17

- Q: Where does the firmware image the board needs come from? → A: The application ships the open-source firmware image with it, so that a board works on a clean system and offline; the image's licence (GPL version 2 or later) is compatible with the project's GPL-3.0, and its origin, version and licence are recorded in the plan. Nothing is read from folders of other software.
- Q: Which USB identities does the driver accept? → A: Only the identity that sigrok lists for this board (`1d50:608c`, 8 channels), shown as the WeAct LogicAnalyzer. Other boards of the same firmware family (a blank Cypress FX2 board, the 16-channel variant) are not accepted and are not listed; they are outside this feature.

- Q: What signal source do the maintainer's local tests with a real board use? → A: An Arduino Nano (on the serial port COM7 of the maintainer's machine) wired to the eight inputs of the analyzer: the first input of the analyzer (channel 0 in the application; the maintainer counts from 1) to pin D2 (PD2), the second to D3 (PD3) and so on in order up to the eighth input to D9 (PB1).
- Q: What role does that rig play in the tests? → A: Both uses: the signal source of the manual verification procedure, and the source of automated tests with the real board that the test code controls over the Nano's serial port (to start and stop chosen patterns). These automated tests are opt-in: they never run by default, never run in continuous integration, and are skipped (not failed) when the rig is absent.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Capture from a WeAct LogicAnalyzer (Priority: P1)

A user plugs in a WeAct LogicAnalyzer V1, starts the application and sees the device in the
device list next to the virtual device, with a name that says what it is. They choose the sample
rate, the channels to record (up to eight) and the length, press Start, and the signals appear in
the signal view exactly as they do for the virtual device: zoom, pan, markers, decoders, saving
and opening a session all work on this capture. The capture is not marked as simulated; the
session records that it came from this device.

**Why this priority**: it is the reason the driver exists and one of the three devices the
product must support. Without it the application can only show invented signals.

**Independent Test**: with the device attached (manual) or with a recorded exchange replayed
(automated), list devices, pick the device, configure 8 channels at 1 MHz, capture 100,000
samples and compare the stored capture with the signal that was fed in; the stored capture equals
the input sample for sample.

**Acceptance Scenarios**:

1. **Given** a WeAct LogicAnalyzer that is plugged in and accessible, **When** the user lists the
   devices, **Then** it appears once, with a stable identity and a name that includes the model,
   and its capabilities are shown: 8 channels, the sample rates it supports up to 24 MHz, no
   trigger.
2. **Given** the device is selected with a valid configuration, **When** the user presses Start,
   **Then** the capture begins within a short time, samples arrive in the signal view as the
   capture runs, and the progress is shown.
3. **Given** a running capture, **When** the user presses Stop, **Then** the capture ends
   promptly, what was captured is kept, and the device is ready for the next capture without
   unplugging it.
4. **Given** a finished capture, **When** the user saves and reopens it, **Then** it opens with
   every sample unchanged and marked as coming from a real device, not simulated.
5. **Given** a configuration the device cannot do (for example a rate above 24 MHz or a channel
   that does not exist), **When** the user tries to apply it, **Then** it is refused before
   anything is sent to the device, with a message that says what to change.

---

### User Story 2 - First connection: the device prepares itself or says what is missing (Priority: P1)

The board has no firmware when it is plugged in. The user should not need to know that: the
application loads what the board needs, waits for it to come back, and shows the device as ready.
When something is missing (the Windows driver is not installed, a Debian user has no permission to
open the device, another program such as PulseView holds the device), the application says exactly
that and what to do about it, in the user's language.

**Why this priority**: a device that cannot be used on first connection is a device that is not
supported; the actionable error is required by the constitution (Principle VI, device access).

**Independent Test**: with a recorded exchange of a blank board, an already-prepared board, a
board that never comes back, a board whose permissions are refused and a board that is busy,
check the outcome of each: ready, ready, a timeout error, a permission error with the rule to
add, an in-use error.

**Acceptance Scenarios**:

1. **Given** a blank board that is plugged in and accessible, **When** the application scans for
   devices, **Then** the firmware is loaded, the device reappears, and it is listed as ready, all
   within a few seconds and without any dialog, and without the list showing it twice or the
   selected device being lost when the board reappears under its new identity.
2. **Given** a board that already runs the firmware (it was used before and not unplugged,
   or another program loaded it), **When** the application scans, **Then** it is used as it is,
   without loading the firmware again.
3. **Given** a board that is present but cannot be opened because no driver is installed
   (Windows) or the user may not access it (Debian), **When** the application scans or the user
   starts a capture, **Then** a typed error says which of the two it is and the documentation it
   points to describes the fix for that system (the driver installation, or the access rule that
   removes the need for administrator rights).
4. **Given** the board does not reappear after the firmware was loaded, **When** the allowed time
   has passed, **Then** the user gets a typed error that says the device did not come back and
   suggests unplugging it, and the application is not blocked while it waited.
5. **Given** the device is in use by another program, **When** the user starts a capture, **Then**
   the error says the device is busy and nothing else is affected.

---

### User Story 3 - Failures never lose data or stop the application (Priority: P2)

A cable is pulled in the middle of a capture, the computer is too busy to read the samples in
time, or a transfer fails. The capture that exists up to that point is kept and clearly flagged as
incomplete with the reason; the positions where samples were lost are recorded as gaps and are
never shown as signal; the application stays responsive; and plugging the device back in makes it
usable again without restarting the application.

**Why this priority**: data integrity and stability are non-negotiable (Principle I), and a
streaming device with no memory loses samples whenever the computer falls behind, so this is the
normal failure mode of this hardware, not a rare one.

**Independent Test**: with injected failures in a recorded exchange (unplug, a short read, a
timeout, a stalled transfer, a lost block of samples), check after each that the stored samples
plus the positions recorded as lost equal the positions the capture claims, and that the
application can start another capture.

**Acceptance Scenarios**:

1. **Given** a running capture, **When** the device is unplugged, **Then** the capture ends with
   a typed error naming the disconnect, the samples captured so far stay available and are marked
   incomplete, and the device disappears from the list.
2. **Given** a running capture, **When** the computer does not read the samples fast enough and the
   board stops the capture, **Then** the capture ends with a typed error that says the stream
   stopped and suggests a lower rate or another USB port, what was captured stays and is marked
   incomplete, and, for a capture of a set length, the positions that were never captured are
   recorded as one gap at the end so that the capture never claims samples it does not have.
3. **Given** the device was unplugged during a capture, **When** it is plugged in again, **Then**
   it is listed again and a new capture works without restarting the application.
4. **Given** the driver fails in any way (an error, a panic, a transfer that never returns),
   **When** it happens, **Then** it is contained: the user gets an error, the application does
   not crash or freeze, and the other devices and the virtual device are not affected.

---

### User Story 4 - The driver can be tested without hardware and verified by hand (Priority: P2)

A maintainer changes the driver and runs the test suite on a machine with no device: a recorded
exchange with a real board is replayed to the driver, failures are injected at chosen points, and
the shared conformance tests of the device abstraction run against it. A tester with a real board
follows a written procedure to verify the same behaviour on Windows and on Debian.

**Why this priority**: the constitution requires every driver to be testable without hardware
and to have a documented manual hardware verification procedure (Principle II); the maintainers
have the hardware only occasionally.

**Independent Test**: run the test suite with no device attached; run the manual procedure with a
device and record the result.

**Acceptance Scenarios**:

1. **Given** no device attached, **When** the test suite runs, **Then** the driver passes the
   shared conformance suite and its own tests against the replayed recording, on Windows and on
   Debian.
2. **Given** a recorded exchange, **When** a failure is injected at each step of a capture,
   **Then** the outcomes are the ones of User Story 3 at every point.
3. **Given** a real board and a signal source of known content, **When** the tester follows the
   manual procedure, **Then** every step has an expected result, and the record of the run can be
   attached to the pull request.
4. **Given** the maintainer's rig (the analyzer wired to the Arduino Nano) and the opt-in switch
   set, **When** the hardware tests run, **Then** the test code starts a chosen pattern on the
   Nano, captures it with the analyzer, and checks that the captured signal is the pattern, for
   every one of the eight channels, and stops the Nano afterwards.
5. **Given** no rig or no opt-in switch, **When** the test suite runs, **Then** the hardware tests
   are skipped with a visible reason and the suite still passes.

---

### User Story 5 - Documentation: getting access, and what the device can and cannot do (Priority: P3)

A new user reads one page and learns how to get the device working on Windows 10/11 and on
Debian 13: which driver to install, which access rule to add, what the lights on the board mean,
and what the limits are: 8 channels, at most 24 MHz, signal levels up to 5.5 V, no trigger, no
memory (so a slow USB path loses samples), and why a USB docking station can hurt.

**Why this priority**: the access steps are what stand between a user and a working device
(constitution, Hardware Support & Technology Constraints); the limits prevent wrong conclusions
from a capture.

**Independent Test**: a person who has not used the device follows the page on a clean Windows
11 and a clean Debian 13 and gets a capture; the time taken is noted.

**Acceptance Scenarios**:

1. **Given** the documentation, **When** a user follows the Windows steps, **Then** the device
   can be opened by the application.
2. **Given** the documentation, **When** a user follows the Debian steps, **Then** the device can
   be opened without administrator rights.
3. **Given** the limits section, **When** a user wonders why 48 MHz is not offered or why a
   signal of 12 V must not be connected, **Then** the page answers both.

### Edge Cases

- A board that was already prepared by another program (PulseView) and left running, or one that
  is plugged in with a different board of the same family at the same time: each is listed by
  its own identity and not confused with the other.
- Two WeAct boards at once: both are listed with stable, distinct identities (their position on
  the USB bus), and a capture on one does not touch the other.
- The board re-enumerates when its firmware is loaded: the device selected before the load must
  still be the selected one after it, and no "device disappeared" for the same physical board
  reaches the user as a failure.
- A USB 1.1 port, a hub or a docking station that cannot carry the stream of the chosen rate: the
  board stops the capture, which ends early with the typed error of FR-017, flagged incomplete;
  it never reports a clean capture.
- A rate that the board can only reach with a precision lower than asked (the sample clock is
  derived from a fixed oscillator): the configuration shows the rate actually used.
- The firmware image missing, damaged or of a version the driver does not know: a typed error,
  not a hang, and nothing is sent to the board.
- The first chunk of data arriving late, or the board sending nothing: the capture ends with a
  timeout error after a bounded time, not never.
- A capture whose length is reached: the capture finishes by itself and the board is left idle.
- A start while a capture already runs on that board, a stop while idle, a configuration change
  while running: each is refused with the answer the device contract defines.
- The device list is requested while a board is mid-renumeration: the call returns promptly and
  never blocks on a board that does not answer.

## Requirements *(mandatory)*

### Functional Requirements

**Discovery and identity**

- **FR-001**: The system MUST list a connected WeAct LogicAnalyzer V1 among the capture devices,
  with a name that includes the model, through the common device abstraction, so that no
  application or interface code depends on this device.
- **FR-002**: Each board MUST have an identity that is stable while it stays plugged in at the
  same place, **including across the re-enumeration that loading its firmware causes**, and
  distinct for two boards plugged in at once.
- **FR-003**: Device listing MUST NOT block on a board that does not answer, and a board in the
  middle of re-enumeration MUST NOT make the list wrong for longer than the time re-enumeration
  is allowed.
- **FR-004**: The driver MUST accept only the USB identity listed for the WeAct LogicAnalyzer V1
  (see the clarification above) and MUST NOT list, open or send anything to any other USB device,
  including other boards of the same firmware family.

**Capabilities and configuration**

- **FR-005**: The driver MUST declare the capabilities of the board: 8 channels, the sample rates
  it can run (from 20 kHz up to 24 MHz), no trigger, and an acquisition that streams without
  memory on the device; the core MUST enforce them.
- **FR-006**: A configuration outside the capabilities (a channel above 7, a rate above 24 MHz or
  not in the list, no channel enabled) MUST be rejected before anything is sent to the board,
  with the typed error of the device contract.
- **FR-007**: The configuration MUST report the sample rate the board will actually use when it
  differs from the one asked for.

**Preparing the board**

- **FR-008**: A board without firmware MUST be prepared by the system without user action: the
  firmware is loaded, the board is given a bounded time to reappear, and it is then listed as
  ready; the whole step MUST run off the interface thread and MUST NOT block the application.
- **FR-009**: A board that already runs the firmware MUST be used as it is, without loading it
  again.
- **FR-010**: The firmware image MUST ship with the application and MUST be checked (present,
  complete, of the version the driver knows) before it is sent; a missing or invalid image MUST
  give a typed error and send nothing to the board. The application MUST NOT read firmware from
  folders of other software or from the network.
- **FR-011**: A board that does not reappear within the allowed time MUST give a typed error that
  says so and suggests unplugging it; the allowed time MUST be bounded and stated in the
  documentation.

**Acquisition**

- **FR-012**: Starting a capture MUST return immediately; all transfers MUST run off the
  interface thread; samples MUST flow into a capture of the signal storage core exactly as for
  every other device, with progress and events observable.
- **FR-013**: Stopping MUST take effect within the limit the device contract sets for every
  driver, close the capture with what was captured, and leave the board ready for the next one.
- **FR-014**: A capture MUST be able to finish by itself when its configured length is reached.
- **FR-015**: At most one capture runs per board; operations in the wrong state (start while
  running, stop while idle, configure while running) MUST be refused as the device contract
  defines.
- **FR-016**: The samples stored plus the positions recorded as lost MUST equal the positions the
  capture claims, for every capture and every failure, and a capture with any lost position MUST
  be marked incomplete.
- **FR-017**: When the stream of samples stops before the capture is complete (the computer or the
  USB path did not keep up, so the board stopped the capture), the capture MUST end with a typed
  error that says so; for a capture of a set length the positions never captured MUST be recorded
  as one gap at the end; the capture MUST be marked incomplete and MUST never be presented as
  clean.
- **FR-018**: A capture whose first data does not arrive, or whose data stops, within a bounded
  time MUST end with a typed timeout error and keep what it has.
- **FR-019**: A capture from this board MUST NOT be marked as simulated, and the session MUST
  record the device and its configuration.

**Failures and containment**

- **FR-020**: On unplugging, a transfer error, a stall, or any driver error, the capture MUST end
  with a typed error, keep the samples captured so far marked incomplete, and leave the
  application responsive; plugging the board in again MUST make it usable without restarting the
  application.
- **FR-021**: A driver error, panic or transfer that never returns MUST be contained: it MUST NOT
  crash or freeze the application, MUST NOT lose user data, and MUST NOT change the behaviour of
  any other driver or of the virtual device.
- **FR-022**: Every failure MUST reach the user as a typed error with a stable code and a message
  in the user's language that says what to do; it MUST NOT contain signal data.
- **FR-023**: The errors MUST distinguish at least: no permission to open the device, no driver
  installed (Windows), the device in use by another program, firmware not available or invalid,
  the board did not reappear, the board disconnected, a transfer failed, and the device is
  not a supported one.

**Testing and documentation**

- **FR-024**: The driver MUST be testable without hardware: a recorded exchange with the board is
  replayed to it, failures are injected at chosen points (unplug, short read, timeout, stall,
  panic, hang, a lost block), and the shared conformance suite of the device abstraction runs
  against it, on both supported operating systems.
- **FR-025**: The repository MUST contain a manual hardware verification procedure for this
  device with an expected result for each step (list, configure, capture at several rates, all
  channels, a known signal, stop, unplug, replug, a docking station or slow port, a busy device)
  and a place to record the run.
- **FR-026**: The documentation, in English, MUST describe how to get access to the device on
  Windows 10/11 (the driver to install and how) and on Debian 13 (the access rule, so that no
  administrator rights are needed), the lights of the board, the limits of the device (8
  channels, 24 MHz, input levels, no trigger, no memory, USB hubs and docking stations), the
  errors of FR-023 with what to do for each, and the firmware's origin and licence.
- **FR-027**: Every new user-facing text (error messages, the device name, the notices) MUST exist
  in the six languages of the product and pass the parity check.
- **FR-028**: Adding this driver MUST NOT change the behaviour of the virtual device or of any
  other driver, and MUST NOT change the device contract.
- **FR-029**: Whatever is taken from another project (the firmware image, any protocol
  knowledge) MUST be recorded with its origin, version, licence and reason in the plan; no code
  of PulseView or libsigrok is to be copied: the driver is written from the protocol, and its
  parsers of data coming from the device are covered by tests of malformed, truncated and
  boundary input.

- **FR-030**: The repository MUST contain what the rig needs: the program for the Arduino Nano
  that produces the chosen patterns on the pins wired to the analyzer's eight inputs, the wiring
  table (analyzer input, application channel, Nano pin and port bit), and the way the test code
  asks the Nano to start and stop a pattern; the documentation of the wiring and of loading the
  program MUST be in English.
- **FR-031**: The automated hardware tests MUST be opt-in: they MUST NOT run unless the maintainer
  switches them on, MUST NOT run in continuous integration, MUST find the Nano's serial port from
  a setting rather than a value fixed in the code, MUST be skipped with a visible reason (not
  failed) when the rig is absent or the switch is off, and MUST leave the Nano and the analyzer
  idle when they end, also when they fail.

### Key Entities

- **WeAct device**: a physical board with an identity (its place on the USB bus), a state
  (blank, preparing, ready, in use, gone), and the capabilities of FR-005.
- **Firmware image**: the program the board needs to be loaded each time it is plugged in; it has
  an origin, a version and a licence, and it is checked before use.
- **Capture configuration**: the channels, the sample rate (and the rate actually used), and the
  length, validated against the capabilities.
- **Recorded exchange**: a file of what was said between the application and a real board during
  a capture, used to replay the board to the driver and to inject failures.
- **Test rig**: the analyzer wired to an Arduino Nano whose pins D2 to D9 (PD2 to PD7, PB0, PB1)
  go to the analyzer's eight inputs in order; the Nano produces known patterns on request and is
  the signal source of the manual procedure and of the opt-in hardware tests.
- **Verification record**: the filled-in result of the manual procedure for one device, one
  operating system and one run.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the board plugged in and accessible, a user goes from starting the application
  to a capture showing in the signal view in under 15 seconds, including the preparation of a
  blank board, with no dialog in between.
- **SC-002**: In a capture of 8 channels at 1 MHz of a known signal, 100% of the stored samples
  equal the signal that was fed in (replayed recording: sample for sample; real board: on the
  verification procedure).
- **SC-003**: In 100% of the tested failure scenarios (unplug at any point, short read, timeout,
  stall, lost block, a firmware image that is missing or damaged, a board that does not come back)
  the user gets a typed, explained error, the captured data is kept and marked incomplete where
  applicable, and the application stays responsive.
- **SC-004**: In 100% of tested scenarios the samples stored plus the positions recorded as lost
  equal the positions the capture claims.
- **SC-005**: A requested stop closes the capture within the limit the device contract sets for
  every driver (100 milliseconds in the replayed tests).
- **SC-006**: 1,000 injected driver panics and hangs across varied points of an acquisition,
  preparation and listing never crash or freeze the application.
- **SC-007**: With no hardware, the shared conformance suite and the driver's own tests pass on
  Windows and on Debian in the continuous integration.
- **SC-008**: On a real board on a directly connected USB 2.0 port, a 60-second capture at 24 MHz
  on 8 channels ends either with every sample, or early with the typed error of FR-017 and the
  capture flagged incomplete with the never-captured positions recorded as a gap at the end; there
  is no third outcome.
- **SC-009**: A person who has not used the device follows the documentation on a clean Windows
  11 and a clean Debian 13 and gets a first capture in under 10 minutes on each.
- **SC-010**: A hot-plug sequence of 100 rapid plug and unplug events leaves the list showing
  exactly the boards that are present, with no board listed twice, and no application restart
  is needed.

- **SC-011**: On the maintainer's rig with the hardware tests switched on, every one of the eight
  channels is captured with a pattern that equals the one the Nano produced, in 100% of the
  runs of the hardware tests; on every other machine, and in the continuous integration, the
  same tests are skipped with a reason and the suite passes.

## Assumptions

- This is the first driver for real hardware: the first to need access to a USB device on both
  operating systems, so it also settles how the project does that (the choice of the library and
  of the way device access is shown is for the plan, under Principle V).
- The device contract, the acquisition behaviour, the test harness with recorded transports, the
  error model and the storage of lost samples are those of feature 004 and are not changed here.
- The board is the only hardware of this feature; the other two mandatory devices (the Pico
  analyzer and the Logic16 clone) are separate features (011, 012) and are not prepared for here
  beyond what the device contract already allows.
- The board has no trigger and no memory, so the driver declares no trigger; triggering in the
  application, if ever wanted for this board, is another feature.
- The input threshold is fixed by the hardware (high above 2 V, low below 0.8 V, 5.5 V at most);
  the application neither shows nor changes it, and the documentation states it.
- The driver does not install an operating system driver or change system settings; it detects
  that one is missing and says what to do.
- The shipped firmware image is a build of the open-source `fx2lafw` firmware; building it from
  its source or taking a published build is for the plan, which records the origin, the version
  and the licence and keeps the notices that the licence requires.
- The vendor repository has no stated licence, so nothing from it is copied; only the facts of
  its README are used. The firmware's own licence (GPL version 2 or later) is compatible with the
  project's GPL-3.0, and its origin is recorded where the plan records every reused item.
- The maintainer's rig is a local aid: the Nano is on the serial port COM7 of the maintainer's
  machine and may be on another port elsewhere; only the maintainer is expected to have it. Tests
  that need it are opt-in, and everything required by the constitution (tests without hardware,
  a manual procedure) stands without it.
- Talking to the Nano over a serial port is something only the opt-in tests need; if it needs a
  library, that library is a dependency of the tests only and is chosen in the plan under
  Principle V.
- The patterns on the Nano are produced by the Nano's own clock, which is not synchronised with
  the analyzer's: a test compares the captured signal with the pattern (its levels and the order
  and relative spacing of its changes, within what the two clocks allow), not sample for sample
  against a computed copy.
- The Nano's pins can only be driven at a few MHz at most, so the rig exercises correctness on
  all eight channels at modest rates; the highest rate of the analyzer (24 MHz) is verified in the
  manual procedure with a source that can reach it, and is not claimed by the rig.
- No capture data or telemetry leaves the computer (constitution, privacy).
