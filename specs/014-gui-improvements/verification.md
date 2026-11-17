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

## Changes after the first implementation

The owner used the first implementation (2026-11-16 and 2026-11-17) and asked for the changes below.
They were built in the same pull request after task T057 and the specification, the contract and this
record were brought up to date afterwards (the Clarifications of `spec.md`; the requirements marked
"amended" and FR-027 to FR-029).

| Change | Commits | Requirement | Covered by |
|--------|---------|-------------|------------|
| Start and Stop in the Capture view, no toolbar; channel buttons 28 px with the number, All and None | `c9c6754` | FR-011, FR-013 | `start_is_in_the_capture_view_and_not_in_a_toolbar`, `eight_channel_buttons_fit_a_row_of_the_panel`, `all_and_none_switch_every_channel_at_once` |
| Comma and point both separate the fraction of the length | `973c0ad` | FR-012 | `a_comma_and_a_point_both_separate_the_fraction_of_the_length_in_every_language` |
| A lone dense column is no mark above or below the signal | `9edc319` | (drawing; no requirement) | `a_dense_band_spans_the_levels_of_the_waveform_and_never_more` |
| The lane is lifted whole while dragged and the others slide aside | `dcfd0f9`, `e08e564` | FR-004 | `the_lanes_between_the_two_slots_make_room_by_the_height_of_the_carried_lane`, `easing_moves_toward_the_target_without_overshooting_and_arrives`, `the_lanes_slide_aside_while_a_lane_is_carried_and_settle_on_their_places` (the lifted lane itself is drawing and has no test) |
| Legend row: the name, and under it the number of the analyzer input | `87d1069` | FR-027 | `a_lane_shows_the_name_above_the_number_of_its_analyzer_channel` |
| Decoder panel: name only, a bin, a fold arrow, no tooltips on the header buttons | `220a726`, `ed6a6a8` | FR-009 | `a_decoder_is_shown_by_its_name_and_its_properties_fold_away`, `the_decoder_header_renders_in_every_language_at_the_minimum_size` (no test for the missing tooltips) |
| The answer to the unsaved-capture question can be remembered (Save, Discard), in the question and in the Settings; Cancel never | `677e71e`, `0a633b2` | FR-028 | `the_question_about_an_unsaved_capture_can_remember_the_answer`, `a_remembered_discard_throws_the_capture_away_without_asking`, `cancel_cannot_be_remembered`, `a_cancel_written_in_the_settings_file_asks_as_before`, `the_settings_have_the_box_and_the_two_answers_that_can_be_remembered` |
| A remembered Cancel never blocks quitting or closing | `bc394af` | FR-028 | `quit_and_close_work_with_a_remembered_discard_or_save`, `a_remembered_cancel_never_blocks_quit_or_close` |
| A cog at the foot of the icon strip opens the Settings | `341f6fe` | FR-029, FR-007 | none (the cog is a button; its look is covered by the screen rendering tests only) |
| A splitter changes the width of the panel (220 to 480 px) and the buttons share it | `166e8e1` | FR-010 | `the_splitter_changes_the_width_of_the_panel_within_its_range_and_keeps_it`, `the_width_of_the_panel_is_kept_within_its_range_and_saved`, `a_width_in_the_settings_file_that_is_out_of_range_is_cut_to_it`, `the_panel_and_its_buttons_render_at_the_narrowest_and_the_widest_width` |
| "Reset channels" is called "Reset" | `c6e751a` | FR-005 | the language parity test (the key `channel-reset` and its translations) |
| The same room on both sides of the content of the panel | `bf4c464` | (style) | none |
| A finished capture never lacks its last samples (the outcome was read after the snapshot) | `21c0205` | (a fix in `la-host`, feature 004 and 006) | the random walk of `la-host` (`a_long_random_walk_keeps_every_invariant_and_loses_no_data`) |

The tests named here are the ones the commits added or changed; the table was written from the commit
history, not from a new run of the whole suite.

## Asked for on 2026-11-17, now built

Specified in `spec.md` (Clarifications and User Story 8) and built after the first record above.

| Change | Requirement | Covered by |
|--------|-------------|------------|
| The Start button as wide as the panel | FR-030 | `the_start_button_is_as_wide_as_the_panel` (panel widths 220, 280 and 480) |
| Fixed-width numbers in the status line | FR-031 | `the_numbers_of_the_status_line_keep_their_width_while_the_digits_change`, `a_text_of_wide_characters_counts_two_columns_each` (the pure padding; the look of the line is not tested) |
| The time under the pointer next to the pointer | FR-032 | `the_time_is_below_and_to_the_right_of_the_crosshair_when_there_is_room`, `the_time_goes_to_the_left_when_the_right_side_is_too_narrow`, `the_time_goes_above_near_the_bottom_and_never_over_the_axis_or_outside_the_view`, `the_box_grows_with_its_text` |
| A hand while the view is moved with the mouse | FR-033 | `the_pointer_is_a_hand_while_the_view_is_moved_sideways_and_not_before`, `dragging_sideways_makes_the_pointer_a_hand_until_the_button_is_released` |

How it is done: the numbers that change (elapsed time, samples, memory) are shown in a monospaced font
and padded with no-break spaces to the widest they have been in the running capture. The box with the
time is an estimate of the text width (the canvas cannot measure a text), so a very wide glyph may touch
its edge. The box with the text of an annotation under the pointer moved down by the height of the time
box so that the two do not overlap. None of it was looked at on a screen by the author.
