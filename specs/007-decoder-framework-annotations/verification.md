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
