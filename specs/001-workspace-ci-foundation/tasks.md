---

description: "Task list for Workspace and CI Foundation"
---

# Tasks: Workspace and CI Foundation

**Input**: Design documents from `/specs/001-workspace-ci-foundation/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first) and FR-021 demand tests for
FR-008–FR-017. In every story phase the test tasks come first and MUST fail before the
implementation tasks are started.

**Organization**: tasks are grouped by user story so each can be implemented and
tested independently (subject to the dependencies listed at the end).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: user story from spec.md (US1–US6)
- Exact file paths are given in every task; all paths are relative to the repository root

## Path Conventions

Rust workspace: `crates/core/` (package `la-core`), `crates/diagnostics/` (package
`la-diagnostics`), `crates/lint-fixture/` (package `la-lint-fixture`, test-only),
`scripts/`, `.github/workflows/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: repository and workspace skeleton

- [X] T001 Create the workspace root `Cargo.toml` with `[workspace] resolver = "3"`, members `crates/*`, `[workspace.package]` (`edition = "2024"`, `rust-version = "1.85"`, `license = "GPL-3.0-only"`, `publish = false`) and `[profile.dev]`/`[profile.release]` with `panic = "unwind"` (research R1, R7, R11)
- [X] T002 [P] Create `rust-toolchain.toml` pinning `channel = "1.100.0"` with components `rustfmt` and `clippy` (research R2)
- [X] T003 [P] Create `rustfmt.toml` (`newline_style = "Unix"`), `.editorconfig` and `.gitattributes` (`* text=auto eol=lf`) (research R10)
- [X] T004 [P] Create empty library crates `crates/core/Cargo.toml` + `crates/core/src/lib.rs` (package `la-core`) and `crates/diagnostics/Cargo.toml` + `crates/diagnostics/src/lib.rs` (package `la-diagnostics`, depends on `la-core`), inheriting `[workspace.package]` and `[lints] workspace = true`
- [X] T005 [P] Create `deny.toml` with `[advisories]` (deny vulnerabilities, unmaintained, yanked), `[licenses]` allow-list (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0, CC0-1.0), `[bans]` (wildcards denied) and `[sources]` (unknown registry/git denied) (research R9)
- [X] T006 Run `cargo build --workspace` to generate and commit `Cargo.lock`; confirm the empty workspace builds

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the enforced lint policy that every later task is written against

**⚠️ CRITICAL**: no user story work can begin until this phase is complete

