<!--
Sync Impact Report
- Version change: 1.3.0 → 1.4.0
  (MINOR: Principle XII materially expanded with third-party/PulseView code reuse
  rules driven by stability; project license GPL-3.0 recorded)
- 1.2.0 → 1.3.0 (earlier): target platforms fixed and localization mechanism
  redefined; no code existed yet, so nothing was invalidated
- Modified: Localization Requirements (Android strings.xml / Windows .resx replaced by
  one shared resource format for Windows and Debian; CLDR plurals); commit-scope
  example; Technology Constraints (target OS, device access)
- Added sections: none (Principles VIII–XII were added in 1.1.0–1.2.0)
- Removed sections: none
- Templates:
  - ✅ .specify/templates/plan-template.md (gates VIII–XII, target platforms)
  - ✅ .specify/templates/tasks-template.md (rendering, decoder, export, generator and
    both-OS tasks; Path Conventions)
  - ✅ .specify/templates/spec-template.md, checklist-template.md (no change needed)
- Follow-up TODOs:
  - TODO(UI_TOOLKIT_AND_L10N_FORMAT): choose the Rust UI toolkit and the shared
    localization format/library in the first plan that needs them.
  - TODO(HOST_API_CONTRACT): define the host API contract location once the first
    feature that introduces it is planned.
-->

# Logic Analyzer Constitution

A logic analyzer application written in Rust, functionally comparable to PulseView
(sigrok): capture digital signals from hardware, display and analyze them, and decode
protocols.

## Core Principles

### I. Stability & Data Integrity (NON-NEGOTIABLE)

The primary goal of the project is a program that does not crash and does not lose
data. This outranks features, performance and convenience.

- Production code MUST NOT panic on any external input (device data, files, user
  input, scripts). `unwrap()`, `expect()`, unchecked indexing and unchecked arithmetic
  on external data are forbidden outside tests; errors MUST be returned as typed
  `Result` values and surfaced to the user.
- `unsafe` code MUST be minimized, isolated in small modules, and documented with a
  `// SAFETY:` justification for each block.
- Captured data MUST NOT be lost: a capture in progress or an unsaved capture MUST be
  protected (periodic or streaming persistence, or an explicit unsaved-changes prompt
  on close). Device disconnects, USB errors and buffer overruns MUST keep everything
  captured up to that point and report the failure.
- Files MUST be written atomically (write to a temporary file, flush, then rename), so
  a crash or power loss never corrupts an existing file. Loaders MUST validate input
  and reject corrupt files with a clear error instead of crashing.
- A fault in one subsystem (a device driver, a protocol decoder, a script) MUST be
  isolated so it cannot take down the application or discard the current session.
- Data loss, crash, or silent corruption bugs are release blockers.

Rationale: a logic analyzer is used to capture rare, often unrepeatable events; losing
a capture is the worst failure the product can have.

### II. Hardware Support Is Mandatory

The following devices MUST be supported, each by a dedicated driver behind a common
device abstraction:

- WeAct LogicAnalyzer — https://github.com/WeActStudio/LogicAnalyzerV1
- mcupro Logic16 clone — https://sigrok.org/wiki/Mcupro_Logic16_clone
- Raspberry Pi Pico Logic Analyzer — https://github.com/gusmanb/logicanalyzer

Rules:

- Application and UI code MUST depend only on the common device abstraction (device
  discovery, capabilities, configuration, acquisition start/stop, sample stream); no
  device-specific logic is allowed outside its driver.
- Each driver MUST declare its capabilities (channel count, sample rates, trigger
  types, buffer depth) and the core MUST enforce them; invalid configuration MUST be
  rejected before it reaches hardware.
- Each driver MUST be testable without hardware through a recorded-transport or
  simulated-device test, and MUST have a documented manual hardware verification
  procedure.
- Adding or changing a driver MUST NOT alter the behavior of the other drivers.
- A release MUST NOT drop support for any of the three devices above.

Rationale: these three devices define the product's scope; the abstraction keeps
adding future hardware cheap and keeps driver faults contained.

### III. Test-First & Verified Merges

- Tests MUST be written before or together with the implementation, and MUST fail
  before the implementation makes them pass for bug fixes and new behavior.
