# Implementation Plan: Workspace and CI Foundation

**Branch**: `001-workspace-ci-foundation` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-workspace-ci-foundation/spec.md`

## Summary

Create the Rust workspace and the safety net every later feature relies on: a CI gate
on Windows and Debian 13 (format, lint, test, MSRV build, dependency audit) that blocks
merges to `main`; workspace-wide lints that forbid panicking constructs in production
code; a shared typed error model; contained worker panics with a process-wide panic
hook; a structured, size-bounded local log; and a contributor guide. Technical approach
and rationale are in [research.md](research.md): `tracing` with a small custom bounded
segment writer (time-based rotation in `tracing-appender` cannot bound size),
`catch_unwind` at the worker thread boundary, `cargo-deny`, a pinned toolchain and
GitHub Actions with a Debian `trixie` container.

## Technical Context

**Language/Version**: Rust, edition 2024; MSRV 1.85; development toolchain pinned in
`rust-toolchain.toml` (initially 1.100.0)

**Primary Dependencies**: `thiserror` (error derive), `tracing`, `tracing-subscriber`
(feature `json`), `directories` (platform log directory). Dev-only: none beyond the
standard test harness. Tooling: `cargo-deny`.

**Storage**: log files on the local disk (JSON lines, size-bounded); no other storage

**Testing**: `cargo test` (unit + integration; child-process re-execution of the test
binary for main-thread-panic and abrupt-termination scenarios); property-style tests
for the bounded writer written with plain loops over generated inputs (no extra
dependency)

**Target Platform**: Windows 10/11 and Debian 13, 64-bit x86 (CI: `windows-latest` and
`debian:trixie` container)

**Project Type**: desktop-app workspace (this feature adds only the foundation crates;
no UI, devices or signal storage)

**Performance Goals**: full CI run under 15 minutes (SC-003); logging overhead
negligible for the expected volume (no signal payloads are logged)

**Constraints**: no panic on external input; bounded log size (default 8 MiB per
process, min 64 KiB); never fail application start because of logging; no network
access from logging; `panic = "unwind"` in all profiles

**Scale/Scope**: 2 library crates, 1 workflow, ~1,000 lines of Rust including tests

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verified against `.specify/memory/constitution.md` v1.4.0. Result before research and
after design: **PASS** (one justified item in Complexity Tracking).

- [x] **I. Stability & Data Integrity**: this feature is its enforcement mechanism —
  deny-level panic lints (R3), typed errors (R4), worker panic containment and
  hook (R7), crash-tolerant log (R5). No file-save feature exists yet; atomic-write
  rule applies to later features.
- [x] **II. Hardware**: not affected (no device code); no driver impact.
- [x] **III. Test-First**: every requirement FR-008–FR-017 has tests written first;
  fmt + clippy + test run in CI on both OSes (FR-003). Fuzzing/property tests apply to
  the bounded writer (generated write sequences).
- [x] **IV. Architecture**: logic lives in UI-independent crates; the only
  OS-specific code (log directory resolution) is isolated in one function in
  `la-diagnostics`. No host API contract change. Memory is bounded.
- [x] **V. Simplicity**: four runtime dependencies, each justified in research.md
  (`thiserror`, `tracing`, `tracing-subscriber`, `directories`; all MIT/Apache-2.0,
  maintained); `cargo-deny` gate added in this feature.
- [x] **VI. Observability**: structured JSON log, panic reports, user-facing messages
  separated from technical detail (`code` + `detail`).
- [x] **VII. Localization**: no UI strings; the error model carries stable `code`s for
  later localization; log text is English by design (documented in the spec).
  Documentation and comments in English.
- [x] **VIII. Rendering**: not applicable.
- [x] **IX. Decoding**: not applicable (worker containment is reusable for decoders).
- [x] **X. Export**: not applicable.
- [x] **XI. Virtual device**: not applicable.
- [x] **XII. Original design & third-party code**: no code reused from PulseView,
  sigrok or Saleae. Third-party crates are listed above with licenses compatible with
  GPL-3.0.
- [x] **Platforms**: builds and tests on Windows 10/11 and Debian 13; OS-specific code
  isolated; no drivers touched.

## Project Structure

### Documentation (this feature)

```text
specs/001-workspace-ci-foundation/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── log-entry-format.md
│   ├── ci-checks.md
│   └── diagnostics-api.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                       # workspace: members, [workspace.package] (rust-version,
                                 #   license), [workspace.lints], [profile.*] panic = "unwind"
Cargo.lock                       # committed
rust-toolchain.toml              # pinned toolchain + rustfmt, clippy
rustfmt.toml                     # newline_style = "Unix"
clippy.toml                      # allow-*-in-tests options
deny.toml                        # advisories, licenses, bans, sources
.gitattributes                   # * text=auto eol=lf
.editorconfig
CONTRIBUTING.md                  # contributor guide (English)
scripts/
└── check-commits.sh             # non-blocking Conventional Commits report

.github/workflows/
└── ci.yml

crates/
├── core/                        # package la-core
│   ├── Cargo.toml
│   ├── src/{lib.rs, error.rs}
│   └── tests/error_model.rs
└── diagnostics/                 # package la-diagnostics
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   ├── config.rs            # LogConfig, validation and clamping
    │   ├── bounded_writer.rs    # size-bounded segment writer
    │   ├── logging.rs           # init_logging, fallback, instance cleanup
    │   ├── paths.rs             # platform log directory (only OS-specific code)
    │   ├── panic_hook.rs        # install_panic_hook
    │   └── worker.rs            # spawn_worker, WorkerFailure, lock_recover
    └── tests/
        ├── bounded_log.rs
        ├── worker_containment.rs
        └── process_scenarios.rs # re-executes the test binary: main-thread panic,
                                 #   abrupt termination, unwritable log directory
```

**Structure Decision**: a Cargo workspace under `crates/` matching the layout in the
plan template and the roadmap. Only the two foundation crates are created now; the
`devices`, `decoders`, `export`, `i18n` and `ui` crates are added by later features.
Crate package names use the `la-` prefix.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Custom size-bounded log writer (~150 lines) instead of a ready-made appender | FR-016 requires a hard on-disk size bound; `tracing-appender` rotates only by time | `tracing-appender` cannot bound size; `file-rotate` would add a dependency for logic that needs its own tests anyway (kept as fallback if the custom writer proves troublesome) |

## Assumptions Recorded During Planning

- `GPL-3.0-only` in crate metadata matches the committed license text; relaxing to "or
  later" is a project-owner decision.
- `windows-latest` (a Windows Server image) stands in for Windows 10/11 on hosted CI.
- MSRV 1.85 may be raised deliberately if a dependency requires it.
