# Verification record: UI Shell and Signal View

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Quickstart**: [quickstart.md](quickstart.md)

Automated checks are named by their test files. Items that need a window or a second platform are
recorded here with the platform, the date and the result. Anything not met is stated as such.

## US1: capture signals and watch them appear

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-14 | Windows 11 | `cargo run -p la-app` opens the window and keeps running (6 s) | passed (GPU renderer) |
| 2026-11-14 | Windows 11 | Headless render of the view with a finished capture (`app::tests::the_view_renders_a_capture_headlessly`, picture written with `LA_SNAPSHOT_DIR`): menu bar, toolbar with the localized labels, time axis with ticks and units, four lanes with names and colours, status line "Idle · 10 ms · 300,000 samples · 111 KiB / 1 GiB · Capture completed · 1 MHz · Simulated · Unsaved changes" | passed; at "fit all" every lane is fully dense (300 samples per column with transitions every ~50 samples), which is the dense-activity mark working as designed; edges appear after zooming (US2) |
| 2026-11-14 | Windows 11 | Interactive start/stop with the mouse | not yet done by a person; the host-level scenarios (`capture_scenarios.rs`) cover start, stop within one second, scripted failures and the memory limit |

## US2: explore a large capture

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-14 | Windows 11 | Frame-plan benchmark in release (`cargo bench -p la-host --bench frame_plan`, 16 channels, virtual device, results in `crates/host/benches/baseline.json`): the plan of the whole 100 M-sample capture takes 1.5 ms at 2000 px and 2.9 ms at 4000 px; the worst case measured (10 M normal profile, whole capture, 4000 px) is 3.3 ms; windows of 1 % and of 4 px per sample take 0.5 to 2.5 ms | passed (the plan stage is well inside the 16 ms frame budget of SC-002) |
| 2026-11-14 | Windows 11 | Probes of the plan for the whole capture from 1 M to 100 M samples (100 times longer) grow by a factor of 1.41 (the benchmark fails above 2.0); `frame_scaling.rs` checks 4 M against 40 M in `cargo test` | passed (Principle VIII: the cost does not depend on the capture length) |
| 2026-11-14 | Windows 11 | Headless render of a zoomed view (`app::tests::the_view_renders_a_zoomed_capture_with_single_edges`): levels and single edges at their exact positions, a renamed channel ("SDA") in its lane with its colour, the channel panel with show checkboxes, colour swatches and order buttons | passed |
| 2026-11-14 | Windows 11 | The draw stage (tessellation of the cached layer) of a 100 M-sample capture measured in the running window with `LA_FRAME_STATS=1`, and mouse wheel, drag and keyboard zoom and pan | **pending**: needs a person at the window; the logging is in place (`frame_log.rs`), the fallbacks of research R4 are not needed unless the numbers say so |

## US6: failures are explained and never stop the application

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-14 | Windows 11 | `error_sweep.rs`: every stable error code of the product (the scan found at least 51) injected one at a time gives exactly one notice, whose text exists in all six languages and contains neither the code, a key nor the technical detail; the log holds every code and the technical detail | passed |
| 2026-11-14 | Windows 11 | `containment.rs`: a scripted driver panic ends the capture with `device.driver_panic`, keeps the 2,000 samples captured before it, shows them and leaves the shell usable; ten identical failures group into one notice with count 10; a code without a message gives the generic text and records a defect | passed |
| 2026-11-14 | Windows 11 | `logging.rs`: the log has command names, codes and technical detail, and a recognisable channel name never appears in it | passed |
| 2026-11-14 | Windows 11 | Headless render with a damaged settings file (`a_damaged_settings_file_shows_a_localized_message_with_its_buttons`): the localized message, "Show log location" and "Dismiss" are on screen | passed |
| 2026-11-14 | Windows 11 | The failure classes in the running window (unplug, corrupt file, read-only folder) | **pending**: needs a person; the session-file failures arrive with US4; no `LA_TEST_FAILURE` switch was built because the scripted-device tests above exercise the same paths |

