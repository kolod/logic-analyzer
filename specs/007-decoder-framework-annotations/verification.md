# Verification log: 007 decoder framework and annotations

Manual checks follow `quickstart.md`. Automated results are in CI; this file records what a person
looked at.

## Phase 3 (User Story 1): decode a signal and see what it says

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (headless render, `iced_test`) | quickstart 8.1 to 8.3: capture on the simulated device, add the sample decoder, choose channels, wait for the pass, render at two zoom levels (`LA_SNAPSHOT_DIR`) | The pictures show the **Decoders** menu and panel, the lane "Clocked bits" with ranges, bit dots and a red incomplete-byte span, the edge arrows on the clock channel and the dots on the data channel. Looked at by the implementer from the written PNG. |
| pending | Windows 11, running application | the same steps by hand, plus click to select, double click to go to, F8 / Shift+F8 | **Not done yet**: needs a person at the window. |
| pending | Debian | the same | **Not done yet**: Debian results are asked of the user, not claimed. |

Known gaps against the task text (to close in the polish phase): no hover readout of an annotation's
text (the selected annotation is described in the panel instead), and no separate `decoders.rs`
module in `la-app` (the panel is in `app.rs` next to the other panels).

## Phase 4 (User Story 6): try decoding without hardware

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (automated) | quickstart 8.4 as tests: `decoder_generator` captures on the simulated device with and without deliberate errors and compares the lane with `expected()` and `expected_errors()`; `round_trip` does 1,000 seeds (10,000 in the ignored variant) | Pass. |
| pending | Windows 11, running application | add the decoder, tick the deliberate errors, start a capture, look for the red marks | **Not done yet**: needs a person at the window. |
| pending | Debian | the same | **Not done yet**: asked of the user. |

Design note: `AddDecoder` needs a capture on screen, so a decoder is added to a capture and then stays
for the next one: `StartCapture` keeps the instances and restarts their passes on the new capture
(`NewCapture`, opening a file and closing remove them). This is how a simulated capture can be made
valid for a configured decoder.

## Phase 5 (User Story 2): a broken decoder never harms the session

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (automated) | `fault_injection`, `decoder_faults`, `decoder_log`, `decoder_settings`: an error, a panic, a decoder that never returns, memory and annotation limits and invalid output next to a healthy decoder | Pass. |
| pending | Windows 11 and Debian, running application | a person reads the six-language fault messages in the panel | **Not done yet**; the translations are machine drafts. |

## Phase 6 (User Story 3): a very large capture stays smooth, also while it grows

Machine: the maintainer's Windows 11 PC, release builds, 2026-11-15. Baselines are in
`crates/decoders/benches/baseline.json` and `crates/host/benches/annotation_baseline.json`.

| Measurement | Result |
|-------------|--------|
| Store insertion (2 M annotations) | 262 ns in start order, 331 ns nearly in order, 1,192 ns in random order; 4 runs |
| Sample decoder over 100 M samples (generated clocked bits, 440 k annotations) | 0.19 s, about 539 M samples/s |
| Lane plan, 1 M and 5 M annotations over 100 M and 1,000 M samples, whole capture, 1 % and 4 px per sample, 2000 and 4000 px | 0.4 to 1.5 ms; the 5 M store is x1.01 of the 1 M store; at most 4,894 cells (limit 4 · W) |
| `measure_decoder_responsiveness` (16 channels, 100 M samples, random signals, sample decoder, 1,900 px) | during capture and decoding the longest call 2.7 ms; zoom and pan update + plan p50 2.35 ms, max 3.64 ms; widget tree max 0.27 ms; language change 8.3 ms; cancel 13 ms |

The bench uses 5 M annotations as the large case because that is the most an instance can hold
(the task text says 10 M). The pictures on a real graphics card and on Debian are not measured here.

## Phase 7 (User Story 4): stack decoders

There is no manual step: the one shipped decoder has nothing to stack, so the tests are the proof
(`stacking` in `la-decoders` with the fixtures `lower_bytes` and `upper_words`, `decoder_stack` in
`la-host`). Automated result on 2026-11-15 (Windows 11): pass.

