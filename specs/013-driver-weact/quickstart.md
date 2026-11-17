# Quickstart: Validating the WeAct Driver

Four kinds of check, from the one that needs nothing to the one that needs the rig. Each says what to
run and what the outcome must be. The contracts have the details: [usb-protocol](contracts/usb-protocol.md),
[driver-behaviour](contracts/driver-behaviour.md), [rig](contracts/rig.md).

## 1. No hardware (CI does this on Windows and Debian)

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo +1.85 check -p la-driver-weact --all-targets        # MSRV
```

Expected: everything passes with no board attached. The driver's tests replay scripted exchanges:

- `protocol`: the payload of each of the 16 rates equals the table; any other rate is an error; a
  control answer of the wrong length, a version of another major, and random bytes never panic.
- `provider`: one board is listed once with a stable identity; two boards, two identities; a board that
  renumerates keeps its identity and is never listed twice; 100 rapid plug and unplug events leave the list
  equal to what is present; a bus that cannot be read does not fail the application.
- `preparation`: a blank board is prepared (upload, return, version) and becomes ready; an already running
  board is used without an upload; a board that never returns gives `weact.not_renumerated` after the
  allowed time; access denied, no WinUSB and in use each give their own code; a board answering another
  major version gives `weact.firmware_rejected`; an empty or oversized image gives `weact.firmware_invalid`
  and sends nothing.
- `acquisition`: a replayed capture of a set length equals the signal fed in sample for sample; disabled
  channels are zero; the last chunk is truncated; `Finished` comes after the last sample; stop closes the
  capture within 100 ms; two captures in a row work; stale bytes before a start never enter a capture.
- `failures`: unplug at every step, a short read, a timeout, a stall, a stream that stops (the books balance:
  received plus lost equals requested, then `weact.stream_stopped`), nothing at all (`weact.no_data`), a
  panic, a hang: each leaves the application responsive and the data kept.
- `conformance`: `la_devices::harness::conformance::run_all` passes against the driver.
- `firmware`: the embedded image has the recorded SHA-256 and fits the 7,168-byte code space.
- The error codes are in all six languages (the parity test) and have advice in their text.

## 2. The Windows driver (once per machine)

The maintainer's board is bound to **libusb-win32**; `nusb` needs **WinUSB**.

1. Plug in the board. In Zadig (Options → List All Devices) select the entry with `1d50:608c`.
2. Choose **WinUSB** as the target driver (not libusb-win32 or libusbK) and press *Replace Driver* (or
   *Install Driver*). Keeping the name `fx2LAFw` is optional.
3. Check: `Get-PnpDeviceProperty -InstanceId '<the instance>' -KeyName DEVPKEY_Device_Service` says
   `WinUSB`.

Expected: the application lists "WeAct LogicAnalyzer V1" and a capture starts. Before step 2 the
application lists it but Start gives `weact.driver_missing` naming the driver it found and the steps above.

## 3. First real run (a board, no rig needed)

1. Start the application with the board plugged in and unused since plugging in (blank).
2. The board appears in the device list within about two seconds, without a dialog.
3. Select it; set 8 channels, 1 MHz, 100,000 samples; press Start.

Expected: the capture runs and finishes; the session records the device and is not marked simulated; the
board's yellow LED lights when the firmware is loaded; unplug and plug again and repeat without restarting
the application. Check in the log (`info`): the preparation time, the firmware version `1.x`, the capture
counts. If the firmware version or the strings differ from the contract, stop and amend the contract
(the **[verify]** items).

## 4. The rig (opt-in, the maintainer's machine)

Wiring and loading the sketch: `rig/arduino-nano/README.md`. Then:

```text
set LA_RIG_PORT=COM7
cargo test -p la-driver-weact --test rig -- --ignored --nocapture
```

(PowerShell: `$env:LA_RIG_PORT = 'COM7'`; Debian: `LA_RIG_PORT=/dev/ttyUSB0 cargo test …`.)

Expected: every pattern test passes for all eight channels (the one-hot walk reaches every channel; the
counter steps by one; run lengths within the tolerance; two captures in a row alike) and each test ends
with the Nano stopped. Without `LA_RIG_PORT` every test prints `SKIPPED` and passes. To keep the exchange
as a fixture: also set `LA_RIG_RECORD=<directory>` and review the files before adding them to
`tests/recordings/`.

## 5. The manual procedure (before a release, each operating system)

`docs/drivers/weact-verification.md`: list, configure, capture at several rates including 24 MHz with a
source that can reach it, all channels, a known signal, stop, unplug during a capture, replug, a hub or
docking station (the capture must end early with `weact.stream_stopped` and be flagged incomplete, never
look clean), another program holding the board (`weact.in_use`), a clean Windows 11 and a clean Debian 13
following only the documentation (under 10 minutes). The record of the run is attached to the pull
request.
