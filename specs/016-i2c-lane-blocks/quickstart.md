# Quickstart: I2C Lane as Blocks

How to see the feature and check it. Contracts: [i2c-lane](contracts/i2c-lane.md),
[decoder API 1.2](contracts/decoder-api-1.2.md).

## 1. Automated checks

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p la-decoders      # i2c_blocks (geometry, round trip), lanes (bands), decoder_api_doc
cargo test -p la-host -p la-app
LA_ROUND_TRIP_SEEDS=10000 cargo test --release -p la-decoders --test i2c_blocks   # once
```

Expected: all pass. The geometry test names the first block that differs (its band, `from`, `to`); the
round trip names the seed.

## 2. Look at it

1. Run the application, choose the virtual device, add the **I2C** decoder with SCL and SDA on two
   channels, start a capture of about 100,000 samples.
2. Zoom to one transaction. In the row of the decoder: an upper band of blocks side by side without gaps,
   in order: **Start** (blue), **Address 0x.. (write)** (yellow), **ACK** (green), **Data 0x..** (yellow),
   **ACK**, **Stop** (blue); a lower band with one green block, **Write to 0x..**, as wide as the upper blocks
   together.
3. Put a pair marker around the start block, the stop block and one ack block: its two lines sit on the edges
   of the block (the places of the owner's markers).
4. Zoom out step by step: the texts disappear whole when they stop fitting (never cut), the colours stay.
5. Turn on **errors** in the decoder's generator settings: the packets with a NAK on the address or on a
   written byte are red in the lower band and have a red NAK block above; a byte cut short is a data block
   with a heavy outline and a red packet.
6. A read: the address and data blocks are green; the last NAK is red, the packet stays green.
7. Select the nak block of an error: the details say it is an error; select the SCL arrow of that bit: the
   text says which byte was not acknowledged.
8. Change the theme (dark, light, high contrast) and the language: the colours stay distinguishable, the
   texts follow the language.

## 3. What a failure looks like

- A block that is a period too wide or too narrow: compare with R2 of the research; the geometry test shows
  `T` and the edges.
- A packet of the wrong colour: the round trip lists the seed and the planted error it disagrees about.
- Packet block above the parts: the planner test of the bands.
