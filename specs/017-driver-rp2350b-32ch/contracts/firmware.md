# Contract: the firmware (layout, build, flash, behaviour)

## Layout

```text
firmware/rp2350b/
├── Cargo.toml, Cargo.lock, rust-toolchain.toml, .cargo/config.toml   # its own workspace and toolchain
├── la-fw/          # the board: start-up, clocks, USB serial, the capture engine (PIO, DMA, memory)
├── la-fw-logic/    # no I/O, no_std, tested on the host: request checks, mode and limit tables,
│                   #   sample packing, compact coding, timestamps
├── la-fw-rig/      # the pattern generator of the verification rig
├── THIRD-PARTY.md  # the embedded dependencies with their licences
└── README.md
crates/la-proto/    # the protocol (contracts/protocol.md), shared with the driver, no_std
```

`la-fw` depends on `la-fw-logic` and `la-proto`; `la-fw-logic` has no hardware crate, so its tests run with
`cargo test` on the computer.

## Build and flash (the commands are what `quickstart.md` runs)

```text
cd firmware/rp2350b
rustup target add thumbv8m.main-none-eabihf          # once; the toolchain file pins the version
cargo build --release -p la-fw                       # the ELF
<tool> la-fw → target/…/la-fw.uf2                    # the UF2 for the RP2350 family
```

The tool that makes the UF2 is named in the README and checked by a script that fails if it is missing. To flash:
hold the board's BOOT button while plugging it in (the board appears as a drive, as `D:\` on the owner's
machine, with `INFO_UF2.TXT`), copy the UF2 to it; the board restarts as the analyzer. Without the button: the
application's command 4 (restart into the bootloader).

## Behaviour that is part of the contract

- **Pins.** The 32 inputs are 32 consecutive GPIO of the 48 and two more carry the trigger lines; the exact
  map, the pins used by the external memory, by USB and by the boot button, and the window of 32 (a base of
  0 or 16) are in `docs/drivers/rp2350b.md`, written from the board's schematic in the first task of the
  firmware. **To confirm**: the schematic was not read when this plan was written.
- **Clock and rates.** The firmware sets the system clock for the rates it offers (R5) and reports `FREQ` and
  `BLASTFREQ` from what it measured at build time of the table, not from a guess.
- **Memory.** The internal memory takes every capture; the external memory takes captures below `PSRAMRATE`
  (R6); a request that does not fit is `ERR_NO_MEMORY`.
- **No panic, no hang.** Every input is a frame, an error reply or a request to wait; a command never blocks
  the USB task; a stuck capture is ended by the host's byte; a watchdog restarts a board that stops serving the
  link (the host finds it again as a new connection). A panic handler stores a code and resets.
- **Identity.** The USB descriptors carry a product name that tells it is the 32-channel analyzer and a serial
  number from the chip's unique id; the driver builds the device identity from them.
- **Version.** `FW:` and `PROTO:` in the information reply; the driver supports a list of versions and refuses
  the others with the version it found.

## What the logic crate must test (no board)

Every request field against the limits; the mode for every channel set; the packing of 1 to 32 channels
including all 32 bits and a gap in the numbers; the compact coding round trip for random, constant, sparse and
worst-case signals and its fall back to raw; the timestamps of bursts across the wrap of the counter; the
parser of `la-proto` for the target as well as the host.
