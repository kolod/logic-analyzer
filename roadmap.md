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
| 005 | `005-i18n-foundation` | Choice of the shared resource format and library (Fluent is the leading candidate), six languages, CLDR plurals, locale formatting, runtime language switch, key-parity check in CI. | 001 |
| 006 | `006-ui-shell-signal-view` | UI toolkit choice, window shell, signal view (zoom, pan, markers), capture control, CJK and long German layout, user-facing error handling. | 002, 004, 005 |
| 007 | `007-decoder-framework-annotations` | Annotation model (lane and signal overlay: points, edge arrows, ranges), decoder stacking, fault isolation, background cancellable execution, indexed lookup. One sample decoder with its generator. | 002, 004, 006 |
| 008 | `008-i2c-decoder-generator` | I2C decoder with overlays (start/stop on SDA, sampling edge on SCL), generator for the virtual device, round-trip tests, deliberate errors (NAK). | 007 |
| 009 | `009-decoded-log-export` | Decoded log export (CSV/text), versioned locale-independent format, streaming write, range selection, errors included in output. | 008 |
| 010 | `010-high-level-decoder-stack` | First higher-level decoder on top of I2C (for example an EEPROM-like one) with its generator; validates end-to-end stacking. | 008 |
| 011 | `011-driver-pico-logic-analyzer` | gusmanb Raspberry Pi Pico driver, recorded-transport tests, manual hardware verification procedure, device access (udev, Windows driver). | 004 |
| 012 | `012-driver-logic16-clone` | mcupro Logic16 clone driver, same requirements. | 004 |
| 013 | `013-driver-weact` | WeAct LogicAnalyzer driver, same requirements. | 004 |
| 014+ | as needed | More decoders (SPI, UART, 1-Wire, ...), raw data export, `.sr` import/export, packaging (Windows installer, `.deb`). | — |

## Notes on the order

- 001–004 form a foundation that is fully verifiable without hardware. Until 011, all
  testing runs against the virtual device.
- Drivers come after the UI and decoders because they need physical devices. There is
  one specification per driver on purpose: under Principle II a change to one driver
  must not affect the others. They can be done in parallel after 004.
- 005 and 006 close the constitution's `TODO(UI_TOOLKIT_AND_L10N_FORMAT)`. The UI
  toolkit must be chosen with fast signal rendering in mind (possibly a custom canvas
  renderer, with the toolkit used for everything else).
- The host API contract (`TODO(HOST_API_CONTRACT)`) first appears in 006/007; its
  location is decided then.

## Risks and open questions

- 003: native session format versus sigrok `.sr` from the start. A native format is
  easier to make robust (Principle I); `.sr` is convenient for exchange with PulseView.
- 013: the WeAct device protocol must be studied from its repository before the
  specification is written.
- 007 is the riskiest item: the annotation model determines overlays, export and
  generators.