## Phase 8 (User Story 5): save and open a capture with its decoders

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (automated) | quickstart 8.5 as tests: `decoder_sessions` saves and reopens a stack and the sample decoder with and without results, a file without decoder sections, damaged `DECO` and `DANN`, an unknown decoder's record byte for byte, a newer version, and 156 fault points of a save; `deco_section` and `dann_section` read hand-written bytes and run mutation fuzzers | Pass. |
| pending | Windows 11, running application | save, close, reopen with and without saved results | **Not done yet**: needs a person at the window. |
| pending | Debian | the same | **Not done yet**: asked of the user. |

The contract was tightened while implementing it (`contracts/deco-section.md`): entries are in
ascending order, a role without a channel has no entry, the start of an item is a signed delta from the
previous end (items may overlap), and the string table lists distinct strings in order of first use. All
of these make "an accepted payload writes back unchanged" true.

## Phase 9 (User Story 7): read the annotations comfortably and in my language

| Date | Platform | Step | Result |
|------|----------|------|--------|
| 2026-11-15 | Windows 11 (automated) | `decoder_language` (names, labels, annotation texts and notices follow the language; literal text and instance names do not), `every_screen_renders_in_every_language_at_the_minimum_window_size` with a decoder and the Decoders panel at 280 px, `string_budget` with the panel controls, the drawing rules of the lane (`span_label`: ellipsis, nothing under three characters, a span never reaches into the next cell) and the contrast tests of the eight annotation colours in three variants | Pass. The rendered PNG was looked at by the implementer. |
| pending | Windows 11, running application | quickstart 8.6 and 8.7: switch the six languages with decoders, the smallest window, a Japanese IME entry of an instance name, high contrast | **Not done yet**: needs a person at the window. |
| pending | Debian | the same | **Not done yet**: asked of the user. |

Notes against the task text: the tab order of the panel is the order of the widgets of iced (text
inputs and buttons take Tab); the accelerators of next, previous and go to selected annotation were
added in phase 3. The contrast test checks the colours against the background (at least 3:1), against
each other and against the danger colour of errors; "against the waveform colours" is checked only as
"not the same colour", because an annotation is drawn as a filled band with an outline, not as a line on
top of a waveform. A hover readout with the full text and the length in time was added to the canvas.

## Phase 10: long runs, measurements and traceability

Machine: the maintainer's Windows 11 PC, release builds, 2026-11-16.

| Run | Result |
|-----|--------|
| `deco_section` and `dann_section` randomized fuzzers, 1,000,000 iterations each | pass (0.3 s and 0.5 s) |
| `round_trip` `ten_thousand_seeds` (with deliberate errors in a third of them) | pass (0.9 s) |
| `random_walk` of 100,000 actions with decoder actions (adding, configuring, stacking, cancelling, re-running, removing, saving results, opening good and damaged decoder files, language changes), seeds 1, 2, 3, 4 and 2026 | pass (about 40 s each in a debug build). The first run found a real bug: a source that does not exist was accepted while the instance was still missing its required channels; fixed (`Setup::validate_partial`) with a regression test |
| `random_walk` of 2,000,000 actions in release | see the line at the end of this section |
| `measure_decoder_responsiveness` and the benchmarks | recorded under Phase 3 and Phase 6 above |

**Not measured, asked of the user:** SC-011 (idle CPU under 1 % of one core with finished decoders, one minute in the real window) and every Debian measurement and manual step. They need the running application on the user's machines; they are `pending` here and not claimed.

### Traceability

Tests are under `crates/<crate>/tests/` unless a path says otherwise.

