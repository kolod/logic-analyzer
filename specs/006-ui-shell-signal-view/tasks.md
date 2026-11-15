---

description: "Task list for UI Shell and Signal View"
---

# Tasks: UI Shell and Signal View

**Input**: Design documents from `/specs/006-ui-shell-signal-view/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first): in every phase the test tasks come
first and MUST fail (usually they do not compile yet) before the implementation tasks start. If a
test passes immediately because earlier work already covers it, run a mutation check (temporarily
break the code it guards, see the test fail, restore) before relying on it. Expected values (zoom
tables, tick steps, formatted numbers in six languages, byte vectors of the `VIEW` section) are
written by hand from the contracts, never computed by the code under test. Frame plans are compared
with a brute-force reference built from the stored transitions.

**Organization**: tasks are grouped by user story (US1–US6, numbered as in spec.md, executed in
priority order US1, US2, US6 (all P1), then US3, US4, US5 (all P2)). The crates, the state/command
core, the frame plan and the shared helpers are Foundational because every story needs them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root. New crates: `crates/host` (package `la-host`) and
  `crates/app` (package `la-app`, binary `logic-analyzer`).

## Path Conventions

Host source in `crates/host/src/`, host tests in `crates/host/tests/`, benchmarks in
`crates/host/benches/`; application source in `crates/app/src/`, assets in `crates/app/assets/`;
resources in `resources/i18n/<lang>/ui.ftl` and `errors.ftl`; the host API document in
`docs/host-api.md`. The contracts are `specs/006-ui-shell-signal-view/contracts/host-api.md` ("the
API contract"), `view-section.md` ("the section contract") and `shell-layout.md` ("the layout
contract"); the entities and rules are in `data-model.md`; decisions in `research.md` (R1–R13).

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and
`unsafe` in production code, in `la-host` **and** `la-app`. Use `get`, `checked_*`, `saturating_*`,
`u128` intermediates and `?`. `f64` is allowed only for the final screen coordinate of a drawing
primitive and for values handed to the localizer. Test code may relax this with the file-level
`#![allow(clippy::unwrap_used, ...)]` used by the other crates' tests. Never commit while clippy
fails: a helper that only a later task uses gets `#[expect(dead_code, reason = "used by T0xx")]`,
removed by that task. Never log signal data or user text; log command names, codes, counts and file
names. Every UI string comes from `ui.ftl` (English first, then the five translations as marked
drafts in the same phase) so the parity check stays green at every commit. All text in files and
code is English except the translations.

---

## Phase 1: Setup

**Purpose**: the two crates, the resource file, licence and CI adjustments, and the two feasibility
spikes (Debian build packages, headless layout testing)

- [X] T001 Create `crates/host/Cargo.toml` (package `la-host`, workspace edition/MSRV 1.85/license, `[lints] workspace = true`; dependencies `la-core`, `la-diagnostics`, `la-storage`, `la-devices`, `la-session`, `la-i18n`, `tracing`, `serde` (derive), `serde_json`, `directories`, current stable versions at least two weeks old, MSRV-1.85 compatible; feature `test-support`) and `crates/host/src/lib.rs` with `#![deny(missing_docs)]`, the crate doc (what it is, "never log signal data or user text"), `pub const HOST_API_VERSION: &str = "1.0";` and empty module declarations per the plan's structure; stub `crates/host/benches/frame_plan.rs` (`fn main() {}`, `[[bench]] harness = false`)
- [X] T002 Create `crates/app/Cargo.toml` (package `la-app`, `[[bin]] name = "logic-analyzer"`, workspace edition/license/lints, its own `rust-version` = highest `rust-version` in the dependency tree measured with `cargo metadata` (1.92 at plan time); dependencies `iced` 0.14 with features `canvas`, `wgpu`, `tiny-skia` and a task executor feature, `rfd`, `la-host`, `la-i18n`, `la-diagnostics`, `tracing`) and `crates/app/src/main.rs`: install logging and the panic hook (`la_diagnostics`), build the localizer from settings, open an iced window whose title is the localized `app-title`; `cargo run -p la-app` opens an empty window
- [X] T003 [P] Create `resources/i18n/<lang>/ui.ftl` for `en`, `de`, `fr`, `es`, `ja`, `uk` (header comment with `### status: draft`, one message `app-title` = "Logic Analyzer" and its translations, which stay the product name); register `ui.ftl` in the embedded list in `crates/i18n/src/resources.rs` and in the unit test that checks every language has the same file names; run `cargo test -p la-i18n` (the parity check must still pass)
- [X] T004 [P] Add `"BSL-1.0"` to the `allow` list in `deny.toml` with a comment naming the two Windows-only clipboard crates of the iced tree (research R13); change the `msrv` job in `.github/workflows/ci.yml` to `cargo +<msrv> build --workspace --exclude la-app` and the same for `test`; update the rows for `msrv` and `deny` in `specs/001-workspace-ci-foundation/contracts/ci-checks.md`
- [X] T005 Run `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`; confirm `Cargo.lock` gains only the two crates and the third-party tree named in research R13 and that every licence is allowed (`cargo deny` if installed, else rely on CI)
- [X] T006 Spike (Debian build packages): push the branch, open a **draft** PR, read the Debian clippy, test and msrv jobs; if the bare `debian:trixie` container lacks packages for `iced`/`winit`/`wgpu`/`rfd`, add them to the "Debian prerequisites" step of each job that builds `la-app` and record the exact list and the reason in `CONTRIBUTING.md` (section "Building the application on Debian"); repeat until the three Debian jobs are green with the empty window
- [X] T007 Spike (headless layout test): write `crates/app/tests/spike_headless.rs` that builds one real `view` (a label inside a width-limited container) and tries the toolkit's headless test support (`iced_test`/simulator) and, failing that, an offscreen tiny-skia render, on Windows and on the Debian CI job; record in `research.md` (new subsection "R10 addendum") whether text bounds can be measured headlessly on both OSes and **decide**: (a) the text-fit check of task T077 measures real layout, or (b) the fallback resource lint of research R10 is implemented instead; delete the spike test when the decision is written down

