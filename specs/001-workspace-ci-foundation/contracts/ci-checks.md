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
| `test (windows)` / `test (debian)` | both | `cargo test --workspace --all-features` | Any test fails. |
| `msrv (windows)` / `msrv (debian)` | both | Build and test with the toolchain named in `rust-version` | Does not build or pass on the MSRV. |
| `deny (windows)` / `deny (debian)` | both | `cargo deny --all-features check` | Known vulnerability, unmaintained/yanked crate, disallowed license, banned or unknown source. Output names the crate and reason. |

## Non-blocking checks

| Check name | OS | Command | Behavior |
|------------|----|---------|----------|
| `commits` | debian | `scripts/check-commits.sh <base>..<head>` | Reports commit messages that do not match Conventional Commits with a scope; `continue-on-error`, warning only. |

## Rules

- Actions are pinned by full commit SHA; the Rust toolchain by `rust-toolchain.toml`.
- Repository owner enables the table above as required checks on `main`; the setup
  command is in `quickstart.md`.
- A new check is added to this table in the same change that adds it to the workflow.
