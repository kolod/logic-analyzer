# Research: Session File Format

Decisions taken while planning. Format: Decision, Rationale, Alternatives.

## R1. Where the code lives

**Decision**: a new crate `la-session` (`crates/session`), depending on `la-core`, `la-storage`,
`la-devices` (for `CaptureDescription`), `la-diagnostics` and `tracing`.

**Rationale**: the storage core stays free of I/O and of the device layer; the UI will depend on
`la-session`, never on file layout. `CaptureDescription` already lives in `la-devices`, so the
dependency is natural.

**Alternatives**: put it in `la-storage` (mixes I/O and format evolution into a crate whose value
is being small and deterministic); move the description type into `la-storage` (it is produced by
the device layer and carries device concepts: ids, triggers).

## R2. Container: tagged sections with per-section CRC-32C

**Decision**: header, a sequence of sections (4-character tag, flags, length, payload, CRC-32C
over everything of the section including its framing), footer. Header and footer repeat the file
length and section count and carry their own CRC. Required sections `META` (description) and
`CAPT` (capture); everything else is optional unless a section's `critical` flag is set.

**Rationale**: every byte is covered by a checksum, so truncation, any flipped bit and appended
bytes are detected (SC-004). Sections are independent, so a later feature adds data without
touching the capture (US5). The critical flag gives an old reader a precise reason to refuse
("needs a newer version") rather than misreading. A fixed, simple framing is easy to implement
independently (SC-012).

**Alternatives**: one global checksum (cannot say which part is damaged, forces reading everything
before trusting any section); an existing container (SQLite, zip, protobuf, CBOR): heavy
dependencies, weaker control over allocation limits, and a database or zip layer would hide the
atomicity story; a text format (JSON): size grows with samples-per-transition cost and parsing
untrusted nested input is a larger attack surface.

## R3. Checksum: CRC-32C implemented in the crate

**Decision**: CRC-32C (Castagnoli), table-driven slicing-by-8, written and tested in the crate
(test vectors from RFC 3720, plus a bytewise reference to cross-check the fast path).

**Rationale**: detects every single-bit error and every burst up to 32 bits; fast (several GB/s
slicing-by-8); no dependency (Principle V). The threat is accidental damage (disk, transfer),
not an adversary, which the spec states as out of scope.

**Alternatives**: crc32fast / crc crates (dependency for ~60 lines); a 64-bit hash such as xxh3
(not a CRC: no burst guarantees, another dependency); SHA-256 (cryptographic strength is not
needed and costs throughput).

## R4. Capture encoding: delta varints with a channel table

**Decision**: `CAPT` holds the fixed fields (channels, sample rate, length, status flags, gap
count), the gaps (start, length as varints), a channel table (initial level, transition count,
encoded byte length per channel) and then each channel's transitions as LEB128 deltas (the first
relative to 0, later ones to the previous transition; every delta of a later transition is at
least 1). The encoder sizes every channel stream in a first pass (a second iteration over the
immutable snapshot) so the table and section length are known before writing.

**Rationale**: matches the storage model (initial level + transition positions), so the file grows
with level changes (FR-005); typical deltas (tens of samples) take one byte, a quarter of the
in-memory size; a sparse signal compresses further. The table lets the loader compute the exact
memory the capture will need and reject it before reading data (FR-017) and lets it check each
stream's declared length against the section (FR-015).

**Alternatives**: fixed 8-byte positions (4–8x larger); storing raw sample words (grows with
samples, wastes idle channels); block images identical to the in-memory blocks (ties the file to
an internal layout that item 002 may change); a general-purpose compressor (dependency, another
failure surface; left to a later optional minor version).

## R5. Integers, text and time

**Decision**: little-endian fixed-width integers and LEB128 varints (at most 10 bytes, canonical
form required: no padding bytes); text as a varint length plus UTF-8 bytes, validated, at most
4,096 bytes per field; the start time as signed seconds since the Unix epoch plus nanoseconds
(`i64`, `u32` below 10^9).

