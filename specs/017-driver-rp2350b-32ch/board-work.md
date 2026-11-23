# What is left that needs the board, and what it needs from the owner

Written when everything that can be done without the board was in `main` (stages 1 to 4 as far as the computer
goes). The task numbers are those of `tasks.md`; what the board work leaves out of the code, the owner decides.

## What the owner provides

| What | Why | When |
|---|---|---|
| The schematic of the board and the pin list (`RP2350B_SCH.pdf` or a table) | Which 32 GPIO are the inputs, the trigger pins, the external memory pins, USB, the BOOT button, the indicator (T002) | First: the firmware cannot be written without it |
| A second RP2350 board (a Pico 2 will do) and wires | The pattern generator of the rig, `la-fw-rig`: channel `n` a square wave of its own period. The pins take 3.3 V only | Before T052 and T039 |
| A third board (or a second RP2350B) | The chain test, two boards and one rig pin (T065) | At the end |
| The owner at the board | The assistant cannot touch the board: it writes and builds, the owner flashes, runs and sends the output | Throughout |
| The choice of CDC or a vendor-specific bulk interface, and a USB id pair of its own | Decided from the benchmark T076; the development pair of the Pico SDK is enough until then (research R13) | Later, not blocking |
| An unlocked GPG key | Commits are signed | Throughout |

## The order of the work with the board

1. T002, the pin map in `firmware/rp2350b/PINS.md` and `docs/drivers/rp2350b.md`.
2. T038, the first firmware: start-up, USB serial, the answer to command 0. The owner flashes it (BOOT held while
   plugging in, the UF2 copied to the drive). The driver already lists such a board.
3. T052, the rig firmware on the second board.
4. T033 and T039, a capture of all 32 channels against the rig; the time from plugging in to the first capture (SC-006).
5. T044 to T046 and T048, the triggers, the external memory, the compact transfer, and the measurements (the highest
   rate at 8, 16, 24 and 32 channels without a lost sample, the jitter, the DMA and bus load, the memory speed and
   `PSRAMRATE`, SC-003).
6. T049, the steps of `quickstart.md` against the list of the original's functions (T040).
7. T051, T053 and T055, the build from a clean checkout, flashing by the drive, the version seen by the application.
8. T059 and T060, the watchdog and the unplugging during a capture (20 trials, SC-008).
9. T065, two boards on the rig (SC-010).
10. T076, the benchmark of CDC against bulk and the registration of a pair of our own.

## What can be done without the board

The logic that is tested on the computer, the options of the driver, the documents and the reviews; and, if the
embedded target can be installed, the firmware crates written and compiled (not run).
