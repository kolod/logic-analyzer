# Research: WeAct LogicAnalyzer Driver

Decisions with their sources. Facts come from the vendor repository, the sigrok firmware and the
libsigrok `fx2lafw` driver (read on 2026-11-17), and from the maintainer's own machine. What could
not be read or checked is listed at the end as **open items**, each with how it will be settled;
nothing below is guessed silently.

## R1. What the board is, and how it identifies itself

**Decision**: the driver recognises the board by the USB identity `1d50:608c` (spec FR-004). A board
runs the firmware when its USB product string is `fx2lafw` and its manufacturer string is `sigrok` **or
not reported** (libsigrok checks both by reading the descriptors; `nusb` on Windows never reports the
manufacturer, "not cached by the OS", found while writing the backend); otherwise it is **blank** and
must be prepared.

**Rationale**:

- The vendor README: CY7C68013A or its clone CBM9002A, 8 channels, 24 MHz, 0 to 5.5 V, USB 2.0, the
  controller "does not hold the firmware", the host loads it. The sigrok wiki lists the board under
  `fx2lafw` as `1d50:608c`, 8 channels.
- **Observed on the maintainer's machine** (Windows, 2026-11-17): the board is present as
  `USB\VID_1D50&PID_608C` with hardware ID revision `REV_0000`, the bound driver is `libusb0`
  (libusb-win32 1.2.6.0, installed that day). The firmware descriptor sets `bcdDevice` 0x0100, so
  `REV_0000` shows a board **without firmware** that gets its VID/PID from its own EEPROM. This
  settles the open question of the first study (the board's identity before the upload is the same
  `1d50:608c`) and confirms that accepting only this identity is enough.
- libsigrok's table has the same ID before and after renumeration for this profile
  (`"fx2grok-flat (before and after renumeration)"`), so the string check, not the ID, tells the two
  states apart.

**Alternatives considered**: accept the blank Cypress identity `04b4:8613` too (rejected by the
clarification: it would match any FX2 board).

**Conflict left in the sources**: the README says the EEPROM is 2 Kbit, the sigrok wiki lists a
256 Kbit part. It does not change the design: the identity is read from the bus, not assumed.

## R2. The protocol of the firmware

**Decision**: implement exactly what the firmware does, from `contracts/usb-protocol.md`:

- Vendor control requests, device recipient: `0xB0` read 2 bytes (firmware version major, minor;
  version 1.4 in the source; the driver requires major 1), `0xB2` read 1 byte (revision, logged),
  `0xB1` write 3 bytes (`flags`, `delay_hi`, `delay_lo`) to start; `wValue` = `wIndex` = 0. Timeout
  100 ms in libsigrok.
- `flags`: bit 6 selects the 48 MHz clock (0 = 30 MHz), bit 5 the 16-bit width (0 for this board),
  bit 4 the analog line (0).
- Samples come from bulk IN endpoint `0x82` (512-byte packets at high speed), one byte per sample,
  bit `i` of the byte is input `i` (`D0` to `D7`; PB0 to PB7 on the chip).
- **Delay rule** (from libsigrok): if `48 MHz % rate == 0` then clock 48 MHz and
  `delay = 48 MHz / rate - 1`, and if that is above 1536 (`MAX_SAMPLE_DELAY = 6 * 256`) it is dropped
  and the 30 MHz rule is tried: `delay = 30 MHz / rate - 1` when `30 MHz % rate == 0`. The firmware
  ignores a start command whose `delay_hi >= 6`, silently.
- There is **no stop command**: the host stops reading, the buffer fills, the firmware ends the
  capture itself (R8).
- The firmware starts when the **first IN request arrives after the start command**: the host must
  have IN transfers submitted before it sends `0xB1` (libsigrok submits first, then commands).

**Rationale**: it is the only definition of the protocol; the vendor repository has none.

**Derived table** (exact for every listed rate; verified against the rule above): clock 48 MHz for
every rate from 50 kHz up to 24 MHz with delays 959, 479, 239, 191, 95, 47, 23, 15, 11, 7, 5, 3, 2, 1
(for 50, 100, 200, 250, 500 kHz, 1, 2, 3, 4, 6, 8, 12, 16, 24 MHz); clock 30 MHz for 20 kHz (delay
1499) and 25 kHz (delay 1199) because their 48 MHz delay exceeds 1536. Every one of the 16 rates is
reached **exactly** (no rounding), so the configuration never reports a different rate (FR-007 is
met by exactness; the field is still filled in).