- [X] T007 Add `[workspace.lints.rust]` (`unsafe_code = "deny"`, `unexpected_cfgs` with `check-cfg` listing `cfg(lint_fixture_unwrap)`, `cfg(lint_fixture_expect)`, `cfg(lint_fixture_index)`, `cfg(lint_fixture_arith)`, `cfg(lint_fixture_panic)`, `cfg(lint_fixture_unsafe)`) and `[workspace.lints.clippy]` (`unwrap_used`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`, `panic`, `unreachable`, `todo`, `unimplemented`, `undocumented_unsafe_blocks` all `deny`) to `Cargo.toml` (research R3)
- [X] T008 Create `clippy.toml` with `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-indexing-slicing-in-tests`, `allow-panic-in-tests` set to `true`; verify against the pinned Clippy which options exist and note any arithmetic relaxation needed (`#![cfg_attr(test, allow(clippy::arithmetic_side_effects))]` in test modules)
- [X] T009 Verify `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo test --workspace` pass on the empty workspace

**Checkpoint**: foundation ready — user stories can begin

---

## Phase 3: User Story 1 - Merge gate that keeps `main` working (Priority: P1) 🎯 MVP

**Goal**: every change is checked on Windows and Debian 13; any failure blocks merging into `main`.

**Independent Test**: push branches with a deliberate failure in each category and confirm the matching check fails on the affected OS and merging is blocked; a clean branch passes everything (quickstart step 7).

### Implementation for User Story 1

- [X] T010 [US1] Create `.github/workflows/ci.yml` with triggers `pull_request`, `push` to `main` and a weekly `schedule`, and a two-entry OS matrix: `windows` (`windows-latest`) and `debian` (`ubuntu-latest` with `container: debian:trixie`, installing `curl`, `build-essential`, `git` and `rustup` first) per contracts/ci-checks.md
- [X] T011 [US1] In `.github/workflows/ci.yml` add jobs `fmt`, `clippy` and `test` per OS with the exact commands from contracts/ci-checks.md; job names `fmt (windows)`, `fmt (debian)`, etc.
- [X] T012 [US1] In `.github/workflows/ci.yml` add the `msrv` job per OS: install the toolchain named by `rust-version` in `Cargo.toml` (read it with `cargo metadata`) and run `cargo build --workspace` and `cargo test --workspace` with it
- [X] T013 [US1] In `.github/workflows/ci.yml` add the `deny` job per OS: install `cargo-deny` and run `cargo deny --all-features check`; make the scheduled run execute only `deny`
- [X] T014 [US1] Add dependency caching (`Swatinem/rust-cache`) to all Rust jobs in `.github/workflows/ci.yml`
- [X] T015 [US1] Pin every third-party action in `.github/workflows/ci.yml` by full commit SHA (resolve each with `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha`), keeping the version in a trailing comment (research R8, FR-006)
- [X] T016 [P] [US1] Create `scripts/protection.json` listing the required status check names from contracts/ci-checks.md (`fmt`, `clippy`, `test`, `msrv`, `deny` for both OSes) for `gh api -X PUT repos/<owner>/<repo>/branches/main/protection`
- [X] T017 [US1] Push the branch and confirm CI is green on both OSes for the clean workspace; fix workflow errors until it is
- [X] T018 [US1] Verify the gate with deliberate failures on throw-away branches (not merged): a formatting error, a clippy warning, a failing test, a dependency with a disallowed license (`deny`), and a change that only builds on the newest toolchain (`msrv`); record the outcome in `specs/001-workspace-ci-foundation/checklists/ci-verification.md`
- [X] T019 [US1] **Manual (repository owner)**: apply `scripts/protection.json` to `main` with `gh api -X PUT repos/kolod/logic-analyzer/branches/main/protection --input scripts/protection.json` and confirm a failing check blocks the merge button

**Checkpoint**: User Story 1 is functional and testable on its own

---

## Phase 4: User Story 2 - Workspace a contributor can build in minutes (Priority: P1)

**Goal**: the workspace builds and tests on both OSes from the guide alone, and the MSRV is stated, enforced and verified.

**Independent Test**: on a clean machine follow quickstart steps 1–2: build and test succeed; an older toolchain is rejected with a message naming the required version.

### Implementation for User Story 2

- [X] T020 [US2] Make both crates inherit the MSRV and license (`rust-version.workspace = true`, `license.workspace = true`) in `crates/core/Cargo.toml` and `crates/diagnostics/Cargo.toml`
- [X] T021 [US2] Install toolchain 1.85 and run `cargo +1.85 build --workspace`; if a dependency or lint requires a newer version, raise `rust-version` in `Cargo.toml` in a dedicated commit and update plan.md and research.md R1
- [X] T022 [US2] Verify an older toolchain (for example `cargo +1.70 build`) fails with a message naming the required `rust-version`; record the exact message in `specs/001-workspace-ci-foundation/quickstart.md` step 2
- [X] T023 [US2] Add the `README.md` build-and-test section (English) with the commands for Windows 10/11 and Debian 13 and the prerequisite `rustup` steps (the full guide is T049)

**Checkpoint**: User Stories 1 and 2 work independently

---

## Phase 5: User Story 3 - Typed errors and no panics on external input (Priority: P1)

**Goal**: production code reports failures as typed errors; panicking constructs are rejected automatically in production code and allowed in tests.

**Independent Test**: quickstart steps 3–4: the lint-policy test proves forbidden constructs fail and test code passes; error-model tests show malformed input yields `Error`, never a panic.

### Tests for User Story 3 (write FIRST, ensure they FAIL)

- [X] T024 [P] [US3] Write `crates/core/tests/error_model.rs`: construct each `ErrorKind`; `code()`/`detail()`/`kind()` accessors; `with_source` preserves the cause via `std::error::Error::source`; `Display` contains code and detail; `Error` is `Send + Sync + 'static`; a sample parser function given empty, truncated, oversized and random byte inputs returns `Err` and never panics
- [X] T025 [P] [US3] Create the lint fixture crate `crates/lint-fixture/` (package `la-lint-fixture`, `publish = false`, `[lints] workspace = true`) with `src/lib.rs` containing a clean function, test code using `unwrap`/`expect`/indexing/`panic!` under `#[cfg(test)]`, and six production violations, each in its own function guarded by its own cfg: `lint_fixture_unwrap` (`.unwrap()`), `lint_fixture_expect` (`.expect("x")`), `lint_fixture_index` (`v[0]`), `lint_fixture_arith` (unchecked `a + b` on parameters), `lint_fixture_panic` (`panic!()`), `lint_fixture_unsafe` (`unsafe` block without a `// SAFETY:` comment)
- [X] T026 [US3] Write `crates/core/tests/lint_policy.rs`: spawn `cargo clippy -p la-lint-fixture --all-targets -- -D warnings` with a temporary `CARGO_TARGET_DIR`; assert success with no extra cfg (clean code and relaxed test code pass); then, once per cfg above, set `RUSTFLAGS=--cfg <name>` and assert the command fails and its output names the expected lint (`unwrap_used`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`, `panic`, `undocumented_unsafe_blocks` or `unsafe_code`) (FR-009, FR-011, SC-008)

### Implementation for User Story 3

- [X] T027 [US3] Implement `ErrorKind`, `Error` (`new`, `with_source`, accessors), `Display`, `std::error::Error` and `Result<T>` in `crates/core/src/error.rs` using `thiserror`, per contracts/diagnostics-api.md and data-model.md; re-export from `crates/core/src/lib.rs`; add the `thiserror` dependency to `crates/core/Cargo.toml`
- [X] T028 [US3] Add rustdoc to the public error API in `crates/core/src/error.rs` documenting the "no user message in the error; derive it from `kind` + `code`" rule (English)
- [X] T029 [US3] Make T024 and T026 pass; run `cargo clippy --workspace --all-targets --all-features -- -D warnings` to confirm the production code itself is clean, and document any `#[allow(..., reason = "...")]` used

**Checkpoint**: User Stories 1–3 (all P1) work independently

---

## Phase 6: User Story 5 - Structured, size-bounded local log (Priority: P2)

> Ordered before US4 because the panic hook (US4) reports through this logging layer.

**Goal**: structured JSON-lines log with a hard on-disk bound, tolerant of crashes and unusable locations.

**Independent Test**: quickstart step 6: write more than 10× the limit; size stays within the limit, newest entries present, every retained line valid JSON; an unwritable directory does not stop initialization.

### Tests for User Story 5 (write FIRST, ensure they FAIL)

- [X] T030 [P] [US5] Write `crates/diagnostics/tests/bounded_log.rs`: (a) generated write sequences of random line lengths totalling ≥ 10× the limit keep the sum of segment sizes ≤ `max_total_bytes` (SC-006) and never split a line; (b) the newest N entries are always present and the oldest are dropped first; (c) every retained line parses as JSON with the required fields from contracts/log-entry-format.md; (d) the first line of each segment is the `log started` entry with `format_version` 1
- [X] T031 [P] [US5] Write `crates/diagnostics/tests/log_config.rs`: `LogConfig::default()` values (8 MiB, 4 segments, 5 instances); clamping of `max_total_bytes` (≥ 64 KiB), `segments` (2..=16), `retained_instances` (1..=50); level override from an environment variable
- [X] T032 [P] [US5] Write `crates/diagnostics/tests/log_fallback.rs`: `init_logging` with a path that is a file (not a directory) and with a read-only directory returns a guard, does not panic, emits the single `logging.unavailable` report, and later events do not fail (FR-017, SC-007)
- [X] T033 [P] [US5] Write `crates/diagnostics/tests/log_instances.rs`: two `init_logging` calls with the same directory produce distinct `la-<timestamp>-<pid>` file sets that are each valid; with `retained_instances = 2` and 4 prior instance files at start-up, only the 2 newest instances remain (research R6)
- [X] T034 [P] [US5] Write a test in `crates/diagnostics/tests/log_fields.rs` that logs an event with structured fields and asserts the JSON has `timestamp` (RFC 3339 UTC, milliseconds), `level`, `target`, `message`, `fields`; and that a payload-sized byte buffer logged via its length contains no payload bytes (FR-019 guidance via an API that only accepts sizes)

### Implementation for User Story 5

- [X] T035 [P] [US5] Implement `LogConfig` with defaults, clamping and the environment level override in `crates/diagnostics/src/config.rs`
- [X] T036 [P] [US5] Implement the platform log-directory resolution (`directories`; Windows `%LOCALAPPDATA%\logic-analyzer\logs`, Debian `$XDG_STATE_HOME/logic-analyzer/logs`, fallback to the data-local dir) as the only OS-specific function in `crates/diagnostics/src/paths.rs`; add the `directories` dependency
- [X] T037 [US5] Implement `BoundedLogWriter` (`std::io::Write`, N segment files, advance-and-truncate rotation, single `write` per line, line-boundary tracking, `log started` header per segment) in `crates/diagnostics/src/bounded_writer.rs`
- [X] T038 [US5] Implement `init_logging(config) -> LogGuard` in `crates/diagnostics/src/logging.rs`: create the directory, prune old instances, open `BoundedLogWriter`, install a `tracing-subscriber` JSON layer with the `MakeWriter` over a mutex-guarded writer, fall back to stderr with a single `logging.unavailable` report on any failure; add `tracing` and `tracing-subscriber` (feature `json`) dependencies
- [X] T039 [US5] Export the public API (`LogConfig`, `LogGuard`, `init_logging`) from `crates/diagnostics/src/lib.rs` and add English rustdoc, including the "never log signal payloads" rule
- [X] T040 [US5] Make T030–T034 pass; run clippy with `-D warnings` and fix without weakening lints

**Checkpoint**: logging works and is independently verifiable

---

## Phase 7: User Story 4 - Worker panics are contained, logged and reported (Priority: P2)

**Goal**: a panic in a worker is caught at the thread boundary, logged and reported; the application and its data survive.

**Independent Test**: quickstart step 5: a deliberately panicking worker is returned as `WorkerFailure`, siblings and shared state remain usable, the panic is in the log.

### Tests for User Story 4 (write FIRST, ensure they FAIL)

- [X] T041 [P] [US4] Write `crates/diagnostics/tests/worker_containment.rs`: a panicking worker yields `Err(WorkerFailure)` with name, message and location; non-string panic payloads are rendered as `<non-string panic payload>`; sibling workers complete normally; a shared `Mutex<Vec<_>>` stays usable via `lock_recover` after a worker panics while holding it and the contents equal the last committed state; a loop test repeats the panic scenario 1,000 times with no process termination (SC-005)
- [X] T042 [P] [US4] Write `crates/diagnostics/tests/panic_log.rs`: after `install_panic_hook()` and `init_logging(...)` into a temp directory, a worker panic produces an `ERROR` entry containing `worker`, `panic_message` and `location` fields; a panic inside the hook's own logging path (simulated failing writer) does not recurse or abort

### Implementation for User Story 4

- [X] T043 [US4] Implement `WorkerFailure`, `WorkerHandle<T>::join` and `spawn_worker` using a named `std::thread` and `std::panic::catch_unwind(AssertUnwindSafe(..))` in `crates/diagnostics/src/worker.rs`, extracting message and location (location captured by the hook and handed over through a thread-local)
- [X] T044 [US4] Implement `lock_recover` (recover a poisoned `Mutex`, log a `WARN` event) in `crates/diagnostics/src/worker.rs` and document the transactional-update rule (build the new value, then swap under the lock) in the module rustdoc
- [X] T045 [US4] Implement `install_panic_hook()` in `crates/diagnostics/src/panic_hook.rs`: chain the previous hook, log message, location, thread name and backtrace (when enabled) through `tracing`, flush the log, guard against re-entrancy with a thread-local flag; install at most once
- [X] T046 [US4] Export `spawn_worker`, `WorkerHandle`, `WorkerFailure`, `lock_recover` and `install_panic_hook` from `crates/diagnostics/src/lib.rs` with English rustdoc
- [X] T047 [US4] Make T041–T042 pass; run clippy with `-D warnings`

**Checkpoint**: User Stories 1–5 work independently

---

## Phase 8: User Story 6 - Documented contribution rules (Priority: P3)

**Goal**: an English contributor guide and a non-blocking commit-message report.

**Independent Test**: quickstart step 8: from the guide alone a contributor can name a branch, write a conforming commit message and run the checks.

### Tests for User Story 6 (write FIRST, ensure they FAIL)

- [X] T048 [P] [US6] Create `scripts/tests/check-commits.test.sh` with fixture messages (valid: `feat(ui): add x`, `fix(drivers/logic16): y`, `docs: z`; invalid: `Update stuff`, `feat: missing scope for code change`, non-English text) asserting the script's exit codes and output

### Implementation for User Story 6

- [X] T049 [US6] Write `CONTRIBUTING.md` (English): build and test commands for Windows 10/11 and Debian 13, the MSRV (link to `Cargo.toml`), toolchain pinning, the English-only documentation rule (docs, code comments, commit messages), Conventional Commits with component scope and examples, `NNN-short-name` branch format, `main` merge rules, the lint policy and how to document an exception (`#[allow(..., reason = "...")]`), how to run `cargo deny`, and a link to the constitution
- [X] T050 [US6] Implement `scripts/check-commits.sh <range>` (bash, no dependencies) validating the Conventional Commits subject format and reporting offenders; make T048 pass
- [X] T051 [US6] Add the non-blocking `commits` job (Debian, `continue-on-error: true`, `fetch-depth: 0`) to `.github/workflows/ci.yml` calling `scripts/check-commits.sh origin/main..HEAD`, and add it to the non-blocking table in contracts/ci-checks.md if names differ
- [X] T052 [US6] Link `CONTRIBUTING.md` from `README.md` and verify, by following only the guide, that a branch name, a commit message and the local checks can be produced (quickstart step 8)

**Checkpoint**: all user stories are independently functional

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: scenarios spanning stories and constitution gates

- [X] T053 Write `crates/diagnostics/tests/process_scenarios.rs`: the test binary re-executes itself (environment-variable switch) to verify (a) a main-thread panic is logged before the process exits non-zero and previously written files are intact, (b) abrupt termination (`std::process::abort` in the child) leaves earlier log lines readable and at most the last line malformed, (c) `init_logging` with an unwritable directory still lets the child start (edge cases in spec.md, SC-007)
- [X] T054 [P] Data-safety review (constitution Principle I): grep production code for `unwrap`, `expect`, `[..]` indexing, `unsafe`, `panic!`; confirm there are no `#[allow]` exceptions without a `reason`, and note the result in `specs/001-workspace-ci-foundation/checklists/ci-verification.md`
- [X] T055 [P] Confirm localization gate (Principle VII): no UI strings were introduced, all documentation and comments are English, log text is intentionally untranslated (spec Assumptions); record in the same checklist
- [X] T056 [P] Confirm the third-party gate (Principle XII / V): list the four runtime dependencies with versions and licenses (`cargo deny list`), none reused from PulseView/sigrok/Saleae
- [X] T057 Run the whole quickstart (`specs/001-workspace-ci-foundation/quickstart.md` steps 1–8) on Windows 10/11 and on Debian 13 and fix any gap; update the quickstart where reality differs
- [X] T058 Update `roadmap.md` to mark item 001 as done and note anything that changed the order of later items (for example the MSRV or toolchain pin); update the constitution's `TODO` list only if a TODO was resolved
- [X] T059 Final local gate before merging to `main`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `cargo deny --all-features check`, and CI green on both OSes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none — start immediately
- **Foundational (Phase 2)**: depends on Setup — blocks all stories (lint policy)
- **US1 (Phase 3)**: depends on Foundational; needs at least the empty workspace to run CI
- **US2 (Phase 4)**: depends on Foundational; T021 may force an MSRV change that affects US1's `msrv` job (re-run T017 if it changes)
- **US3 (Phase 5)**: depends on Foundational; independent of US1/US2
- **US5 (Phase 6)**: depends on Foundational and on `la-core` existing (T004); independent of US3's implementation
- **US4 (Phase 7)**: depends on US5 (the hook logs through `init_logging`/`tracing`)
- **US6 (Phase 8)**: depends on US1 (T051 edits `ci.yml`); otherwise independent
- **Polish (Phase 9)**: depends on all stories

### User Story Dependencies

- **US1, US2, US3**: independent of each other after Phase 2
- **US5**: independent; **US4** requires US5
- **US6**: T051 requires US1's workflow file

### Within Each User Story

- Tests are written first and MUST fail before implementation
- Configuration/models before services; core before integration
- Run clippy with `-D warnings` at the end of every story

### Parallel Opportunities

- Setup: T002, T003, T004, T005 in parallel
- After Phase 2: US1, US2, US3 and US5 can proceed in parallel (different files, except the shared `ci.yml` edited in US1 then US6)
- US3 tests T024, T025 in parallel; US5 tests T030–T034 in parallel; US5 implementation T035, T036 in parallel
- Polish: T054, T055, T056 in parallel

---

## Parallel Example: User Story 5

```text
# Launch all tests for User Story 5 together:
Task: "bounded_log.rs property-style tests in crates/diagnostics/tests/bounded_log.rs"
Task: "log_config.rs tests in crates/diagnostics/tests/log_config.rs"
Task: "log_fallback.rs tests in crates/diagnostics/tests/log_fallback.rs"
Task: "log_instances.rs tests in crates/diagnostics/tests/log_instances.rs"
Task: "log_fields.rs tests in crates/diagnostics/tests/log_fields.rs"

# Then the independent implementation pieces:
Task: "LogConfig in crates/diagnostics/src/config.rs"
Task: "Platform log directory in crates/diagnostics/src/paths.rs"
```

---

## Implementation Strategy

### MVP First (all P1 stories)

1. Complete Phase 1: Setup and Phase 2: Foundational (lint policy)
2. Complete US1 (CI gate) — from here on every change is checked on both OSes
3. Complete US2 (MSRV) and US3 (typed errors + lint-policy proof)
4. **STOP and VALIDATE**: quickstart steps 1–4 and 7 on both OSes. This is the minimum that makes `main` trustworthy.

### Incremental Delivery

1. Setup + Foundational → empty workspace with enforced lints
2. US1 → merge gate live (first real value)
3. US2 → MSRV verified
4. US3 → error model, lint policy proven
5. US5 → logging; US4 → panic containment
6. US6 → contributor guide and commit report
7. Polish → process-level scenarios, constitution gates, owner enables branch protection

### Parallel Team Strategy

After Phase 2: one person on US1/US6 (CI and docs), one on US3 (`la-core`), one on US5 then US4 (`la-diagnostics`).

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] labels map tasks to user stories for traceability
- T019 is a manual step for the repository owner (it changes repository settings)
- T015 requires network access to GitHub to resolve action SHAs
- Commit after each task or logical group; messages follow Conventional Commits with a
  component scope (`ci`, `core`, `diagnostics`, `docs`)
- Avoid: weakening a lint to make a test pass, same-file conflicts, `unwrap`/`expect` in
  production code
