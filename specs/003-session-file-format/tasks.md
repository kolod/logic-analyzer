---

description: "Task list for Session File Format"
---

# Tasks: Session File Format

**Input**: Design documents from `/specs/003-session-file-format/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first): in every phase the test tasks
come first and MUST fail (usually they do not compile yet) before the implementation tasks
start. If a test passes immediately because earlier work already covers it, run a mutation
check (temporarily break the code it guards, see the test fail, restore) before relying on it.

**Organization**: tasks are grouped by user story (US1–US6, numbered as in spec.md). The pure
building blocks (checksum, varints, framing, codecs, file-system seam) and the additions to
`la-storage` and `la-devices` are Foundational because every story needs them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root; the new crate is `crates/session`
  (package `la-session`)

## Path Conventions

Source in `crates/session/src/`, tests in `crates/session/tests/`, benchmarks in
`crates/session/benches/`; additions in `crates/storage/` and `crates/devices/`. The contracts
are `specs/003-session-file-format/contracts/file-format.md` ("the format contract") and
`session-api.md` ("the API contract"); the entities and rules are in `data-model.md`.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!`
and `unsafe` in production code. Use `get`, `checked_*`, `saturating_*`, `u128` intermediates
and `?`. Test code may relax this with the file-level `#![allow(clippy::unwrap_used, ...)]`
used by the other crates' tests. Never commit while clippy fails: a helper that only a later
task uses gets `#[expect(dead_code, reason = "used by T0xx")]`, removed by that task. Never
log signal data or text fields; log sizes, counts, codes and durations only.

---

## Phase 1: Setup

**Purpose**: the new crate exists, builds and is wired into the workspace and CI