## US3: measure time between events

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-15 | Windows 11 | `markers.rs`: placement clamps to the capture, ids are unique and never reused, at most 64 markers, snapping to the nearest transition of a channel within about twelve pixels (from either side, none beyond), snap off keeps the exact sample, a marker keeps its sample's pixel position while zooming | passed |
| 2026-11-15 | Windows 11 | `measurement.rs`: exact distance and frequency, 1.5 ms and 666.7 Hz in all six languages with their units and separators, the frequency keeps only the digits the distance resolves | passed |
| 2026-11-15 | Windows 11 | Headless render with two markers 200 samples apart at 1 MHz: lines and labels in the view, panel with "Distance: 200 µs" and "Frequency: 5 kHz" | passed |
| 2026-11-15 | Windows 11 | Right-click to place, dragging a marker line, and the menu with a known-period signal in two languages | **pending** (T061): needs a person at the window |

A defect found by the render test and fixed: the plan refresh key lacked the marker version, so markers placed in the running window would not have been drawn.

## US4: save, reopen and never lose a capture

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-15 | Windows 11 | `view_section.rs`: the reader against byte vectors written out from the contract, round trips of the default, typical and maximal states (32 channels, 64 markers, 256-byte names), a failing fixture for every validation rule, a newer section version, and a mutation fuzzer (every single-byte change of the small payloads, sampled for the 20 KB one, every truncation and extension): nothing panics and every accepted payload re-encodes to the same bytes | passed |
| 2026-11-15 | Windows 11 | `session` crate: the writer emits format minor 1, a `VIEW` section round-trips unchanged, the 1.0 fixtures still load, a new `v1_1` fixture directory pins the writer, the independent reader and writer agree on minor 1 | passed |
| 2026-11-15 | Windows 11 | `session_scenarios.rs`: save then open in a fresh shell restores every transition and gap, description and simulated mark, names, colours, order, visibility, markers, selection and window; the default look writes no `VIEW` section; save after open rewrites the same path atomically; progress is reported and a cancelled save leaves the old file byte-identical with no temporary file; a cancelled load changes nothing; damaged, truncated, empty, foreign, unknown-critical and missing files give their stable code and leave the capture alone; an invalid `VIEW` section opens with defaults and a warning and is not written back; a newer one is kept untouched with an info message; unknown sections are written back; recent files (newest first, 10, missing skipped, remembered) | passed |
| 2026-11-15 | Windows 11 | `save_interruptions.rs` (SC-007): 19 file-system calls of a save, each failed with 6 error kinds (114 fault points, some with a half-written buffer): the previous file is exactly as before or the complete new one, it always loads, one `session.io_write` notice, the capture unchanged and unsaved, no temporary file after the next save | passed |
| 2026-11-15 | Windows 11 | `unsaved_guard.rs` (SC-011): Clean/Dirty/Running by new/open/quit/close, plus start over an unsaved capture; Cancel, Discard, Save with and without a known path, a failed save keeps the command pending, cancelling the question during a save; a random walk of 20,000 commands never replaces unsaved data without Discard or a successful save. A mutation that removes the guard from new-capture is caught by 7 tests including the walk | passed |
| 2026-11-15 | Windows 11 | App tests: the title shows the file name and a `*` for unsaved changes; closing with unsaved data shows the question (picture checked, backdrop dimmed); the progress dialog with Cancel; the File menu enables Open/Save/Save as by state; the window starts and runs for 6 s | passed |
| 2026-11-15 | Windows 11 | The native file choosers, closing a running capture and answering each way, opening a corrupted copy of a real file | **pending** (T073): needs a person at the window |

Deviations from the task text, on purpose: starting a capture while one is running is ignored (the interface offers no Start then) instead of asking, because it cannot lose data without the user also stopping the running one; `guard.rs` was not made a separate file, the guard lives in `shell.rs` and `sessions.rs`; the "too large" file case is covered by the session crate's own tests.

A defect found and fixed on the way: the dimmed backdrop behind dialogs was opaque, hiding the whole window.

## US5: use the application in my own language

