# Research: Workspace and CI Foundation

All Technical Context unknowns are resolved below. Library facts were checked against
current documentation (tracing, cargo-deny).

## R1. Minimum supported Rust version (MSRV)

- **Decision**: MSRV `1.85` (the first release with edition 2024), declared as
  `rust-version` in the workspace `Cargo.toml`. CI builds and tests with exactly this
  toolchain in addition to the pinned development toolchain. If a dependency forces a
  higher floor, the MSRV is raised deliberately in its own commit and the guide is
  updated.
- **Rationale**: a recent but not bleeding-edge floor gives Debian 13 and Windows
  users a wide margin, supports `[workspace.lints]` and edition 2024, and is cheap to
  verify mechanically.
- **Alternatives considered**: tracking latest stable only (no verified floor, fails
  FR-002/FR-004); an older floor such as 1.75 (forbids edition 2024, more
  compatibility work for no user benefit).

## R2. Development toolchain pinning

- **Decision**: `rust-toolchain.toml` pins an exact stable release (initially
  `1.100.0`, the version installed on the maintainer machine) with the `rustfmt` and
  `clippy` components. `Cargo.lock` is committed. A toolchain bump is a reviewed
  commit.
- **Rationale**: FR-006 requires reproducible CI; unpinned `stable` changes lint
  results under contributors' feet.
- **Alternatives considered**: `channel = "stable"` (not reproducible); nightly (not
  needed).

## R3. Enforcing "no panic on external input" (FR-009–FR-011)

- **Decision**: workspace-level `[workspace.lints]` set to `deny` for the clippy
  lints `unwrap_used`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`,
  `panic`, `unreachable`, `todo`, `unimplemented`, `undocumented_unsafe_blocks`, and
  the rustc lint `unsafe_code` (`deny`, so a module that legitimately needs `unsafe`
  must opt in with `#[allow(unsafe_code)]` plus a `// SAFETY:` comment and review).
  Test code is relaxed through `clippy.toml` (`allow-unwrap-in-tests`,
  `allow-expect-in-tests`, `allow-indexing-slicing-in-tests`, `allow-panic-in-tests`)
  and, where no such option exists (arithmetic), a `#![cfg_attr(test, allow(...))]`
  in the test modules. Any exception in production code is a local
  `#[allow(clippy::...)]` with a `reason = "..."` string (stable lint attribute
  reasons), visible in review. CI runs clippy with `-D warnings` so `warn`-level
  lints also fail.
- **Rationale**: lints are the cheapest automatic, reviewable enforcement; no custom
  tooling to maintain.
- **Alternatives considered**: a custom CI grep (false positives/negatives, easy to
  bypass); `#![forbid(...)]` per crate (cannot be overridden with a documented
  exception, and duplicates configuration).
- **To verify during implementation**: the exact set of `allow-*-in-tests` options
  supported by the pinned Clippy; adjust the test relaxation mechanism, not the
  production rules.

## R4. Typed error model (FR-008)

- **Decision**: crate `la-core` exposes `Error` = a struct with `kind: ErrorKind` (a
  closed enum of failure categories), a stable `code` (a `&'static str` identifier
  such as `io.read`, used later by the localization layer to look up the user-facing
  text), an English `detail` for the log, and an optional boxed `source` cause.
  `Result<T>` alias. Derived with `thiserror`. The user-facing message is NOT stored
  in the error: it is derived from `kind` + `code` at the UI layer so it can be
  localized (roadmap item 005).
- **Rationale**: separates user-facing text from technical detail (spec US3
  scenario 4, Principle VI) and avoids hardcoded UI strings (Principle VII).
- **Alternatives considered**: `anyhow` (type-erased, no stable categories to
  localize against); one error enum per crate without a shared kind (no common
  user-message mapping).

## R5. Logging stack and size bound (FR-015–FR-019)

- **Decision**: `tracing` + `tracing-subscriber` (feature `json`) for structured
  events; entries written as one JSON object per line. A small in-project writer
  `BoundedLogWriter` (in `la-diagnostics`) implements `std::io::Write` and keeps the
  total size bounded using a fixed number of segment files (default 4 segments of
  `limit / 4` each): when the active segment is full the next segment is truncated
  and becomes active, so the oldest segment is always the one discarded. Each log
  line is written with a single `write` call and the writer tracks line boundaries,
  so a crash can damage at most the last line. Writes are synchronous (log volume is
  small; payload data is never logged, FR-019), so panic reports are durable.
- **Rationale**: `tracing-appender` only rotates by time (hourly/daily/never) and
  cannot enforce a size limit, so it does not satisfy FR-016. A ~150-line writer with
  thorough tests avoids an extra dependency (Principle V); it is listed in
  Complexity Tracking.
- **Alternatives considered**: `tracing-appender` rolling (time-based only, fails
  FR-016); the `file-rotate` crate (workable, but adds a dependency for behavior we
  need to test anyway; remains the fallback if the custom writer proves
  troublesome); `log` + `env_logger` (no structured fields).
- **Defaults**: total bound 8 MiB per process (4 segments × 2 MiB); minimum 64 KiB;
  both configurable via `LogConfig`.

## R6. Log location and multiple instances

