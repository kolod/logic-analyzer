# WeAct LogicAnalyzer V1

An 8-channel USB logic analyzer built on a Cypress FX2 chip (CY7C68013A or CBM9002A). The application
supports it directly: plug it in, give your user access once (below), and it appears in the **Device**
list as **WeAct LogicAnalyzer V1**.

## What the device can do, and what it cannot

| | |
|---|---|
| Channels | 8, sampled together |
| Sample rates | 20 kHz, 25 kHz, 50 kHz, 100 kHz, 200 kHz, 250 kHz, 500 kHz, 1 MHz, 2 MHz, 3 MHz, 4 MHz, 6 MHz, 8 MHz, 12 MHz, 16 MHz and 24 MHz, exactly. Up to 24 MHz; the 48 MHz some tools offer is not offered (see below) |
| Input levels | 0 to 5.5 V. A level above 2 V reads as high and below 0.8 V as low; the thresholds are fixed and not adjustable. Do not connect anything above 5.5 V |
| Trigger | no trigger. A capture starts when you press Start |
| Memory | none. The board streams every sample straight to the computer |

Because there is no memory, a slow or shared USB path **stops the capture early**: when the computer
does not take the data fast enough the board's own buffer overflows and the firmware ends the capture.
The application then keeps everything captured so far, shows the capture as incomplete and reports
`weact.stream_stopped`; it never presents it as a clean measurement. At 24 MHz the stream is 24 MB/s,
which is close to what a USB 2.0 port can sustain, so a USB hub or a docking station can easily make
it fail: plug the analyzer directly into the computer, or use a lower sample rate. 48 MHz is not
offered because the board cannot stream it.

The indicator lights of the board are not used by the application and their meaning is the seller's;
use the application's own messages to see what is happening.

## Access on Windows 10 and 11

The application opens the board through the **WinUSB** driver. Windows does not choose it for this
board, and a board that was used with another tool may be bound to libusb-win32 or libusbK, which the
application cannot use (it says so with `weact.driver_missing` and names the driver it found).

1. Download **Zadig** from <https://zadig.akeo.ie/> and run it.
2. Plug in the analyzer. In Zadig choose **Options → List All Devices**.
3. Select the analyzer in the list. A board without firmware may be shown as `Cypress FX2` or `Unknown
   Device`; the USB ID is `1D50 608C` (after the application has loaded the firmware the name is
   `fx2lafw`). If the list shows `04B4 8613` instead, the board has not been prepared yet: start the
   application first and let it prepare the board, then return to Zadig.
4. Choose **WinUSB** as the target driver. Replace the existing driver (the arrow shows
   `libusb-win32` or `libusbK` → `WinUSB`) and click **Replace Driver** or **Reinstall Driver**.
5. Unplug the analyzer and plug it in again. Start the application: the board is listed.

Some tools tell you to give the device the name `fx2LAFw`. The application does not need it: it
identifies the board by its USB identity.

## Access on Debian 13

Give your user access with the udev rule, once, no administrator rights are needed afterwards:

```text
sudo cp packaging/linux/60-logic-analyzer-weact.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Unplug the analyzer and plug it in again. The rule lets the user logged in on the machine open the
board (`uaccess`) and members of the group `plugdev`. Without it the application reports
`weact.access_denied`.

## The first connection

The board has no firmware of its own: every time it is powered it waits to be given one. When it is
plugged in the application loads the firmware (it ships with the application; see below) and the board
restarts as the analyzer. This is automatic and takes a second or two. During that time the board is
listed but cannot be used yet (`weact.preparing`: wait a moment and try again). Nothing is written to
the board permanently; unplugging it forgets the firmware.

## Messages and what to do

| Code | What it means | What to do |
|------|---------------|------------|
| `weact.access_denied` | the system does not let the application open the board | Debian: install the udev rule above, replug. Windows: run the application as a normal user with WinUSB installed |
| `weact.driver_missing` | the board is not bound to WinUSB (the message names the driver found) | Install WinUSB with Zadig as above |
| `weact.in_use` | another program (for example PulseView) has the board | Close the other program and try again |
| `weact.firmware_rejected` | the board did not accept the firmware or reports a version this program does not know | Unplug it and plug it in again |
| `weact.not_renumerated` | the board did not come back within 5 seconds after the firmware was loaded | Unplug it, wait a few seconds, plug it in again; try another port or cable |
| `weact.preparing` | the board is still being prepared | Wait a moment and start again |
| `weact.no_data` | no data arrived within 2 seconds of the start | Check the cable, try another USB port, start again |
| `weact.stream_stopped` | the stream stopped early; the capture is incomplete | Use a lower sample rate, avoid hubs and docking stations; everything captured so far is kept |
| `weact.firmware_invalid` | the firmware that came with the program is damaged | Reinstall the application |
| `device.disconnected` | the board was unplugged | Plug it in again; the data captured before is kept and the capture is marked incomplete |
| `device.transport` | a transfer on the USB bus failed | Check the cable and try another port |
| `device.not_found` | the selected device is not there any more | Choose the device again from the list |

## The firmware

The firmware is `fx2lafw` of the open-source **sigrok** project, the file
`fx2lafw-sigrok-fx2-8ch.fw` of the release sigrok-firmware-fx2lafw 0.1.7. It is licensed under the
GNU **GPL** version 2 or later (the helper library inside it under the LGPL 2.1 or later). It ships
inside the application: nothing is downloaded and nothing is read from anywhere else. The licence
texts, the exact release, the checksum and the place of the source are in
`crates/driver-weact/firmware/SOURCE.md`; the application's own notice is in
`docs/third-party-notices.md`. The image is loaded into the analyzer's own processor as data; it is
not part of the program.

## Verifying a board

[weact-verification.md](weact-verification.md) is the step-by-step procedure with the expected result
of each step, and a record template. `rig/arduino-nano/README.md` describes the Arduino Nano pattern
generator used for it.
