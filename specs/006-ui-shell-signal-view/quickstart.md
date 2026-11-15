# Quickstart: validating the UI shell and signal view

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

Run from the repository root on Windows 10/11 and on Debian 13. Items marked *(manual)* need a
window and are recorded in `verification.md` with the platform, date and result.

## 1. Automated checks (no display needed)

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p la-host --release -- --ignored        # long random walks, interruption sweep
cargo bench -p la-host --bench frame_plan           # compare with benches/baseline.json
cargo test --workspace --exclude la-app             # with the MSRV toolchain (CI job)
```

Expected: everything passes; the host API document test (`host_api_doc`) agrees with
`HOST_API_VERSION`; the frame-plan benchmark stays within the recorded baseline (a growth with
capture length is a failure, Principle VIII).

Scenario coverage in `la-host` (each is a test, mapped to the spec):

| Check | Spec |
|-------|------|
| virtual device capture with a fixed seed ends with the expected samples; scripted failure keeps the data | US1, SC-001 |
| zoom/pan tables and invariants; frame plan equals a brute-force reference | US2, FR-007..015 |
| planted one-sample glitch visible at every zoom level | SC-003 |
| markers: snap, drag, measurement values in six languages | US3, FR-016/017 |
| save → open reproduces capture, presentation, markers, flags; interrupted saves leave the old file | US4, SC-007 |
| unsaved guard for close/open/new/quit, running capture stopped with data kept | SC-011 |
| every stable error code produces one localized notice and one log entry | US6, SC-006 |
| 100,000 random actions with injected failures: no panic, no invariant violation, data kept | SC-005 |
| hostile channel names (RTL, controls, long, emoji) sanitized | FR-026 |
| damaged/missing settings; invalid `VIEW` section; unknown extra section preserved | edge cases, FR-020/022 |
| every command in the shortcut table is reachable; table has no conflicts | FR-036 |

## 2. Run the application

```text
cargo run -p la-app --release
```

*(manual)* On a machine with no hardware:

1. The window opens; the device list shows the simulated device marked as simulated (US1.1, FR-006).
2. Choose 16 channels, a sample rate, a length of 100 million samples, seed `1`, press **Start**
   (F5). Signals grow while it runs; **Stop** (Shift+F5) ends it within a second (US1.2–3).
3. Zoom with the wheel and `+`/`-`, pan by dragging and with the arrow keys, press **Ctrl+0** to fit
   all. Zoomed out, busy columns show the hatched dense mark; zoom into one and see every
   transition (US2.1–3, FR-010). Frame time stays smooth (SC-002).
4. Rename a channel to `シグナル` with the Japanese IME, hide another, change a colour (FR-013,
   FR-025).
5. Place two markers with **M** and read the distance and frequency; drag one near a transition with
   snapping on (US3).
6. Press **Ctrl+S**, save, close the window and reopen the file: names, colours, order, markers and
   the simulated badge are back (US4).
7. Corrupt a copy of the file (change a byte with a hex editor) and open it: a localized refusal,
   the application keeps running (US4.4).
8. Open **Settings** and switch through all six languages with a capture open: every text changes at
   once, channel names do not, nothing is cut off at the minimum window size 800 × 560 (US5, SC-008,
   SC-009).
9. Start a capture and close the window mid-way: the unsaved-changes dialog appears; choose Cancel,
   then Save, and confirm the capture was kept (SC-011).
10. Leave the application idle for a minute and check that processor use is under 1 % of one core
    (SC-010).

## 3. Layout and text check

The headless text-fit check (or its fallback, see research R10) renders each screen at 800 × 560 in
German and in Japanese and reports clipped, overlapping or missing-glyph elements. Expected: none.
*(manual)* On Debian without CJK system fonts, the application's own Japanese text still renders
(bundled subset, research R5).

## 4. Responsiveness *(manual, both OSes)*

Run a 100 M-sample capture and, while it runs and after, zoom, pan, save and switch language; the
built-in frame log (`LA_FRAME_STATS=1`) must show no frame over 50 ms (worst) and p95 under 16 ms,
and no input stall above 100 ms (SC-002, SC-004).

## 5. Constitution and CI

Confirm the CI jobs for the setup phase went green on both OSes: clippy, fmt, test, deny (with
`BSL-1.0` allowed), msrv (excluding `la-app`), and the documentation check. Record the Debian system
packages needed to build `la-app` (if any) in `CONTRIBUTING.md`.