- **Decision**: the log directory is resolved by a platform helper using the
  `directories` crate: Windows `%LOCALAPPDATA%\logic-analyzer\logs`, Debian
  `$XDG_STATE_HOME/logic-analyzer/logs` (default `~/.local/state/...`). The directory
  can be overridden by `LogConfig` (used by tests). Each process writes to its own
  files named `la-<start-timestamp>-<pid>.<segment>.log`, so concurrent instances can
  never interleave or corrupt each other's logs. At start-up, old instance logs
  beyond the 5 most recent instances are deleted, so total disk use is bounded by
  `5 × limit`.
- **Rationale**: per-process files avoid cross-process locking (a risk source for the
  stability goal) while keeping disk usage bounded.
- **Alternatives considered**: one shared file with advisory locking (cross-platform
  locking semantics differ, rotation races); no cleanup (unbounded disk use across
  runs).

## R7. Worker panic containment (FR-012–FR-014)

- **Decision**: `la-diagnostics::worker::spawn_worker(name, f)` wraps `f` in
  `std::panic::catch_unwind` inside a named `std::thread`, returning a handle whose
  `join()` yields `Result<T, WorkerFailure>` where `WorkerFailure` holds worker
  name, panic message and location. A process-wide panic hook installed once by
  `install_panic_hook()` logs every panic (message, location, thread name,
  backtrace if enabled) through `tracing` and flushes the log before the process
  continues or exits; it covers the main thread too. Release and dev profiles set
  `panic = "unwind"` explicitly (required for containment). Shared state guidance:
  workers mutate shared state transactionally (build the new value, then swap under
  the lock); the helper `lock_recover()` recovers a poisoned `std::sync::Mutex` and
  logs the event instead of propagating the poison.
- **Rationale**: `catch_unwind` at the thread boundary is the standard mechanism and
  needs no dependency; the hook gives the main-thread logging that
  `catch_unwind` alone cannot.
- **Alternatives considered**: `panic = "abort"` (cannot contain, violates
  Principle I); `parking_lot::Mutex` (no poisoning, but adds a dependency and does
  not prevent half-updated state — the transactional rule does).

## R8. CI design (FR-003–FR-007)

- **Decision**: one GitHub Actions workflow `ci.yml` triggered on `pull_request` and
  `push` to `main`. Matrix:
  - **Windows**: `windows-latest` runner (a Windows Server image; it is the hosted
    stand-in for Windows 10/11 — the documented assumption).
  - **Debian 13**: `ubuntu-latest` runner executing the job steps inside a
    `debian:trixie` container (Debian 13 is "trixie"); Rust is installed with
    `rustup` in the container.
  Jobs per OS: `fmt` (`cargo fmt --all --check`), `clippy`
  (`cargo clippy --workspace --all-targets --all-features -- -D warnings`), `test`
  (`cargo test --workspace --all-features`), `msrv` (build + test with the MSRV
  toolchain), `deny` (`cargo deny check`, installed with a pinned
  `taiki-e/install-action`). A separate non-blocking `commits` job (Debian only)
  checks Conventional Commits with a dependency-free script
  (`scripts/check-commits.sh`); it uses `continue-on-error` and reports a warning.
  All third-party actions are pinned by full commit SHA (resolved with
  `gh api` at implementation time) with the version in a trailing comment.
  Required status checks are the per-OS `fmt`, `clippy`, `test`, `msrv`, `deny`
  jobs; the repository owner enables them in branch protection (command documented
  in the quickstart). A weekly scheduled run re-executes `deny` so a newly published
  advisory surfaces without a code change.
- **Rationale**: matches FR-003–FR-007 and the edge cases; caching (`Swatinem/rust-cache`,
  pinned) keeps runs under the SC-003 budget.
- **Alternatives considered**: self-hosted runners (maintenance burden); a Debian
  VM action (slow, less reproducible than a container).

## R9. Dependency audit tool

- **Decision**: `cargo-deny` with a committed `deny.toml`: `[advisories]` (deny
  vulnerabilities and unmaintained, yanked = deny), `[licenses]` with an allow-list of
  GPL-3.0-compatible licenses (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0,
  Zlib, MPL-2.0, CC0-1.0) and a clear error naming the crate, `[bans]`
  (wildcards denied, duplicate versions warned), `[sources]` (unknown registries and
  git sources denied).
- **Rationale**: one tool covers vulnerabilities and licenses (FR-007) with
  readable output naming the offending crate; `cargo audit` covers advisories only.
- **Alternatives considered**: `cargo audit` + a separate license tool (two tools,
  two configs).

## R10. Repository hygiene (line endings, formatting)

- **Decision**: add `.gitattributes` (`* text=auto eol=lf`), `.editorconfig`, and
  `rustfmt.toml` with `newline_style = "Unix"`. This also removes the LF/CRLF warnings
  seen on Windows commits.
- **Rationale**: spec edge case on Windows vs Debian conventions; identical bytes on
  both OSes keep `cargo fmt --check` stable.
- **Alternatives considered**: relying on `core.autocrlf` (per-machine setting, not
  reproducible).

## R11. Project license metadata

- **Decision**: workspace `license = "GPL-3.0-only"` matching the committed `LICENSE`
  (GPL-3.0 text). Whether to relax to "or later" is a project owner decision, recorded
  as an assumption in the plan.
- **Rationale**: the literal reading of "GPL-3.0"; narrower is the safe default.
- **Alternatives considered**: `GPL-3.0-or-later` (needs an explicit owner decision).
