# Research: I2C Decoder and Generator

Every decision below was taken against the code of feature 007 (`crates/decoders`, `docs/decoder-api.md`)
and the spec including its three clarifications. There was no `NEEDS CLARIFICATION` left in the
Technical Context. Protocol knowledge comes from the I2C-bus specification (NXP UM10204); no code of
PulseView, libsigrokdecode or any other project is used (R13).

## R1. Determining the bus period automatically

**Decision.** The bus period `T` is the typical distance between rising edges of SCL, found from the
signal itself, with integer arithmetic only:

- Every pair of consecutive SCL rising edges with no lost samples between them gives an interval `d`.
- Intervals go into a histogram of quarter-octave bins (`bin = 4·floor(log2 d) + next two bits of d`,
  256 bins in all, each with a count and a sum). The state is a few kilobytes.
- The decoder keeps the last 32 intervals and, after each interval, in order, checks the *calibration
  condition*: the best bin together with its two neighbours holds at least 16 of those 32 intervals.
  The first interval after which it holds fixes `T` as the mean of the intervals in those three bins
  (sum divided by count), forever for this pass. (The first design asked for half of *all* intervals;
  a long noisy start-up could then keep the clock from ever being recognised, so only the latest
  intervals count.)

**Why.** Inside a byte the SCL rising edges are one period apart, so any real traffic fills one bin
quickly: a transaction with an address and one byte gives 17 intervals. Gaps between transactions and
noise before the MCU set up its interface are spread over many bins, so they cannot fill 16 of the last
32 intervals, and once the clock comes 16 of its intervals are enough, however long the noise was. A mean of a bin is exact for a steady clock and within a quarter octave otherwise,
which is all the rule "idle for at least one period" needs. The check runs interval by interval, so
the moment it first holds depends only on the signal and never on how it was cut into chunks (FR-009,
FR-024). Clock stretching makes a few intervals longer; they land in other bins.

**Alternatives rejected.** The minimum interval (one glitch spoils it); the first transaction only (it
may be junk, and it cannot be known to be the first real one); a user option for the period (the user
asked for automatic detection, and a wrong value silently hides traffic); half of all intervals so far
(a long noisy start-up would keep the clock from being recognised).

## R2. Synchronising on the bus, and waiting for `T`

**Decision.** The decoder has two states before it reads transactions:

