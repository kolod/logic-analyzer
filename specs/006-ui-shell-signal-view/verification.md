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
