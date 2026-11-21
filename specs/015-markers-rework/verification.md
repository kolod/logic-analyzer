# Verification: Single and Pair Markers

Branch `015-markers-rework` from main 75b19af. Checked on Windows 11 with Rust 1.100 (`la-host` and
`la-storage` also with 1.85). Tests were run with `CARGO_PROFILE_TEST_DEBUG=0` (the disk had little room).

## Commands run and results

| Check | Result |
|-------|--------|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo +1.85 check -p la-host -p la-storage` | clean |
| `cargo test --workspace --no-fail-fast` | everything passes **except** `decoder_growth::annotations_appear_while_the_capture_runs_and_the_capture_is_not_harmed`, which fails the same way on a clean `origin/main` (checked in a separate worktree): a timing check of the decoder pass, unrelated to markers |
| `cargo bench -p la-host --bench markers` | 1 M samples: plan with 48 markers 1.8 ms, pointer resolution 0.6 µs; 8 M samples: 4.0 ms and 0.9 µs (x1.4 for a capture 8 times longer: no growth with the length); baseline in `crates/host/benches/markers_baseline.json` |
| `cargo deny check` | not run here (the tool is not installed); the CI job runs it; no dependency was added |

## Where each requirement is tested

| Requirement | Tests |
|-------------|-------|
| Model, numbering, limit, ordering, conversions, remap, window helpers (FR-001, 005, 006, 009, 010, 011, 017, 019, 026) | `crates/host/tests/markers.rs`, `marker_commands.rs` |
| Placement state machine, the pointer to a sample, refusals, pairs (FR-007, 027–031) | `crates/host/tests/placement.rs` |
| Snapping, lane gating, gaps, the nearest edge against a scan (FR-032, 033, SC-009) | `crates/host/tests/snapping.rs` |
| The crowded-zone zoom, the saved view and its restore, 1,000 edges 1–3 samples apart picked exactly (FR-034, SC-008) | `crates/host/tests/placement_zoom.rs` |
| Cut left, cut right, trim (background, cancel, memory failure, always asked, decoders rerun), fit (FR-012–016, SC-003, 004) | `crates/host/tests/marker_actions.rs` |
| Axis counting from the origin, negative times (FR-011) | `crates/host/tests/axis.rs`, `crates/i18n/tests/formats.rs` |
| `VIEW` payload version 2: bytes by hand, every rejection, mutation fuzz (FR-022–024) | `crates/host/tests/view_section.rs` |
| Round trip, old files, the file of this feature (FR-022–024, SC-006) | `session_scenarios.rs`, `sessions_compat.rs` (fixture `markers_v2.lasession`) |
| The cropped copy (FR-012, 015) | `crates/storage/tests/crop.rs` (hand cases, gaps, flags, budget before copying, cancel, property test) |
| Duration and frequency in six languages (FR-014, SC-002) | `crates/host/tests/measurement.rs` |
| 100,000 random actions with every marker command (SC-005) | `crates/host/tests/random_walk.rs` (`Markers::check` and the placement/zoom invariants after every action) |
| Marks, lines, events, the panel, the action list, the dialog, the dwell clock (FR-002–004, 020, 021, SC-007) | `crates/app/src/marker_marks.rs`, `signal_view.rs`, `dwell.rs`, `app.rs` tests |
| Host API document in step with the code | `crates/host/tests/host_api_doc.rs` |

Tests that passed at once were mutation-checked (a guarded line broken, the test seen to fail, the line
restored): crop fold of the first transition, tie to the earlier edge, lane gating, restore of the view,
clearing the saved view by a hand zoom, `Guarded::Crop` excluded from a remembered answer, the remap in
`apply_crop`, the roles of a dragged pair, centring against revealing, the side of the squares, the right
button cancelling, the limit text, the action list.

## Deviations from the plan and decisions taken during the work

- `la_storage::crop` also takes the `Limits` (the snapshot does not carry them).
- The eight marker colours are the eight annotation colours (`Variant::marker` = `Variant::annotation`),
  already chosen for legibility on the three themes; no new table.
- A right click on a mark selects the marker, shows the Markers view and opens its action list **there**
  (the same list as a click on the row), not an overlay at the pointer.
- The fit margin is computed in closed form (`ViewWindow::fit_range`), not by iteration.
- Pixel checks of the drawing (planned in T016, T025, T031) are replaced by tests of the geometry
  (`marker_marks.rs`: tip on the line, squares left/right of their lines) and of the events that produce
  what is drawn; the picture itself can be written with `LA_SNAPSHOT_DIR` for a look.
- `Pointer.step` and the arrow keys are gone: the dwell zoom replaced them (spec clarification).
- The confirmation texts say "the marker" and not its name (the question does not carry the id).
- The state `activity` is `Cropping`; saves and loads never set `Saving`/`Loading` (they use `operation`),
  so the refusals test both.
- Not done: a test that injects a panic into the crop worker (the worker catches panics and reports
  `host.internal`; by reading, not by test); the manual walk through `quickstart.md` on both systems
  (this session had no screen); `cargo deny` locally.
- `Cargo.toml` has `debug = "line-tables-only"` for the dev profile (changed by the owner during the
  work to save disk space); it is not part of this feature.

## Changes after looking at the running application

Decided by the owner after trying the first version; the spec, plan, research, data model, contracts and
quickstart were brought in line with them (the task list keeps the history as it was written):

- Marks are smaller (pentagon 18 px, squares 18 × 14), have no outline, carry the number only and are drawn
  over the ruler; the lines are 1 px (1.5 px selected), start at the tip of the pentagon or at the top
  corner of the square of a pair.
- Each new marker takes the next of the eight colours.
- The Markers view: the add buttons fill the width; each marker has its own outline, the selected one a
  stronger one; the `⋯` button, the bin and the checkboxes are gone; a click on a row opens its actions; the
  pair is made by "Make a pair" and a click on the second single marker in the panel or in the view
  (the rest shaded; Esc cancels).
- A pair's row shows the times on one line and the duration and frequency on the next.
- The snap ring is shown while a placed marker is dragged.
- The keyboard shortcuts work on non-Latin layouts (a letter that is not Latin is taken by the place of its
  key); this is outside the markers but was found with them.

## CI

Pull request #27 (merged): fmt, clippy, test, deny and msrv passed on Debian and Windows, and the commit-subject check passed. The first run of `test (debian)` caught a real fault that the local runs had not hit: a drag whose marker was removed, joined into a pair or replaced by an opened session stayed in `State.drag` (the random walk found it at action 78494). It is fixed in `e28798a` with a test that fails without the fix. `cargo deny` is thus covered by CI.
