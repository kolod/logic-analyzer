# CI Gate Verification: Workspace and CI Foundation

**Purpose**: Evidence that the merge gate (User Story 1) behaves as specified, and the
results of the constitution gate reviews done at the end of the feature.

**Created**: 2026-11-14

**Feature**: [spec.md](../spec.md)

## Clean change (T017)

Pull request `001-workspace-ci-foundation` → `main`: all 10 blocking checks
(`fmt`, `clippy`, `test`, `msrv`, `deny` × `windows`, `debian`) and the non-blocking
`commits` check passed.

## Deliberate failures (T018)

One throw-away draft pull request per defect, against the feature branch. Each was
closed and its branch deleted after the run.

| Defect introduced | Checks that failed | Checks that passed | Result |
|-------------------|--------------------|--------------------|--------|
| Unformatted code | `fmt (windows)`, `fmt (debian)` | everything else (9 of 11) | Blocked as expected |
| Production `v[0]` (panic-prone indexing) | `clippy (windows)`, `clippy (debian)` | everything else (9 of 11) | Blocked as expected |
| Failing test | `test (windows)`, `test (debian)`, `msrv (windows)`, `msrv (debian)` (the MSRV job also runs the tests) | `fmt`, `clippy`, `deny`, `commits` (7 of 11) | Blocked as expected |
| License allow-list without MIT and Apache-2.0 | `deny (windows)`, `deny (debian)` | everything else (9 of 11) | Blocked as expected |
| API stabilized after the MSRV (`slice::get_disjoint_mut`, Rust 1.86) | `msrv (windows)`, `msrv (debian)`, and also `clippy (windows)`, `clippy (debian)` | `fmt`, `test`, `deny`, `commits` (7 of 11) | Blocked as expected |

Observations:

- A failure is reported against the affected operating system and category by name.
- `clippy::incompatible_msrv` (part of `-D warnings`) flags use of an API newer than
  `rust-version` even before the MSRV job runs, so MSRV violations are caught twice.
- The `commits` job first failed because git refused the container checkout
  (`dubious ownership`); fixed with `safe.directory`, then it passed.
- The `rustup` installer prints `$HOME differs from euid-obtained home directory` in
  the Debian container. It is harmless (the jobs pass); it could be silenced later.

## Branch protection (T019)

Applying `scripts/protection.json` to `main` changes repository settings and was a
manual step for the repository owner. Status: applied (10 required checks, `strict: true`, `enforce_admins: false`, force pushes and deletions disabled).

## Constitution gates (T054–T056)

**Data safety, Principle I (T054)**: a search of `crates/*/src` for `unwrap(`,
`expect(`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `unsafe` and
`#[allow`/`#![allow` finds nothing in `la-core` and `la-diagnostics`. All matches are
in `la-lint-fixture`, which holds the deliberate, cfg-gated violations and test code
used by the lint-policy test. No lint exception is used anywhere in production code.

**Localization, Principle VII (T055)**: no UI strings were introduced. Errors carry a
stable `code` and an English technical `detail` (no user text); documentation, code
comments and commit messages are English; log text is intentionally untranslated
(spec Assumptions).

**Third-party code, Principles V and XII (T056)**: `cargo deny list` shows all
licenses are MIT, Apache-2.0, MPL-2.0, Unicode-3.0 or Unlicense/MIT dual (the own
crates are GPL-3.0-only). Runtime dependencies: `thiserror` 2.0.21, `tracing` 0.1.44,
`tracing-subscriber` 0.3.23 (features `fmt`, `json`, `std`), `directories` 6.0.0.
`serde_json` 1.0.154 is a dev-dependency used only by tests to parse log lines (the
plan listed no dev-dependencies; this is a recorded deviation). Nothing was reused
from PulseView, sigrok or Saleae.

## Not verified here

- SC-002 (a new contributor builds in under 30 minutes on a clean machine) was not
  measured on a clean machine; the Debian 13 build runs only in CI (container), not on
  a physical Debian installation.
- Because `enforce_admins` is false, repository admins can still bypass the required checks.
