# Third-party notices

The application is free software under the GNU General Public License, version 3. It includes or
builds on the following. Each keeps its own licence; the texts ship with the sources of each
package and, for the fonts, next to the font file.

## Fonts

| Font | Where | Licence |
|------|-------|---------|
| Fira Sans (Latin, Latin Extended, Greek, Cyrillic) | bundled by the iced toolkit as its default font | SIL Open Font License 1.1 |
| Noto Sans JP, a subset of the regular weight (Japanese) | `crates/app/assets/fonts/NotoSansJP-subset.ttf`, built by `tools/build-font-subsets.py` from the pinned release of github.com/google/fonts | SIL Open Font License 1.1, text in `crates/app/assets/fonts/OFL-NotoSansJP.txt` |

Fonts are used as separate data files. The SIL Open Font License permits embedding them in software
that is distributed under another licence, including the GPL, as long as the fonts are not sold on
their own and the licence and copyright notices travel with them.

## Main libraries

| Library | Used for | Licence |
|---------|----------|---------|
| iced (with wgpu, cosmic-text, winit) | the window, drawing and text | MIT |
| rfd | the native file chooser dialogs | MIT |
| fluent-bundle and related | message formatting of the six languages | Apache-2.0 or MIT |
| serde, serde_json | settings and structured data | MIT or Apache-2.0 |
| directories | where the per-user settings live | MIT or Apache-2.0 |
| tracing, tracing-subscriber | the log | MIT |
| winresource (Windows builds only) | puts the icon into the executable | MIT |

The complete list, with exact versions and licences, comes from `cargo deny list` (the `deny` job
of the CI keeps it within the licences the project allows).

## Icons

The application icon and the session-file icon (`crates/app/assets/icon/`) are original works of the project,
drawn for it, and are under the same licence as the code (GPL-3.0). They contain no third-party artwork or font.

## Firmware of the WeAct LogicAnalyzer

The WeAct LogicAnalyzer V1 has no firmware of its own, so the application ships the open-source firmware
`fx2lafw` of the sigrok project and loads it into the board when it is plugged in
(`crates/driver-weact/firmware/`). The image is the file `fx2lafw-sigrok-fx2-8ch.fw` of the release
sigrok-firmware-fx2lafw 0.1.7, licensed under the GNU GPL version 2 or later (the helper library inside it
under the GNU LGPL version 2.1 or later). The licence texts, the checksum of the image, the exact release
and tag, and the source are recorded in `crates/driver-weact/firmware/SOURCE.md`. The image is loaded into
another processor and is shipped as data; it is not linked into the program.

| Library | Used for | Licence |
|---------|----------|---------|
| nusb | access to the board over USB (pure Rust) | Apache-2.0 or MIT |
| serialport (tests only) | the rig tests talk to an Arduino Nano | MPL-2.0 |
| sha2 (tests only) | a test checks the checksum of the firmware image | MIT or Apache-2.0 |

