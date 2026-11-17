# Test rig: Arduino Nano pattern generator

A small sketch that turns an Arduino Nano into a pattern generator for the eight inputs of the
WeAct LogicAnalyzer V1. It is the signal source of the manual verification procedure
(`docs/drivers/weact-verification.md`) and of the opt-in hardware tests
(`cargo test -p la-driver-weact --test rig -- --ignored`).

## Wiring

| Analyzer input | Application channel | Nano pin | AVR port bit |
|----------------|---------------------|----------|--------------|
| first (CH1) | 0 | D2 | PD2 |
| second | 1 | D3 | PD3 |
| third | 2 | D4 | PD4 |
| fourth | 3 | D5 | PD5 |
| fifth | 4 | D6 | PD6 |
| sixth | 5 | D7 | PD7 |
| seventh | 6 | D8 | PB0 |
| eighth | 7 | D9 | PB1 |

Join the Nano's **GND** to the analyzer's **GND** with a wire. The Nano's 5 V logic is inside the
analyzer's input range (0 to 5.5 V). Do not connect the Nano's 5 V pin to the analyzer.

The 8-bit value of a pattern is written so that analyzer input *i* is bit *i* of the value, the same
convention as a sample word of the application.

## Loading the sketch

Arduino IDE: open `la_rig/la_rig.ino`, choose the board **Arduino Nano**, the processor
**ATmega328P** (choose **ATmega328P (Old Bootloader)** for most clones), the port, and upload.

`arduino-cli`:

```text
arduino-cli compile --fqbn arduino:avr:nano:cpu=atmega328old rig/arduino-nano/la_rig
arduino-cli upload  --fqbn arduino:avr:nano:cpu=atmega328old -p COM7 rig/arduino-nano/la_rig
```

(Use `cpu=atmega328` for a Nano with the new bootloader.)

## Serial protocol

115200 baud, 8N1, ASCII lines ending in `\n`. Opening the port resets the Nano; after a reset it
prints `LA-RIG 1` once. Every line you send is answered with one line.

| Command | Answer | Effect |
|---------|--------|--------|
| `HELLO` | `LA-RIG 1` | identification and protocol version |
| `STOP` | `OK` | all eight pins low, pattern stopped |
| `HOLD <hex>` | `OK` | the byte `<hex>` (two hex digits) is held on the pins |
| `COUNTER <step_us>` | `OK` | an 8-bit up counter that wraps; changes every `step_us` microseconds |
| `WALK <step_us>` | `OK` | one-hot walk `01, 02, 04 … 80, 01 …`; changes every `step_us` |
| `SQUARE <step_us>` | `OK` | all eight pins toggle together every `step_us` |
| anything else | `ERR` | nothing changes |

`step_us` is a decimal number from 20 to 1,000,000; outside that range the answer is `ERR`. A pattern
starts on the answer and runs until the next command. The sketch reads the serial port between steps,
so a command takes effect within one step. The Nano's clock is independent of the analyzer's, so a
capture is compared with a pattern by its structure (run lengths within a tolerance), never sample for
sample.