## R3. USB library

**Decision**: `nusb` 0.2 (pure Rust; Linux usbfs, Windows WinUSB; Apache-2.0 OR MIT; edition 2021,
`rust-version = 1.85`, equal to the workspace MSRV; the Context7 documentation lists blocking calls:
`list_devices`, `DeviceInfo` with `bus_id`, `device_address`, `port_chain`, vendor/product IDs and the
strings, `Device::control_in/out(.., timeout).wait()`, `Interface::endpoint::<Bulk, In>(addr)`,
`Endpoint::submit`, `wait_next_complete(timeout)`, `cancel_all`, `clear_halt`, typed error kinds
`Disconnected`, `Busy`, `PermissionDenied`, `NotFound`, `Unsupported`).

**Rationale**: no C library (Principle V), the API is safe so the workspace's `unsafe_code = deny`
holds, the error kinds map onto the errors of FR-023, and a timeout on a bulk wait fits the contract's
50 ms `read`.

**Alternatives considered**: `rusb` (binds libusb-1.0: needs the C library built or installed on both
systems and a system package on Debian; also unsupported on Windows for libusb-win32) and the direct
OS interfaces (`unsafe`). Rejected.

**Risk to settle in the first task**: the transitive dependencies of `nusb` must build on 1.85 and
pass `cargo deny`; a spike compiles an empty use of it on both systems and in the MSRV job before any
other code depends on it.

**Result of the spike (task T002, 2026-11-17, Windows 11)**: `nusb` 0.2.9 and `serialport` 4.10.1 build
with `cargo +1.85 check -p la-driver-weact --all-targets` and the crate passes clippy with `-D warnings`.
(`cargo deny` is not installed on the development machine; it runs in CI, and the licences of the new
crates are MIT or Apache-2.0 and MPL-2.0, all in the allowed list.) `examples/probe.rs` lists the bus with
what `nusb` reads without opening a device, and showed on the maintainer's machine:

- `DeviceInfo::driver()` exists on Windows and returned `Some("libusb0")` for the board and
  `Some("FTDIBUS")` for the Nano's FTDI chip: the **bound driver name is available** (open item 3 is
  settled), so `weact.driver_missing` names it.
- The blank board shows `manufacturer None | product None | serial None`, `rev 0000`, class `ff`: it has
  **no strings at all**. The rule "the strings are `sigrok` / `fx2lafw` ⇒ the firmware runs, anything
  else ⇒ blank" holds.
- `bus_id()` is a **string**, not a number: on Windows the path of the host controller
  (`PCIROOT(0)#PCI(1400)#USBROOT(0)`), on Linux the bus number as text. The `BoardKey` is therefore a
  string (the bus id as given) and the chain of port numbers (`[13, 2]` here: port 2 of the hub on root
  port 13); it must be built and compared as text.
- **Control transfers on Windows go through the interface**: in `nusb` 0.2.9 `Device::control_in/out`
  exist only on Linux, macOS and Android; on Windows they are methods of `Interface`. The backend therefore
  claims interface 0 when it opens a board and sends every control transfer (the download too) through it,
  on every platform, as libsigrok does when it opens. Whether the blank board exposes interface 0 is
  confirmed on the real board (T038).
- The board here sits behind a hub (`05e3:0608` on root port 13). Whether that hub keeps the board from
  24 MHz is checked in the manual procedure, not assumed.

## R4. The Windows driver

**Decision**: the documented and supported driver on Windows is **WinUSB**, installed with Zadig
(select the board, choose WinUSB, Install). The vendor's README instruction to rename the device to
`fx2LAFw` in Zadig is kept as a convenience (it only names the entry).

**Rationale**: `nusb` on Windows opens only devices bound to WinUSB. The maintainer's board is bound
to **libusb-win32**, which is not WinUSB: replacing it is the first step of the manual procedure, and
the driver reports this state with its own error (`weact.driver_missing`) that names the driver found
(when `nusb` gives it, else the open error) and the fix. The application does not install drivers
(spec assumption).

**Alternatives considered**: supporting libusb-win32 or libusbK (no safe Rust crate; sigrok's own
libusb-1.0 does not support libusb-win32 either).

## R5. The firmware image: source, form, licence

