# Roadmap

Planned feature specifications, in implementation order. Each item is one Spec Kit
feature developed in its own branch (`NNN-short-name`) and merged into `main` only
after tests pass. The order follows the constitution
(`.specify/memory/constitution.md`): everything up to the first hardware driver can be
built and verified without capture hardware, using the virtual device.

## Specifications

| No. | Branch | Scope | Depends on |
|-----|--------|-------|------------|
| 001 | `001-workspace-ci-foundation` (merged) | Rust workspace, MSRV, fmt/clippy/test/`cargo deny` in CI on Windows and Debian, size-bounded logging, error type, "no panic on external input" rule. | — |
| 002 | `002-signal-storage-core` (merged) | Transition-based signal storage with an index for O(log N) lookup, summaries for zoomed-out views, bounded memory, benchmarks with a recorded baseline. | 001 |
| 003 | `003-session-file-format` (merged) | Atomic write and safe read of the native session format, validation, fuzzing, "simulated" flag. Sigrok `.sr` import/export is a later, separate item. | 002 |
| 004 | `004-device-abstraction-virtual-device` (merged) | Common device abstraction (discovery, capabilities, configuration, sample stream), seeded virtual device, configuration validation, disconnect and buffer-overrun handling. | 002 |
| 005 | `005-i18n-foundation` (merged) | Choice of the shared resource format and library (Fluent is the leading candidate), six languages, CLDR plurals, locale formatting, runtime language switch, key-parity check in CI. | 001 |
| 006 | `006-ui-shell-signal-view` (merged) | UI toolkit choice, window shell, signal view (zoom, pan, markers), capture control, CJK and long German layout, user-facing error handling. | 002, 004, 005 |
| 007 | `007-decoder-framework-annotations` (merged) | Annotation model (lane and signal overlay: points, edge arrows, ranges), decoder stacking, fault isolation, background cancellable execution, indexed lookup. One sample decoder with its generator. | 002, 004, 006 |
| 008 | `008-i2c-decoder-generator` (merged) | I2C decoder with overlays (start/stop on SDA, sampling edge on SCL), generator for the virtual device, round-trip tests, deliberate errors (NAK). | 007 |
| 009 | `009-decoded-log-export` | Decoded log export (CSV/text), versioned locale-independent format, streaming write, range selection, errors included in output. | 008 |
| 010 | `010-high-level-decoder-stack` | First higher-level decoder on top of I2C (for example an EEPROM-like one) with its generator; validates end-to-end stacking. | 008 |
| 011 | `011-driver-pico-logic-analyzer` | gusmanb Raspberry Pi Pico driver, recorded-transport tests, manual hardware verification procedure, device access (udev, Windows driver). | 004 |
| 012 | `012-driver-logic16-clone` | mcupro Logic16 clone driver, same requirements. | 004 |
| 013 | `013-driver-weact` (merged) | WeAct LogicAnalyzer driver, same requirements. | 004 |
| 014 | `014-gui-improvements` (merged) | Interface rework (no change to the host API contract of decoding or storage), in one feature or split in two when planning: see the list under "GUI improvements" below. | 006, 007, 008 |
| 015 | `015-markers-rework` (merged) | Two kinds of marker, single (T) and pair (P), replacing the one kind of 006: lines with a pentagon-arrow or two squares, placed with the mouse and snapped to transitions, colours, comments, time origin, cutting and trimming the data, fit and scroll to a marker, duration and frequency of a pair; the Markers panel with two add buttons. | 006, 014 |
| 016 | `016-i2c-lane-blocks` | The lane of the I2C decoder as two bands of solid blocks: the parts of a packet (start, address, data, ack/nak, stop) above, the whole packet below, green or red; decoder API 1.2 (block shapes, rows of a lane). | 008 |
| 017 | `017-driver-rp2350b-32ch` | Driver for a 32-channel logic analyzer: the continuation of the [gusmanb/logicanalyzer](https://github.com/gusmanb/logicanalyzer) project on the RP2350B chip, on the WeAct RP2350B Core Board ([schematic](https://github.com/WeActStudio/WeActStudio.RP2350BCoreBoard/blob/main/HDK/RP2350B_SCH.pdf)), with 32 channels instead of 24, up to five boards chained (160 channels, so the application's limit of 32 channels per capture rises with it). It needs firmware written in Rust that extends the support from 24 to 32 channels: see "The 32-channel driver" below. Recorded-transport tests, manual hardware verification procedure, device access. | 004, 011 |
| 018+ | as needed | More decoders (SPI, UART, 1-Wire, ...), raw data export, `.sr` import/export, packaging (Windows installer, `.deb`). | — |

## Notes on the order

- 001–004 form a foundation that is fully verifiable without hardware. Until 011, all
  testing runs against the virtual device.
- Drivers come after the UI and decoders because they need physical devices. There is
  one specification per driver on purpose: under Principle II a change to one driver
  must not affect the others. They can be done in parallel after 004.
- 005 and 006 close the constitution's `TODO(UI_TOOLKIT_AND_L10N_FORMAT)`. The UI
  toolkit must be chosen with fast signal rendering in mind (possibly a custom canvas
  renderer, with the toolkit used for everything else).
- The host API contract (`TODO(HOST_API_CONTRACT)`) appeared in 006: it lives in
  [docs/host-api.md](docs/host-api.md), versioned and checked against the code by a test.
- The decoder contract appeared in 007: it lives in [docs/decoder-api.md](docs/decoder-api.md),
  versioned and checked against the code by a test. 008 and 010 add decoders by registering a
  descriptor (see "Adding a decoder" in [CONTRIBUTING.md](CONTRIBUTING.md)); they need no change to
  the application shell.

## GUI improvements (014)

What the feature covers, as asked by the owner; the plan decides the order and whether it is split.

- **Recent files as a submenu**: the File menu's "open recent" entry becomes a submenu of the 15 most
  recently used files.
- **Icons**: an application icon and icons for the session files (Windows file association and
  Debian desktop entry and MIME type). Original artwork only (Principle XII).
- **A right side panel, like the left bar of VS Code**: Decoders, Markers and the capture settings are
  views of one panel on the right; only one is visible at a time and an icon strip switches between
  them.
- **Channels edited in the left strip of the signals** (the Channels panel goes away): a click on the
  vertical colour bar left of a name (the bar the strip already draws, in the colour of the channel,
  next to the name in the same colour) changes the channel's colour; a click on the name renames it;
  the height of a signal changes by dragging its border; signals are reordered by dragging them.
- **Capture settings in a new right-hand panel**: device, sample rate, length (in seconds), the
  channels to capture (buttons, eight in a row; a disabled channel has the text and border in the
  channel's colour, an enabled one the border and the background in the channel's colour), and the
  other settings of the capture.
- **Quick scroll to the next edge**: when the mouse nears the left or the right end of a signal a
  button with an arrow appears; it scrolls to the next edge of that signal without changing the zoom.
  Going right puts the edge a quarter of the view from the left; going left, a quarter from the right,
  so that more of what follows is in view.
- **The decoder menu**: "Decoders > Add decoder" and the D key do nothing since a second decoder was
  added (feature 008, `verification.md`, "Known issues"); the menu offers one entry per decoder, or the
  key opens a choice.
- (More items may be added here before the specification is written.)

## The 32-channel driver (017)

What the feature covers, as asked by the owner; the specification decides the details.

- **The device**: a logic analyzer built as the gusmanb Raspberry Pi Pico one, but on an RP2350B chip,
  which has the pins for 32 channels, on the WeAct RP2350B Core Board. The board's schematic is the
  reference for which pins are free for the inputs.
- **The driver**: one specification and one driver of its own (Principle II), next to the gusmanb Pico
  driver of 011, whose protocol it continues; the number of channels is 32, not 24.
- **The firmware**: written in Rust, extending the support from 24 to 32 channels. Its functions must be
  at least as good as those of the original firmware, at least for 24 channels: nothing the original
  does with 24 channels may be missing or worse (the list of those functions is made from the original's
  documentation and behaviour at the start of the specification, and each one gets a test or a
  verification step).
- **Hardware verification**: a procedure for the owner, with the board, as for the other drivers.

## Risks and open questions

- 003: native session format versus sigrok `.sr` from the start. A native format is
  easier to make robust (Principle I); `.sr` is convenient for exchange with PulseView.
- 013: the WeAct device protocol must be studied from its repository before the
  specification is written.
- 017: the protocol and the licence of the original gusmanb firmware must be studied before the
  specification is written; the Rust firmware is written from the behaviour of the protocol and not by
  copying its code (Principle XII). Whether the 32 channels fit the existing protocol or need an extension
  of it, and what the RP2350B's speed and memory allow at 32 channels, are open.
- 007 was the riskiest item: the annotation model determines overlays, export and
  generators. It is merged; the open points it leaves are in its pull request (the wording of
  Principle IX's overlay bound against Principle VIII, process isolation for future user-written
  decoders, and the manual checks on Debian).