- [X] T001 Create `crates/session/Cargo.toml` (package `la-session`, workspace edition/MSRV/license, `[lints] workspace = true`, dependencies `la-core`, `la-storage`, `la-devices`, `la-diagnostics`, `tracing` with the same versions as `crates/devices/Cargo.toml`, feature `test-support = []`, dev-dependency on itself with `test-support`, `[[bench]] session_io` with `harness = false`) and `crates/session/src/lib.rs` with `#![deny(missing_docs)]`, the crate doc (what it is, the rule "never log signal data or text fields") and empty module declarations; add a stub `crates/session/benches/session_io.rs` (`fn main() {}`)
- [X] T002 [P] Create `crates/session/src/error.rs`: the `session.*` code constants of the API contract (`NOT_A_SESSION`, `UNSUPPORTED_VERSION`, `TRUNCATED`, `DAMAGED`, `INCONSISTENT`, `UNKNOWN_CRITICAL_SECTION`, `TOO_LARGE`, `MEMORY_BUDGET`, `INVALID_INPUT`, `IO_READ`, `IO_WRITE`, `CANCELLED`, `WORKER_FAILED`, `INTERNAL`) and `pub fn` constructors building `la_core::Error` with the right `ErrorKind` (Corrupt, InvalidInput, Io, Resource, Cancelled, Internal) and an actionable English `detail`; a unit test checks that every code starts with `session.`, is unique and that every constructor produces its code
- [X] T003 Run `cargo build --workspace` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`; confirm `Cargo.lock` gains only the workspace package `la-session`

**Checkpoint**: the empty crate builds and lints clean

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the storage and device additions and the pure building blocks every story uses

**CRITICAL**: no user story can start until this phase is complete

### la-storage additions

- [X] T004 [P] Write `crates/storage/tests/restore.rs` (must fail first: the API does not exist): a capture rebuilt with `CaptureRestorer` from the transitions of a recorded capture answers `level_at`, `transitions`, `viewport`, gaps, length, channel count, sample rate and status exactly like the original (use `la_storage::testing` and random signals, with and without gaps, with the interrupted and stopped-by-limit flags); `Snapshot::initial_high` and `Snapshot::transition_count` match what was recorded; every rule of `data-model.md` is enforced with the matching `storage.*` error: transition position 0, not increasing, at or beyond the length, inside a gap, at or before the first stored position (capture starting with a gap), gap with length 0, overlapping or adjacent gaps, gap past the length, a non-zero initial level for a channel without stored samples, a transition count that does not match the transitions pushed, a channel count mismatch, and a memory estimate above the budget (rejected in `new`/`begin_channel` before pushing); a rejected build leaves no capture; the restored capture is closed, reports `complete` exactly when `lost == 0 && !stopped_by_limit && !interrupted`, and rejects `append` with `storage.closed`; probe counts of a viewport query on the restored capture equal those on the recorded one
- [X] T005 Add the new error constructors needed by the restorer to `crates/storage/src/error.rs` (`storage.invalid_restore` with a detail naming the broken rule; reuse `storage.memory_limit` for the budget) and document them
- [X] T006 Implement `Snapshot::initial_high(channel)` and `Snapshot::transition_count(channel)` in `crates/storage/src/query.rs` (O(1): the channel's initial level; the last block's `first_rank + len`, including the open tail), with rustdoc
- [X] T007 Implement `CaptureRestorer` and `RestoredStatus` in `crates/storage/src/restore.rs` per the API contract and `data-model.md`: validate the config, gaps (sorted, `length >= 1`, within `length`, not adjacent or overlapping) and the flags in `new`; in `begin_channel` check the order of calls, the initial level rule and the exact memory estimate (`4 * transitions + 32 * ceil(transitions / 1024)` plus segment and gap overhead, using the recorder's own constants from `block.rs` and `segment.rs`) against the budget before anything is built; `push_transition` validates strictly increasing positions in `1..length`, not inside a gap and after the first stored position, feeding the same `OpenBlock`/`segment::push_block` path the recorder uses; `finish` checks that every channel received exactly its declared transitions, sets the memory accounting, `closed = true` and the flags, and publishes; export it from `crates/storage/src/lib.rs`; make T004 pass
- [X] T008 Run `cargo test -p la-storage --all-features` (all existing storage tests stay green) and `cargo clippy -p la-storage --all-targets --all-features -- -D warnings`; update `specs/002-signal-storage-core/contracts/` and `data-model.md` with the new accessors and the restorer (a short "restoring a capture" section)

### la-devices additions

- [X] T009 [P] Write `crates/devices/tests/description_restore.rs` (must fail first): `CaptureDescription::to_saved()` followed by `restore` gives a description equal in every accessor (including simulated, seed, trigger, start time before and after 1970 and with nanoseconds); `restore` rejects with `device.invalid_capabilities`/`device.invalid_channel` style typed errors: zero or more than 32 channels, an enabled mask that is zero or has a bit at or above the channel count, a rate of zero (requested or applied), a trigger on a disabled channel, an empty or colon-less device id, an empty provider, text longer than 4,096 bytes, nanoseconds of 10^9 or more; the existing `compile_fail` doctests of `CaptureDescription` still hold (no setter, no struct literal)
- [X] T010 Implement `SavedDescription` (plain public fields, documented) and `CaptureDescription::to_saved` / `CaptureDescription::restore` in `crates/devices/src/description.rs`, with rustdoc stating that `restore` is the loading path and that no setter exists; export from `crates/devices/src/lib.rs`; make T009 pass; update `specs/004-device-abstraction-virtual-device/contracts/acquisition-api.md` with the two functions

### Pure building blocks of la-session

- [ ] T011 [P] Write the unit tests in `crates/session/src/crc.rs` first (`#[cfg(test)]`): the CRC-32C check value of `"123456789"` is `0xE3069283`, RFC 3720 test vectors (32 zero bytes, 32 `0xFF`, ascending, descending), empty input, and the fast slicing-by-8 path equals a bytewise reference for 10,000 random buffers of random lengths and split points (incremental updates equal one-shot)
- [ ] T012 [P] Implement `crates/session/src/crc.rs`: table-driven CRC-32C with an incremental `Crc32c` type (`new`, `update(&[u8])`, `finish`) and a slicing-by-8 fast path; tables built in a `const fn`; make T011 pass
- [ ] T013 [P] Write the unit tests in `crates/session/src/varint.rs` first: encode/decode round trip for 0, 1, 127, 128, 16,383, 16,384, `u32::MAX`, `u64::MAX` and 100,000 random values; `encoded_len` equals the encoded size; decoding rejects with a typed error: a non-canonical encoding (trailing zero group), an encoding longer than 10 bytes, a 10th byte above 1, an input that ends inside a varint; the decoder never reads past the slice end
- [ ] T014 [P] Implement `crates/session/src/varint.rs`: `encode_into(value, &mut Vec<u8>)`, `encoded_len(value)` and a bounded `decode(&[u8]) -> Result<(u64, usize)>` plus a `Cursor` over a byte slice with checked `read_u8/u16/u32/u64/i64/varint/bytes/string(max)` that returns `session.damaged` on any overrun or violation; make T013 pass
- [ ] T015 [P] Create `crates/session/src/limits.rs`: format constants (`FORMAT_MAJOR = 1`, `FORMAT_MINOR = 0`, header, section and footer sizes, `MAX_TEXT_BYTES = 4096`, `MAX_SECTIONS = 64`, the 64 KiB I/O buffer size) and `LoadLimits` (`storage: la_storage::Limits`, `max_file_bytes` default 64 GiB) with `Default` and a `validate()`; unit tests for the defaults and `validate`
- [ ] T016 [P] Write the unit tests in `crates/session/src/format.rs` first: header and footer encode/decode round trip and the exact byte layout of the format contract (write the expected bytes by hand in the test, independent of the encoder); section frame round trip; every rejection of the contract with the right code — wrong magic or too short (`not_a_session`), major 2 (`unsupported_version` naming both versions), minor 7 accepted, reserved header or section flag bit set, header CRC wrong, footer magic or CRC wrong, header and footer lengths differing, file length different from the real size (`truncated` when shorter, `damaged` when longer), section count 0, 1 or 65, bad tag characters, tags out of order or duplicated (`damaged`), `META`/`CAPT` not first and second, payload length beyond the remaining bytes (`truncated`), section CRC wrong
- [ ] T017 Implement `crates/session/src/format.rs` (pure, no I/O): `Header`, `SectionFrame`, `Footer` encode/decode, `SectionTag` validation, `check_order(tags)` (META, CAPT, then strictly ascending), the checks above returning typed errors; make T016 pass
- [ ] T018 [P] Write the unit tests in `crates/session/src/meta.rs` first: round trip of descriptions covering simulated with and without seed, a trigger of every kind, multi-byte device names, an empty-but-valid name accepted only if `restore` accepts it, start times before 1970, at nanosecond 999,999,999 and far in the future; the exact byte layout of one hand-written example; rejections with `session.damaged` or `session.inconsistent`: reserved flag bits, seed bit with missing bytes, text length above 4,096 or beyond the payload, invalid UTF-8, trailing bytes after the last field, an enabled mask outside the channels, a trigger on a disabled channel, nanoseconds above 10^9 - 1
- [ ] T019 Implement `crates/session/src/meta.rs`: `encode(&CaptureDescription) -> Result<Vec<u8>>` (checks the text length and the representable start time, `session.invalid_input` otherwise) and `decode(&[u8]) -> Result<CaptureDescription>` going through `SavedDescription` and `CaptureDescription::restore`, mapping `restore` errors to `session.inconsistent`; make T018 pass
- [ ] T020 [P] Write the unit tests in `crates/session/src/fs.rs` first: the real file system creates a file exclusively (a second creation of the same name fails with `AlreadyExists`), writes, syncs, renames over an existing file replacing it atomically (old content gone, new complete), removes, lists a directory and reports a file's age; `sync_dir` succeeds on Unix and is a documented no-op on Windows
- [ ] T021 Implement `crates/session/src/fs.rs`: the `FileSystem` trait (`create_exclusive`, an object-safe `WriteSink` with `write_all`/`flush`/`sync_all`, `open_read`, `rename`, `remove_file`, `sync_dir`, `list_dir`, `modified_age`) and `RealFileSystem`; `sync_dir` uses `File::open(dir)?.sync_all()` on Unix and does nothing on Windows with a rustdoc note of the platform limit; make T020 pass