**Checkpoint**: both crates build, lint and pass CI on both OSes; the two spikes are decided

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the error codes, the shared helpers, the frame plan, the state/command core with its
panic boundary

**⚠️ CRITICAL**: no user story work begins until this phase is complete

### Tests first (all fail before implementation)

- [X] T008 [P] Write `crates/host/tests/sanitize.rs` for `sanitize_text(&str, max_chars) -> String` (the display sanitizer, data-model "Channel presentation"): ASCII passthrough, control characters removed (C0, C1, DEL), bidi controls U+202A–202E and U+2066–2069 removed, zero-width and BOM characters removed, combining marks kept, Japanese/Cyrillic/emoji kept, text longer than 64 characters truncated at a character boundary (never inside a surrogate-like sequence or grapheme cluster of combining marks where avoidable), all-control input becomes empty, idempotence (`sanitize(sanitize(x)) == sanitize(x)`) over a seeded corpus of 10,000 random strings including invalid-looking sequences
- [X] T009 [P] Write `crates/host/tests/window.rs` for `ViewWindow` (data-model "View window and zoom"): hand-written tables for `zoom`, `pan_samples`, `pan_pixels`, `fit_all`, `zoom_to`, `reveal` across capture lengths 0, 1, 2, 1000, 10⁹, `u64::MAX/2` and widths W = 1, 800, 4096; the minimum length `max(1, ceil(W/64))`, the maximum `capture_length`, no overflow at the top of the `u64` range, the anchor sample keeps its fractional position across a zoom (±1 sample), pan at both ends clamps, empty capture uses the nominal window; then a seeded random walk of 100,000 operations asserting the three invariants after every step
- [X] T010 [P] Write `crates/host/tests/axis.rs` for tick selection and labels (research R11): step chosen from {1, 2, 5}×10^k seconds with ticks at least 80 px apart for spans from 10 ps to 1000 s and widths 400–4096 px; major ticks at multiples of the next decade; the label digits follow the step (never `0.30000000000000004`); first tick at or after the window start; a window of one sample; hand-written expectations for ten windows
- [X] T011 [P] Write `crates/host/tests/notices.rs` for `NoticeList` (data-model "Message"): add, identical consecutive notices group (`count` increases, `last_at` updates), different notices do not, the list is bounded to 50 (oldest dropped), dismiss one and all, the text is produced from the current language at display time (a language change changes the text)
- [X] T012 [P] Write `crates/host/tests/settings.rs` for `Settings` (data-model "Settings", research R8): defaults, round trip, clamping (UI scale 75–200, memory limit within the storage limits, recent files ≤ 10), window geometry off-screen ignored, unknown fields preserved on save, and the damage table: missing file → defaults and no notice, unreadable → `host.settings_unreadable`, not UTF-8 / invalid JSON / wrong version / larger than 1 MiB → `host.settings_invalid`, defaults used in every case; the write goes through the temp-file/rename discipline (use the file-system seam of `la-session` with `FaultFs`; a failure at any step leaves the old file readable and reports `host.settings_write`)
- [X] T013 [P] Write `crates/host/tests/plan.rs` for the frame plan (data-model "Frame plan"): build seeded random captures (1–32 channels, 0 to 10⁶ transitions, with and without gaps) with the storage API, compute the plan for several windows and widths, and compare with a brute-force reference written in the test from `Snapshot::transitions` (levels per column, single edges with their position, dense columns with counts, gaps, beyond-end); invariants: segments contiguous, sorted, non-overlapping and covering `[0, W)`, adjacent equal levels merged, a quiet channel is exactly one segment, a window past the end is `Beyond`, an empty capture gives `state = Empty`, invisible channels are omitted, the order follows the presentation; an injected failure in the viewport call yields `state = CannotDraw(host.render_failed)` and no panic
- [X] T014 [P] Write `crates/host/tests/panic_boundary.rs` (research R9): a test-support command that panics inside `apply` leaves `State` unchanged (same `version`), adds one `host.internal` notice, logs the command name, and the shell keeps working; an `apply` that returns a typed error adds a notice and changes nothing else; `poll` with nothing happening returns empty `Changes` and `wants_ticks()` is `Slow`

### Implementation