**Rationale**: a byte-for-byte deterministic encoding (FR-004) independent of platform; canonical
varints mean one capture has exactly one encoding, so "bit flip changes meaning" cannot hide in a
non-canonical form; epoch seconds cover times before 1970 and far in the future without platform
`SystemTime` limits (out-of-range values are rejected at save with a typed error).

**Alternatives**: platform-native `SystemTime` encoding (not portable); big-endian (no benefit);
non-canonical varints accepted (ambiguity, two files for one capture).

## R6. Atomic save and the file-system seam

**Decision**: write to a unique temporary file in the target's directory (`.<name>.<16 hex>.lasave-tmp`,
created exclusively), write through a buffered writer, flush, `sync_all`, rename over the target,
then (Unix only) `sync_all` on the directory. On any error or cancellation the temporary file is
removed. All operations go through a small `FileSystem` trait (create exclusive, write, sync,
rename, remove, sync directory, list directory, metadata); a fault-injecting implementation
(feature `test-support`) fails the n-th operation of each kind, which gives the 20+ injection
points of SC-003. Leftover temporary files of earlier crashed saves that match the strict name
pattern of the same target and are older than one hour are removed best-effort by the next save;
no other file is ever touched.

**Rationale**: same-directory rename is the atomic step on both platforms (Rust's `rename`
replaces an existing target; on current Windows it uses POSIX-semantics rename); a unique exclusive
name means two concurrent savers can never share a temporary file, so the target is always one
complete file (last rename wins, never a mix); the seam is the only practical way to test disk-full
and refused-rename paths deterministically.

**Alternatives**: a fixed temporary name (concurrent savers corrupt each other); a journal or
write-ahead log (more machinery for no extra guarantee on top of rename); copy-then-rename inside
the target (non-atomic window); `fs2`-style locks (dependency, advisory only).

**Known limit, documented**: the standard library cannot fsync a directory on Windows, so after a
power failure the rename itself may not be durable there; atomicity (old or new, never mixed) holds
on both platforms.

## R7. Safe, bounded loading

**Decision**: a streaming reader over `Read` with a fixed-size buffer. Order: header (check magic,
version, declared length against the real file length, CRC) → each section frame (check tag
validity, length against remaining bytes, count) → `META` and `CAPT` decoded while their CRC is
accumulated and verified at the end of the section → footer. Before any capture data is decoded the
channel table gives the exact worst-case memory; it is checked against the budget
(`session.memory_budget`). Transitions are pushed into the storage restorer as they are decoded;
the restorer enforces order, range, gap exclusion and the memory budget incrementally, and its
result is discarded if any later check fails. Nothing is allocated from a claimed size: vectors grow
only with data actually decoded, and every claimed count is first compared with the bytes left in
the section.

**Rationale**: SC-006 (a lying file costs at most 1 MiB beyond the file size) and FR-015; constant
memory for the reader itself; the loader never trusts a length it has not checked against the file.

**Alternatives**: read the whole file then parse (doubles peak memory for large captures); memory-map
(a file changing underneath yields undefined reads: violates FR-019 and needs `unsafe`);
verify the whole file's CRC before decoding (two passes over multi-GiB files; the per-section
check at the end of each section is sufficient because the result is discarded on failure).

## R8. Saving a running capture

**Decision**: the saver takes a `Snapshot` (cheap, immutable, never blocks the recorder) and the
description, and writes it. A snapshot of a capture that is not closed is written with the
`interrupted` flag set, so the loaded capture reports "incomplete, interrupted"; the `closed` flag
is always true in a loaded capture (it cannot be appended to).

**Rationale**: FR-030 with no new mechanism: the storage core's snapshot is exactly the consistent
prefix; SC-009 holds because nothing is locked while writing.

