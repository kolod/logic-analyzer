# Contract: CI Checks

Workflow file: `.github/workflows/ci.yml`. Triggers: `pull_request`, `push` to `main`,
weekly `schedule` (dependency check only).

## Required status checks (blocking)

Operating systems: **windows** (`windows-latest`) and **debian** (`ubuntu-latest`
running in a `debian:trixie` container).

| Check name | OS | Command | Fails when |
|------------|----|---------|------------|
| `fmt (windows)` / `fmt (debian)` | both | `cargo fmt --all --check` | Any file is not formatted. |
| `clippy (windows)` / `clippy (debian)` | both | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Any lint or warning, including the panic-safety lints. |
| `test (windows)` / `test (debian)` | both | `cargo test --workspace --all-features` | Any test fails. This includes the localization parity check of `la-i18n` (`tests/parity.rs`): a key missing in a language, a changed placeholder, a missing plural form, an unused or undefined key and the other defects of `specs/005-i18n-foundation` fail the build with a report naming the file and key. |
| `msrv (windows)` / `msrv (debian)` | both | Build and test with the toolchain named in `rust-version` | Does not build or pass on the MSRV. |
| `deny (windows)` / `deny (debian)` | both | `cargo deny --all-features check` | Known vulnerability, unmaintained/yanked crate, disallowed license, banned or unknown source. Output names the crate and reason. |

## Non-blocking checks

| Check name | OS | Command | Behavior |
|------------|----|---------|----------|
| `commits` | debian | `scripts/check-commits.sh <base>..<head>` | Reports commit messages that do not match Conventional Commits with a scope; `continue-on-error`, warning only. |
| `bench (windows)` / `bench (debian)` | both | `cargo bench -p la-storage --bench viewport`, `--bench ingest`, `cargo bench -p la-devices --bench virtual_throughput`, `cargo bench -p la-session --bench session_io` and `cargo bench -p la-i18n --bench lookup` (workflow `bench.yml`, manual and weekly) | Records timing numbers as artifacts to compare with `crates/storage/benches/baseline.json` , `crates/devices/benches/baseline.json`, `crates/session/benches/baseline.json` and `crates/i18n/benches/baseline.json`; never blocks. The merge-blocking complexity check is the deterministic probe-count test `crates/storage/tests/complexity.rs`, part of `cargo test`. |
| `fuzz (windows)` / `fuzz (debian)` | both | `cargo test -p la-session --release --test fuzz -- --ignored` (one million mutated session files) , the 1,000-kill save test and `cargo test -p la-i18n --release --test stress -- --ignored` (one million hostile localization operations) (workflow `fuzz.yml`, manual and weekly) | Informational, never blocks; uploads the failing input as an artifact. A 20,000-input fuzz run and a 150-kill save test are part of the normal `cargo test`, which does block. |

## Rules

- Actions are pinned by full commit SHA; the Rust toolchain by `rust-toolchain.toml`.
- Repository owner enables the table above as required checks on `main`; the setup
  command is in `quickstart.md`.
- A new check is added to this table in the same change that adds it to the workflow.
