# Implementation Plan: Localization Foundation

**Branch**: `005-i18n-foundation` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-i18n-foundation/spec.md`

## Summary

A new UI-free crate, `la-i18n`, turns message keys and error codes into text in one of six
languages. Messages live in **Project Fluent** (`.ftl`) resource files, one set per language under
`resources/i18n/<lang>/`, embedded in the binary; English defines the keys. The Rust implementation
is `fluent-bundle` (concurrent bundles, CLDR plural rules through its `intl_pluralrules`) with
`fluent-syntax` for the checker and `unic-langid` for language tags. Fluent's own number display is
not locale-aware, so a small formatter in the crate, driven by a CLDR-derived table for the six
languages, writes numbers, frequencies, sample counts, file sizes, durations and dates, and is
exposed to the resources as Fluent functions so that translators control units and wording. A
`Localizer` handle holds all six languages, the current language (switchable at run time, each
message wholly in one language), the English fallback and the diagnostics for every fallback.
User-supplied text is inserted as data, wrapped in bidi isolates after the bidi controls are
stripped. A message for every existing stable error code ships in all six languages (machine drafts,
marked as such). The parity check is a test (so it runs in CI on both platforms) that parses all
resources and scans the sources for used keys, with fixture tests for every defect class.

## Technical Context

**Language/Version**: Rust 2024 edition, MSRV 1.85, toolchain pinned by `rust-toolchain.toml`

**Primary Dependencies**: `fluent-bundle` (messages, plurals, bidi isolation), `fluent-syntax` (AST for the checker), `unic-langid` (language tags), `sys-locale` (system language; the only crate with platform code), `la-core`, `la-diagnostics`, `tracing`. The error catalogue reads the code constants of `la-storage`, `la-devices` and `la-session` in a test only (dev-dependencies), so the runtime crate stays small. Licenses: MIT or Apache-2.0 (already allowed in `deny.toml`).

**Storage**: resource files compiled into the binary (`include_str!`); nothing is read from the user's disk.

**Testing**: `cargo test` — unit tests, hand-written CLDR tables for plurals, formats and language negotiation, a parity check with a fixture per defect class, a concurrency and fuzz-style stress test, a resource-file loader robustness test, error-catalogue completeness against the product's error codes.

**Target Platform**: Windows 10/11 and Debian 13 (x86_64); the output is identical on both

**Project Type**: desktop-app workspace; this feature adds the UI-independent crate `crates/i18n` and `resources/i18n/`

**Performance Goals**: a message lookup and format under 100 µs, a language switch under 10 ms (SC-005); the full parity check under 10 s (SC-009)

**Constraints**: no panic, hang or unbounded allocation on any key, tag or placeholder value; a damaged language falls back to English as a whole; no resource lookup on the user's disk

**Scale/Scope**: six languages; about 44 error messages now (growing with every feature); a few dozen shared format strings; a parity check over every `.ftl` file and every `.rs` file of the workspace

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after Phase 1 design (still passes).*

- [x] **I. Stability & Data Integrity**: lookups return text for any input; parse and format errors
  are values, never panics (`fluent-bundle` returns errors and bounds placeables); a damaged language
  degrades to English; the workspace lints apply. No file is written by this feature.
- [x] **II. Hardware**: not affected.
- [x] **III. Test-First**: hand-written expected tables (plurals, formats, tags) are written before
  the code; every defect class of the parity check has a failing-fixture test; a stress test covers
  hostile placeholder values and concurrent switching; fmt, clippy and tests run in CI on both OSes.
- [x] **IV. Architecture**: all logic is in `la-i18n`, which has no UI dependency; the language is a
  plain value the settings store; no background work; memory is bounded (six small resource sets).
- [x] **V. Simplicity**: four small third-party crates, justified in research R2 (the format library,
  its syntax crate for the checker, language tags, system language); ICU4X is evaluated and rejected
  for now with explicit reconsideration triggers (R4); number/date formatting is a small table-driven
  module checked against hand-written CLDR expectations.
- [x] **VI. Observability**: every fallback and damaged resource is logged with language, key and
  file only (never placeholder values); errors have stable codes.
- [x] **VII. Localization**: this feature *is* the mechanism: one shared format (FTL) on both OSes,
  English source of truth and fallback, CLDR plurals through the library, runtime switch independent
  of the system language, locale formatting, key-parity check in CI, drafts marked. It closes the
  constitution's follow-up item `TODO(UI_TOOLKIT_AND_L10N_FORMAT)` for the format and library (the UI
  toolkit remains with item 006).
- [x] **VIII. Rendering**: not affected.
- [x] **IX. Decoding**: not affected; decoder output is displayed as is (FR-021, FR-022).
- [x] **X. Export**: not affected; exported files are locale-independent by their own rules, and the
  formatter is not used for them.
- [x] **XI. Virtual device**: not affected.
- [x] **XII. Original design & third-party code**: libraries are used as dependencies under their
  licenses (MIT/Apache-2.0); nothing is copied from PulseView, sigrok or Saleae; CLDR data is a Unicode
  standard (Unicode-3.0 license, already allowed) reproduced as a small table with its provenance.
- [x] **Platforms**: Windows 10/11 and Debian 13; the only OS-specific code is inside `sys-locale`.

## Project Structure

### Documentation (this feature)

```text
specs/005-i18n-foundation/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and alternatives
├── data-model.md        # Phase 1: entities, validation rules
├── quickstart.md        # Phase 1: how to validate the feature
├── contracts/
│   ├── i18n-api.md          # Rust API of la-i18n
│   └── resource-format.md   # what translators and developers write (FTL conventions)
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
resources/i18n/
├── en/  errors.ftl  common.ftl      # source of truth for keys
├── de/  errors.ftl  common.ftl
├── fr/  ...   es/  ...   ja/  ...   uk/  ...
crates/i18n/                         # NEW crate la-i18n
├── Cargo.toml
├── src/
│   ├── lib.rs              # public API, `#![deny(missing_docs)]`
│   ├── language.rs         # Language, parsing and negotiation of tags
│   ├── resources.rs        # the embedded files, per-language file lists
│   ├── key.rs              # Key, key rules, error-code to key mapping
│   ├── args.rs             # Arg values; user text isolation (bidi)
│   ├── localizer.rs        # Localizer: bundles, current language, lookup, fallback, diagnostics
│   ├── errors_catalogue.rs # message for an error (kind, code)
│   ├── locale_data.rs      # CLDR-derived table for the six languages
│   ├── numbers.rs          # integers, decimals, frequencies, sizes, durations
│   ├── datetime.rs         # civil date from epoch, date and time patterns
│   ├── functions.rs        # Fluent functions exposing the formatter to resources
│   └── parity.rs           # the parity check (resource parsing, usage scan, report)  [feature `check`]
├── tests/
│   ├── error_messages.rs   plurals.rs      formats.rs        fallback.rs
│   ├── language_choice.rs  runtime_switch.rs  user_content.rs
│   ├── parity.rs           # the real resources pass; fixture per defect class fails
│   ├── stress.rs           # 1,000,000 hostile lookups on several threads
│   └── fixtures/parity/    # one small broken resource set per defect class
└── benches/lookup.rs       # informational timing of lookup, format, switch
docs/localization.md        # walk-through for developers and translators (SC-010)
```

**Structure Decision**: a separate crate keeps the localization machinery and its third-party
libraries away from the storage, device and session crates (which only define error codes); the
resources sit at the repository root (`resources/i18n`, as in the project layout of the plan
template) so translators find them without entering a crate. The parity check lives in the crate
behind a `check` feature so the runtime build does not carry the syntax tree code it does not need
at run time (the tests enable it).

## Complexity Tracking

No constitution violations. Two deliberate choices are recorded in research: a small table-driven
formatter next to Fluent instead of ICU4X (R4), and a caller-supplied UTC offset instead of time zone
rules (R9).
