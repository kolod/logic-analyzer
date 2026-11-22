# Quickstart: validating the I2C decoder

Run from the repository root. Everything runs without hardware on Windows 10/11 and Debian 13.
Commands are the same on both (`cargo` from the stable toolchain; the MSRV job uses 1.85).

## 1. The decoder is correct (automated)

```text
cargo test -p la-decoders --features test-support
```

Expected: all tests pass, among them
- `i2c` unit tests (bits, acknowledgements, 10-bit address, repeated start, errors, synchronisation,
  calibration of the bus period);
- the recorded-signal tests over `crates/decoders/tests/fixtures/i2c/*` (a malformed fixture is rejected);
- the round trip over 1,000 seeds (`LA_ROUND_TRIP_SEEDS` raises it) with 7/10-bit, errors on and off,
  start-up on and off, and lengths that cut transactions: zero differences (SC-001);
- chunk invariance: the same result for chunk limits from one edge to the whole capture and for a
  capture fed in pieces (SC-005);
- the random-signal test: a million random chunks, no panic (SC-004);
- the stacking test with a test-only decoder, including the held-back watermark;
- `decoder_api_doc`: the document says `1.1` and the constant agrees.

A longer sweep in release mode: `cargo test -p la-decoders --release -- --ignored` (ten thousand seeds).

## 2. The host works end to end (automated)

```text
cargo test -p la-host
```

Expected: the tests of feature 007 still pass untouched, and the new ones: a virtual-device capture
with an I2C instance decodes to the generated transactions; the decoder appears in the catalogue
without any change to the shell; a capture of ten million samples decodes without an interface pause of
more than 100 ms (SC-006); options and results survive saving and opening a session.

## 3. Localization and contract checks (automated)

```text
cargo test -p la-i18n
cargo test --workspace --exclude la-app
```

Expected: no missing or unused key in any of the six languages.

## 4. Look at it (manual, on both systems)

1. Start the application, choose the virtual device, enable two channels, open the decoder panel and add
   **I2C**. Assign SCL and SDA to the two channels. Start a capture with a seed (for example 7).
2. A lane appears under the channels with transactions: an address with *write* or *read*, the data
   bytes, ACK points. Zoom in on one byte: nine arrows on SCL on the rising edges, a dot on SDA at the
   start and at the stop. Zoom out: marks disappear when they cannot be told apart, the lane stays
   readable (feature 007).
3. Turn on **errors** in the decoder panel (rate 4): NAKs and cut-short bytes appear in the error
   style, each with a message where it happened; the transactions around them still decode.
4. Change **Address** to the 8-bit form: 0x50 becomes 0xA0 (write) or 0xA1 (read); the old results stay,
   flagged outdated, until the new ones replace them.
5. Save the session, close and reopen: the instance, its channels and options are back, and so are the
   results if you saved them.
6. Stack a decoder (the test decoder is not shown in the release build; this step is for the
   automated stacking test).

Start-up (R2, spec clarifications): in the host tests, a capture whose first part is noise and low
lines decodes to the same transactions as the same capture without the noise.

## 4a. The edges and the panel (after the owner's review)

- Generated signal: SDA changes in the sample where SCL falls; in a start, a repeated start and a stop its
  edge is 8 samples from an edge of SCL (automated: `i2c_round_trip.rs`, the signal-shape test).
- Turn channel 3 off in the capture settings: the lists of SDA and SCL of a decoder do not offer it; a
  decoder already assigned to it shows it as unavailable and does not run.
- The comboboxes of a decoder fill the width of the panel; the panel has no frame of its own, each decoder
  has one.

## 5. Performance and the baseline (automated, manual comparison)

```text
cargo bench -p la-decoders --bench decode_throughput
```

Expected: the I2C case is within the recorded baseline (`benches/baseline.json`); decoding dense traffic
costs a constant per edge and the drawing cost does not grow with the capture length.

## What a failure looks like

- A round-trip difference names the seed and the first differing item; reproduce with that seed.
- A chunk-invariance difference names the chunk limits that disagree.
- A panic in a decoder is caught and reported as `decoder.failed` by the framework; the fault-injection
  tests of 007 prove the containment, and the random-signal test here is what keeps this decoder from
  needing it.