| Date | Platform | Check | Result |
|------|----------|-------|--------|
| 2026-11-15 | Windows 11 | `language.rs`: system languages `uk-UA`, `pt-BR`+`de`, `zz`, none choose `uk`, `de`, `en`, `en`; an explicit `SetLanguage` wins on the next start; the change is immediate for shell text, notices and measurements and flags `LANGUAGE`; six languages give six different menu words and notices; channel and marker names and the description never change; ten switches while a save runs are safe; no key or placeholder is ever shown | passed |
| 2026-11-15 | Windows 11 | `settings_commands.rs`: theme (with the new high-contrast choice) and scale (clamped to 75–200 %) are applied and remembered; the memory limit is validated, applies to the next capture and never to a running one; the window geometry is kept and written when the application saves; a settings file that cannot be written gives `host.settings_write` while the choice stays in effect; a damaged file gives the defaults | passed |
| 2026-11-15 | Windows 11 | `shortcut_coverage.rs` (FR-036): the `Command` variants are read from the source, so a new one must be in the shortcut table or in a justified list (settings dialog, focusable controls, dialog answers, pointer gestures with an equivalent, not user triggered); accelerators are unique and avoid what Windows and the Debian desktops keep | passed |
| 2026-11-15 | Windows 11 | `string_budget.rs` (option b of T077): menu, button, label and title texts of all six languages within 1.6 × English or English + 16 characters, and within an absolute maximum per class (a wide character counts two); a German button and a Japanese menu entry that are far too long are caught | passed |
| 2026-11-15 | Windows 11 | Every screen (main window with a capture, a marker and a failure message; settings; about) rendered headless in all six languages at the minimum size 800 × 560: all labels found, pictures checked for German and Japanese (the toolbar wraps to two rows, long messages and the status line wrap, nothing clipped) | passed |
| 2026-11-15 | Windows 11 | Japanese subset of Noto Sans JP built by `tools/build-font-subsets.py` (pinned and checksummed): 970,976 bytes for 3,415 characters; a test checks that every Japanese-block character of the resources is in the font; the font is registered with the toolkit | passed |
| 2026-11-15 | Windows 11 | Settings dialog: language, theme, scale and memory limit apply at once and the dialog switches language itself; Ctrl+`=`/Ctrl+`-` step the scale; Ctrl+`,` and F1 open settings and about; the window size is restored and the place when it is plausibly on a screen | passed in tests; the window itself pending |
| 2026-11-15 | Windows 11 | That the Japanese interface renders on a machine with no CJK font (a Debian container), Japanese IME entry of a channel name, the high-contrast theme, 150 % and 200 % scale, and restoring the window on a second monitor | **pending** (T080, T084): needs a person and a clean Debian machine |

Notes: the settings are written at once and not in the background (the file is a few hundred bytes), which keeps a failure visible immediately; ellipsis with a hover text was not needed because channel and marker names live in editable fields. No separate UI-font subset was built: iced bundles Fira Sans, which already covers Latin, Greek and Cyrillic. `cargo deny` is checked by the CI job only (not installed locally); `rfd` and `read-fonts` are new dependencies.


## Phase 9: stability, measurements and traceability

### Long runs and measurements (T085, T090, T091)

Machine: Windows 11, Intel Core i9-14900KF (32 threads), release profile, 2026-11-15. Numbers
are also in `crates/host/benches/baseline.json`.

| Check | Result |
|-------|--------|
| `random_walk` (SC-005), 100,000 actions in the normal tests, seeds 2026, 1, 2 and 3 | passed (about 29 s each in debug) |
| `random_walk`, the ignored variant, 2,000,000 actions in release | passed in 510 s |
| `view_section` random mutation fuzzer, 1,000,000 mutations in release | passed in 20 s |
| Frame plan benchmark (`cargo bench -p la-host --bench frame_plan`) | plan for the whole 100 M-sample, 16-channel capture: 1.5 ms at 2000 px, 2.9 ms at 4000 px; probes grow x1.41 for a 100 times longer capture (limit 2.0); unchanged from the baseline |
| Responsiveness test on 100 M samples x 16 channels (`measure_responsiveness_on_a_hundred_million_samples`, ignored) | the capture takes 0.7 s; the interface thread is busy at most 5.0 ms per call during the capture; zoom and pan take 4.2 ms at the median and 4.6 ms at the 95th percentile (update and frame plan), the widget tree 0.1 ms; the longest single call during save and open is 21 ms |
| Language change on that capture (SC-009) | 4.9 ms of work plus 20 ms of drawing, about 25 ms in total, against 200 ms |
| Idle CPU (SC-010), the real window with nothing running, 60 s, three runs | 0.81 %, 0.60 % and 0.26 % of one core, against 1 % |

