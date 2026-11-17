# WeAct LogicAnalyzer V1: manual hardware verification

This is the procedure the maintainer follows with a real board before the driver is accepted, and
that anyone can repeat. Every step has an **expected result**. Fill in the record template at the end
and attach it to the pull request.

The board setup and the access rules (WinUSB on Windows, the udev rule on Linux) are in
[weact.md](weact.md). The pattern generator used below is the Arduino Nano rig described in
`rig/arduino-nano/README.md`; any other known signal source works for the steps marked *(any source)*.

## Setup

1. Wire the rig: analyzer input 1 to D2, 2 to D3, 3 to D4, 4 to D5, 5 to D6, 6 to D7, 7 to D8, 8 to D9
   (application channels 0 to 7), and the grounds together.
2. Load `rig/arduino-nano/la_rig/la_rig.ino` into the Nano. Open a serial terminal at 115200 baud and
   type `HELLO`. **Expected:** `LA-RIG 1`.
3. Start the application with the analyzer plugged in **without** any other program using it.

## Checks

| # | Step | Expected result |
|---|------|-----------------|
| 1 | Open the device list. | One entry, **WeAct LogicAnalyzer V1**. A board that has just been plugged in (no firmware) shows up at once and becomes usable within a few seconds, without being listed twice. |
| 2 | Select it and open the capture settings. | 8 channels; sample rates from 20 kHz to 24 MHz; no trigger; no capture-length limit from the board. |
| 3 | In the terminal send `WALK 100`. Capture 20,000 samples at 1 MHz, all eight channels. | One channel high at a time, in the order 1, 2, … 8, each for about 100 samples; all eight channels are seen. |
| 4 | Send `COUNTER 100`. Capture 300,000 samples at 1 MHz. | The eight channels read as an 8-bit counter that counts up and wraps; no value is skipped. |
| 5 | Send `HOLD 55`, then `HOLD AA`. Capture at 1 MHz each time. | Channels 1, 3, 5, 7 high and 2, 4, 6, 8 low, then the opposite. |
| 6 | Capture at **20 kHz**, **1 MHz**, **12 MHz** and **24 MHz** with a signal that changes slowly enough for the rate. The rig reaches 1 MHz; use another source *(any source)* for the others. | Every capture completes with the length asked for and is marked complete. The 24 MHz capture completes on a direct port. |
| 7 | Start an open-ended capture, then press Stop. | The capture ends within about a tenth of a second, keeps what was captured and is marked as stopped by the user. |
| 8 | Start a long capture and **unplug the board** during it. | An error that says the board was disconnected; the samples captured before the unplug are kept; the capture is marked incomplete. |
| 9 | Plug the board in again. | It is listed again within a few seconds. A new capture works **without restarting** the application. |
| 10 | Connect the board through a **hub or docking station** and capture at 24 MHz. | Either the capture completes, or it ends early with the message that the stream stopped, is flagged incomplete and the missing part is shown as lost. It never looks clean when samples are missing. |
| 11 | Open the board in **PulseView** (or another program) and try to capture in the application. | An error that another program is using the board (`weact.in_use`); no capture is created. Close the other program: it works. |
| 12 | *(Windows)* With the board still on the old libusb-win32 driver, try to capture. | An error that the board needs the WinUSB driver, naming the driver in use (`weact.driver_missing`). After installing WinUSB (see weact.md) it works. |
| 13 | *(Debian)* Without the udev rule, try to capture. | An error that access was denied (`weact.access_denied`) with advice about the rule. After installing the rule and replugging, it works. |
| 14 | On a **clean Windows 11** and a **clean Debian 13**, follow only `docs/drivers/weact.md`. | A new user reaches a first capture; note the time it took. |

## The automated version of steps 3 to 5

With the Nano on its serial port, from the repository:

```text
set LA_RIG_PORT=COM7
cargo test -p la-driver-weact --test rig -- --ignored --nocapture
```

On Debian use `LA_RIG_PORT=/dev/ttyUSB0`. Without the variable every test prints `SKIPPED` and passes.
`LA_RIG_RECORD=<directory>` also writes the USB exchange of each test as a recording
(`tests/recordings/`), which can be added to the replay tests after review.

## Record template

Copy this into the pull request and fill it in.

```text
Date:
Operating system and version:
Application version (commit):
Board (silkscreen / seller):
USB port (direct / hub / dock):
Rig used (Nano on COM?/tty?):

Step 1  result:
Step 2  result:
Step 3  result:
Step 4  result:
Step 5  result:
Step 6  20 kHz / 1 MHz / 12 MHz / 24 MHz:
Step 7  result:
Step 8  result:
Step 9  result:
Step 10 result:
Step 11 result:
Step 12 result:
Step 13 result:
Step 14 Windows 11 time / Debian 13 time:
Automated rig run (pass / fail / skipped):

Facts to confirm for the notes of the driver (change them in the contract if they differ):
- download chunk size that the board accepted:
- driver name reported for the blank board and for the running board:
- whether the blank board accepts control transfers through interface 0 on Windows:
- product string of the running board as shown by the system:

Notes:
```