**Checkpoint**: storage and devices additions merged in the branch; checksum, varints, framing, description codec and file-system seam are tested; `cargo test --workspace --all-features` and clippy are green

---

## Phase 3: User Story 1 - Save a capture and load it back unchanged (Priority: P1) MVP

**Goal**: a capture and its description survive a save and a load exactly, deterministically
and compactly.

**Independent Test**: `cargo test -p la-session --test round_trip`

### Tests for User Story 1 (write first, must fail)

- [ ] T022 [P] [US1] Write `crates/session/tests/common/mod.rs`: helpers to record a capture from a seeded generator (random signals with a chosen mean gap, optional gaps at chosen positions, optional `close`/`close_interrupted`), to build a `CaptureDescription` (through `la_devices` `VirtualProvider` acquisition or `SavedDescription` + `restore`), a temporary-directory guard that removes its directory on drop, and `assert_same_capture(a, b)` comparing length, channels, sample rate, status, gaps, `initial_high`, `transition_count`, every transition of every channel via `transitions`, `level_at` at sampled positions and viewport probe counts for a few windows
- [ ] T023 [P] [US1] Write `crates/session/tests/round_trip.rs` (FR-001..006, SC-001, SC-002): at least 1,000 seeded random captures (1 to 32 channels, lengths from 0 to 200,000, mean gaps from 1 to 5,000, with and without gaps and flags) save and load equal (`assert_same_capture` and `description` accessors equal); an empty capture, a one-sample capture, a 32-channel capture and a capture whose first position is a gap; the capture description with a multi-language device name, a seed, a trigger and a start time before 1970; saving the same session 100 times produces 100 identical files; two committed reference files `crates/session/tests/fixtures/reference/*.lasession` (generated once by an ignored test `regenerate_reference_files`, committed) are byte-identical to what the writer produces now; the file size of a 1 M-sample capture with 1,000 transitions is within a few hundred bytes of the formula in `research.md` R4 and an idle channel adds a constant number of bytes; loading uses at most the capture's own memory (`Capture::memory_used` of the loaded capture is equal to a recorded one for the same data)
- [ ] T024 [P] [US1] Write `crates/session/tests/capture_codec.rs` or the unit tests of `crates/session/src/capture_codec.rs` first: encode then decode of hand-built snapshots; the exact byte layout of one small hand-written capture (gaps with deltas, a channel table, streams); the sizing pass equals the real encoded size for 1,000 random captures; the decoder rejects each violation of the format contract's CAPT rules with `session.damaged` or `session.inconsistent` (flag bits, stream length different from the decoded bytes in both directions, transition count larger than the stream can hold, trailing bytes, gap deltas of zero after the first, gap beyond the length, length 0 with transitions, initial level above 1, level 1 for a channel without stored samples)

