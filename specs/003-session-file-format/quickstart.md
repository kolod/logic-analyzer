# Quickstart: Validate the Session File Format

Prerequisite: the workspace builds (`cargo build --workspace`). All commands run from the
repository root on Windows 10/11 and Debian 13.

## 1. Storage additions

```text
cargo test -p la-storage --test restore
```

Expected: a capture rebuilt from its transitions answers every query like the recorded one; every
rule of the data model (order, range, gaps, flags, memory budget) is enforced and a rejected build
leaves nothing behind.

## 2. Round trip (User Story 1)

```text
cargo test -p la-session --test round_trip
```

Expected: at least 1,000 generated captures (empty, one sample, 32 channels, gaps, incomplete) come
back with identical queries and metadata; the same session saved 100 times gives identical bytes; the
committed reference files match byte for byte; the file size follows the number of transitions.

## 3. Safe saving (User Story 2)

```text
cargo test -p la-session --test atomic_save --test kill_save
```

Expected: a failure injected at every step (20+ points: create, each write, flush, sync, rename,
directory sync, remove) leaves the previous file intact and no temporary file; 1,000 killed child
processes leave either the old or the new complete file; a leftover temporary file never blocks a save.

## 4. Hostile and damaged files (User Story 3)

```text
cargo test -p la-session --test corrupt_files --test truncation_and_flips --test fuzz
```

Expected: every truncation length and every single-bit flip of representative files is rejected with
a typed error; lying counts and sizes are rejected without large allocation; 20,000 mutated inputs
pass in seconds. The long run:

```text
cargo test -p la-session --release --test fuzz -- --ignored
```

runs one million inputs (also scheduled weekly in CI) with no panic, no hang and no over-budget
allocation.

## 5. Marks and compatibility (User Stories 4 and 5)

```text
cargo test -p la-session --test marks --test compatibility --test independent_reader
```

Expected: simulated, seed and incompleteness survive repeated save and load, and an edit of them inside a
file is detected; an unknown optional section survives; an unknown critical section and a newer major
version are refused with clear errors; every fixture in `tests/fixtures/` loads; the independently
written reader agrees with the loader on all fixtures.

## 6. Background operation and a running capture (User Story 6)

```text
cargo test -p la-session --test background --test running_capture
```

Expected: starting returns in milliseconds, progress is monotonic, cancel leaves no temporary file and no
change to the target, a worker panic becomes `session.worker_failed`; saving while the virtual device is
recording gives a valid, incomplete session and does not pause the recording.

## 7. Throughput (informational, recorded baseline)

```text
cargo bench -p la-session --bench session_io
```

Expected: save and load of 100 M samples on 16 channels (typical signal) each under 10 s (SC-007); the
numbers are compared with `crates/session/benches/baseline.json` and written to
`target/la-session-bench/session_io.json`.

## 8. Whole gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny --all-features check
```

The same checks run in CI on both operating systems.