| Requirement | Proof |
|-------------|-------|
| FR-001, FR-029 | `descriptors` (descriptor, registry), `a_registered_test_decoder_appears_without_changing_the_shell` (`la-app`) |
| FR-002, FR-003 | `decoder_commands` (add, configure, limit of 8 instances, names sanitized) |
| FR-004, FR-039 | `docs/decoder-api.md` and `decoder_api_doc`; `docs/host-api.md` and `host_api_doc` |
| FR-005 | `la-decoders` and `la-host` have no toolkit dependency; everything is tested without a window |
| FR-006, FR-007, FR-008, FR-009 | `model`, `chunking` (validation, counting of refused records), `text` and the sanitizer cross-check in `sanitize` |
| FR-010 | `index` (window, rank and neighbour queries against brute force, probe counts) |
| FR-011 | `index` (limits), `fault_injection`, `decoder_settings` |
| FR-012, FR-013, FR-014 | `execution`, `worker`, `chunking` (progress, cancel under one second, snapshots only) |
| FR-015, FR-016 | `execution` (growing capture, gaps), `clocked_bits` (gaps), `decoder_growth` |
| FR-017 | `decoder_commands`, `decoder_growth` (a setup change starts a new pass, old results flagged outdated until the swap) |
| FR-018, FR-019 | `stacking`, `decoder_stack`, `descriptors` (cycle, kind, depth) |
| FR-020, FR-021, FR-022, FR-023 | `fault_injection`, `decoder_faults`, `decoder_log` |
| FR-024, FR-025, FR-026, FR-027 | `lanes`, `decoder_plan`, the renders of `la-app`; manual check pending |
| FR-028 | `decoder_plan` (select, next, previous, go to), hover readout in the canvas; click and double click manual, pending |
| FR-030 | `la-i18n` parity, `string_budget`, `decoder_language`, `error_sweep` |
| FR-031, FR-032, FR-033, FR-034 | `deco_section`, `dann_section`, `deco_sections_carried`, `decoder_sessions` |
| FR-035, FR-036, FR-037, FR-038 | `clocked_bits`, `round_trip`, `composite`, `decoder_generator` |
| FR-040 | no network, no telemetry and no loading of code were added (dependency list unchanged apart from `serde_json` as a dev-dependency) |
| FR-041 | nothing was taken from another project; the decoders and generators are fresh implementations |
| FR-042 | `every_screen_renders_in_every_language_at_the_minimum_window_size`, `string_budget`; idle processor use: pending (see SC-011) |
| SC-001 | the renders of the sample decoder in `la-app`; the 60-second manual check is pending |
| SC-002 | `annotation_plan` benchmark (0.4 to 1.7 ms for stores of 1 and 5 million annotations over 100 and 1,000 million samples; x0.95 to x1.01 between sizes), `measure_decoder_responsiveness`. The benchmark builds the stores directly; the 16-channel 100-million-sample frame times with a one-million-annotation decode were not measured together |
| SC-003 | `a_planted_one_sample_annotation_is_represented_at_every_zoom_among_a_million` (`lanes`) |
| SC-004 | `measure_decoder_responsiveness` (longest call of the interface thread 2.7 ms, cancel 13 ms) on Windows; Debian pending |
| SC-005 | `fault_injection`, `decoder_faults`, `decoder_log` (each fault: capture unchanged, neighbour unchanged, one notice with a code, a log entry). The capture is checked by its transitions and length, not byte for byte on disk |
| SC-006 | `round_trip` (1,000 seeds in the normal run, 10,000 in the ignored one, with and without planted errors) |
| SC-007 | `random_walk` (100,000 actions in the normal run, 2,000,000 in the nightly one) |
| SC-008 | `decoder_sessions` (156 interruption points of a save; a file from before, a damaged one and a newer one) |
| SC-009 | `annotations_appear_while_the_capture_runs_and_the_capture_is_not_harmed` (`decoder_growth`) |
| SC-010 | the render test in all six languages at 800 x 560 with a decoder, `string_budget`; no clipped or overlapping element was found by the tests, glyphs checked by the rendering of the existing fonts; the check by eye is pending |
| SC-011 | **not measured** (needs the real window for a minute); pending |
| SC-012 | `a_registered_test_decoder_appears_without_changing_the_shell`, `CONTRIBUTING.md` "Adding a decoder"; the review by a person who did not write the contract is pending |

**Requirements not met or not proven here:** SC-011 (not measured), the Debian and manual items above,
and the reviewer of SC-012. No functional requirement is knowingly unmet.

`random_walk` of 2,000,000 actions in release (seed 4711): pass, 719 s.

One run of `cargo test --workspace` on 2026-11-16 ended `decoder_sessions` with `STATUS_ACCESS_VIOLATION`
(exit code 0xc0000005); six runs of that test alone and two further full runs passed. The likely cause
is a decode worker still running while the process was torn down (a test ended with passes running).
`Drop for DecoderRuntime` now cancels the passes and gives them up to two seconds to stop. The cause was
not proven; if it comes back in CI it should be looked at again.

Final local gate on 2026-11-16: `cargo fmt --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace` (twice), `cargo +1.85 check --workspace
--exclude la-app --all-targets`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: all clean.
The push, the draft pull request and the CI results on both systems are not done yet (they wait for the
user's word).