**Decision**: take the published **binary release 0.1.7** of sigrok-firmware-fx2lafw (the newest in
the download directory, 2019-11-14, 53 KB tarball) and keep `fx2lafw-sigrok-fx2-8ch.fw` (raw binary,
converted from Intel HEX at build time with `objcopy -Iihex -Obinary`) in the repository, embedded
with `include_bytes!`. Beside it: `SOURCE.md` (the release, the tag, the SHA-256, the address of the
source and an offer to supply it), `COPYING` (GPL-2.0) and `COPYING.LESSER` (LGPL-2.1, for fx2lib).
A test checks the SHA-256 and that the size is within the 16,384 bytes of program memory of the chip.
(Checked at fetch time: the image is 8,120 bytes and starts with an 8051 `LJMP` at address 0, so it is
loaded from address 0; the `0x1c00` of the linker flags is the size of the code area the firmware is
linked for, not the size of the file.)

**Rationale**: the README states "GPL version 2 or later" for the whole firmware (fx2lib inside it:
LGPL-2.1 or later), compatible with the project's GPL-3.0; the image is loaded into another
processor, so it is data shipped with the program, not linked code. Building it needs `sdcc` and an
autotools chain; a pinned published build is simpler and reviewable. Embedding means the image cannot
be missing or damaged at run time; the checks of spec FR-010 are the compile-time checksum test plus
a cheap run-time sanity check (non-empty, within the code size) that the tests drive through the seam
with a bad image.

**Alternatives considered**: build from source in the build script (heavy toolchain, not reproducible
on both systems); read the firmware from a sigrok installation (rejected by the clarification).

**Open item**: the SHA-256 and the exact tag name are recorded when the tarball is downloaded in the
first task (the tarball was not downloaded during planning); a task verifies the licence texts inside
it.

## R6. Loading the firmware into the FX2

**Decision**: the standard Cypress FX2 download, written with control transfers of the vendor type to
the device: hold the 8051 in reset by writing `0x01` to the CPUCS register `0xE600`, write the image to
internal RAM from address 0 in chunks of at most 4,096 bytes with request `0xA0` and `wValue` equal to
the address, release the reset by writing `0x00` to `0xE600`. The board then leaves the bus and returns
with its new descriptors (renumeration).

**Rationale**: this is how the FX2's built-in loader works (the `0xA0` "firmware load" request is
handled by the chip's ROM) and how `fxload` and libsigrok's `ezusb` code do it. The libsigrok source
file that implements it (`ezusb.c`) could not be fetched during planning (the path returned 404), so
the chunk size and the exact register values come from the Cypress documentation as known to the
author and are **to be confirmed against the real board**: the first rig run prepares a blank board
and checks that the strings become `sigrok`/`fx2lafw`, the revision `0100` and that the version
request answers 1.x.

