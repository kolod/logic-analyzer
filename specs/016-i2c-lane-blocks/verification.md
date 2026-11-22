# Verification: I2C Lane as Blocks

What proves each requirement, what was checked by mutation, and what is left to a person.

## Baseline (T001)

Before this feature `cargo test -p la-decoders` passes. `la-host`'s `decoder_growth` (a timing check:
annotations must appear within a second of the data of a capture that runs at full speed) fails on the
development machine with and without this feature; CI passes it. It is not touched here. The signal of the
generator is the one of PR #32 (SDA changes with the falling edge of SCL), on which this branch is based.

## Traceability

| Requirement | Proved by |
|-------------|-----------|
| FR-001 two bands, all rectangles | `i2c_blocks.rs::a_good_write_is_six_blocks_side_by_side_and_one_block_below_them` (every lane annotation is a `part` or a `whole`); `lanes.rs` band tests; `decoder_i2c.rs` (host): the frame plan of the I2C lane has two rows, `Part` above and `Whole` below |
| FR-002 places of data, address, ack | `i2c.rs`, `i2c_options.rs`, `i2c_errors.rs` (hand-written `[from, to)` from R2); `i2c_blocks.rs::with_an_odd_period_…` (T = 15 and 17); `i2c_round_trip.rs::check_blocks` over the generator's seeds (address, data, ack and nak blocks from the items) |
| FR-003 start / repeated start | `i2c.rs::a_repeated_start_…`, `i2c_options.rs` (10-bit with a repeated start); round trip: a start block at `edge - 8` for every start item |
| FR-004 stop | `i2c_errors.rs` (stop after a cut byte, after an empty start), `i2c_blocks.rs::a_stop_right_after_the_eighth_bit_…`; round trip: a stop block ending at every stop item |
| FR-005 no overlap | `i2c_blocks.rs::a_stop_and_the_next_start_less_than_a_period_apart_never_overlap`; round trip: no two upper blocks overlap, every one lies inside a packet block (1,000 + 400 + 300 generated sequences) |
| FR-006 packet block | `i2c.rs`, `i2c_errors.rs`, `i2c_blocks.rs` (from the start block's `from` to the stop block's `to`; unfinished: to the end of the last block) |
| FR-007 to FR-009 colours and texts | the same tests (colour index and key of every block); `decoder_view.rs` tests: the five colours are pairwise distinguishable and carry text with contrast ≥ 4.5 on the dark, light and high-contrast variants |
| FR-010 packet colour | `i2c_round_trip.rs::check_packets`: red exactly when an error lies inside the packet, over the planted-error, cut-capture and 10-bit seeds (SC-001); `i2c_errors.rs` |
| FR-011 text whole or none | `decoder_view.rs::a_block_shows_its_whole_text_or_none_never_a_cut_text` |
| FR-012 colours on every theme | `decoder_view.rs` (the two colour tests above) |
| FR-013 nothing else changes | the I2C items, chunking, sync, stacking, options and recorded-signal suites pass without a change in what they compare; the one change is the text of the SCL arrow of an unacknowledged error bit (`i2c_errors.rs`, fixtures `nak_on_address`, `nak_on_data`) |
| FR-014 selecting a block | blocks are annotations like any other; the interface helpers and the planner are tested; the look is left to a person |
| FR-015 six languages | no key was added or removed; the parity, budget and font checks pass |
| SC-001 | `i2c_round_trip.rs::check_packets` |
| SC-002 | the hand-written geometry tests above |
| SC-003 | `decoder_view.rs::a_block_shows_its_whole_text_or_none_never_a_cut_text` |
| SC-005 | the whole I2C suite of 008 passes; the lane tests were rewritten to blocks, the items and overlays are compared as before |

Decoder API 1.2 (`contracts/decoder-api-1.2.md`): `model.rs` (block shapes refused on points and edges),
`lanes.rs` (bands, dense cell for a taken place, all rows present, bound of the plan for 100,000 blocks),
`dann_section.rs` (codes 5 and 6 round-trip, code 7 refused), `descriptors.rs` (`lane_rows` 1 to 3),
`decoder_api_doc.rs` (the document names the additions and version 1.2).

## Changes to the contract made while implementing

- A stop block is held until what follows is known, so that a start less than a period after it cuts it
  short (FR-005); it is written when the next block comes or when the pass ends.
- A packet that was cut by lost samples is red (FR-010 lists lost samples among the errors); one that is only
  unfinished is grey; one that is unfinished and already had an error (a cut byte) is red.
- The header and the low byte of a 10-bit address are two address blocks that both name the whole address,
  each followed by its ack block.
- The recorded fixtures of 008 keep their signals (a start of 4 samples); their lane expectations were
  regenerated from the decoder and each line was read against R2 (the start block is cut at the first bit's
  cell, since the old fixtures have the first clock pulse 8 samples after the start). The period of
  `preinit_low_then_noise` is measured as 14, so its blocks have cells of 7 samples either side and do not
  touch.

## Mutation checks

- Packet colour: `text::transaction` made to ignore the error (always green) fails `i2c_errors.rs` (three
  tests) and the round trip's `check_packets`; restored.
- Stop block: `from = p - T` without the end of the last block fails `i2c.rs` (four tests) and
  `i2c_blocks.rs`; restored.

## Benchmark (T036)

`cargo bench -p la-decoders --bench decode_i2c`: 30 M samples in 0.51 s (about 17,000 µs per million
samples) against the recorded 25,798: not slower; 1.43 M annotations.

## Other runs

`LA_ROUND_TRIP_SEEDS=10000 cargo test --release -p la-decoders --test i2c_round_trip -- --include-ignored`
passes; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean;
`cargo test --workspace --no-fail-fast` fails only `decoder_growth` (the baseline above) and, once under
load, `la-storage`'s `hostile_input` (passes alone in 16 s; not touched by this feature).

## Left to a person

- Look at the lane on **Windows 10/11 and Debian 13** with the virtual device (quickstart, section 2) in the
  dark, light and high-contrast themes: the two bands, the colours, texts that hide whole, a pair marker on
  the edges of a block.
- The colour of an unfinished packet (grey) and the text of an error on the SCL arrow rather than in the
  block (the owner may want another place).
- The translations are unchanged; the long German and Japanese texts hide whole in narrow blocks.