**Alternatives**: a separate "partial" marker (more states for the UI to understand; "interrupted" is
already the storage core's word for data that should have followed and is missing).

## R9. Background operations

**Decision**: `SaveOperation` and `LoadOperation` handles built on `la_diagnostics::spawn_worker`
(panic containment), with `progress()` (bytes done of bytes total, monotonic, lock-free), `cancel()`,
`outcome()` and `wait_timeout()`, mirroring `Acquisition` from item 004. Cancellation is checked at
every buffer boundary (64 KiB), so it completes within the time of one write. A worker panic becomes
`session.worker_failed`. The blocking functions `save_blocking` and `load_blocking` run the same code
on the caller's thread for tests and tools.

**Rationale**: consistency with the device layer, containment of faults (FR-031), simplest way to meet
FR-028..032.

**Alternatives**: async runtime (a dependency and a different threading model for no benefit);
callbacks for progress (re-entrancy hazards for UI callers).

## R10. Fuzzing without new dependencies

**Decision**: a deterministic mutation fuzzer in `tests/fuzz.rs` (xorshift-driven: bit flips, byte
sets, truncation, insertion, deletion, duplication of a region, splicing of two files, boundary values
at length and count fields, valid CRC recomputation after a mutation so structural checks are reached).
Seeds are fixed and printed on failure. The default run does 20,000 inputs; an `#[ignore]`d test runs
1,000,000 and is executed by `.github/workflows/fuzz.yml` weekly and on demand (release build,
informational, not a required check). Every input is run under a watchdog (5 s) and a counting allocator
bound in the test to check SC-006. Exhaustive tests (a truncation at every length, a flip at every bit of
representative files) run in the normal suite.

**Rationale**: constitution Principle III requires the loader to be fuzzed or property-tested; the
mutation fuzzer with CRC repair reaches deeper than random bytes; no nightly toolchain or libFuzzer
setup (cargo-fuzz works poorly on Windows) and no dependency. A libFuzzer target can be added later
without changing the loader.

**Alternatives**: `cargo-fuzz` (nightly, unsupported on Windows MSVC in practice); `proptest`/`arbitrary`
(dependencies; structure-aware generation is still needed for CRC repair); AFL (platform).

## R11. Versioning and compatibility

**Decision**: format version `1.0` (major `u16`, minor `u16`). A reader accepts its major version with any
minor (a newer minor only adds optional sections or reserved flag values that older readers must be able to
ignore); a higher major is rejected naming both versions. Fixtures written by every released version are
committed under `crates/session/tests/fixtures/<major>.<minor>/` and loaded on every build; a test that
regenerates version 1.0 files and compares them byte for byte guards the writer against accidental format
changes.

**Rationale**: SC-011, FR-023/026; the byte-identical regeneration test turns an accidental format change
into a failing test.

**Alternatives**: no minor version (every addition would need a major bump); feature flags in the header
(optional sections already serve).

## R12. Changes in other crates

**Decision**:

- `la-storage`: `Snapshot::initial_high(channel)` and `Snapshot::transition_count(channel)` (read-only
  accessors, O(1)); `CaptureRestorer` — a builder that takes the capture parameters and limits, then for each
  channel the initial level and its transitions in order, then the gaps and the status flags, and
  returns a closed `Capture`. It validates strictly increasing positions in `1..length`, no transition
  inside a gap or before the first stored sample, gap ordering without overlap or adjacency, flag consistency,
  and enforces the memory budget with the same accounting as recording.
- `la-devices`: `CaptureDescription::to_saved()` (a plain `SavedDescription` value of every field) and
  `CaptureDescription::restore(parts)` (validating: channel count, enabled mask within channels, rates above
  zero, trigger channel enabled, text lengths) documented as the loading path; no setters are added, so a
  consumer still cannot clear the simulated mark of a description it holds.

**Rationale**: a loaded capture is built by code that shares the recorder's block layout and memory
accounting; the compile-fail guarantees of item 004 stay intact because the only way to produce a description
with arbitrary values is an explicit, documented restore that a file's checksum protects.

**Alternatives**: expose `Capture::append`-style replay of reconstructed sample words (O(samples) work and
memory churn for a sparse capture: a 1-billion-sample sparse capture would take minutes); make storage internals
public (breaks the abstraction item 002 established).

## R13. Independent reader for the specification

**Decision**: `tests/independent_reader.rs` contains a second, separately written parser (std only, no use of
the crate's decoder) that follows `contracts/file-format.md` and checks every committed fixture and a set of
generated files, including the CRC and varint rules, against the crate's loader.

**Rationale**: SC-012 — proves the documentation is sufficient and the format is not defined by the code alone.
