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