### Implementation for User Story 1

- [ ] T025 [US1] Implement `crates/session/src/capture_codec.rs`: `encoded_size(&Snapshot) -> Result<u64>` (the sizing pass), `encode(&Snapshot, &mut impl SectionWriter, control)` streaming the fixed fields, gaps, channel table and per-channel varint streams through a writer that updates the section CRC and the progress counter (`interrupted` is set when the snapshot's status is not closed), and `decode(payload_reader, limits) -> Result<Capture>` that reads the fixed fields, gaps and channel table, builds a `CaptureRestorer`, then decodes each stream with bounded reads, checks the stream length and count exactly and maps `storage.invalid_restore` and `storage.memory_limit` to `session.inconsistent` and `session.memory_budget`; make T024 pass
- [ ] T026 [US1] Implement `crates/session/src/session.rs`: `SectionTag`, `Section`, `SessionData` (`new`, `with_section` rejecting reserved and duplicate tags, `from_session`), `Session` (`description`, `reader`, `snapshot`, `extras`) per the API contract, with unit tests for the tag and duplicate rules
- [ ] T027 [US1] Implement `crates/session/src/write.rs`: `save_blocking(path, data, control)` and `save_with(fs, path, data, control)`: validate the path (non-empty, not a directory, parent exists) and the inputs before touching the disk, compute the whole file size, create the unique temporary file exclusively (`.<name>.<16 hex>.lasave-tmp`, the hex from process id, a counter and the clock), write header, sections in canonical order (META, CAPT, extras ascending), footer through a 64 KiB buffer with per-section CRC, flush, `sync_all`, rename over the target, `sync_dir`; the temporary file is removed on every error path (a small drop guard); returns `SaveReport`
- [ ] T028 [US1] Implement `crates/session/src/read.rs`: `load_blocking(path, limits, control)` and `load_with(fs, path, limits, control)`: open, check the real length against `max_file_bytes` (`session.too_large`) and the header, read the header, then each section frame through a bounded reader (the section's remaining length is checked before reading), verify each section CRC after its payload, decode `META` and `CAPT` as described in `research.md` R7, keep other sections as `Section` values (counted in the memory budget; an unknown critical section is `session.unknown_critical_section`), read and check the footer and that no byte follows, apply the cross-checks of the format contract, return `Session`
- [ ] T029 [US1] Wire the public API in `crates/session/src/lib.rs` (re-exports of `SessionData`, `Session`, `Section`, `SectionTag`, `LoadLimits`, `Control`, `Progress`, `OperationState`, `SaveReport`, `save_blocking`, `load_blocking`) with a minimal `Control` (cancel flag and progress atomics, completed in US6); write and commit the reference files with the ignored regeneration test; make T023 pass
- [ ] T030 [US1] Mutation checks (record in the commit message): drop the sort of optional sections, change one varint delta by one, skip the gap list, write the wrong initial level, forget the interrupted flag; confirm `round_trip`/`capture_codec` tests fail each time, then restore
- [ ] T031 [US1] Run `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`; mark T004–T031 done and commit the phase (Conventional Commits, scope `session`/`storage`/`devices`, one commit per logical change)

**Checkpoint**: round trip works; US1 independently demonstrable

---

## Phase 4: User Story 2 - Saving never damages or loses data (Priority: P1)

**Goal**: a save either completes entirely or changes nothing, whatever fails.

**Independent Test**: `cargo test -p la-session --test atomic_save --test kill_save`

### Tests for User Story 2 (write first, must fail)

- [ ] T032 [P] [US2] Implement the fault-injecting file system in `crates/session/src/fault.rs` (feature `test-support`): a `FaultFs` wrapping `RealFileSystem` with a `FaultPlan` that fails the n-th call of a chosen operation kind (`create`, `write`, `flush`, `sync_all`, `rename`, `sync_dir`, `remove`, `list`) with a chosen `io::ErrorKind` (including `StorageFull`/`WriteZero`, `PermissionDenied`, `NotFound`), counts every call, and can short-write; unit tests that each injection point fires exactly once
- [ ] T033 [P] [US2] Write `crates/session/tests/atomic_save.rs` (FR-007..012, SC-003): for a good existing file, enumerate every call the save makes (record a clean run with `FaultFs`, then fail call k for every k and every kind, at least 20 distinct points): after each, the old file is byte-identical and loads, no `.lasave-tmp` file remains, the error is typed (`session.io_write` with an actionable detail, never a panic) and the report names the step; the same for saving a new file (the target does not exist afterwards); disk full on the last write; a refused rename; a failing `sync_dir` after the rename is reported but the new file is complete (documented outcome: the save is reported as failed only if the rename did not happen — decide and test one rule); an empty path, a directory, a path whose parent does not exist and a read-only directory fail before any byte is written (assert the directory listing is unchanged); a leftover temporary file of the same target older than one hour is removed by a later save, a younger one and unrelated files (including a file with a similar but different name) are never touched; two saves of different content to the same path from two threads end with exactly one of the two complete files; a path with non-ASCII characters and spaces works; a save whose data hits the byte count promised by the sizing pass exactly
- [ ] T034 [P] [US2] Write `crates/session/tests/kill_save.rs` (SC-003, FR-007): the test binary re-runs itself as a child (`std::env::current_exe()` with an environment variable and `--exact kill_save_child`) that saves a 20 MB-scale session to a given path in a loop with alternating content A and B; the parent kills it at 1,000 random moments (seeded delays from 0 to 40 ms) and after each kill checks that the target is absent-or-complete: it loads and equals A or B exactly (never a mixture, never truncated); leftover temporary files are tolerated and then removed by one final save; the parent restarts a child each time (spawn cost kept low by reusing a small session); the test is marked with a generous time budget and skips with a clear message if the child cannot be spawned

### Implementation for User Story 2

- [ ] T035 [US2] Route `crates/session/src/write.rs` completely through `FileSystem` (every call goes through the trait), add the stale-leftover cleanup (`list_dir`, strict pattern `.<target name>.<16 hex>.lasave-tmp`, age above one hour, best effort, errors ignored and logged at `debug`), make the temporary-file guard remove the file on every error, cancellation and panic path, map I/O errors to `session.io_write` with a detail that says what to do (free space, check permissions, choose another location) and the operating system error as the source; make T033 pass
- [ ] T036 [US2] Make T034 pass: add the child entry point and fix whatever the kills expose; mutation checks: rename before `sync_all`, write directly to the target, skip the temporary-file removal on error; confirm the tests catch each
- [ ] T037 [US2] Run fmt, clippy, tests; mark T032–T037; commit the phase

**Checkpoint**: saving is atomic under injected failures and process kills

---

## Phase 5: User Story 3 - Loading rejects corrupt and hostile files safely (Priority: P1)

**Goal**: every damaged, truncated, lying or crafted file is a typed error, never a crash, hang or
over-budget allocation.

**Independent Test**: `cargo test -p la-session --test corrupt_files --test truncation_and_flips --test fuzz`

### Tests for User Story 3 (write first)

- [ ] T038 [P] [US3] Write `crates/session/tests/truncation_and_flips.rs` (FR-014, SC-004): for three representative files (small with gaps and flags, a multi-channel one with several thousand transitions, one with an unknown optional section) a truncation at every length from 0 to size-1 fails with `session.not_a_session`, `session.truncated` or `session.damaged`; a single-bit flip at every bit of the file fails with a typed error and never loads as valid; appending 1, 2, 3 and 4,096 bytes fails; each result is checked for the expected code family by region (magic region: `not_a_session`; header: `damaged`/`unsupported_version`; a section: `damaged`)
- [ ] T039 [P] [US3] Write `crates/session/tests/corrupt_files.rs` (FR-013, FR-015..019, SC-006): files that are not session files (empty, one byte, another format's magic such as PNG and ZIP, random bytes, text) → `not_a_session`; structurally valid files with valid CRCs (build them with a test-only builder that writes arbitrary payloads and recomputes CRCs) whose content breaks each data-model rule (transitions out of order, equal positions, a position beyond the length, inside a gap, before the first stored sample, a gap past the end, adjacent gaps, a channel count that disagrees with the description, an applied rate that disagrees) → `session.inconsistent`; a file declaring huge counts and sizes (channel stream length `u64::MAX`, transition count `u64::MAX`, gap count `u64::MAX`, text length `u64::MAX`, payload length `u64::MAX`, 65 sections) is rejected with `too_large`/`truncated`/`damaged` and the test's counting allocator (a `#[global_allocator]` in this test file) shows no allocation above 1 MiB beyond the file size (SC-006); a file whose capture needs more than the budget (`Limits { memory_budget_bytes: 1 MiB, .. }`) fails with `session.memory_budget` before reading transitions (assert the number of bytes read from the file stays small through a counting `Read` wrapper via `load_with` and a test file system); a file larger than `max_file_bytes` → `too_large`; a file that changes while it is read (the test file system appends or truncates mid-read) → a typed error or a correct session; an unreadable path (missing, a directory, permission denied through `FaultFs`) → `session.io_read` with an actionable detail; loading the same file twice at once from two threads; deep or repetitive structures cannot recurse (the format has no nesting: assert by test of a maximal section count)
- [ ] T040 [P] [US3] Write `crates/session/tests/fuzz.rs` (FR-033, SC-005): a deterministic mutation fuzzer (xorshift, seeds printed on failure) over the fixtures and generated files: bit flips, byte sets, truncation, insertion and deletion of bytes, duplication and swapping of regions, splicing two files, boundary values (0, 1, 127, 128, 255, 65,535, `u32::MAX`, `u64::MAX`) written at the offsets of length, count and delta fields, and — in half of the cases — recomputing the affected CRCs (header, section, footer) so structural and semantic checks are reached; every input runs under a 5 s watchdog (a thread with a channel timeout) and the counting allocator asserts the peak allocation stays below the configured budget plus 1 MiB; the outcome must be a typed error or a session that passes `assert_consistent` (every transition inside the length and outside gaps, gaps sorted); `fuzz_default` runs 20,000 inputs, `#[ignore] fuzz_one_million` runs 1,000,000; the failing input is written to `target/fuzz-failures/` with its seed
- [ ] T041 [P] [US3] Run T038–T040 now: whatever fails is a defect of `read.rs`/`format.rs`/the codecs (list them); if a test passes immediately, run a mutation check (skip the section CRC check, skip the footer length check, accept a non-canonical varint, preallocate from a claimed count) and confirm it fails

### Implementation for User Story 3

- [ ] T042 [US3] Harden `crates/session/src/read.rs` and `crates/session/src/capture_codec.rs` until T038–T041 pass: every claimed size is compared with the bytes remaining in the file or section before any read or allocation; vectors grow only with decoded data (no `with_capacity` from a file value); the memory estimate from the channel table is checked before decoding streams; a counting reader keeps total reads bounded; the reader tolerates a file that changes size mid-read; all error details name what is wrong and which limit was exceeded, without file content
- [ ] T043 [US3] Add `.github/workflows/fuzz.yml`: weekly (`schedule`) and `workflow_dispatch`, `ubuntu-latest` and `windows-latest`, release build, `cargo test -p la-session --release --test fuzz -- --ignored`, `continue-on-error: true`, upload `target/fuzz-failures/*` as an artifact on failure; pinned action versions copied from `bench.yml`; document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md` (informational, not in `scripts/protection.json`)
- [ ] T044 [US3] Run fmt, clippy, tests (default fuzz in debug and the one-million run once locally in release, record the time and result for `verification.md`); mark T038–T044; commit the phase

**Checkpoint**: hostile files cannot hurt the loader; the fuzzers are in CI

---

## Phase 6: User Story 4 - "Simulated" and "incomplete" survive the file (Priority: P2)

**Goal**: the marks and the seed cannot be lost by saving, loading or editing.

**Independent Test**: `cargo test -p la-session --test marks`

### Tests for User Story 4 (write first)

- [ ] T045 [P] [US4] Write `crates/session/tests/marks.rs` (FR-020..022, SC-010): a real acquisition on the virtual device (`la_devices::DeviceRegistry` with a seed, a finite length) is saved and loaded: `description().simulated()` is true and `seed()` equal; after five more save/load cycles (`SessionData::from_session`) both are unchanged; a hardware-style description (`simulated == false`, built with `SavedDescription`) stays false; an interrupted capture (device disconnect injected through a scripted virtual failure), a capture with gaps and a capture stopped by the memory limit stay incomplete with equal gaps, `lost_positions` and flags after save and load; a snapshot of a capture that is still recording is saved as interrupted; for the mark bytes (simulated bit, seed bit and seed value in META, the flag byte in CAPT) every single-bit change inside the file is rejected (CRC) and every change followed by CRC repair is either rejected or yields a session whose marks differ from the original only if the repair was valid (assert the original is never reported with a cleared mark: the changed file's loaded marks equal the changed bytes, proving what the file says is what the loader reports, and that without repair it is rejected); a compile-fail doctest on `Session::description` documents that no setter exists (reuse the `CaptureDescription` guarantees); mutation checks: drop the simulated bit when encoding, drop the interrupted flag, write seed 0 for none

### Implementation for User Story 4

- [ ] T046 [US4] Fix whatever T045 exposes in `meta.rs` and `capture_codec.rs` (the marks are encoded in the META flags and the CAPT flags exactly as the format contract says); document in the `Session` rustdoc that the marks are read-only and why; make T045 pass
- [ ] T047 [US4] Run fmt, clippy, tests; mark T045–T047; commit the phase

**Checkpoint**: simulated and incomplete are carried by the file and cannot be cleared

---

## Phase 7: User Story 5 - The format can grow without breaking files (Priority: P2)

**Goal**: optional sections survive, unknown critical sections and newer majors are refused
clearly, and old files stay loadable forever.

**Independent Test**: `cargo test -p la-session --test compatibility --test independent_reader`

### Tests for User Story 5 (write first)

- [ ] T048 [P] [US5] Write `crates/session/tests/compatibility.rs` (FR-023..027, SC-011): a file with two unknown optional sections (`TEST`, `ZZZZ`, payloads of 0 and 1 MB) loads, the extras are exposed in ascending tag order with their payloads and flags, and load → `SessionData::from_session` → save gives a file byte-identical to the input; an unknown section with the critical bit → `session.unknown_critical_section` naming the tag and saying a newer program is needed; major 2 → `unsupported_version` with both versions in the detail; minor 5 with only known sections loads; a duplicate tag, descending tags, a tag with a space or non-ASCII byte, and `META`/`CAPT` supplied as extras (`with_section` → `session.invalid_input`) are rejected; every file under `crates/session/tests/fixtures/` (the reference files of US1 plus `v1_0/*.lasession` written by this version: a plain capture, a capture with gaps and flags, a simulated capture with trigger and seed, a capture with an extra section) loads to the content recorded next to it in `expected.rs` (a hand-written table: lengths, channel counts, transition counts, a hash-free list of sampled levels, flags); a test regenerates the `v1_0` files from code and compares them byte for byte, failing with a message that explains that a format change needs a new version and a new fixture directory
- [ ] T049 [P] [US5] Write `crates/session/tests/independent_reader.rs` (SC-012): a second parser written from `contracts/file-format.md` only (std only; its own CRC-32C, varint, header, section, footer, META and CAPT decoders, sharing no code with the crate) that parses every fixture and 200 generated files and compares every field, every gap and every transition with the crate's loader; it also writes one file from a description of the contract (its own encoder) that the crate loads and compares; the test file starts with a comment stating that it must not import crate internals

### Implementation for User Story 5

- [ ] T050 [US5] Create the `v1_0` fixtures with an ignored `regenerate_fixtures` test in `crates/session/tests/compatibility.rs` and commit the generated files and `expected.rs`; fix whatever T048 and T049 expose in `format.rs`/`read.rs`/`write.rs` (canonical order, flag handling, extras preservation) and in the contract text where the independent reader found an ambiguity (update `specs/003-session-file-format/contracts/file-format.md`)
- [ ] T051 [US5] Run fmt, clippy, tests; mark T048–T051; commit the phase

**Checkpoint**: the format can be extended and every released file keeps loading

---

## Phase 8: User Story 6 - Saving and loading never block the application (Priority: P2)

**Goal**: save and load run in the background with progress and cancel, and a running capture can
be saved.

**Independent Test**: `cargo test -p la-session --test background --test running_capture`

### Tests for User Story 6 (write first, must fail)

- [ ] T052 [P] [US6] Write `crates/session/tests/background.rs` (FR-028, FR-029, FR-031, FR-032, SC-008): `start_save`/`start_load` of a 50 MB-scale capture return in under 10 ms (debug: 50 ms; two named constants as in item 004), `progress()` is non-decreasing across 1,000 observations, never above the total, equal to the total and `Succeeded` at the end, and the outcome is set exactly once and identical on every read; `cancel()` during a save returns at once, the operation ends within 100 ms (debug: 500 ms) with `session.cancelled` and state `Cancelled`, the old target is byte-identical and no `.lasave-tmp` remains; cancel during a load ends with `session.cancelled` and no session; cancel before the worker starts and after it finished are harmless; a save that fails (read-only directory through `FaultFs` via the internal `start_save_with`) reports the same typed error `save_blocking` returns; a worker panic injected through a panicking `FileSystem` (test double) becomes `session.worker_failed` without crashing the test and leaves the in-memory capture usable; `wait_timeout` never waits forever; two operations run side by side independently; dropping the handle detaches the worker without a panic
- [ ] T053 [P] [US6] Write `crates/session/tests/running_capture.rs` (FR-030, SC-009): start an acquisition on the virtual device without a length limit, take its description and `capture()` reader, save it with `start_save` while it records (several times, at different moments); each file loads as a valid session that is closed, interrupted and incomplete, whose length is at most the length at the end of the save and whose samples equal the recording's prefix (compare with the final capture); the acquisition keeps running and its final capture is complete and identical to a control run with the same seed that was not saved (assert equal transitions); the worst `Capture::append` latency observed during saves, measured by sampling `progress().positions` growth with 1 ms ticks in a helper thread, never shows a stall above 10 ms in release builds (debug: 100 ms); stop the acquisition cleanly at the end

### Implementation for User Story 6

- [ ] T054 [US6] Implement `crates/session/src/operation.rs`: `Control` (an `AtomicBool` cancel flag, `AtomicU64` done and total bytes, an `AtomicU8` state; cheap to clone), `Progress`, `OperationState`, `SaveOperation`, `LoadOperation`, `start_save`, `start_load`, `start_save_with`/`start_load_with` (taking a `FileSystem`, `pub(crate)` plus `test-support` re-export) using `la_diagnostics::spawn_worker`; the worker body calls `save_with`/`load_with`, the supervisor stores the outcome once (a `Mutex<Option<..>>` plus `Condvar`, recovered with `lock_recover`) and turns a `WorkerFailure` into `session.worker_failed`; `write.rs` and `read.rs` check the cancel flag at least every 64 KiB and update progress monotonically; a cancelled save removes its temporary file, a cancelled load drops the half-built restorer; make T052 pass
- [ ] T055 [US6] Make T053 pass: ensure `SessionData` captures the snapshot once at creation (a running capture is written from that consistent prefix, flagged interrupted by the codec because `status().closed` is false) and that nothing in the save path takes a lock shared with the recorder; fix whatever the measurements expose
- [ ] T056 [US6] Run fmt, clippy, tests; mark T052–T056; commit the phase

**Checkpoint**: all user stories (US1–US6) are independently functional

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: throughput evidence, documentation, constitution gates

- [ ] T057 [P] Implement `crates/session/benches/session_io.rs` (`harness = false`, same style as `crates/storage/benches/ingest.rs` and `crates/devices/benches/virtual_throughput.rs`): record a 100 M-sample, 16-channel typical capture (mean gap 50) through the real storage, then time `save_blocking` and `load_blocking` (file size, MB/s of file, million positions per second), also for 1 and 32 channels and a sparse signal; write `target/la-session-bench/session_io.json` under the workspace `target` directory (resolve it from `CARGO_MANIFEST_DIR` as the devices benchmark does) and merge a `session` section into `crates/session/benches/baseline.json` with `--write-baseline`; fail if SC-007 (each under 10 s for the 16-channel case) is not met or a result is more than 5x slower than the baseline
- [ ] T058 Run the benchmark on the maintainer machine, commit the first `crates/session/benches/baseline.json` (machine description and numbers) and record the SC-007 result in `specs/003-session-file-format/checklists/verification.md`; if SC-007 is not met, profile and record what limits it before deciding whether to adjust the criterion honestly
- [ ] T059 [P] Extend `.github/workflows/bench.yml` with a step `cargo bench -p la-session --bench session_io` (non-blocking, artifacts include `target/la-session-bench/*.json`) and document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md`; do not add it to `scripts/protection.json`
- [ ] T060 [P] Review the public API against `specs/003-session-file-format/contracts/*.md` and update the contracts where the implementation differs (names, signatures, extra codes, behaviour fixed during implementation, the rule chosen for a failing `sync_dir`); ensure rustdoc exists for every public item (`#![deny(missing_docs)]`) and that `RUSTDOCFLAGS="-D warnings" cargo doc -p la-session -p la-storage -p la-devices --no-deps --all-features` passes
- [ ] T061 [P] Data-safety review (constitution Principle I): search `crates/session/src` and the new storage and devices code for `unwrap(`, `expect(`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow` and `#[expect`; matches are allowed only in `#[cfg(test)]` modules and in the clearly commented fault-injection code (`fault.rs`, feature `test-support`); also search for `with_capacity`, `vec![0` and `reserve` fed by values read from a file and for `as usize`/`as u32` casts; record the result in `specs/003-session-file-format/checklists/verification.md`
- [ ] T062 [P] Record the remaining constitution gates in the same checklist: localization (no UI strings, stable `session.*` codes, English docs), third-party code (`Cargo.lock` gains only workspace packages; nothing reused from PulseView/sigrok/Saleae; no new dependency), Principle I (atomic write, validated loader, fuzzing, kill test), Principle X note (the atomic-write routine is reusable by the log exporter), platforms (Windows locally, Debian 13 and MSRV 1.85 through CI; the Windows directory-fsync limit)
- [ ] T063 Run the whole quickstart (`specs/003-session-file-format/quickstart.md`, steps 1–8) on Windows; fix any gap and update the quickstart where reality differs
- [ ] T064 Update `roadmap.md` (mark item 003 implemented) and the `README.md` status line (session files: save and load of captures, atomic and checksummed; still no UI and no real drivers)
- [ ] T065 Final local gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo +1.85 test --workspace`, then push the branch, open a pull request and wait for all required checks on both operating systems; fix any CI-only failure (item 004 showed a timing race that only a slower runner exposed: wait for states instead of asserting them instantly)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup — blocks all stories. The storage and devices additions (T004–T010) are independent of each other and of the pure blocks (T011–T021), which can proceed in parallel; `capture_codec` (T025) needs the restorer (T007), `meta` (T019) needs `restore` (T010)
- **US1 (Phase 3)**: depends on Foundational (needs the codecs, framing and the file-system seam)
- **US2 (Phase 4)**: depends on US1 (it hardens `write.rs`); the fault file system (T032) only needs Foundational
- **US3 (Phase 5)**: depends on US1 (it hardens `read.rs`); the tests can be written as soon as US1's loader exists
- **US4 (Phase 6)**: depends on US1; independent of US2 and US3
- **US5 (Phase 7)**: depends on US1; independent of US2–US4
- **US6 (Phase 8)**: depends on US1 and US2 (cancellation and cleanup in `write.rs`) and on US3 (cancellation in `read.rs`)
- **Polish (Phase 9)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Pure code before I/O, I/O before background operations
- Run clippy with `-D warnings` at the end of every phase and never commit a failing gate

## Parallel Execution Examples

- Foundational: T004 + T009 + T011 + T013 + T015 + T016 + T018 + T020 (different files, tests first)
- US1: T022 + T023 + T024 (tests in different files)
- US2: T032 + T033 + T034
- US3: T038 + T039 + T040
- US4 and US5 can be developed side by side after US1 (different test files and small code areas)

## Implementation Strategy

**MVP**: Phases 1–3 (Setup, Foundational, US1): a capture and its description survive save and
load exactly. Then US2 and US3 (the two guarantees that make files safe to trust, both P1), then
US4–US6, then Polish. Commit after every phase; keep the gate green at every commit.