What these numbers do not show: the drawing figure (21 ms) is a render to a picture with a 2x
readback, so it is an upper bound for a frame on screen and not a measurement of the graphics card;
the 16 ms frame budget of SC-002 is met by the plan and widget stages (under 5 ms) but **a measured
frame on screen at the 95th percentile during continuous zoom and pan has not been recorded**
(`LA_FRAME_STATS=1` in the running window is the way, and needs a person). **No Debian
measurement was made** (no Debian machine here): SC-002, SC-004, SC-009 and SC-010 are pending
there. The idle CPU was 1.6 % to 4 % before the fix below, and 0.3 % to 0.8 % after it.

Found and fixed by this phase's work:

- The random walk found three inconsistencies: the window was computed from an unclamped pixel
  width, a window made for an empty capture outlived it when the capture stayed short, and a
  command that waited for a save and then failed cleared the capture on screen (the capture now
  stays and only the message is new). A fourth, found by the CI before: discarding a running
  capture left the shell blocked.
- The idle redraw: subscribing to all window events turned every redraw request into a message,
  which made the toolkit redraw again and again (1.6 % to 5 % idle CPU). Only moves and size
  changes are listened to now.
- Every window move and resize was written to the log as an info-level command; high-frequency
  commands are logged at debug level now.

About the report that the window "starts and for a long time nothing happens": the host's own
startup takes 2 ms, the window handle exists 0.3 s after launch and the interface is fully drawn
1.5 s after launch on this machine, in the debug and the release build alike. It could not be
reproduced here; see the open question in the final report.

### Traceability (T092)

Automated checks are in `crates/host/tests`, `crates/session/tests` and the tests of `la-app`
(`app::tests`, `fonts::tests`) unless named otherwise. "Manual pending" means it needs a person at
the window and is listed in the sections above.

