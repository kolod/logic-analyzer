# Verification: GUI Improvements

Written when the implementation was complete (all tasks of `tasks.md` up to the last run of the
quickstart). It records what was checked, by which test, and what is left to a person.

## Automated checks

| Check | Result |
|-------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo +1.85 check -p la-host -p la-storage -p la-session --all-targets` (MSRV) | clean |
| `cargo test --workspace` | all pass (the two flaky tests of earlier features are listed under "Known issues") |
| `cargo test --release -p la-storage --test transition_search -- --include-ignored` | pass: a thousand next/previous queries on a capture of 100,000,000 samples in under 50 ms in total |
| `LA_WALK_SEED=7` and `9001`, `LA_WALK_STEPS=50000`, `random_walk` | pass; the default seed (2026, 100,000 steps) passes with the five new commands drawn |
| `cargo bench -p la-host --bench frame_plan` | gated case 1.52 ms (budget 16 ms); probes grow x1.41 from 1 M to 100 M samples (limit x2); the plan now includes the edge flags; baseline rewritten in `crates/host/benches/baseline.json` |
| `cargo deny check` | not installed on the development machine; runs in CI. One build dependency was added (`winresource`, MIT, Windows builds only) with its few dependencies (`toml` family, MIT or Apache-2.0) |

## Traceability

| Requirement | Tests |
|-------------|-------|
| FR-001 colour bar click | `signal_view.rs::a_click_on_the_colour_bar_steps_the_colour`; host `SetChannelColour` tests |
| FR-002 in-place name editor | `signal_view.rs::a_click_on_the_name_opens_the_editor_over_that_lane`; `app.rs` editor tests (accept, cancel, empty name, rendering in two languages) |
| FR-003 border resize | `signal_view.rs::dragging_a_border_sets_the_height_and_a_double_click_makes_it_automatic`; `lane_layout.rs` (zones, limits); host `channel_height.rs` |
| FR-004 move by dragging | `signal_view.rs::a_drag_on_the_name_moves_the_lane_and_a_small_movement_does_not`; `lane_layout.rs` (landing); host `MoveChannel` tests |
| FR-005 Channels panel removed | no `panel-channels` key or message remains; the i18n parity test fails on unused keys |
| FR-006 at any time | host `channel_height.rs` (without a capture, while capturing) |
| FR-007, FR-008 one panel, icon strip | `side_panel.rs` (`next_state`, sizes), host `side_panel.rs`, `app.rs` tests of the three views |
| FR-009 Decoders and Markers views unchanged | the tests of features 006 and 007 run in their views |
| FR-010 panel width | fixed 280 px (`side_panel.rs`): the signals give way, never the panel; see "Deviations" |
| FR-011 Capture view contents | `app.rs` capture view tests (three languages, minimum size, while capturing) |
| FR-012 length in seconds | host `seconds.rs` (exact examples, refusals, round trips); `app.rs` (typed seconds kept, error texts) |
| FR-013, FR-014 channel buttons | `app.rs::the_channel_buttons_are_eight_to_a_row`, theme contrast test for 16 colours in three variants, inert while capturing, no channel explained |
| FR-015 channel that is off | host `ScrollToEdge` and plan tests (a hidden channel is not in the plan) |
| FR-016 to FR-019 edge buttons | `signal_view.rs` (button geometry, press, decoder lanes, overlay only); host `scroll_to_edge.rs` (quarter, clamps, gaps, follow off) |
| FR-020 cost | storage `transition_search.rs` (random captures against a scan; the 100 M timing); benchmark above |
| FR-021 recent files | host `recent_files.rs`; `app.rs` (submenu, missing marker, remove, clear, empty, rendering in English and Japanese) |
| FR-022 icons | `crates/app/tests/icons.rs` (files, sizes, declarations); the window icon is built in `main.rs` with a logged fallback |
| FR-023 Add decoder and D | `app.rs` (submenu lists the decoders by name, adds the chosen one, key D, disabled without a capture, inside the window in English and German) |
| FR-024 localisation | `la-i18n` parity test, `string_budget.rs` (two German texts shortened), headless tests at 800 x 560 |
| FR-025 nothing else changes | host `sessions_compat.rs`; the earlier test suites are unchanged except where the old panel or the flat recent list was replaced |
| FR-026 tests without a display | every interaction above is a unit or headless test |
| SC-001 | the four legend interactions each take one or two interactions by construction (click, click and typing, drag, drag) |
| SC-002 | at 1180 px: the old right column was 280 px wide, so the signals had 900 px; the strip alone is 39 px, so with the panel hidden they have 1141 px (+26.8 %, the 25 % asked); with a panel shown they have 861 px |
| SC-003 | the Capture view holds device, rate, length, seed and the channel buttons; the number of samples is `round(seconds x rate)` exactly (`seconds.rs`) |
| SC-004 | the scroll is two O(log N) searches per shown channel and a window computation; measured above (a thousand searches on 100 M samples under 50 ms) |
| SC-005 | integer division of the window length: the edge is within one sample of a quarter (`scroll_to_edge.rs` with windows of 400 and 4,000) |
| SC-006 | `recent_files.rs` (16 files keep 15; a settings file with 30 is cut to 15); a missing entry has no press action (`app.rs`) |
| SC-007 | tested with the two built-in decoders and with three (a registered test decoder); the menu code iterates the catalogue, so the count does not matter; one and eight were not tested separately |
| SC-008 | see "Texts to read" |
| SC-009 | `sessions_compat.rs`: both fixtures written by the code before the feature open with nothing lost; a file without a custom height has no `LAYT`; the section is kept byte for byte by a reader that does not know it |

## Mutation checks

Each guarded line was broken, the named test run, and the line restored (the working tree was clean
afterwards). Every mutation was caught.

| Mutation | Caught by |
|----------|-----------|
| border band above the lane 3 px -> 2 px, and below 4 px -> 5 px | `lane_layout` tests |
| `ScrollToEdge` quarter `/ 4` -> `/ 3` (Next), `3 / 4` -> `2 / 4` (Previous) | `scroll_to_edge.rs` |
| maximum height 240 -> 250 | `channel_height.rs` |
| `MAX_RECENT_FILES` 15 -> 14 | `recent_files.rs` |
| `next_state` never hides the panel | `side_panel.rs` unit test |
| `next_transition` off by one, `previous_transition` at the wrong bound | `transition_search.rs` |
| seconds rounded down instead of half up | `seconds.rs` |

## Texts to read

All new texts exist in the six languages (the parity test passes). The headless tests render the
new screens at the minimum window size in English, German, Ukrainian and Japanese. A person who
reads those languages should look at:

- Japanese: `capture-length-*`, `capture-channels`, `menu-file-recent-*` (the longest is
  `menu-file-recent-empty`, 30 columns);
- German, the longest after the budget check: `Fehlende Einträge entfernen` and
  `Keine letzten Dateien` (the first wording of both was over the string budget and was shortened).

## Deviations from the plan

- The panel is 280 px wide at first and its width is changed with a splitter (220 to 480 px, kept in the
  settings); the signals give way and the minimum window size still holds the widest panel.
- The contract said a resize "sends the command once, on release". The height is sent live while the
  border moves, so the lane follows the pointer; the value is the same.
- The accessible text `edge-previous` / `edge-next` for the status line was not added: a canvas has no
  tooltips, the arrow is the whole affordance, and unused keys would fail the parity test.
- `winresource` is used for the Windows icon; the build script only warns when the resource compiler is
  missing. CI (`cargo deny`, MSRV) has not run yet on this branch.

## Known issues

- The storage timing test on Windows and the `the_interface_scale_steps…` test of the application are
  flaky now and then; neither is touched by this feature.
- The fixture of feature 008 holds no annotations: the file was written from a random simulated
  signal, so `sessions_compat.rs` checks the decoder, its saved state and the absence of warnings,
  not a count of results.

## Left to a person

- Quickstart section 4 on Windows and on Debian (the look of the legend, the panel and the buttons).
- The icon in the Explorer and in a Debian file manager (the installer of a later feature registers
  the file type; `packaging/` holds what it must register).
- The translations (above).
