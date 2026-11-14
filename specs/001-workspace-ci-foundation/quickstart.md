# Quickstart: Validate the Workspace and CI Foundation

Run on Windows 10/11 and on Debian 13. Prerequisite: `rustup` installed (the pinned
toolchain from `rust-toolchain.toml` is fetched automatically).

## 1. Build and test (User Story 2)

```text
cargo build --workspace
cargo test --workspace
```

Expected: both succeed with no extra steps.

## 2. MSRV is enforced (User Story 2)

```text
rustup toolchain install 1.85
cargo +1.85 test --workspace
```

Expected: passes. With an older toolchain (for example `cargo +1.70 build`) the build
fails with a message naming the required `rust-version`.

## 3. Style, lints, dependencies (User Story 1, 3)

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny --all-features check
```

Expected: all pass. Adding `let x = some_vec[0];`, `.unwrap()` or `.expect("…")` to a
production file makes clippy fail; the same code inside a `#[cfg(test)]` module
passes.

## 4. Typed errors, no panic on malformed input (User Story 3)

```text
cargo test -p la-core
```

Expected: the malformed-input tests return typed `Error` values; none panics.

## 5. Worker containment (User Story 4)

```text
cargo test -p la-diagnostics worker
```

Expected: a deliberately panicking worker is reported as `WorkerFailure`; sibling
workers and shared state remain usable; the log contains the panic message, location
and worker name. The test is repeated 1,000 times in a loop test (SC-005).

## 6. Bounded log (User Story 5)

```text
cargo test -p la-diagnostics log
```

Expected: after writing more than 10× the configured limit, total size on disk stays
within the limit, newest entries are present, every retained line is valid JSON;
an unwritable directory does not stop initialization.

## 7. CI gate (User Story 1)

1. Push a branch with a deliberate failure (format, lint, test, a disallowed
   dependency, a build that breaks only on the MSRV) and open a pull request.
2. Expected: the matching check fails on the affected OS and merging is blocked.
3. Repository owner enables required checks once (replace `OWNER/REPO`):

```text
gh api -X PUT repos/OWNER/REPO/branches/main/protection --input protection.json
```

where `protection.json` lists the check names from
[contracts/ci-checks.md](contracts/ci-checks.md) under `required_status_checks`.

## 8. Contributor guide (User Story 6)

Open `CONTRIBUTING.md`: it must state build/test commands for both OSes, the MSRV, the
English-only rule, Conventional Commits with scope, the `NNN-short-name` branch format
and the merge rules. `scripts/check-commits.sh origin/main..HEAD` reports
non-conforming commits.
