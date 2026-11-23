# Research: 32-Channel RP2350B Logic Analyzer

Decisions that the plan rests on. What could not be confirmed from sources read here is marked **to measure** or
**to confirm**; the tasks put a step for each. Nothing is left that blocks the design.

## R1. Sample words and channel sets for up to 160 channels

**Findings** (code read): a sample is one `u32` with a bit per channel throughout: the driver trait
(`Driver::read(&mut Vec<u32>)`), the supervisor, `Capture::append(start, &[u32])` and `ingest_chunk` in storage,
the signal sources of the generators; the mask of enabled channels is a `u32` (`DeviceConfig`, `ResolvedConfig`,
the description of a capture, the saved description, the presentation's visible mask); `MAX_CHANNELS` is 32 in
storage, in the `LAYT` and `VIEW` sections and in the capability check. Storage itself is **per channel**
(transitions per channel), so only the entry of the samples is bound to 32 bits.

**Decision**: keep the word and add a stride. `CaptureConfig` gets `words_per_sample` (1 to 5, from the channel
count: `ceil(channels / 32)`); a chunk is `n × words_per_sample` words in sample order, word `k` of a sample holding
channels `32k` to `32k + 31`; `ingest_chunk` takes the stride. The mask becomes `ChannelSet`, a type that is used **only through its methods** (`contains`, `insert`,
`remove`, `count`, `iter` in ascending order, `and`, `highest`, `first_word`): no code handles its words or
its width, so no code depends on how many channels there are. Inside it is a fixed bitset whose size is computed
from one constant, `MAX_CHANNELS` (160), which is also the safety bound for what a file or a device may
declare; raising the bound later is changing that constant. A growing list (a sorted vector) was considered and
not taken: the set stays `Copy` (it is held in the configuration, the description and the presentation and is
copied freely), has no allocation, and tests membership in constant time, which the drawing and the ingest
do for every channel and sample. Everything for `words_per_sample == 1` is the present code path (the same functions, the same results, the
same bytes in files).

**What the bound is**: the channel number is a `u8` in the decoders, the setups, the sections and the
interface, so the bound cannot pass 256 without moving it to `u16` (a change of every place that stores a
channel number, in files too); that is not done here because 160 is enough for five boards, and the move is
a separate change. A file is read from an untrusted source (Principle I), so the channel count in it has an upper
bound whatever the representation.

**Alternatives**: `u128` for the word (stops at 128; wastes bandwidth at 32 channels); one capture per group of
32 channels (splits one device, loses the alignment, multiplies the sessions and the markers); a bitset of
`Vec<bool>` per sample (slow, no).

## R2. How a chain works in the original, and in this application

**Findings** (the original's documents): each board of a chain is its own serial port (or network address); the
original's program connects to each, arms the slaves first and the master last, the boards are wired so that the
master's trigger output starts the slaves, and the program merges the data and offers a shift to correct a
board whose timing is a sample off. The master can use only the pattern triggers; there is no edge trigger.

**Decision**: the chain is **host-side**: the driver opens one serial link per board, arms the slaves, then the
master, merges the words of the boards into one stream (`words_per_sample` = the total over boards, channels of
board 0 first), and applies a per-board shift in samples that the user sets. The firmware of a board does not know it
is in a chain except that it is told its role (master or slave) in the capture request. A chain is a device the
user **defines** (the boards, in order, are chosen in the application and saved in its settings; the driver
cannot know the wiring). A chain is listed only while all its boards are present; a missing one is named.

**Alternatives**: a bus between boards that the firmware drives (the original does not, and it would need
wiring and firmware the owner did not ask for).

## R3. The protocol: the original's, extended

**Findings**: see the spec's study section. Framing `55 AA … AA 55` with `F0` escapes; commands 0 to 6; replies
are text lines, then the samples. The capture request is a packed C structure of up to 24 channels; the
capture mode (0, 1, 2) sets 1, 2 or 4 bytes per sample; the information reply is text lines.
**To confirm** (not read): the limits in the original's headers, the layout of the blast request, and the exact
byte alignment of the request structure (it was computed, not read).

**Decision**: the original's commands 0 to 6 are kept exactly. The extension adds: a **feature line** in the
information reply (`FEATURES:` and names, read by a driver that knows it and ignored by one that does not, since
every line is `NAME:value`); a **capture request 2** (command 7) with an explicit little-endian layout (no
structure padding) that carries up to 32 channels, the capture mode 3 (25 to 32 channels, 4 bytes per sample,
the same as mode 2 on the link), the samples before and after the trigger, the trigger, the burst count, the
role in a chain and the **encoding** (raw or compact); **encodings** of the reply (R7). The original's request
(command 1) stays valid for up to 24 channels and returns what the original returns. A firmware that does not
know command 7 answers `ERR_UNKNOWN_MSG`, which is how a driver knows it talks to an older board.

**Alternatives**: a protocol of the project's own (rejected by the owner's answer); changing command 1's
structure (breaks the original software for up to 24 channels).

## R4. The firmware: language, runtime and layout

**Decision**: Rust, `no_std`, for `thumbv8m.main-none-eabihf`, in `firmware/rp2350b/` as its own workspace:
`la-fw` (the board: start-up, USB serial, the capture engine), `la-fw-logic` (no I/O: request checking, the
mode and limit tables, the sample packing, the compact coding, the timestamps), `la-fw-rig` (the verification
rig, R10). The protocol crate `la-proto` is shared with the driver. The runtime is **Embassy** (`embassy-rp`
has support for the RP2350, including a feature for the 48-pin RP2350B, with PIO, DMA and USB, and
`embassy-usb` provides the CDC serial class) because it is the maintained, no-allocator path for this chip with
those peripherals; the PIO programs are assembled at build time with the `pio` crate. **To confirm** in the first
task: the versions, that the feature for the 48-pin chip is there, and the licences (`cargo deny` in the
firmware workspace). The panic handler logs a code to a reserved RAM area and resets the board. The
image is a UF2 for the board's bootloader (`picotool` or `elf2uf2-rs`, whichever builds the image from the ELF
with the RP2350 family identifier; named in the contract). A CI job builds the firmware for the target and
runs the tests of `la-fw-logic` and `la-proto` on the host; the host build and tests do not need the target.

