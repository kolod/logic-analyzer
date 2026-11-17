# The firmware image of the WeAct LogicAnalyzer V1

The board has no firmware of its own. This application loads the image in this directory into the
board each time it is plugged in (see `specs/013-driver-weact/research.md`, R5 and R6).

| | |
|-|-|
| File | `fx2lafw-sigrok-fx2-8ch.fw` (raw binary, 8,120 bytes; it starts with an 8051 `LJMP` at address 0 and is loaded from address 0) |
| SHA-256 of the file | `b667d878d5455f854bd912704c68cc2cf25702032e72ff825393409890a86e37` |
| What it is | `fx2lafw` for the 8-channel sigrok FX2 board (USB `1d50:608c`), firmware version 1.4 |
| Release | sigrok-firmware-fx2lafw **0.1.7**, 2019-11-14 |
| Taken from | <https://sigrok.org/download/binary/sigrok-firmware-fx2lafw/sigrok-firmware-fx2lafw-bin-0.1.7.tar.gz> (54,529 bytes, SHA-256 `c876fd075549e7783a6d5bfc8d99a695cfc583ddbcea0217d8e3f9351d1723af`) |
| Built from | the git tag `sigrok-firmware-fx2lafw-0.1.7` (commit `b6ec4813b592757e39784b9b370f3b12ae876954`) of <https://github.com/sigrokproject/sigrok-firmware-fx2lafw> |
| Fetched | 2026-11-17 |

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

The complete corresponding source of this image is the repository above at the tag above, built with
`sdcc` 3.4.0 or newer by `./autogen.sh && ./configure && make` (the `.fw` file is the Intel HEX output
converted with `objcopy -Iihex -Obinary`). On request, the maintainers of this repository supply the
same source.

## Checking it

`crates/driver-weact/tests/firmware.rs` computes the SHA-256 of the embedded image and compares it with
the value in this file; change the image and this file together, or the test fails.