- [X] T015 Implement `crates/host/src/error.rs`: the seven `host.*` codes of data-model "New error codes" as constants and constructors on `la_core::Error` (kinds as in the other crates, technical English detail, stable codes), unit tests for code strings and kinds
- [X] T016 [P] Implement `crates/host/src/sanitize.rs` (T008 passes)
- [X] T017 [P] Implement `crates/host/src/window.rs`: `ViewWindow`, `ViewSize`, and the total operations with checked or `u128` arithmetic (T009 passes)
- [X] T018 [P] Implement `crates/host/src/axis.rs`: step selection, `Tick`, label formatting through the localizer's duration formatter with the digits rule (T010 passes)
- [X] T019 [P] Implement `crates/host/src/notices.rs`: `Notice`, `NoticeId`, `Severity`, `NoticeList` with grouping and bound; text production through `la-i18n` (T011 passes; the six-language messages come in T025)
- [X] T020 Implement `crates/host/src/settings.rs`: the model, validation and clamping, `load(path)`/`save(path)` with the atomic write and the damage handling (T012 passes); the directory comes from `directories` behind a function taking an override path for tests
- [X] T021 Implement `crates/host/src/plan.rs`: `FramePlan`, `ChannelPlan`, `Segment`, `FrameState`, `build(snapshot, window, pixels, presentation_view, markers_view)` using `Snapshot::viewport`, merging columns into segments, the axis from T018; a panic or error inside returns `CannotDraw` (T013 passes). Presentation and markers are passed as small read-only views so this task does not depend on later stories (empty lists until US2/US3)
- [X] T022 Implement `crates/host/src/state.rs` and `command.rs`: `State`, `Versions`, `Activity`, `Unsaved`, `CaptureView`, `ProgressView`, `Command`, `Outcome`, `Changes`, `Decision`, `Tick`, exactly as in data-model and the API contract (only the commands of the stories already built are executable; the rest return `Outcome::Ignored(host.invalid_command)` until their phase lands, with the full enum defined now)
- [X] T023 Implement `crates/host/src/shell.rs` and `lib.rs` exports: `Shell::new(ShellConfig)` (loads settings, builds the `Localizer` with `from_settings`, builds the `DeviceRegistry` with the virtual provider), `apply` with the working-copy-and-panic-boundary execution of data-model "Command execution", `poll`, `state`, `frame_plan`, `text`, `notice_text`, `wants_ticks`, `log_location`; a `test-support` command to provoke a panic (T014 passes)
- [X] T024 Add the seven `host.*` messages to `resources/i18n/en/errors.ftl` (what failed, what to do, English wording following the existing style) and the five translated drafts (`de`, `fr`, `es`, `ja`, `uk`); register the codes in the error catalogue of `la-i18n` (`crates/i18n/src/errors_catalogue.rs`) and its completeness test via a dev-dependency on `la-host` (if cargo rejects the dev-dependency cycle, keep a literal list of the seven codes in the test and add a test in `la-host` asserting it equals the constants); run `cargo test --workspace` (catalogue and parity must pass)
- [X] T025 Run `cargo fmt`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`; commit the phase

**Checkpoint**: the host core exists with a tested frame plan, window maths, notices, settings and a
panic-safe command executor; no UI yet

---

## Phase 3: User Story 1 - Capture signals and watch them appear (Priority: P1) 🎯 MVP

**Goal**: the application starts, lists devices, configures and runs a capture, shows the signals as
they arrive, and keeps the data on stop or failure

**Independent Test**: start with no hardware, choose the virtual device, capture with a fixed seed
and length, compare the displayed sample count and waveform with the expected data; repeat with a
scripted failure and check the earlier data stays (spec US1)

### Tests first

- [X] T026 [P] [US1] Write `crates/host/tests/capture_scenarios.rs` against `Shell`: the device list contains the virtual device marked simulated and selected by default; editing the config (channels, rate, length, trigger, seed) validates through `la_devices::validate` and reports per-setting problems with the allowed values; `StartCapture` with problems is ignored; a valid start runs the acquisition and `poll` reports `ACTIVITY | CAPTURE | PROGRESS`; completion with a fixed seed yields exactly the configured sample count and equals the virtual device's reference signal (use the generator's own reference API or a second run with the same seed); `StopCapture` ends within 1 s with end reason "stopped by user" and the samples so far kept; a scripted disconnect, driver error and unresponsive device mid-capture keep everything captured and report the matching end reasons and one notice each; the memory limit stops the capture with its end reason and the data kept; a second `StartCapture` while running is ignored; `NewCapture` on a clean state clears the capture
- [X] T027 [P] [US1] Write `crates/host/tests/status_text.rs`: for each end reason and activity, the status text key and arguments (`end-reason-*`, `status-*`), simulated/incomplete flags, memory used/limit and sample count with plural forms in English and Ukrainian (hand-written expectations: "1 sample", "21 samples", "1 семпл"-style forms exactly as the shared `count-samples` message produces)

### Implementation

- [X] T028 [US1] Implement `crates/host/src/devices.rs`: `DeviceEntry`, `ConfigDraft`, `ConfigEdit`, `ConfigProblem`, defaults from the device capabilities, re-validation after every edit; wire `RefreshDevices`, `SelectDevice`, `EditConfig` into `Shell::apply`, and registry hot-plug events into `poll` (selection falls back to the virtual device when the selected one disappears, with an info notice)
- [X] T029 [US1] Implement `crates/host/src/capture.rs`: start (guard hook comes in US4 — for now `StartCapture`/`NewCapture` are allowed when no capture is unsaved), stop, `poll` of acquisition progress and events, `CaptureView` creation from the live `Capture` snapshot at 20 Hz and at the end, `EndReason` mapping from `AcquisitionOutcome`, incomplete flag from gaps and abnormal ends, `wants_ticks()` (`Fast` while running, `Slow` otherwise); T026 passes
- [X] T030 [US1] Implement the status text helpers in `crates/host/src/state.rs` (or `status.rs`): `Shell::status_line(&self) -> StatusLine` returning keys and arguments for activity, samples, memory, end reason, flags; T027 passes
- [X] T031 [P] [US1] Add to `resources/i18n/en/ui.ftl` the messages for the toolbar, device and config fields, Start/Stop, status line, end reasons (`end-reason-completed`, `-stopped`, `-memory-limit`, `-disconnected`, `-device-error`, `-samples-lost`, `-loaded`), `simulated` label, activity names; then the five translated drafts in `de`, `fr`, `es`, `ja`, `uk`; counted text reuses `count-*` messages; `cargo test -p la-i18n` (parity) passes
- [X] T032 [US1] Implement `crates/host/src/shortcuts.rs`: `ShortcutEntry` table (layout contract) with the rows for the commands implemented so far (Start, Stop, Quit, New) and `Shell::shortcut_table()`; test in `crates/host/tests/shortcuts.rs` that every row's command exists, accelerators are unique, and menu path keys exist in `ui.ftl` of all languages (later phases append rows and the test keeps guarding)
- [X] T033 [US1] Implement `crates/app/src/app.rs`: the iced application (`State` wrapping `la_host::Shell`, `Message`, `update` that maps messages to `Command`s and calls `poll` on the tick subscription whose period follows `wants_ticks()`, `view` composing the regions of the layout contract), `crates/app/src/theme.rs` (original palette, light and dark following the system theme, 16 channel colours distinguishable in both), and `crates/app/src/views/` for the menu bar (driven by the shortcut table), toolbar (device picker, rate, length, trigger, seed, Start/Stop), side panel, status bar and the simulated badge; pure mapping functions (`message → Command`, keyboard event → `Command` through the table) unit-tested in `crates/app/src/app.rs` tests
- [X] T034 [US1] Implement `crates/app/src/signal_view.rs`: a `canvas::Program` that asks `Shell::frame_plan(ViewSize)` (only when the version key `(capture, window, size, presentation, theme, scale)` changed, via `canvas::Cache`), draws levels, edges, dense columns (hatched band plus outline), gaps, beyond-end, the time axis with labels from `Shell::text`, and a hover time readout in a cheap overlay layer; no input handling beyond reporting the size through `SetViewSize` yet
- [X] T035 [US1] Run the application with the virtual device (`cargo run -p la-app --release`): capture 10 million samples with a seed, watch the signals grow, stop, check the simulated badge and status line; fix defects found; record what was seen in `specs/006-ui-shell-signal-view/verification.md` (create it with the header and the US1 section)
- [X] T036 [US1] `cargo fmt`, clippy `-D warnings`, `cargo test --workspace`; commit the phase

**Checkpoint**: a user can capture from the virtual device and see the signals; the MVP

---

## Phase 4: User Story 2 - Explore a large capture by zooming and panning (Priority: P1)

**Goal**: zoom, pan, fit, follow, channel names/colours/order/visibility, dense and gap drawing, and
the performance bound on large captures

**Independent Test**: generate 100 M samples on 16 channels with a planted one-sample glitch, zoom
and pan by script and by hand, measure frame times, and confirm the glitch is visible at every zoom
level (spec US2, SC-002, SC-003)

### Tests first

- [X] T037 [P] [US2] Write `crates/host/tests/view_commands.rs`: `Zoom`, `ZoomStep`, `Pan`, `PanStep`, `FitAll`, `GoToStart`, `GoToEnd`, `ZoomToRange`, `SetFollowLatest` through `Shell::apply` change only `WINDOW` (and `version_parts.window`), keep the invariants, and with `follow_latest` on a growing capture keeps the window end at the capture end but never moves a window the user changed (follow turns off on manual pan); `SetViewSize` re-clamps the window
- [X] T038 [P] [US2] Write `crates/host/tests/glitch_sweep.rs` (SC-003): build a capture with a long quiet stretch and a single-sample glitch, then for 60 zoom levels from the whole capture to 64 pixels per sample, at several offsets, build the plan and assert the glitch shows as a transition (an `Edge` or a `Level` change) or as a `Dense` column in the column containing it; assert dense columns are visibly distinct data (`Dense` vs `Edge` vs `Level`) and that zooming into a dense column reveals every one of its transitions
- [X] T039 [P] [US2] Write `crates/host/tests/presentation.rs`: defaults (empty names, palette cycled, all visible, index order), rename (sanitized, 64-character limit, empty shows the default label key), colour (palette index and custom RGB), visibility, `MoveChannel` (permutation stays valid, out-of-range ignored), `ResetPresentation`, hostile names (RTL, controls, emoji, 10,000 characters), edits mark `Unsaved::Dirty` and bump only `version_parts.presentation`, only the capture's channels are listed
- [X] T040 [P] [US2] Write the benchmark `crates/host/benches/frame_plan.rs` (criterion, as the other crates' benches do): 16 channels × captures of 1 M, 10 M and 100 M samples (virtual device sparse and dense profiles), plan for whole-capture, 1 %-window, one-sample-per-4-pixels windows at W = 2000 and 4000; report time and probes; `benches/baseline.json` layout copied from `crates/session/benches/baseline.json`; the check "time for the 10× longer capture is under 2× the shorter" is a test in `crates/host/tests/frame_scaling.rs` using probe counts (`stats.probes`) so it is machine-independent

### Implementation

- [X] T041 [US2] Implement `crates/host/src/presentation.rs` (T039 passes) and wire the channel commands into `Shell::apply`; the frame plan uses presentation order, names, colours and visibility (extend T021's read-only view)
- [X] T042 [US2] Implement the view commands in `Shell::apply` (T037 passes), follow-latest update in `poll` for a running capture; add the rows for zoom, pan, fit, start, end, follow to `shortcuts.rs`
- [X] T043 [US2] Fix what T038 and T040 expose in `plan.rs` and `window.rs` (if the tests pass at once, run the mutation check from the Tests note: break the dense detection and the gap handling, see failures, restore); run the benchmark and record the baseline numbers in `crates/host/benches/baseline.json` with the machine and date
- [X] T044 [P] [US2] Add to `resources/i18n/en/ui.ftl` the view, channel and zoom messages (menu entries, tooltips, "Channel N" default label, follow latest, fit all, hover readout labels) and the five translated drafts; parity passes
- [X] T045 [US2] Extend `crates/app/src/signal_view.rs` with input: wheel zoom around the pointer, Ctrl+wheel and `+`/`-`, Shift+wheel and arrows to pan, drag to pan, drag on the axis to select a range (`ZoomToRange`), Home/End, Ctrl+0; coalesce rapid input into one command per frame; the channel label column (name with ellipsis and full text on hover, colour swatch, visibility toggle, drag or buttons to reorder, rename in place with the text input and IME), vertical scrolling of lanes for many channels with the axis fixed; add `crates/app/src/frame_log.rs` writing frame times when `LA_FRAME_STATS=1` (count, p50, p95, max, to the log at `info` every 5 s)
- [ ] T046 [US2] Manual check on a 100 M-sample, 16-channel virtual capture: continuous zoom and pan, the planted glitch visible at every level, frame statistics from `LA_FRAME_STATS`; if p95 > 16 ms or worst > 50 ms apply the fallbacks of research R4 in order (coarser tessellation, then the shader widget, then the latest-wins plan worker), re-measure, and record numbers and decisions in `verification.md`
- [X] T047 [US2] `cargo fmt`, clippy, `cargo test --workspace`, `cargo bench -p la-host` smoke run; commit the phase

**Checkpoint**: large captures can be explored smoothly; channel cosmetics work

---

## Phase 5: User Story 6 - Failures are explained and never stop the application (Priority: P1)

**Goal**: every failure becomes one localized message with a log entry, the data on screen stays
intact, and a failing background task or drawing step is contained

**Independent Test**: inject every stable error code and a worker panic while a capture is on
screen; each gives one localized message, no crash, no data loss, one log entry (spec US6, SC-006)

### Tests first

- [X] T048 [P] [US6] Write `crates/host/tests/error_sweep.rs` (SC-006): enumerate every stable error code from `la-storage`, `la-devices`, `la-session` and `la-host` (the same enumeration `la-i18n`'s catalogue test uses); for each, raise it through the real code path where one exists (scripted device failures, a damaged and a truncated file opened, an invalid configuration, a full-disk save through `FaultFs`, an invalid settings file) or through a test-support injector otherwise; assert exactly one notice, its text in all six languages is non-empty, is not the key or the code, does not contain the technical detail, and that a log record with the code exists (capture `tracing` output with a test subscriber); the capture on screen is unchanged
- [X] T049 [P] [US6] Write `crates/host/tests/containment.rs`: a scripted driver panic and an unresponsive device mid-capture keep the data and produce `device.driver_panic` / `device.unresponsive` notices; a panic injected into a save worker leaves the capture and `Unsaved` state intact and gives `session.worker_failed`; `frame_plan` failure gives the cannot-draw state while `SaveSession` still works; ten identical failures in a row give one grouped notice with count 10 and the list never exceeds 50 entries; an unknown code (injected) shows the generic localized message and logs a "missing message" defect
- [X] T050 [P] [US6] Write `crates/host/tests/logging.rs`: the log contains command names, lifecycle events and notices with codes and technical detail, **never** signal data or user text (use a capture whose channel names and description contain a recognisable marker string and assert the marker is absent from the log output), and `log_location()` returns the configured path

### Implementation

- [X] T051 [US6] Route every failure path of the host (device events, acquisition outcomes, session operations, settings, view-section problems later) through one `report(error)` function that adds the notice, logs the code and detail at the right level and returns `Outcome::Failed`; unknown codes use the generic message and log a defect; make T048–T049 pass
- [X] T052 [US6] Implement command and lifecycle logging (`tracing` spans or events per command with non-sensitive arguments only) and `log_location()` (T050 passes)
- [X] T053 [P] [US6] Add to `ui.ftl` (English, then five drafts) the messages for the notice banner (dismiss, dismiss all, "show log location", "N more", count of repeats), the cannot-draw state text and the recent-messages list; parity passes
- [X] T054 [US6] Implement the notice banner and the recent-messages panel in `crates/app/src/views/notices.rs` (severity, localized text from `Shell::notice_text`, repeat count, dismiss, "Show log location" opening the folder through the platform opener or showing the path), the cannot-draw overlay in `signal_view.rs` when `FramePlan.state` is `CannotDraw`, and the Help menu entry for the log location (rows added to `shortcuts.rs`)
- [ ] T055 [US6] Manual check: trigger each failure class in the running application (unplug via scripted virtual failure from a hidden test option `LA_TEST_FAILURE=<kind>` that is compiled only with the `test-support` feature, corrupt file, read-only folder) and record in `verification.md`; `cargo fmt`, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: failures are explained in all six languages and contained

---

## Phase 6: User Story 3 - Measure time with markers (Priority: P2)

**Goal**: place, drag, snap, rename, remove and jump to markers; read distance and frequency

**Independent Test**: place two markers on a capture whose period is known by construction and check
the time, distance and frequency in several languages (spec US3)

### Tests first

- [X] T056 [P] [US3] Write `crates/host/tests/markers.rs`: placement clamps to the capture; ids unique and never reused; at most 64 markers (the 65th is refused with `host.invalid_command` and an info notice); snapping on moves a marker to the exact position of the nearest transition on the given channel (hand-written examples, ties go to the earlier one, no transition → stays), snapping off keeps the position; rename sanitizes; remove one / all; `GoToMarker` reveals the marker without changing the zoom unless it cannot fit; markers stay attached to their sample while zooming and panning (plan x positions recomputed); the two selected markers default to the two most recently placed
- [X] T057 [P] [US3] Write `crates/host/tests/measurement.rs`: distance in samples and seconds exact from `position/sample_rate`, frequency = rate / distance; for rates 1 kHz … 500 MHz and distances 1 sample … 10⁹ samples assert the formatted strings in English, German, French, Spanish, Japanese and Ukrainian against hand-written expectations (decimal separators, units, spacing), the digit count never exceeds what one sample period resolves, a zero distance gives "no frequency" text, and the same two markers swapped give the same distance

### Implementation

- [X] T058 [US3] Implement `crates/host/src/markers.rs` (marker model, snapping via `Snapshot::transitions` with a bounded search window, measurement and formatting through the localizer) and wire the marker commands into `Shell::apply` and the plan's marker layer (T056–T057 pass); add the marker rows to `shortcuts.rs`
- [X] T059 [P] [US3] Add to `ui.ftl` (English, then five drafts) the marker menu entries, side-panel section labels, measurement labels ("Distance", "Frequency", "no frequency"), default marker name pattern; parity passes
- [X] T060 [US3] Implement the marker interaction in `crates/app/src/signal_view.rs` (Alt+click or `M` places at the pointer or the view centre, drag to move, hover highlight, delete key), the marker overlay layer (does not invalidate the waveform cache), the Markers section of the side panel (list with name editing, selection of the two for measurement, go to marker) and the measurement readout
- [ ] T061 [US3] Manual check with a known-period virtual signal in two languages; record in `verification.md`; `cargo fmt`, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: measurements work and are localized

---

## Phase 7: User Story 4 - Save, open and never lose a capture (Priority: P2)

**Goal**: save and open sessions with presentation and markers, the unsaved-data guard, and refusal
of damaged files

**Independent Test**: capture, save, close, reopen and compare everything; corrupt the file and open
it; close with unsaved data and check the prompt (spec US4, SC-007, SC-011)

### Tests first

- [ ] T062 [P] [US4] Write `crates/host/tests/view_section.rs` from `contracts/view-section.md`: a hand-written byte vector for a default-ish, a typical (3 channels, 2 markers, window) and a maximal (32 channels, 64 markers, 256-byte names) payload decoded by the reader (independent of the writer); round trips of the three states; each validation rule in the contract has a failing fixture (bad version, non-canonical varint, field out of range, duplicate index, duplicate id, reserved flag bit, invalid UTF-8, marker beyond capture, window beyond capture, trailing bytes) producing `host.view_invalid`; a newer section version is preserved and yields the info notice, not the warning; the writer is deterministic (same state → same bytes, run twice, shuffle internal insertion order); a mutation fuzzer (every single-byte change, every truncation and extension of the three payloads) never panics and any accepted payload re-encodes to identical bytes
- [ ] T063 [P] [US4] Write in `crates/session/tests/` a test (new file `view_section_carried.rs`) that a session with an extra optional section tagged `VIEW` round-trips unchanged through save and load and that the writer now emits format minor `1` (update the expectation in `crates/session/src/format.rs` tests and the independent reader/writer test of feature 003 for the minor); existing files with minor 0 still load
- [ ] T064 [P] [US4] Write `crates/host/tests/session_scenarios.rs`: capture with the virtual device, rename channels, place markers, `SaveSessionAs` to a temp dir, `OpenSession` in a fresh `Shell` and compare capture (every transition and gap), names, colours, order, visibility, markers, window, description, simulated flag; `Save` after `OpenSession` writes to the same path atomically; progress and `CancelOperation` for both save and load (a cancelled save leaves the old file byte-identical); damaged, truncated, empty, foreign, too-large, newer-major and unknown-critical-section files each give their stable code and message and leave the open capture unchanged; an invalid `VIEW` section opens with defaults plus `host.view_invalid` and is dropped on the next save; recent files list (add on save/open, max 10, missing files skipped when shown)
- [ ] T065 [P] [US4] Write `crates/host/tests/save_interruptions.rs` (SC-007): save a capture through `FaultFs` failing at every step index for at least 100 fault points (write, flush, fsync, rename, directory sync) and assert after each that the previous file is intact and readable, no temp file remains after the next save, the shell state is `Dirty`, one `session.io_write` notice appears and the capture is unchanged; a kill-style test is not repeated here (feature 003 has it)
- [ ] T066 [P] [US4] Write `crates/host/tests/unsaved_guard.rs` (SC-011): table over (unsaved state ∈ Clean, Dirty, Running) × (command ∈ NewCapture, StartCapture with a capture, OpenSession, Quit, RequestClose): Clean runs at once; Dirty/Running returns `NeedsDecision` and changes nothing; Running capture is stopped with its data kept before the decision is returned; `Decide(Cancel)` drops the command and keeps the capture; `Decide(Discard)` runs it; `Decide(Save)` saves (to the known path or asks for `SaveSessionAs` via `Outcome::Ignored` carrying the "needs path" reason) and runs the command only after success, a failed save keeps the command pending and the capture; no sequence of commands reaches a state where unsaved data is discarded without `Decide(Discard)` (a seeded random walk of 20,000 commands asserts it)

### Implementation

- [ ] T067 [US4] Change the writer in `crates/session/src/format.rs` to emit minor `1` and document `VIEW` in `specs/003-session-file-format/contracts/file-format.md` (evolution rule 4 now points to `specs/006-ui-shell-signal-view/contracts/view-section.md`); T063 passes; run the session crate's whole test suite and benchmarks smoke
- [ ] T068 [US4] Implement `crates/host/src/view_section.rs` (reader, writer, validation, constants from the contract) and make T062 pass
- [ ] T069 [US4] Implement `crates/host/src/sessions.rs`: `SaveSession`, `SaveSessionAs`, `OpenSession`, `CancelOperation`, progress in `poll`, building `SessionData` from the capture description plus the `VIEW` section (only when non-default), applying a loaded `Session` (capture, presentation, markers, window, `Unsaved::Clean`), the recent-files list in settings, the memory limit from settings passed to the load limits; T064–T065 pass
- [ ] T070 [US4] Implement `crates/host/src/guard.rs` and route the guarded commands through it (T066 passes); update `capture.rs` so `StartCapture`/`NewCapture` use the guard now
- [ ] T071 [P] [US4] Add to `ui.ftl` (English, then five drafts) the File menu entries, the unsaved-changes dialog texts, save/open progress and cancel, "unsaved" status, recent files label, file dialog titles and the `.lasession` filter name; parity passes
- [ ] T072 [US4] Implement `crates/app/src/dialogs.rs` (native open/save dialogs through `rfd` as iced tasks, extension filter, default file name from the capture) and `crates/app/src/views/dialogs.rs` (the unsaved-changes dialog driven by `Outcome::NeedsDecision`, progress bar with Cancel for save/load, window title with an unsaved marker, recent-files submenu, intercept the window close request with `exit_on_close_request(false)` and send `RequestClose`; the window closes only when the host reports `QUIT`); rows for open, save, save as, new, quit added to `shortcuts.rs`
- [ ] T073 [US4] Manual check: save, reopen, corrupt a copy, close mid-capture and answer each way; record in `verification.md`; `cargo fmt`, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: nothing the user captured is lost without an explicit choice

---

## Phase 8: User Story 5 - Use the application in my own language (Priority: P2)

**Goal**: runtime language choice, correct Japanese and Cyrillic rendering, long German text that
fits, locale-correct numbers, settings dialog

**Independent Test**: switch through all six languages with a capture open and check every text; run
the layout check at the minimum window size with the longest German and Japanese strings (spec US5,
SC-008, SC-009)

### Tests first

- [ ] T074 [P] [US5] Write `crates/host/tests/language.rs`: first start with system languages `["uk-UA"]`, `["pt-BR", "de"]`, `["zz"]`, `[]` selects `uk`, `de`, `en`, `en`; an explicit `SetLanguage` wins on the next `Shell::new` (settings round trip); the change takes effect at once for `Shell::text`, notices and measurement strings, flags `LANGUAGE`, and does not alter channel names, marker names or the description; switching ten times while a capture runs and saves is safe; missing translation falls back to English and logs the gap, never shows a key
- [ ] T075 [P] [US5] Write `crates/host/tests/settings_commands.rs`: `SetTheme`, `SetUiScale` (clamped), `SetMemoryLimit` (validated against the storage limits, applied to the next capture, never to a running one), persistence written in the background and `host.settings_write` on failure; defaults after a damaged settings file
- [ ] T076 [P] [US5] Write `crates/host/tests/shortcut_coverage.rs` (FR-036): every `Command` variant that a user can trigger appears in the shortcut table or in an explicit list of commands that are reachable only by pointer *and* have a menu equivalent; accelerators are unique; no accelerator uses a modifier combination reserved by Windows or Debian desktops (fixed deny list in the test); every menu path key exists in all six languages
- [ ] T077 [US5] Write the text-fit check chosen in T007 in `crates/app/tests/text_fit.rs` (option a: render every screen and dialog at 800 × 560 for each language, compare measured text bounds with container bounds, fail on clipping, overlap or missing glyph) **or** `crates/host/tests/string_budget.rs` (option b: every `ui.ftl` message of class `button`, `menu`, `label`, `title` has an English-relative length budget of 1.6× and an absolute maximum per class, with the messages' classes taken from a comment tag in the resource file; the longest German and Japanese strings are listed in the failure message); it must fail first on a deliberately over-long fixture string

### Implementation

- [ ] T078 [US5] Implement `SetLanguage`, `SetTheme`, `SetUiScale`, `SetMemoryLimit` in `Shell::apply` and the background settings writer (T074–T075 pass); rows for settings added to `shortcuts.rs` and T076 passes (it fails until every earlier phase's commands are in the table; fix the table, not the test)
- [ ] T079 [P] [US5] Build the font subsets: add `tools/build-font-subsets.py` (documented, pins the source font versions and checksums, uses `pyftsubset`) producing `crates/app/assets/fonts/` files: a Latin/Latin-Ext/Greek/Cyrillic UI font subset and a Japanese subset limited to the characters of `resources/i18n/ja/*.ftl`, kana and JIS level 1 kanji; commit the subsets with their SIL OFL licence texts and add `docs/third-party-notices.md` (fonts, iced and its main dependencies with licences); record sizes in `research.md` R5 (target: both subsets together under 1.5 MB)
- [ ] T080 [US5] Implement `crates/app/src/fonts.rs` (embed with `include_bytes!`, register with iced, default family order: UI font, Japanese subset, then system fallback) and use it in `main.rs`; manual check on a Debian container without CJK system fonts that the Japanese UI text renders (record in `verification.md`)
- [ ] T081 [P] [US5] Add to `ui.ftl` (English, then five drafts) the settings dialog (language names in their own language, theme, interface scale, memory limit with its unit text), About text and the layout strings still missing; parity passes
- [ ] T082 [US5] Implement the settings dialog and About dialog in `crates/app/src/views/settings.rs` (language list applies on selection and the dialog itself switches, theme, scale with Ctrl+=/Ctrl+- shortcuts, memory limit input with validation messages), the theme following the system (including `High contrast` palette and the rule that signal states never rely on colour alone), interface scale applied to all sizes, minimum window size 800 × 560, restore of the saved window geometry when it is on a visible monitor
- [ ] T083 [US5] Apply the layout rules of the layout contract across all views: measured containers, wrapping labels, the toolbar wrapping into two rows, ellipsis with hover/focus text for channel and marker names, bidirectional isolation of user text in labels (`la-i18n` argument isolation for text inserted into messages; for raw user text in widgets, the sanitized text only); T077 passes in all six languages at the minimum size
- [ ] T084 [US5] Manual check: switch through all six languages with a capture open; Japanese IME entry of a channel name; high-contrast theme; 150 % and 200 % scale; record in `verification.md`; `cargo fmt`, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: the application is usable in all six languages

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: the long-run stability test, documentation, the contract document, final measurements

- [ ] T085 Write `crates/host/tests/random_walk.rs` (SC-005): a seeded random walk of 100,000 actions (default; 2,000,000 in an `#[ignore]` variant run in the nightly/fuzz workflow) over every command class — start, stop, zoom, pan, markers, rename, save, open (including damaged files), language change, resize, invalid edits, scripted device failures and panics — against a real `Shell` with the virtual device and a temp directory; after every action assert the state invariants (window, markers, presentation, at most one activity, `Unsaved` consistency, notices bounded), no panic, no thread leak at the end, and that the data of every capture that was ever shown is either still shown, saved to disk, or explicitly discarded by a `Decide(Discard)`; failures print the seed and the action index
- [ ] T086 Add the long variants to `.github/workflows/fuzz.yml` (the `#[ignore]` random walk and the `VIEW` mutation fuzzer at 1,000,000 iterations, same schedule as the existing jobs) and add the rows for the host API document check and the frame-plan benchmark to `specs/001-workspace-ci-foundation/contracts/ci-checks.md`; add the frame-plan benchmark to `.github/workflows/bench.yml` in the existing style
- [ ] T087 Write `docs/host-api.md` from `specs/006-ui-shell-signal-view/contracts/host-api.md` (the final, authoritative text, any changes made during implementation included, version line `Version: 1.0`) and `crates/host/tests/host_api_doc.rs` that fails if the document's version line differs from `HOST_API_VERSION`, if a command named in the document's table does not exist in `Command`, or if a `Command` variant is missing from the table; point the design-source contract at the new location
- [ ] T088 [P] Write `docs/user-guide.md` (English): starting, devices, capturing, zoom and pan, channels, markers, saving, language and settings, where the log is, Debian fonts note (`fonts-noto-cjk`) and device-access placeholders for the drivers to come; link it from `README.md` with a screenshot-free description; update `README.md` status table and `roadmap.md` (006 → "(implemented)"; and 005 → "(merged)")
- [ ] T089 [P] Update `CONTRIBUTING.md`: how to add a UI string (key area prefixes, `ui.ftl` in all languages), how to add a command (the enum, the shortcut table row, the API document row, a test), the Debian packages from T006, and the rule that `la-app` contains no domain logic
- [ ] T090 Run the long variants once locally (`cargo test -p la-host --release -- --ignored`) and the benchmarks (`cargo bench -p la-host`), record results, machine and date in `crates/host/benches/baseline.json` and `verification.md`
- [ ] T091 Measure responsiveness and idle CPU on Windows and on Debian with a 100 M-sample capture: frame statistics (`LA_FRAME_STATS=1`) during capture, zoom, pan, save, open and a language change (SC-002, SC-004, SC-009), and one minute idle (SC-010); record in `verification.md`; if the Debian measurement cannot be done from this machine, ask the user for the result and record it as pending rather than claiming it
- [ ] T092 Complete `specs/006-ui-shell-signal-view/checklists/verification.md`-style traceability in `verification.md`: each FR-001…FR-037 and SC-001…SC-011 mapped to the test or manual check that proves it, each manual item with platform, date and result; any requirement not met is stated as such with the reason
- [ ] T093 Final gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, the msrv commands with the pinned MSRV toolchain excluding `la-app`, the parity check, `cargo doc` warnings; push, open the PR, wait for all required CI checks on both OSes; list in the PR description the follow-ups that need the user (constitution TODO wording, native-speaker review of the six languages' `ui.ftl`, accessibility trigger)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none; T006 and T007 are spikes whose decisions feed T077 and CI
- **Foundational (Phase 2)**: depends on Setup — blocks all stories; tests T008–T014 are independent of each other, T015–T024 implement them (T021 needs T017 and T018; T023 needs T019–T022; T024 needs T015)
- **US1 (Phase 3)**: depends on Foundational; delivers the MVP
- **US2 (Phase 4)**: depends on US1 (the view and the capture); the frame-plan tests are independent of the UI
- **US6 (Phase 5)**: depends on US1 (real failure paths) and benefits from US2; its tests can be written earlier, and each later phase must route its failures through `report` (T051)
- **US3 (Phase 6)**: depends on US2 (window, plan layers)
- **US4 (Phase 7)**: depends on US1 and US2 (presentation) and US3 (markers are saved); T067 (session minor bump) is independent and can land first
- **US5 (Phase 8)**: depends on US1; the settings and language commands are independent of US2–US4, but the shortcut coverage test (T076) and the text-fit check (T077) need the screens of earlier phases to exist
- **Polish (Phase 9)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Host logic and resources (English first, then five translations) before the UI that needs them
- Run clippy with `-D warnings` and `cargo test --workspace` at the end of every phase and never
  commit a failing gate; commit after every phase (the foundational and per-story phases may be
  split into smaller logical commits)
- The parity check of feature 005 is the judge for resources: every phase that adds UI text runs
  `cargo test -p la-i18n`

## Parallel Execution Examples

- Setup: T003 + T004 (different files) while T001/T002 are written
- Foundational: T008 + T009 + T010 + T011 + T012 + T013 + T014 (tests), then T016 + T017 + T018 + T019 in parallel
- US1: T026 + T027 (tests), T031 (resources) alongside T028–T030
- US2: T037 + T038 + T039 + T040 (tests); T044 (resources) alongside T041–T043
- US4: T062 + T063 + T064 + T065 + T066 (tests); T067 can be done before everything else in the phase
- US5: T074 + T075 + T076, and T079 (fonts) alongside the host work

## Implementation Strategy

**MVP**: Phases 1–3 (Setup, Foundational, US1): the application opens, captures from the virtual
device and shows the signals. Then US2 (exploration) and US6 (failure handling), both P1 and both
required before the application is something a person should use on a real capture. Then US3
(markers), US4 (sessions and the unsaved guard — the data-safety promise, so do not postpone it
beyond US3), and US5 (language and layout polish). Polish closes with the long-run test, the
contract document and the measurements. Every phase keeps the parity check, clippy and the tests
green and ends in a commit; the manual checks are recorded as they are done, not at the end.