| Requirement | Proved by | Status |
|-------------|-----------|--------|
| FR-001 window, menu, toolbar, view, status, messages on both systems | `app::tests` renders; CI test jobs on both systems; the window started and drew on Windows | Windows met; Debian window **pending** |
| FR-002 devices through the abstraction, virtual always, hot plug | `capture_scenarios`, `containment` (a plugged scripted device) | met |
| FR-003 configuration within the capabilities, invalid refused with a reason | `capture_scenarios` | met |
| FR-004 start, progress, stop within one second, background | `capture_scenarios`, the 100 M measurement | met |
| FR-005 end reasons, data always kept | `capture_scenarios`, `containment` | met |
| FR-006 simulated mark everywhere, not removable | `session_scenarios`, status line render, `la-session` compile-fail test | met |
| FR-007 waveforms, names, axis with units, time under the pointer | `plan`, `axis`, `app::tests` renders; the hover readout is drawn by `signal_view` | met in tests; the pointer readout **manual pending** |
| FR-008 zoom, pan, jumps, zoom to a range, all from the keyboard | `view_commands`, `shortcut_coverage` | **partly met**: no scroll bar and no gesture to select a range with the mouse (the `ZoomToRange` command exists and is tested) |
| FR-009 cost independent of the capture length | `plan`, `frame_scaling`, the benchmark (x1.41 for 100 times longer) | met |
| FR-010 dense columns marked, glitches never lost | `glitch_sweep` (with a mutation check) | met |
| FR-011 gaps drawn as gaps, incomplete captures marked | `plan`, `capture_scenarios` | met |
| FR-012 live follow without jumping or slowing | `view_commands` (follow test), the 100 M capture (0.7 s, no samples lost) | met |
| FR-013 rename, reorder, hide, colours, legible palette | `presentation`; three themes in `theme.rs` | met for the editing; legibility on light and high contrast **manual pending** |
| FR-014 zoom bounded at both ends | `window`, `view_commands`, `random_walk` invariants | met |
| FR-015 redraw only on change, idle CPU | idle measurement (0.26 to 0.81 %) | met on Windows; Debian **pending** |
| FR-016 markers: place, drag, snap, rename, remove, jump, attached | `markers`; drag and right-click in the window | met in tests; the gestures **manual pending** |
| FR-017 times, distance, frequency in the language's format, honest precision | `measurement` (six languages) | met |
| FR-018 markers saved; a file without them opens | `session_scenarios`, `view_section` | met |
| FR-019 save and open in the background with progress and Cancel, failures leave the old file | `session_scenarios`, `save_interruptions` (114 fault points) | met |
| FR-020 names, order, visibility, colours, markers, description, simulated flag, unknown sections kept | `session_scenarios`, `view_section`, `view_section_carried` | met |
| FR-021 unsaved data protected, running capture stopped first, unsaved shown | `unsaved_guard` (with a mutation check), `random_walk`, `app::tests` | met |
| FR-022 damaged, truncated, foreign, newer files refused with a message | `session_scenarios` (damaged, truncated, empty, foreign, unknown critical section), `la-session` tests for the newer major version and the size limit | met |
| FR-023 all text from the resources, no key shown, fallback logged, six languages | `parity` (`la-i18n`), `language`, `string_budget`; the five translations are drafts marked as such | met; native-speaker review **pending** |
| FR-024 language changes at once and is remembered | `language`, `settings_commands`, `app::tests` | met |
| FR-025 Japanese correct, German fits at the minimum size, mixed-direction text harmless | `string_budget`, renders of every screen in six languages at 800 x 560, `fonts::tests`, `sanitize` | met on Windows; Japanese on a machine without CJK fonts and right-to-left text in the window **manual pending** |
| FR-026 user text as typed, limited, sanitized | `sanitize`, `presentation`, `language` | met |
| FR-027 numbers, units and dates follow the language; the axis is relative | `measurement`, `la-i18n` format tests, `axis` | met |
| FR-028 every failure shown as the message of its code | `error_sweep` (every stable code, six languages), `containment` | met |
| FR-029 failures and panics contained | `containment`, `panic_boundary`, `random_walk` (scripted failures and panics) | met |
| FR-030 repeats grouped, recent messages, log location | `notices`, `containment`, `app::tests` (banner and buttons) | met |
| FR-031 the log has commands, lifecycle, codes, details and no signal data or names | `logging` | met |
| FR-032 never blocks over 100 ms, never crashes, memory bounded by the visible limit | the longest interface-thread call is 21 ms; `random_walk`; `settings_commands` | met on Windows; Debian **pending** |
| FR-033 only the documented versioned host API, display-less tests | `docs/host-api.md`, `host_api_doc`; every host test runs without a display | met |
| FR-034 toolkit chosen with alternatives recorded | `research.md` (R1 and the toolkit decision) | met |
| FR-035 original design | no code, assets, names or layouts of other products were used; the palette and layout are our own (`theme.rs`) | met (a statement, not a test) |
| FR-036 keyboard operation of every command, labels for assistive technology, display scale | `shortcut_coverage`, the scale setting | **partly met**: every command is reachable from the keyboard and the scale follows the setting; labels for assistive technology and the system text size are **not provided** (the toolkit offers little here) |
| FR-037 a place for later panels, commands and decoder lanes | the side panels and the menus exist | **not met as specified**: there is no registration mechanism and no decoder lane area; the panels are fixed in `app.rs` |

| Criterion | Proved by | Status |
|-----------|-----------|--------|
| SC-001 a new user sees a waveform within 30 s | not measured with a person | **manual pending** |
| SC-002 frames under 16 ms (p95) and 50 ms at 100 M x 16, independent of the length | the plan and widget stages are under 5 ms; the plan cost grows x1.41 for 100 times the length | **partly proved**: the frame on screen was not measured; draw 21 ms in the picture test |
| SC-003 a planted glitch at every zoom | `glitch_sweep` | met |
| SC-004 never unresponsive over 100 ms on both systems | the longest call 21 ms | Windows met; Debian **pending** |
| SC-005 100,000 random actions, no crash, hang or loss | `random_walk` (100,000 in the tests, 2,000,000 nightly and run once) | met |
| SC-006 every error code, one localized message and a log entry | `error_sweep` | met |
| SC-007 exact round trip, 100 interruption points | `session_scenarios`, `save_interruptions` (114) | met |
| SC-008 zero clipped, overlapping or missing text on both systems | `string_budget`, renders at 800 x 560, `fonts::tests` | Windows met by these checks; missing glyphs on Debian **pending** |
| SC-009 language change repaints in under 200 ms | 25 ms | Windows met; Debian **pending** |
| SC-010 idle under 1 % of a core | 0.26 to 0.81 % | Windows met; Debian **pending** |
| SC-011 the question in 100 % of the scripted cases | `unsaved_guard` | met |
