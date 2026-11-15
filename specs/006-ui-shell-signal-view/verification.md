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
