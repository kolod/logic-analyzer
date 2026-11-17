# The firmware image of the WeAct LogicAnalyzer V1

The board has no firmware of its own. This application loads the image in this directory into the
board each time it is plugged in (see `specs/013-driver-weact/research.md`, R5 and R6).

| | |
|-|-|
| File | `fx2lafw-sigrok-fx2-8ch.fw` (raw binary, 8,120 bytes; it starts with an 8051 `LJMP` at address 0 and is loaded from address 0) |
| SHA-256 of the file | `d3cf2a0f583433d0d3976364c26235f179820b3c5c973aa5a5f454135c5ceb4b` |
| What it is | `fx2lafw` for the 8-channel sigrok FX2 board (USB `1d50:608c`), **modified**, firmware version **1.5** (upstream 0.1.7 is 1.4) |
| Release | sigrok-firmware-fx2lafw **0.1.7**, 2019-11-14 |
| Upstream binary (not shipped, for comparison) | <https://sigrok.org/download/binary/sigrok-firmware-fx2lafw/sigrok-firmware-fx2lafw-bin-0.1.7.tar.gz> (54,529 bytes, SHA-256 `c876fd075549e7783a6d5bfc8d99a695cfc583ddbcea0217d8e3f9351d1723af`) |
| Built from | the git tag `sigrok-firmware-fx2lafw-0.1.7` (commit `b6ec4813b592757e39784b9b370f3b12ae876954`) of <https://github.com/sigrokproject/sigrok-firmware-fx2lafw> |
| Fetched | 2026-11-17 |
| Modification | `fx2lafw-stop.patch` in this directory, applied to the tag above (see below) |
| Built with | SDCC 4.5.0 by `build.sh` in this directory |

## What was changed, and why

The patch `fx2lafw-stop.patch` (a `git diff` against the tag above, under the same licence as the file
it changes) makes three changes:

1. The START command no longer waits for ever for the previous acquisition to end by itself. Upstream
   spins in `while (!(GPIFTRIG & 0x80));` in the main loop, which ends only when the host has stopped
   reading and the FIFO is full; a START sent while a capture was being read left the board deaf to every
   request until it was unplugged. The patch aborts the waveform (`GPIFABORT`) and resets the FIFO
   instead, with a bounded wait.
2. The FIFO of endpoint 2 is reset by every START, so samples of an earlier capture cannot appear at
   the start of the next one.
3. A new command `CMD_STOP` (`0xB3`, no data) aborts the acquisition and empties the FIFO.

The minor version number is raised from 4 to 5; the major version (the protocol of START and the
stream) is unchanged, so host programs that know fx2lafw 1.x work with it as before. The driver uses
`CMD_STOP` only when the board reports 1.5 or later.

## Licence

Quoted from the `README` of the release:

> sigrok-firmware-fx2lafw is licensed under the terms of the GNU General Public License (GPL), version
> 2 or later. It uses additional helper code (fx2lib, Hantek 6022BE, and Sainsmart DDS120 firmware),
> licensed under the GNU LGPL (version 2.1 or later). While some individual source code files are
> licensed under the LGPLv2.1+, and some files are licensed under the GPLv2+, this doesn't change the
> fact that the firmware as a whole is licensed under the terms of the GPLv2+.

The licence texts of the release are next to the image: `COPYING` (GPL version 2) and `COPYING.LESSER`
(LGPL version 2.1). The firmware as a whole is GPL-2.0-or-later, which is compatible with the
GPL-3.0-only licence of this project. The image is loaded into another processor; it is shipped with the
program as data and is not linked into it.

## Source

The complete corresponding source of this image is the repository above at the tag above with
`fx2lafw-stop.patch` applied. To build it without autotools, from a scratch checkout of the tag
(`fx2lib` is inside it):

```text
sh crates/driver-weact/firmware/build.sh <checkout> fx2lafw-sigrok-fx2-8ch.fw crates/driver-weact/firmware/fx2lafw-stop.patch
```

`build.sh` follows the steps of the project's `Makefile.am` for `hw/sigrok-fx2-8ch` and, for SDCC 4,
rewrites two spellings of the old compiler in the build copy (`__interrupt NAME` and `__at 0x88+1`).
The image above was built with SDCC 4.5.0 and is not byte-identical to the upstream binary, which was
built with an older compiler.

## Checking it

`crates/driver-weact/tests/firmware.rs` computes the SHA-256 of the embedded image and compares it with
the value in this file; change the image and this file together, or the test fails.
