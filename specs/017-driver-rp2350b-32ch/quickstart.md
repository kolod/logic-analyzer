# Quickstart: 32-Channel RP2350B Logic Analyzer

How to check each stage. Contracts: [protocol](contracts/protocol.md), [device abstraction](contracts/device-abstraction.md),
[firmware](contracts/firmware.md).

## 1. Stage 1: wide channels (no hardware)

```text
cargo test -p la-storage -p la-devices -p la-session -p la-host -p la-app
cargo bench -p la-storage        # the benchmark with 160 channels is in the set
```

Expected: a capture of 33 to 160 channels, ingested in any chunking, equals the one built from the channels one
by one; a session of 160 channels is saved and reopened identically; a session of 32 channels or fewer written by
the previous version opens unchanged and a new one is byte for byte the same; the virtual device runs with 160
channels and the interface lists 160 channel buttons (twenty rows of eight) and scrolls.

## 2. Stage 2: the abstraction (no hardware)

```text
cargo test -p la-devices
```

Expected: every new capability has a test that accepts a value at the limit and refuses one past it with the
error named in the contract; every other driver's tests pass unchanged.

## 3. Stage 3: protocol, driver, firmware

Without the board:

```text
cargo test -p la-proto -p driver-rp2350b                       # frames, fuzz, recorded transport, compact coding
cd firmware/rp2350b && cargo test -p la-fw-logic -p la-proto   # the logic of the firmware, on the computer
cd firmware/rp2350b && cargo build --release -p la-fw -p la-fw-rig --target thumbv8m.main-none-eabihf
```

Expected: the tests pass on Windows and Debian; the firmware builds for the target; a test of the 32-channel
packing fails if the eight extra channels are lost or mixed.

With the board (see `docs/drivers/rp2350b-verification.md`, written in this stage):

1. Put the board in its bootloader (BOOT button while plugging in; a drive with `INFO_UF2.TXT` appears) and copy
   the UF2 of `la-fw` to it. The board restarts; the application lists "RP2350B 32-channel analyzer".
2. Flash a second board with `la-fw-rig` and wire its pins to the analyzer's inputs as the document says.
3. Capture all 32 channels: channel `n` shows the rig's pattern `n` and no other.
4. Capture 24 channels at 100 MHz: no lost sample; each trigger type begins within one sample of its pattern;
   burst, blast and abort do what the list of the original's functions says.
5. Capture all 32 channels at the rates the capabilities declare; find and write down the highest that works.
6. Unplug the cable during a capture: it ends, the data is kept and marked incomplete.

## 4. Stage 4: a chain

```text
cargo test -p driver-rp2350b chain      # two simulated boards: alignment, shifts, missing board
```

With two boards wired as the document says: define the chain in the application, wire one rig pin to both
boards, capture, and see the signal on both within one sample (or after the correction).

## What a failure looks like

- A mismatch of the protocol names the field or the byte offset of the first difference.
- A compact coding that does not round trip names the seed and the first sample that differs.
- A wide-channel difference names the channel and the sample position of the first difference.
- A chain that is not aligned shows the shift in samples between its boards.
