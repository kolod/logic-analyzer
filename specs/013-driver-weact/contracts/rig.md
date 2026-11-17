# Contract: The Test Rig (Arduino Nano) and the Opt-in Hardware Tests

The maintainer's local rig: the WeAct analyzer wired to an Arduino Nano that produces known patterns on
the eight inputs. It is the signal source of the manual procedure and of automated tests that the
test code controls over the Nano's serial port (spec clarification of 2026-11-17, FR-030, FR-031).

## Wiring

| Analyzer input (board label) | Application channel | Nano pin | AVR port bit |
|------------------------------|---------------------|----------|--------------|
| first (CH1 in the maintainer's counting) | 0 (`D0`) | D2 | PD2 |
| second | 1 | D3 | PD3 |
| third | 2 | D4 | PD4 |
| fourth | 3 | D5 | PD5 |
| fifth | 4 | D6 | PD6 |
| sixth | 5 | D7 | PD7 |
| seventh | 6 | D8 | PB0 |
| eighth | 7 | D9 | PB1 |

A common ground wire joins the Nano's GND to the analyzer's GND. The Nano's 5 V logic is within the
analyzer's 0 to 5.5 V input range (high above 2 V, low below 0.8 V). The Nano is an FTDI-based clone
(`0403:6001`); on the maintainer's machine it is **COM7**.

The 8-bit value `v` of a pattern is written as: `PORTD` bits 2 to 7 = `v` bits 0 to 5, `PORTB` bits 0 and
1 = `v` bits 6 and 7. So input `i` is bit `i` of `v`, the same convention as a sample word.

## Serial protocol (115200 baud, 8N1, ASCII lines ending `\n`)

The sketch answers every line with one line. It prints `LA-RIG 1` once when it starts (after a reset).

| Command | Answer | Effect |
|---------|--------|--------|
| `HELLO` | `LA-RIG 1` | identification and protocol version |
| `STOP` | `OK` | all eight pins low, pattern stopped |
| `HOLD <hex>` | `OK` | the byte `<hex>` (two hex digits) is held on the pins |
| `COUNTER <step_us>` | `OK` | an 8-bit up counter that wraps; the value changes every `step_us` microseconds |
| `WALK <step_us>` | `OK` | one-hot walk `01, 02, 04, … 80, 01, …`; the value changes every `step_us` |
| `SQUARE <step_us>` | `OK` | all eight pins toggle together every `step_us` (a clock-like check of every input) |
| anything else | `ERR` | nothing changes |

`step_us` is a decimal number from 20 to 1,000,000; outside it the answer is `ERR`. While a pattern runs
the sketch polls the serial port between steps with interrupts off during a step, so a command takes
effect within one step. The pattern starts on the answer and runs until the next command; a **reset**
(opening the port) stops it, so the tests always start from `STOP` and end with `STOP`.

## What the tests check (all opt-in)

For each pattern, a capture at a rate whose period divides the step cleanly (for example 1 MS/s with a
step of 100 µs) is compared with the pattern **by structure**:

- `WALK`: every captured value has exactly one bit set, the values follow the cycle, each of the eight
  channels is seen, run lengths are within -20 % to +25 % of `step * rate`.
- `COUNTER`: consecutive different values differ by +1 modulo 256, run lengths within the same
  tolerance, and all 256 values appear over a long enough capture.
- `HOLD`: the captured value is constant and equal to the byte.
- `SQUARE`: every channel toggles together, the half period within the tolerance.
- Rates: the table of the protocol is exercised for the rates the rig can drive (the step must be at
  least 20 µs and a multiple of the sample period within the tolerance).
- Failure and recovery: unplugging is not automated; a capture stopped by the test leaves the board
  ready for the next one (two captures in a row give the same structure).

The Nano's clock is independent of the board's, so sample-for-sample comparison is not asserted. The
maximum rate of 24 MS/s is not claimed by the rig (the Nano cannot drive it); it is part of the manual
procedure with a faster source.

## Opt-in rules (FR-031)

- The tests are `#[ignore]`d. They run only with `cargo test -p la-driver-weact --test rig -- --ignored`.
- The serial port is read from the environment variable `LA_RIG_PORT` (for example `COM7`, or
  `/dev/ttyUSB0`); nothing is fixed in the code.
- With the variable missing, or no board, or no answer to `HELLO` within 5 s, every test prints
  `SKIPPED: <reason>` and passes.
- The tests never run in CI (the workflow does not pass `--ignored`) and never run by default.
- Every test ends with `STOP` on the Nano and the analyzer idle, also after a failure (a guard value
  that does this when dropped).
- `LA_RIG_RECORD=<directory>`: the real USB exchange of a test is also written there as a recording
  (research R11), to become a replayable fixture after review.

## The sketch

`rig/arduino-nano/la_rig/la_rig.ino`: configures D2 to D9 as outputs, implements the protocol above with
direct port writes and `delayMicroseconds`, reads the serial port by registers between steps; about 150
lines. `rig/arduino-nano/README.md` has the wiring table, how to load the sketch (Arduino IDE or
`arduino-cli`, board "Arduino Nano", the old bootloader for clones), and the serial protocol. The
documentation is in English.