- Signal-processing, protocol-parsing, file-format and device-protocol code MUST have
  unit tests including malformed and boundary inputs; file loaders and device-protocol
  parsers MUST additionally be fuzzed or property-tested.
- Every bug fix MUST include a regression test.
- CI MUST run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` on
  every supported platform; a failing step blocks merge.

Rationale: stability (Principle I) is only credible if it is demonstrated mechanically.

### IV. Layered Architecture & Clear Contracts

- The domain logic (acquisition, sample storage, decoding, file formats) MUST live in
  platform-independent Rust crates with no UI dependencies, so it is testable
  headlessly.
- UI shells and platform integrations MUST talk to the core only through a documented
  host API contract. The contract is versioned; a change to it MUST be documented,
  implemented in the core and on every platform that consumes it, and merged
  together (see Version Control).
- Long-running work (acquisition, decoding, loading) MUST NOT block the UI thread and
  MUST be cancellable.
- Memory use during capture MUST be bounded and predictable; limits MUST be explicit
  and exceeding them MUST produce a handled error or controlled spill to disk, never
  an out-of-memory crash.

Rationale: a clean core/shell split keeps the Rust core stable and lets the same
logic serve every platform.

### V. Simplicity & Minimal Dependencies

- Prefer the Rust standard library and a small set of well-maintained crates. Each new
  dependency MUST be justified in the plan (purpose, maintenance status, license).
- Dependencies MUST be pinned via `Cargo.lock` and audited (`cargo audit` or
  `cargo deny`) in CI.
- Start with the simplest design that satisfies the spec (YAGNI); complexity MUST be
  recorded and justified in the plan's Complexity Tracking table.

### VI. Observability & Diagnosability

- The application MUST log structured diagnostics (device events, errors, capture
  lifecycle) to a local log file with bounded size.
- Every user-visible error MUST state what failed and, where possible, what the user
  can do; raw internal errors are for the log, not for the dialog.
- Crash information MUST NOT be silently discarded; panics in worker threads MUST be
  caught at the thread boundary, logged, and reported while preserving user data.

### VII. Internationalization From Day One

All user-facing text is localized under the rules in "Localization Requirements".
No feature is complete until it is localized in all six languages.

### VIII. Fast Signal Rendering

- Rendering the visible part of a signal MUST cost O(log N + W), where N is the number
  of stored samples/transitions and W is the number of visible pixels. It MUST NOT
  be O(N) in the capture size, so zooming and panning stay smooth on very large
  captures.
- To achieve this, signal storage MUST be indexed for logarithmic lookup (for example,
  transition-based storage with a binary-searchable index, plus a multi-resolution
  summary of transition counts per time bucket for zoomed-out views).
- Zoomed-out views MUST NOT lose information silently: a pixel that contains several
  transitions MUST be visibly distinguished from one that contains none (for example,
  by a dense-activity indication), so glitches are not hidden.
- Rendering performance MUST be covered by benchmarks with a recorded baseline; a
  regression of the complexity class is a merge blocker.

Rationale: a logic analyzer is only useful if large captures remain explorable
interactively.

### IX. Protocol Decoding

- The application MUST support protocol decoders in the spirit of PulseView (sigrok)
  and Saleae Logic 2: decoders are stackable (a decoder may consume the output of
  another), configurable per channel, and produce time-stamped annotations from the
  captured signals.
- Compatibility with PulseView or Logic 2 decoder formats is NOT required. The decoder
  interface is defined by this project and MUST be documented in English as part of
  the host/decoder contract.
- Decoders MUST be treated as untrusted-quality code: a decoder error, panic, hang or
  excessive memory use MUST be contained, reported, and MUST NOT affect the signal
  data or other decoders (see Principle I). Decoding MUST run in the background and
  be cancellable.
- A decoder's output MUST be able to place annotations in two ways:
  1. in the decoder's own lane (ranges, text, packets), and
  2. as an overlay drawn directly over a signal channel — for example, a colored dot
     on SDA for I2C start/stop conditions, or an arrow on the SCL edge at which data
     is sampled.
  The annotation model MUST therefore support at least: a time range, a point, an edge
  marker (direction), a target channel (or the decoder lane), a style (color, shape),
  and text — all independent of any UI toolkit.
- Overlay rendering MUST keep the O(log N + W) bound of Principle VIII: only
  annotations in the visible range are processed, via an indexed lookup.
- Built-in decoders MUST have tests with recorded signals, including malformed and
  truncated input.

### X. Decoded Data Export

- The application MUST export decoded results as a human- and machine-readable log,
  e.g. the decoded I2C packets (address, direction, data bytes, ACK/NAK) with
  timestamps, in addition to exporting raw captured data.
- Export formats MUST be documented, stable and versioned. Machine-readable output
  (such as CSV) MUST be locale-independent (fixed decimal separator, ISO-style
  timestamps or explicit time units, English or fixed column names), unlike on-screen
  text, which follows the user's locale.
- Export MUST be able to cover the whole capture or a selected range, MUST stream
  large captures without loading all output into memory, MUST write files atomically
  (Principle I), and MUST be cancellable.
- Export MUST report decoder errors and incomplete regions in the output rather than
  omit them silently.

### XI. Virtual Device & Generated Signals

- The application MUST offer a virtual capture device selectable like any hardware
  device, so that every feature (capture, rendering, decoding, export) can be tried
  without capture hardware. It is a regular driver behind the common device
  abstraction (Principle II), always available, and is in addition to the three
  mandatory hardware devices.
- The virtual device generates signal data itself. The data is produced according to
  the configuration: if a decoder (for example I2C) is configured on the channels —
  including a higher-level decoder stacked on top of it — the generated signals MUST
  be valid for that decoder and decode correctly through the whole stack. With no
  decoder configured, it generates generic random digital signals.
- A decoder that can be used with the virtual device MUST provide a signal generator
  as its counterpart: it produces random but protocol-valid content at its own level
  and encodes it down through the lower-level decoders to the channel signals. For
  protocols that define error conditions, the generator SHOULD be able to produce
  them deliberately (for example NAK or framing errors), so the error paths can be
  explored.
- Generation MUST be reproducible: the user can supply a seed, and the same seed and
  configuration MUST produce identical signals. Generation MUST NOT block the UI and
  MUST be cancellable.
- Round-trip tests are REQUIRED for every decoder that has a generator: generated
  content decoded by the decoder MUST equal the content that was generated.
- Simulated data MUST be clearly labelled as simulated in the UI, and in saved
  captures and exports (a metadata flag), so it can never be mistaken for a real
  measurement.
- The virtual device MUST also serve as the basis for hardware-free tests and for
  large-capture rendering benchmarks (Principles III and VIII).

Rationale: users can evaluate and learn the full feature set without hardware, and
the decoders get an automatic correctness check.

### XII. Original Design, Careful Inspiration

- The UI design takes inspiration from Saleae Logic 2 where it serves usability (it
  is the author's preferred reference), but Logic 2 is a commercial product and MUST
  NOT be copied.
- It is forbidden to reuse Saleae's code, assets, icons, fonts, names, trademarks,
  logos, screenshots, exact color schemes or pixel-level layouts, or to present the
  product as affiliated with Saleae. Interaction concepts and general ideas (for
  example, overlay markers on signals) MAY be adopted when re-implemented
  independently.
- PulseView/sigrok are a source of ideas, protocol knowledge and test data, NOT a
  codebase to port. PulseView is known to crash frequently, which directly contradicts
  Principle I; code taken from it (or from libsigrok / libsigrokdecode and their
  decoders) MUST be treated as untrusted quality.
- Any code, algorithm or decoder derived from PulseView/sigrok or another third-party
  project MUST be recorded in the plan (origin, version, why reuse instead of a fresh
  implementation) and MUST, before it is merged:
  1. be reviewed line by line against Principle I (no panics on external input, no
     unchecked indexing or arithmetic, no unbounded memory, errors returned as typed
     values), and rewritten idiomatically in safe Rust rather than transliterated from
     C/C++/Python;
  2. be covered by tests, including malformed, truncated and boundary input, and
     fuzzing where it parses external data;
  3. be isolated so that a defect cannot crash the application or lose user data.
- Reuse is the exception, justified per case; reimplementation from the protocol
  specification is the default.
- Independently of stability, the project is licensed under GPL-3.0: incorporated code
  MUST have a compatible license, and attribution MUST be preserved.
- Visual identity (name, icons, color palette, layout details) MUST be original.

## Hardware Support & Technology Constraints

- **Language**: Rust (stable toolchain; the minimum supported Rust version is recorded
  in the repository and changes only deliberately).
- **Target operating systems**: Windows 10/11 and Debian 13. Every feature MUST work
  on both; CI MUST build and test on both. OS-specific code MUST be isolated behind
  platform-independent interfaces (Principle IV).
- **Device access**: the documentation MUST describe how to get USB/serial device
  access on each OS (driver installation on Windows; udev rules, so no root is needed,
  on Debian). Missing permissions MUST produce an actionable error (Principle VI).
- **Performance**: sample ingestion MUST keep up with the highest sample rate declared
  by each driver without dropping samples; if samples are lost the capture MUST be
  flagged as incomplete, never silently accepted as valid.
- **Interoperability**: the application SHOULD read and write sigrok-compatible
  session files (`.sr`) so captures can be exchanged with PulseView.
- **Privacy**: the application MUST NOT transmit captured data or telemetry over the
  network without explicit user consent.
- **Documentation language**: all project documentation is written in English —
  constitution, specifications, plans, tasks, README, code comments, commit messages,
  the host API contract description, and the documentation of any scripts (device or
  protocol-decoder scripts).

## Localization Requirements

- The UI supports six languages: English (`en`, base and fallback), German (`de`),
  French (`fr`), Spanish (`es`), Japanese (`ja`), Ukrainian (`uk`).
- No UI string may be hardcoded in code. Strings live in external resource files that
  use ONE shared format on all supported operating systems (Windows and Debian); the
  format and its Rust library are chosen in the first plan that introduces UI text.
- The chosen mechanism MUST support CLDR plural rules, automatic fallback to English,
  and selecting the language at runtime independently of the system language (see
  below).
- String keys are shared by all platforms by construction. The English resource file is
  the single source of truth for keys. A missing translation MUST automatically fall
  back to English.
- Plural forms MUST be resolved through the localization mechanism, never by ad-hoc
  code. Ukrainian has multiple plural forms and Japanese has none; both MUST be
  correct.
- Dates, numbers, frequencies and file sizes MUST be formatted according to the user's
  locale.
- The user MUST be able to change the app language in the application settings
  independently of the system language.
- Fonts and layout MUST render Japanese (CJK) correctly and MUST accommodate long
  German strings without truncation or clipping.
- User- or third-party-supplied content (channel names, annotations, labels, output of
  third-party protocol decoders and scripts) is NOT translated and is displayed as is.
- A new language or a new UI string MUST NOT ship in a release until translations for
  all six languages exist. Machine translation is acceptable as a draft.
- CI MUST verify key parity across languages and flag missing or unused keys.

## Version Control & Development Workflow

- `main` is always in a working state: every commit on `main` builds and passes all
  tests.
- All work is done in feature branches named in the Spec Kit format
  `NNN-short-name` (e.g., `001-logic16-driver`). Merging to `main` happens only after
  tests pass.
- Commit messages are in English and follow Conventional Commits with a component
  scope, e.g. `feat(ui): ...`, `fix(drivers/logic16): ...`, `docs: ...`.
- One commit is one logical change. A change to the host API contract and its
  implementation on all platforms land in `main` together.
- Every plan MUST pass the Constitution Check below before research and again after
  design; every pull request MUST be reviewed against this constitution.

## Governance

This constitution supersedes all other practices. Where a spec, plan, task or habit
conflicts with it, the constitution wins until it is formally amended.

- **Amendments**: proposed as a change to this file in a feature branch, with a
  rationale and a Sync Impact Report; merged only after dependent templates and
  documents are updated in the same change.
- **Versioning** (semantic): MAJOR — removal or incompatible redefinition of a
  principle or governance rule; MINOR — a new principle/section or materially
  expanded guidance; PATCH — clarifications and wording fixes.
- **Compliance review**: every plan includes a Constitution Check; every pull request
  verifies compliance; violations MUST be justified in the plan's Complexity Tracking
  table or fixed. Principle I violations cannot be waived.
- **Runtime guidance**: day-to-day agent guidance lives in the repository's agent
  guidance file (e.g., `CLAUDE.md`) and MUST NOT contradict this document.

**Version**: 1.4.0 | **Ratified**: 2026-11-14 | **Last Amended**: 2026-11-14
