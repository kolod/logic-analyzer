# Verification Record: Signal Storage Core

**Purpose**: Evidence for the measurable success criteria and constitution gates of
feature 002, filled in as the corresponding tasks are completed.

**Feature**: [spec.md](../spec.md)

## Performance baseline (T029)

Recorded by `cargo bench -p la-storage --bench viewport -- --write-baseline` into
`crates/storage/benches/baseline.json` on the maintainer machine (Windows, x86_64,
Intel64 Family 6 Model 183, 32 logical cores), release profile, 2,000 pixels:

| Capture | Channels | Median | p95 | Probes |
|---------|----------|--------|-----|--------|
| synthetic, 10^6 transitions per channel | 16 | 446 us | 470 us | 367,584 |
| synthetic, 10^7 | 16 | 647 us | 720 us | 511,968 |
| synthetic, 10^8 | 16 | 1,527 us | 1,676 us | 703,952 |
| synthetic, 10^9 | 16 | 2,739 us | 3,063 us | 895,968 |
| real (appended), 10^6 | 1 | 30 us | 41 us | 22,974 |
| real (appended), 10^7 | 1 | 75 us | 102 us | 31,998 |

- **SC-002** (2,000 px x 16 channels on 10^9 transitions under 20 ms): **met**, 2.7 ms
  (about 7x headroom).
- **SC-003** (1,000x more transitions costs under 3x): **met**, probe growth from 10^6 to
  10^9 is 2.44x (deterministic, enforced by `tests/complexity.rs`).
- Caveat: synthetic captures are cache-friendly (every block shares one payload), so
  their timings are optimistic; the real-data rows are the more honest numbers.
- Mutation check of the complexity test: replacing the galloping search with a linear
  scan makes 4 of the 6 tests fail (10^9: about 998,000 probes against a bound of
  about 479,000); the original code was restored.

## Data safety, Principle I (T057)

Pending.

## Other constitution gates (T058)

Pending.