1. *Calibrating*: `T` is not known. It reads SCL rising edges from position 0 (a decoder may read
   anywhere in the snapshot of the chunk, and does so without any copy) and feeds R1, at most 262,144
   edges per call (four chunks' worth), so one call still takes milliseconds. Nothing is output.
2. *Seeking*: `T` is known. From position 0 it follows the levels of both lines (edges and the level at
   position 0) and finds the first start: a falling edge of SDA while SCL is high, preceded by at least
   `T` samples with both lines high without a break (a run that began at position 0 counts). That is the
   first accepted start, and from there it decodes (R4). Everything before is ignored silently (spec
   clarifications 1 and 2). A stop-like edge (SDA rising while SCL is high) before the first start is
   ignored; a gap in the capture breaks the idle run.

Once `T` is found the decoder walks the part of the capture it has not yet processed, in slices of at
most 262,144 edges per call, so a long start-up never makes one call long. Coverage is reported honestly
meanwhile (R3 below). `finish` drains what is left (a finished capture ends with everything processed;
a cancelled pass ends cut and incomplete, as always).

After the first start the decoder never needs `T` again: re-synchronisation after errors was rejected in
the clarification (Option B, not C).

**Why this and not a second pass by the framework.** The framework runs one pass in chunks, and a
growing capture cannot be read twice. A decoder that reads ahead and behind inside the snapshot, with a
small state, gives the "two passes" the user anticipated without changing how passes run. Where the
data is on disk or large, nothing is copied: the reader is an iterator over the snapshot's transitions.

## R3. A small, optional addition to the decoder API (1.0 → 1.1)

**Problem.** While calibrating (or catching up) a decoder has not finished the chunks it was handed.
The framework reports the end of the chunk as the coverage of the results. That is the watermark that a
stacked decoder trusts ("everything before it is final", `stack.rs`); if the I2C decoder emitted items
later, they would arrive behind a watermark that had already passed them.

**Decision.** Add one optional method to `Run`, a minor version (`docs/decoder-api.md`, Versioning):

```text
fn final_through(&self) -> Option<u64> { None }   // default: the end of the last chunk
```

`Pass::advance` reports `min(chunk end, final_through)` as the coverage after every chunk; `finish`
reports the end of the pass as before. Existing decoders do not change and the sample decoder's tests
pass untouched. The I2C decoder returns the position it has actually processed.

This is the one change to the contract, so it is recorded in the Complexity Tracking table of the plan
(FR-022). `DECODER_API_VERSION` becomes `1.1`; the document test of feature 007 keeps the two in step.

**Alternatives rejected.** Letting the decoder lag without telling the framework (breaks the watermark
guarantee for stacking, the very thing feature 010 needs); a "restart from position X" request (a
larger change to the chunk loop, and nothing else needs it); doing the whole replay inside one call
(unbounded call length for a start-up of many millions of edges, which the supervisor would abandon).

## R4. The transaction state machine

Levels and edges: roles are listed `scl`, `sda` in the descriptor, so simultaneous edges are delivered
SCL first. A data bit is the level SDA had *before* an SDA edge at the same position is applied, which
is the level it held while SCL rose. Positions are samples; no time units are needed.

```text
Idle ──start──▶ Address(bit 0..8) ──▶ Data(bit 0..8) ──▶ Data … ──stop──▶ Idle
                    ▲                                         │
                    └────────── repeated start ───────────────┘
```

- **Start / repeated start**: SDA falls while SCL is high. From Idle it is a start; inside a
  transaction (between bytes or in the middle of one) it is a repeated start; in the middle of a byte
  it is also a violation (R5) and the byte is reported as cut short.
- **Stop**: SDA rises while SCL is high. It ends the transaction; in the middle of a byte it reports the
  byte cut short.
- **Bit**: a rising edge of SCL is a candidate; it is a bit only when its pulse ends (SCL falls) with
  no SDA edge in it, because an SDA edge while SCL is high is a start or a stop and the pulse before it
  was only its set-up (this is how a repeated start and a stop are told from a data bit). The bit is
  the level SDA had at the rising edge. Nine bits make a byte and its acknowledgement; the ninth is
  the acknowledge bit (low = ACK, high = NAK). A capture that ends before the pulse of a bit ends does
  not count that bit.
- **Address**: the first byte after a start. The 8th bit is the direction (0 write, 1 read). A first
  byte of the form `11110xxD` is the first byte of a 10-bit address: if `D` is 0, the next byte is the
  low 8 bits and the two are shown as one 10-bit address; if `D` is 1 after a repeated start in a
  transaction whose first part was such a write, it is the read part of the same 10-bit address and
  shows the address already known. Any other `11110xxD` is an ordinary address (no error).
- **Acknowledgement**: after the address byte and after a byte the controller writes, NAK is an error.
  After a byte the controller reads, the controller itself answers; NAK there is normal and never an
  error. Whether the transaction continues afterwards is not judged.
- **Gaps**: a gap in the capture (lost samples) ends the byte in progress as an error ("samples lost
  here") and the transaction as incomplete; the decoder looks for the next start on the other side with
  the idle rule of R2 (a gap breaks an idle run).
- **No start seen** is not an error (R2). A start immediately followed by a stop (no clock between) is
  an error: "empty transaction".

## R5. What is drawn and what is emitted

Per transaction, in the lane: one range for the whole transaction (start to stop, text "Write to 0x50"
or "Read from 0x50"; an unfinished one is drawn to the end of the capture, with the text "Incomplete
transaction", not as an error), one range per byte (address or data, from its first bit's rising edge to
and including the acknowledge edge), one point per acknowledgement ("ACK", or "NAK" in the error style
where R4 says error), and one point for start, repeated start and stop. Errors are ranges or points with
`error = true` and a message.

On the signals: a dot (`Shape::Dot`, no colour named, so SDA's own colour) on SDA at every start,
repeated start and stop; an arrow (`Shape::Arrow`, `rising: true`, no colour named, so SCL's own colour)
on SCL at every sampled bit including the acknowledge bit, exactly on the rising edge. The overlays are
placed on the sample of the event, so the planner of 007 draws them centred and only when legible.

Volume: about 11 annotations per byte (two ranges, nine arrows) and a few per transaction. The default
store limits of 007 were checked against the sample decoder's volume of one point per bit; the same
order of magnitude holds here. The plan keeps the bit points *out of the lane* (the arrows on SCL carry
them) so the lane stays readable and the volume stays down; a test with dense traffic proves the limit
is not reached by a capture of ten million samples and that reaching it is handled (as `decoder.limit`
of 007) when it is.

Display text: the address in 7-bit form (`0x50`) or as the 8-bit value with the direction bit (`0xA0`,
`0xA1`), values in hexadecimal, decimal or binary, per the two options (R8). Text uses message keys with
the value as an argument; the direction is two keys (write, read), not an argument, so it is translated.

## R6. What a stacked decoder receives

An item has one `kind`, and `Input::Items(kind)` names exactly one, so the decoder produces **one kind**,
`i2c`, whose payload is bytes with a small fixed layout (contracts/i2c-items.md): the event type
(start, repeated start, address, data, stop), the direction, the acknowledgement, a flag for a 10-bit
address, and the value. The item's span is the exact positions of the construct; its `error` flag is set
when the same construct has an error annotation. Items are emitted at the moment the construct is
complete, in non-decreasing order of `end`, which is what the sink requires and what makes the watermark
safe. A byte that is cut short produces no item (only the error annotation); a transaction that is
incomplete at the end produces the items of everything that completed.

## R7. The generator

**Decision.** A pure function of the position, like the generator of the sample decoder, so any chunking
gives the same signal and the cost per sample is constant.

- Time is divided into **frames** of fixed length. Frame `k` holds at most one transaction, whose content
  (address, direction, bytes, whether a repeated start splits it, whether and where an error is planted)
  is drawn from `mix(seed, k)`. The frame length is sized for the longest transaction allowed by the
  generator option `max_bytes` (default 4), plus idle: a lead-in of four bit periods of both lines high
  (comfortably more than `T`), the transaction, and idle to the end of the frame.
- One SCL period is **16 samples** (SCL high for samples 4..12 of a bit slot; SDA changes at sample 0 of
  a slot, while SCL is low). The generator does not need the capture's sample rate; the bus then runs at
  one sixteenth of it. That is "a bit rate that suits the sample rate" (FR-013) for every rate and keeps
  `T` known exactly (16) for tests.
- **Errors** (`errors` = 1, `error_rate` = about one transaction in this many; the keys the shell
  already writes for the sample decoder's generator, so the shell is untouched): the address is not
  acknowledged; a written data byte is not acknowledged; the transaction is cut short by a stop in the
  middle of a byte. Rate 0 or `errors` = 0 plants nothing.
- Generator options only tests use, kept in the same `Setup::generator` map: `ten_bit` (use the 10-bit
  address form), `max_bytes`, and `startup` (a start-up before the first frame: the lines held low and
  then pulsed with highs shorter than `T`, which the decoder must ignore silently; this is how the
  "MCU not initialised yet" case of the spec is round-tripped).
- `expected(setup, seed, length)` returns the items a correct decode of the first `length` samples must
  yield, including a capture that ends in the middle of a transaction (the items completed before the
  end), and `expected_errors` returns the planted errors with their positions, as for the sample
  generator.
- Two instances on different channels differ: the instance number is mixed into the seed, as before.

**Alternatives rejected.** Variable-length transactions laid end to end (the position of transaction `k`
then depends on all earlier ones, so the signal is no longer a pure function of the position and
chunking independence would need state); using the capture's sample rate to scale the period (needs a
change to the claim interface of 007 for no visible gain).

## R8. Options

`address` (choice): `addr7` (default; the address without the direction bit) or `addr8` (the 8-bit value
with the direction bit). `radix` (choice): `hex` (default), `dec`, `bin`. Both change only how text is
formatted, never what is decoded or emitted as items (items always carry the 7-bit or 10-bit address
as a number). A change re-runs the decoder under the rules of 007; the options are saved with the
session by the existing `DECO` section, which needs no change (an instance of an unknown decoder or
option is kept raw by 007, so a file saved by this version opens in an older build).

## R9. Localization

Keys follow the scheme of the contract: `decoder-i2c-name`, `-description`, `-signal-scl`,
`-signal-sda`, `-option-address`, `-choice-address-addr7`, `-choice-address-addr8`, `-option-radix`,
`-choice-radix-hex`, `-choice-radix-dec`, `-choice-radix-bin`, and the annotation messages
`ann-i2c-start`, `-repeated-start`, `-stop`, `-transaction-write`, `-transaction-read`, `-incomplete`,
`-address-write`, `-address-read`, `-data`, `-ack`, `-nak`, `-error-address-nak`, `-error-data-nak`,
`-error-byte-cut`, `-error-empty`, `-error-gap`, each with its arguments documented beside it in
`ui.ftl`. No plural is needed: no message counts anything. All keys exist in six languages as machine
drafts (the owner reviews them, as for earlier features), and the existing parity check covers them.

## R10. Tests, recorded signals and fuzzing

- **Unit tests** per behaviour of R4 on hand-built signals (a builder that turns a list of bus events
  into samples, in the test helpers of the decoders crate).
- **Recorded signals** (constitution Principle IX): a small set of waveform files under
  `crates/decoders/tests/fixtures/i2c/`, stored as run-length lists of `(sda scl) × samples` in a
  documented text format, covering: a write with two bytes, a read, a repeated start, a 10-bit address,
  a NAK on the address, a NAK on data, a cut-short byte, a capture that begins mid-byte, one that begins
  before initialisation (stuck low and noisy), one that ends mid-transaction, clock stretching, and a
  start with no clock before the stop. Their expected decodes are checked in next to them. A loader
  test checks the files themselves (a malformed fixture is rejected).
- **Round trip**: for a thousand seeds and the option combinations (7/10-bit, errors on/off, `startup`
  on/off, lengths that cut transactions), generate, decode, compare items and errors exactly (SC-001).
- **Chunk invariance**: the same signal decoded with chunk limits from one edge to the whole capture,
  and while a growing capture is fed in pieces, gives identical results (SC-005); a seed sweep.
- **Random signals**: a property test feeds a million random chunks of arbitrary levels, including gaps
  and one-sample pulses, and requires no panic and valid output (SC-004); the existing fault-injection
  tests of 007 cover the containment of a decoder that does panic.
- **Stacking**: a test-only decoder reading kind `i2c` is stacked on it and receives exactly the
  transactions of the lane (US5), including with the watermark held back during calibration (R3).
- **Host**: the virtual device with an I2C instance produces traffic that decodes to the generated
  transactions end to end; a capture of ten million samples decodes without an interface pause longer
  than 100 ms (SC-006), using the test of 007 that measures `poll`.

## R11. Benchmarks

The decode benchmark of 007 (`crates/decoders/benches/decode_throughput.rs`, `baseline.json`) gains an
I2C case: decoding dense generated traffic, edges per second, with the baseline recorded in the same
file. A regression of the complexity class (cost depending on the capture length for a fixed number of
edges) is a merge blocker as before.

## R12. Documentation

`docs/decoder-api.md` (1.1: `final_through`), a new `docs/decoders/i2c.md` (what the decoder reads, the
synchronisation rule, the options, every annotation and overlay, the item format, the generator and its
options), the user guide (a section "Decoding I2C": address notation, how to read the arrows and dots,
what start-up looks like), and `CONTRIBUTING.md` unchanged (the "Adding a decoder" steps are followed, not
altered).

## R13. Third-party code

None. The decoder and the generator are written from the I2C-bus specification (NXP UM10204, revision
7.0, sections on bit transfer, start and stop conditions, acknowledge, 10-bit addressing). PulseView and
libsigrokdecode were not consulted for code; their I2C decoder is, as the constitution says, treated as
untrusted and not ported. No new dependency.

## R14. The application shell

Nothing changes in `crates/app` or in the host's shell: the decoder appears through the registry
(`Registry::builtin`), the configuration dialog and the lane are built from its descriptor, and the
generator options the shell edits (`errors`, `error_rate`) are the keys the I2C generator reads. The
constants the shell uses for these keys live in the sample decoder's module today and stay there: the
I2C generator reads the same literal key strings (`errors`, `error_rate`) and the keys are documented
in `docs/decoder-api.md` as the generator options every generator may use. No file of the shell is
touched, which is what SC-008 asks.

## R15. The edges of the generated signal (FR-025)

**Problem**: in `generators::i2c::Frame::levels` the start is 4 samples of SDA low with SCL high and the
repeated start has SCL high for 4 samples; SCL then falls in the very sample where SDA takes the first
bit, so the pulse of a start is half as wide as the others and, when the first bit is 1, SDA and SCL
change together.

**Decision** (the owner's, 2026-11-22; a first version with a gap of 4 samples was replaced): SDA
changes in the sample where SCL falls, so the bit of a slot is set 4 samples before its clock pulse and
held to the end of the pulse (set-up and hold of 8 samples either side of the rising edge); in a start, a
repeated start and a stop the edge of SDA is 8 samples from an edge of SCL. Start = SDA low with SCL high
for 8, then SCL falls and SDA takes the first bit, 4 samples before that bit's pulse (12 in all).
Repeated start = SDA is set high when SCL falls at the end of the acknowledge slot; SCL low for 4, high
for 8, SDA falls and 8 samples later SCL falls with the first bit set (24 in all). Stop = SDA is set low
when SCL falls; SCL low for 4, SCL high for 8, SDA rises (12). `START`, `RESTART` and `STOP` become 12, 24
and 12, so the frame length (1008 with the defaults) and every position computed from them (`slot_start`,
the expected-item times: the repeated start at the SDA fall, 8 after the SCL rise; the stop 12 after the
end of the body) follow from the constants; the lead-in stays 64 samples with both lines high.

**Alternatives**: a gap of 4 samples everywhere (the first answer; the owner preferred the look of SDA
changing with the falling edge of SCL); a gap of 1 sample (hides defects from a decoder that samples
coarsely). The decoder needs no change: it reads SDA at the rising edge of SCL and reacts to SDA edges
while SCL is high.

## R16. The Decoders panel (FR-026 to FR-028)

**Decision**: in `crates/app/src/app.rs` (`decoder_block`): (a) build `channel_choices` from the channels
that are recorded (the bit of the channel in `state.config.enabled_channels`), keeping the channel that is
already assigned even if it is not recorded, marked with the key `decoder-channel-unavailable`; a decoder
whose channel is not recorded is not ready (`Shell::channels_are_recorded` in `setup_is_ready`, with
`Shell::recorded_channels`: the channels of the capture on screen, or those being recorded while a capture
runs) and the panel says so (new keys in six languages); (b) every `pick_list` of the block gets `width(Length::Fill)`; (c) the panel's outer frame is
removed and the block of each decoder is a `container` with the `bordered_box` style, as the blocks of the
Markers panel.

**Alternatives**: dropping the assignment when the channel is switched off (loses the user's setting
silently); forbidding switching the channel off (a capture setting would depend on a decoder).
