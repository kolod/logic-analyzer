# Quickstart: Validate the Device Abstraction and Virtual Device

Prerequisite: the workspace builds (`cargo build --workspace`). All commands run from the
repository root on Windows 10/11 and Debian 13.

## 1. Discovery and identity (User Story 1)

```text
cargo test -p la-devices --test discovery
```

Expected: with no hardware the list is exactly the virtual device (simulated, 32 channels,
full capabilities); a test provider's devices appear in the same shape; identities are
stable; a provider that hangs or lists an invalid device is omitted and `list()` still
returns instantly.

## 2. Configuration validation (User Story 2)

```text
cargo test -p la-devices --test config_validation
```

Expected: at least 30 invalid configurations (unknown channel, no channel enabled,
unsupported rate, unsupported trigger, trigger on a disabled channel, length beyond the
buffer depth, missing length on a buffered device, over-budget length, wrong state) are
rejected with the documented codes, and a counting device shows it received no call; the
valid ones are accepted and resolve to exactly what was requested.

## 3. Acquisition into the store (User Story 3)

```text
cargo test -p la-devices --test acquisition
```

Expected: start returns immediately; the capture grows while another thread reads it;
stop closes it within the documented time; a finite length finishes by itself with a
complete capture; the storage budget ending an acquisition leaves intact data and a
typed error; a second start on a busy device fails; two devices run independently.

## 4. Failures keep data (User Story 4)

```text
cargo test -p la-devices --test failures --test panic_injection
```

Expected: for disconnect, transport error, overrun, driver panic and driver hang the data
delivered before the failure reads back exactly, the capture is closed and incomplete, a
typed error is reported, the application keeps running, and delivered plus lost positions
equal the positions accounted for; 1,000 injected panics never crash anything.

## 5. Virtual device (User Stories 5 and 6)

```text
cargo test -p la-devices --test virtual_device
```

Expected: identical captures for the same seed (100 runs) and for any chunking and both
pacing modes, different captures for different seeds, the simulated mark on every capture
from the virtual device and on none from others, scripted losses at exact positions, edge
and level triggers with a reported position, no failures unless scripted.

## 6. Hot-plug and the harness (User Stories 7 and 8)

```text
cargo test -p la-devices --test hotplug --test conformance
```

Expected: 100 rapid connect/disconnect events arrive exactly once and in order; the shared
conformance suite passes for the virtual device and for scripted devices; a replayed
`RecordedTransport` script reproduces the same samples, gaps and errors every run.

## 7. Latency and throughput (informational, recorded baseline)

```text
cargo test -p la-devices --test ui_latency
cargo bench -p la-devices --bench virtual_throughput
```

Expected: every UI-facing call returns well within 10 ms even with a hanging driver
(test, generous margin); the benchmark reports the virtual device's rate for a sparse
32-channel signal (SC-010: at least 100 MS/s) and appends a section to
`crates/devices/benches/baseline.json`.

## 8. Whole gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny --all-features check
```

The same checks run in CI on both operating systems.