**Timing** (from libsigrok's `dev_open`): at least 300 ms for the board to leave the bus, then poll
every 100 ms for it to return, up to `MAX_RENUM_DELAY_MS` = 3000 ms. The driver waits 300 ms, polls
every 100 ms, and allows **5 s** in total (a slower machine, a hub) before `weact.not_renumerated`.

## R7. Listing, identity and preparation

**Decision**:

- `scan()` lists the USB bus with `nusb`, keeps the entries with `1d50:608c`, updates a table of
  boards and returns descriptors from that table. It never uploads and never waits.
- A board's identity is `weact:<bus id>-<port chain>` (its position on the bus). Re-enumeration
  changes the device address but not the position, so the identity is stable (spec FR-002); two
  boards on two ports have two identities.
- A blank board found by a scan is handed to the **preparation worker** (a contained worker of
  `la-diagnostics`): upload, wait for return, check the version. While it works the board stays in the
  table as `Preparing` and **stays listed** even when it is off the bus, for at most the allowed
  time, so the list does not flicker and a selection survives.
- `open()` on a board that is not `Ready` returns the typed reason: `weact.preparing`, or the stored
  failure (`weact.not_renumerated`, `weact.firmware_rejected`, `weact.access_denied`,
  `weact.driver_missing`, `weact.in_use`); reading a stored failure clears it so a retry prepares again.
- A failed board stays listed while it is on the bus (and for 30 s after it vanished during
  preparation) so that the failure can reach the user through `open()`; a board that is gone and has
  no failure to report is removed.

**Rationale**: the contract gives `scan` one second and `start` two, preparation takes seconds, and
the user must see the cause of a failure even though a provider cannot push a notice.

## R8. Streaming, stopping, and what "lost samples" means for this board

**Decision**:

- `start`: drain stale data left by a previous capture (R8 below), create the transfers (buffer size
  `10 ms * rate` bytes rounded up to a multiple of 512, at most 32 in flight, covering at least
  100 ms; libsigrok uses the same shape), **submit them first**, then send `0xB1`.
- `read`: wait up to 40 ms for a completed transfer; turn its bytes into words (bit `i` = input `i`,
  disabled channels masked to zero); count samples; honour the set length by truncating the last chunk
  and returning `Finished`; return `Idle` when nothing came; resubmit the buffer.
- **The firmware stops the capture when the computer cannot keep up** (the buffer fills; the firmware
  resets it and waits for a new start; no lost-sample counter, no signal). The driver therefore sees
  *empty or failed transfers*. After the board has been silent for a bounded time (the larger of 2 s
  and four transfer periods; 2 s from the start for the first data) the driver ends the capture:
  for a set length it returns `Lost(length - received)` once, so the books balance, and then the typed
  error `weact.stream_stopped` (`weact.no_data` if nothing at all came); for an open-ended capture only
  the error. This is the amendment of spec FR-017.
- `stop`: cancel the transfers, wait a bounded time for them to return, release the interface. No
  command is sent (there is none).
- **Stale data**: after a cancelled capture the board can hold up to 4 KB (quad-buffered 1 KB) of old
  samples until its buffer fills and the firmware resets it; at 20 kHz that takes about 200 ms. The next
  `start` therefore first drains the endpoint with short waits until the stream has been quiet for two
  packet periods (bounded by 500 ms), so old samples never enter a new capture.

**Rationale**: it follows the firmware's behaviour as read from `gpif-acquisition.c`; the alternative of
inventing mid-capture gaps would claim knowledge the board does not give.

## R9. Capabilities

**Decision**: 8 channels; sample rates as the **list** `20 k, 25 k, 50 k, 100 k, 200 k, 250 k, 500 k,
1 M, 2 M, 3 M, 4 M, 6 M, 8 M, 12 M, 16 M, 24 M` Hz; no trigger; no buffer depth (streaming);
`Flow::Realtime`.

**Rationale**: those are exactly the rates the firmware can produce from its two clocks with delays in
range (R2); 48 MHz is in libsigrok's list but is above the 24 MHz the vendor states and the board's
8-bit path is specified for; the 16-bit width does not exist on this board. A list (not a range) because
a range would admit rates the board cannot make.

## R10. Error codes (new, domain `weact`, kind Device)

| Code | When | Advice in the message |
|------|------|-----------------------|
| `weact.access_denied` | the OS refuses to open the board (Linux permission) | install the udev rule, or replug after adding it |
| `weact.driver_missing` | Windows: the board is not bound to WinUSB | install WinUSB with Zadig; names the driver found |
| `weact.in_use` | another program holds the board | close the other program (for example PulseView) |
| `weact.firmware_rejected` | the board answered the version request with a major other than 1, or refused the image | replug the board |
| `weact.not_renumerated` | the board did not come back within 5 s after the upload | unplug and plug it again |
| `weact.preparing` | `open` on a board still being prepared | try again in a moment |
| `weact.no_data` | no data arrived within 2 s of the start | check the USB connection, try another port |
| `weact.stream_stopped` | the stream stopped before the capture was complete | lower the rate, avoid hubs and docking stations |
| `weact.firmware_invalid` | the image fails the sanity check (empty, larger than the chip's program memory (16 KB)) | reinstall the application |

The existing codes are reused where they mean the same: `device.disconnected` (unplug),
`device.transport` (a transfer failed), `device.not_found` (not a board of this driver), `device.busy`
(second acquisition). The codes live in the crate's `error.rs`; the host's error-code test finds them
there, and the parity test requires all six languages.

## R11. The test seam and recorded exchanges

**Decision**: `UsbBackend` (`list`, `open`) and `UsbLink` (`control_in`, `control_out`, a bulk stream
with `submit`, `wait_next_complete(timeout)`, `cancel_all`, `release`) are traits of the driver; `nusb`
implements them in one file; `RecordedUsb` (feature `test-support`) implements them from a script of
steps (control answer, bulk chunk, short chunk, timeout, disconnect, error kind, a board that appears,
disappears or renumerates after a given number of calls, a hang, a panic) and logs what the driver
sent. A recording is a **text file** of those steps, readable and diffable; `RecordingUsb`, a wrapper
around the real link used by the rig tests when `LA_RIG_RECORD=<dir>` is set, writes new recordings
from real hardware. Until a real board has been recorded the first recordings are written by hand from
the protocol facts and say so in their header.

**Rationale**: the harness `Transport` is a byte stream (no control transfers); a driver-level seam
tests the whole driver including preparation and renumeration, which is where the board differs from
other devices.

## R12. The rig (Arduino Nano) and the opt-in tests

**Decision**: the Nano is wired to the eight inputs as in the clarification (D2..D7 = PD2..PD7 to inputs
0..5, D8 = PB0 and D9 = PB1 to inputs 6 and 7, so input `i` is bit `i` of an 8-bit value that the sketch
writes to `PORTD` bits 2..7 and `PORTB` bits 0..1). The sketch has no timing dependence on the host:
it runs a pattern on its own and is told over the serial port which pattern, its step and when to
stop (`contracts/rig.md`). The tests are `#[ignore]`d and read the port from `LA_RIG_PORT`; without it
they print why they were skipped and pass.

**Observed on the maintainer's machine**: the Nano is an FTDI-based clone (`0403:6001`, "USB Serial
Port (COM7)"). Opening the port toggles DTR, which **resets** a Nano with an auto-reset circuit and the
bootloader runs for about 1 to 2 s; the test waits for the sketch's greeting (`LA-RIG 1`) with a
bounded timeout instead of a fixed sleep, and drops DTR where the port allows it.

**Timing**: the Nano's clock is independent of the board's; the tests compare structure (the order of
values, the one-hot walk, the run lengths within a tolerance of -20 % to +25 % of `step * rate`) not
sample-for-sample. `delayMicroseconds` is accurate to a few microseconds, so steps below 20 µs are not
used; the fastest pattern the rig can drive is about 100 kHz per step, so 24 MHz is verified in the
manual procedure with a source that can reach it (the board's own test pad or a generator), not with the
rig.

**Serial crate**: `serialport` 4, dev-dependency, `default-features = false` (no `libudev` on Linux; on
Windows it uses the Win32 API). MPL-2.0 is in the allowed list of `deny.toml`.

## R13. Access on Debian

**Decision**: a udev rule `packaging/linux/60-logic-analyzer-weact.rules`
(`SUBSYSTEM=="usb", ATTRS{idVendor}=="1d50", ATTRS{idProduct}=="608c", TAG+="uaccess"`, plus
`MODE="0660", GROUP="plugdev"` for sessions without a seat), described in `docs/drivers/weact.md`; the
error `weact.access_denied` points to it. The vendor's `setup_linux.sh` was not read and is not used.

## R14. Composition

**Decision**: the application registers the provider: `Shell::add_provider` (today test-only) becomes a
normal method and `la-app` calls it with `la_driver_weact::provider()` at start-up. `la-host` does not
depend on the driver crate (Principle II: the host knows only the abstraction).

## Open items (to be settled during implementation, each by a named task)

1. ~~SHA-256 and exact tag of the firmware 0.1.7 image; licence texts~~: settled by T003, recorded in
   `crates/driver-weact/firmware/SOURCE.md` (tag `sigrok-firmware-fx2lafw-0.1.7`, the licence text of the
   release confirms GPL-2.0-or-later with LGPL-2.1-or-later helper code).
2. Chunk size and register values of the FX2 download, and the meaning of the sample delay at its lowest
   values (0 or 256 clocks in the GPIF delay encoding): confirmed on the real board with the rig
   (a 1 MS/s capture of a 100 µs step must show about 100 samples per step; each rate of the table is
   checked).
3. ~~Whether `nusb` reports the bound Windows driver name~~: settled by T002, it does (`libusb0` seen).
4. ~~That the transitive dependencies of `nusb` build on Rust 1.85~~: settled on Windows by T002; Debian
   and `cargo deny` are checked by the CI of the first pull request.
5. The vendor's `Tools` folder (`WeAct LogicAnalyzer Write.exe`, `setup_linux.sh`) was not read; they
   are not needed.