**Alternatives**: `rp235x-hal` with `usb-device` (works, but more glue for the DMA and the PIO that this
design leans on); C with the Pico SDK (the owner asked for Rust); writing to the registers (more code, no gain).

## R5. The capture engine

**Findings** (the original, and the chip from its datasheet; **to confirm** on the board): the original samples
with a PIO state machine (`in pins`) into a FIFO that two chained DMA channels empty into a ring buffer, and
runs the pattern trigger in another PIO block; the RP2350 has **three PIO blocks** (12 state machines), each of
which reads 32 pins at a time from a window of the 48 (a base of 0 or 16), DMA channels, 520 KB of RAM, and a
USB controller that is **Full Speed only** (12 Mbit/s).

**Decision**: the 32 inputs are 32 **consecutive** pins, so one PIO state machine reads them with one `in pins,
32`; which window (0 to 31 or 16 to 47) and which two pins carry the trigger lines are chosen from the
schematic (the memory chip's select pin and the USB and boot pins decide), and the pin map is part of the
contract and the documents. Capture: PIO block 0 samples into the FIFO, two chained DMA channels write the
ring buffer (the same shape as the original). The pattern trigger runs in PIO block 1 (up to 16 channels,
about 66 MHz, as the original) and the fast trigger as a jump table (up to 5 channels, 100 MHz); the edge
trigger is the pattern trigger on one channel. Pre-trigger samples are the ring buffer before the trigger
point; post-trigger samples are counted by the capture program. Burst: the capture program is repeated the
given number of times, the time of each burst is read from a free-running counter and returned with the data.
Blast: a capture without trigger logic at the highest rate.

**Rate**: 100 MHz needs a PIO clock of 100 MHz. The system clock of the RP2350 is 150 MHz by default, which
gives a divider of 1.5 (a fractional divider, so uneven sample intervals); a clock of 200 MHz gives an exact
divider of 2. **Decision**: the firmware runs the system clock at the value that makes every offered rate an exact
divider or declares the rates that are not, and **measures** (the verification) the jitter at each. If 200 MHz
needs a higher core voltage, the firmware sets it (a documented step with a limit). **To measure**: the
highest rate that works with 32 channels, the DMA and bus load at 400 MB/s (100 MHz × 4 bytes), and the effect on
the other peripherals; the result is written to the documents and declared in the capabilities.

## R6. The external memory (ESP-PSRAM64H, 8 MB)

**Findings** (from the datasheets; **to confirm** on the board): the RP2350 has a memory interface (the QMI)
that maps an external quad-SPI memory into the address space (a second window, with its own select pin), at a
speed set by the clock and the divider; the ESP-PSRAM64H is an 8 MB quad memory. The sustained write speed
through the interface is far below the 400 MB/s of 32 channels at 100 MHz.

**Decision**: the **internal memory** takes captures at every rate (about 120 thousand samples at 32 bits, as
the original's buffer scaled to the chip). The **external memory** takes a capture only when the rate at the
mode in use is below a threshold **measured** on the board (the sample rate × the bytes per sample must be
below the sustained write speed with a margin); then the buffer is up to 2 million samples at 32 bits (8
million at 8 bits). The capabilities declare the depth for a rate range, and the validation refuses a capture the
memory cannot take at the rate asked. The initialisation of the memory (the commands that put it in quad mode)
is part of the firmware's start-up; a board without the memory chip (the same firmware on another board) uses
the internal memory only and declares so.

**Alternatives**: always using the external memory (too slow for the high rates); a second buffer chained after
the first (more complexity for little).

## R7. The transfer: raw, and the optional compact coding

**Findings**: the link is a serial port over Full Speed USB, about 1 MB/s in practice. A raw capture of 120
thousand 32-bit samples takes about half a second to transfer, and 8 MB about eight seconds. The original sends
raw words.

**Decision**: the default is the original's raw transfer. The firmware may announce a **compact coding**
(`FEATURES:compact`); when the request asks for it the firmware sends, instead of one word per sample, a
sequence of records for the samples whose selected channels differ from the previous sample (the distance in
samples as a variable-length number, then the changed channels packed into the bytes the selected channels
need), ending with the total number of samples; it is **lossless** and the driver rebuilds exactly the raw
words. If the firmware finds that the coding would be longer than the raw data (a signal that changes at
every sample) it sends raw and says so in the header of the reply. The packing of only the selected channels
applies to the raw transfer too in the new mode. This is how a large capture of a bus with few changes is
usually a hundred times shorter on the link. The exact bytes are in `contracts/protocol.md`.

**Alternatives**: a general compressor (needs code and memory in the firmware, and the structure of logic data
is better used by the coding above); a faster USB interface (about a tenth to a fifth gain for the price of
compatibility with the original software, rejected in the discussion with the owner; it can come later as an
addition next to the serial port).

## R8. What the device abstraction gains

**Findings**: `Capabilities` has the channel count, the rates, a list of single-channel trigger kinds
(`Level`, `RisingEdge`, `FallingEdge`, `AnyEdge`) and one `buffer_depth`; `DeviceConfig` has the mask, the rate,
one `Trigger { channel, kind }`, an optional length and `DeviceOptions` (non-exhaustive, per device);
`samples_before_trigger` in the description means the samples skipped waiting for the trigger.

**Decision** (contract in `contracts/device-abstraction.md`): `TriggerKind` gains `Pattern` and `FastPattern`
and `Trigger` gains a pattern (channels, levels); `Capabilities` gains the trigger limits (the widest pattern and
fast pattern, whether a trigger is possible in a chain), a **pre-trigger and a post-trigger limit by capture
mode**, a **buffer depth by rate range** (a list), burst support (the most bursts) and the highest rates by
channel count; `DeviceConfig` gains the samples before the trigger (as data kept in the capture) and the burst
count. The validation checks each against the capabilities and gives a typed error that names the limit, as
for the existing ones. The meaning of `samples_before_trigger` is renamed in the description to what it is
(skipped) and a new field records the pre-trigger samples kept. Devices that do not support the new things declare
none and behave as before (every existing driver's tests pass unchanged).

## R9. What changes for 160 channels, by layer

- **Storage** (`la-storage`): R1. Property tests: a capture of 33 to 160 channels ingested in any chunking equals
  the one made from the same samples as a list of channels; at 32 channels the results equal the present ones.
- **Devices** (`la-devices`): `ChannelSet` for the masks, the capability check, the supervisor and the driver
  trait with the stride; the virtual device generates up to 160 channels (Principle XI).
- **Session** (`la-session`, `la-devices::description`): the description's `enabled_mask` is a `u32` in the file.
  A **minor** version of the format adds the wide form (the words after the first, present only when the channel
  count is above 32); a file of 32 channels or fewer is byte for byte what it was; an older reader refuses a file of
  more than 32 channels with its typed error for a channel count out of range.
- **Host** (`la-host`): the configuration, the presentation (`visible_mask` becomes a `ChannelSet`), the `VIEW` and
  `LAYT` sections (`MAX_CHANNELS` 160; their formats already count channels, so they only raise the limit),
  the decoders' channel choices (`u8`, enough), the markers (positions, not channels).
- **Interface** (`la-app`): the channel buttons are four rows of eight at 32 and twenty at 160 (the panel scrolls);
  the legend and the lanes already scroll.
- **Costs**: the planner and the drawing touch only the shown channels (a benchmark with 160 channels is added
  to the existing ones); the memory of a capture grows with the channels that have transitions.

## R10. The verification rig

The Arduino Nano rig of 013 drives a few pins and cannot drive 32. **Decision**: `la-fw-rig`, a second firmware
for the same kind of board, that drives all 32 (or 48) pins from a PIO program with known patterns: channel `n`
carries a square wave whose period depends on `n`, a counter on all pins, and pulse patterns for the triggers; a
text line on its serial port starts each. The procedure wires the rig's pins to the analyzer's inputs, captures,
and checks in the application (and in an opt-in test) that channel `n` shows the pattern of pin `n` and no
other. A chain is verified with two analyzers wired to the same rig pin.

## R11. Licences and sources

Read from the original: the README and wiki pages and the firmware's main sources (named in the spec's study
section). The original is GPL-3.0; this project is GPL-3.0-only. No file of the original is copied: the
protocol crate and the firmware are written from the behaviour and from the protocol's facts, which are described
in `contracts/protocol.md` so that the driver and the firmware can be checked against one document. The
embedded dependencies are recorded with their licences in `firmware/rp2350b/THIRD-PARTY.md` and checked by
`cargo deny`.

## R12. Order of delivery

1. **Wide channels** (R1, R9) and the virtual device at 160, with tests that fail at 33 channels first; no
   driver needed. Mergeable alone.
2. **The abstraction** (R8), with the validation tests; mergeable alone (no driver uses it yet except the
   virtual device's declared limits).
3. **`la-proto`, the firmware's logic crate and the driver** with the recorded transport, then **the firmware**
   on the board (R3 to R7), then the rig (R10): the hardware work, with a manual verification at the end.
4. **Chaining** (R2): the driver's chain, the user's settings for it, its tests with two simulated boards, then
   the hardware verification with two boards.
5. **Documents** throughout, `verification.md` at the end of each stage.

Each stage is its own pull request or more; the spec's story 1 and 2 are met at the end of stage 3, story 6 at
the end of stage 4.

## R13. USB identity and the transport: to analyse later

Kept for a later decision (nothing here is decided except the development pair).

**Decision for now.** During development the firmware presents the Pico SDK's default pair for a USB serial
device, `0x2E8A:0x000A`, and the transport is CDC (a serial port). The driver therefore asks every port with that
pair for the device information and lists only the ones that answer as this analyzer. Consequences: another
board running SDK firmware with the default pair gets one information request (a frame of 5 bytes) written to
it, and a silent one costs up to 500 ms in a listing. This is acceptable on a development machine and is not for
a release.

**What the original does** (read from `gusmanb/logicanalyzer`, V2 firmware): its own pair `0x1209:0x3020` with
manufacturer and product strings, set in `CMakeLists.txt` (`USBD_VID`, `USBD_PID`); registered in pid.codes
(`1209/3020`, owner `gusmanb`, licence GPL-3.0); the PC program finds the port by that pair (Windows registry,
Linux `/sys/bus/usb/devices`) and takes the serial number from the USB descriptor. Its V1 firmware kept the SDK
defaults. That pair is theirs and must not be reused by this project.

**To decide before a release.**
- A pair of our own: pid.codes (VID `0x1209`, free, open hardware, the owner opens a pull request in
  `pidcodes/pidcodes.github.com`) or Raspberry Pi's free PIDs for products on their chips (`raspberrypi/usb-pid`).
  The owner has to apply; to be checked against the current rules of each.
- CDC or a vendor-specific bulk interface. The VID/PID identify the product, not the class, so a change of class
  does not by itself need a new pair; but Windows keeps the driver it bound to a VID/PID (and interface), so a PC
  that has seen the CDC firmware may keep `usbser` for a bulk firmware with the same pair. Ways out: raise
  `bcdDevice`, or give the bulk firmware its own PID. Bulk needs WinUSB (automatic with Microsoft OS 2.0
  descriptors in the firmware, else Zadig) and, on Linux, a udev rule; CDC needs neither (Linux: the `dialout`
  group). The driver's transport is one trait (`SerialLink`); `la-proto` does not change.
- **A benchmark** to choose: sustained throughput and the latency of the abort byte for CDC and for bulk on the
  RP2350 with the firmware's real transfer (a capture returned at its end, up to 96,000 samples of 4 bytes and
  more with the external memory), on Windows and Linux; and the time from the end of a capture to the last byte
  on the host. The rough expectation (to be checked, not relied on) is that CDC on the RP2350 is enough for
  this design, because the data comes back once after the capture and not as a stream.
