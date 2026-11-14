# Verification Record: Localization Foundation

**Feature**: [spec.md](../spec.md) | **Recorded**: 2026-11-14 on the maintainer machine
(Windows 11, x86_64, 32 logical cores; toolchain 1.100.0; MSRV 1.85 locally; Debian 13 through CI)

## Test evidence

`cargo test --workspace --all-features`: 460 tests pass, 0 fail. `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (and without `--all-features`) and
`RUSTDOCFLAGS="-D warnings" cargo doc -p la-i18n --no-deps --all-features` are clean. `cargo deny` is not
installed on the maintainer machine; CI runs it.

| Success criterion | Evidence |
|-------------------|----------|
| SC-001 error messages | `error_messages`: the 44 stable codes of the storage, device and session crates (found by scanning `error.rs` files) have a message in all six languages; none empty, none equal to its key, English messages are sentences that advise the user, none contains technical vocabulary, the technical `detail` never appears; a code without a message gives the generic text with the code and a problem |
| SC-002 plurals | `plurals`: 18 counts × 6 languages against a hand-written CLDR table (Ukrainian one/few/many/other, Japanese one form, French 0 and 1 as one, fractions), plus a sweep that pins the categories the library produces per language |
| SC-003 formats | `formats` and the unit tests of `numbers`, `quantity`, `datetime`: hand-written expectations for numbers (15 values), frequencies, sizes, durations and a fixed instant in six languages; every day of years 1 to 9999 checked against a reference calendar; no `f64` bit pattern panics (1,000,000 tested) |
| SC-004 parity | `parity`: the real resources and sources pass; 19 tests: a correct fixture, the real data and one broken copy per defect class (missing, extra, unused, undefined key, renamed placeholder also inside a plural variant, missing plural form, duplicate, empty message, syntax error with line, unregistered and missing file, missing status header, Windows line endings, raw count, error code without message) and each is reported exactly |
| SC-005 timing | see below |
| SC-006 stress | `stress`: 100,000 operations in the normal suite and 1,000,000 in a release run (1 s) on four threads while the language is switched: hostile keys, tags, numbers and values (megabyte strings, syntax look-alikes, bidi controls): no panic, no hang (watchdog), no mixed-language message |
| SC-007 damaged language | `fallback`: a syntax error in any file degrades that language to English as a whole (also when its other files are fine), one problem names the file, the other languages are unaffected |
| SC-008 system language | `language_choice`: 25 tags (variants, scripts, `_` and `.UTF-8` forms, unsupported, empty, malformed, over-long) |
| SC-009 | the parity check takes 0.17 s on the real resources; the same result on Windows and Debian is checked by CI |
| SC-010 walkthrough | `docs/localization.md` was followed literally: adding `count-samples-lost` to English only made the check name the five missing languages and the unused key in one report; adding it everywhere made it pass (about five minutes) |

Mutation checks done while writing the tests (each break was caught, then reverted): deleting a German
message, renaming a French key, appending the technical detail to a message, not recording a missing
translation, returning an empty string for a missing key, letting a damaged language keep its other files,
removing the Ukrainian `few` form, a Japanese `[one]` variant, case-sensitive language tags, swapped German
separators, no Spanish minimum grouping, decimal size prefixes, not scanning plural variants, not reporting
unused keys and ignoring plural categories.

Findings while implementing: the plural rules in the shipped `intl_pluralrules` predate CLDR 42, so French
and Spanish do not select `many` for exact millions yet (the resources already carry those forms and the
check allows them; the tests pin the library's behaviour so an update is noticed); Fluent's `NUMBER` does not
write locale separators, which is why the crate has its own formatter (research R4).

## SC-005: lookup and switch timing (`benches/lookup.rs`)

Release build on the maintainer machine; baseline in `crates/i18n/benches/baseline.json`:

| Operation | Time |
|-----------|------|
| plain message | 72 ns |
| counted message (English, Ukrainian) | 804 ns, 774 ns |
| error message by code | 135 ns |
| frequency, date and time | 338 ns, 385 ns |
| message with user text | 298 ns |
| language switch | 1 ns |

**SC-005 is met** with a wide margin (limits: 100 µs per lookup, 10 ms per switch).

## Data-safety review (Principle I)

Search of `crates/i18n/src` for `unwrap(`, `expect(`, `panic!`, `todo!`, `unimplemented!`,
`unreachable!`, `unsafe`, `#[allow`, `#![allow`, `#[expect` and `as` casts:

- no `unsafe`, `todo!`, `unreachable!`, `unimplemented!` or `expect(` and no `panic!` or `unwrap(` outside
  `#[cfg(test)]` modules;
- `#[allow(clippy::cast_*, reason = ...)]` on five numeric conversions (the sign and range are checked by the
  caller or the value is rounded for display: `bytes as f64`, `epoch as f64`, `u64 as f64` in the argument
  types, `f64 as i64` for an instant that is range-checked afterwards, `f64 as u64` for a non-negative value
  capped below 2^53);
- `#[allow(clippy::arithmetic_side_effects)]` on `civil_from_days` (bounded day numbers) and on test modules;
- no log call receives a placeholder value (problems and events carry kind, language and key, file or
  function name only; a test uses a unique marker value to check it).

## Remaining constitution gates

- **Localization (Principle VII)**: this feature is the mechanism. Status of the resources: all twelve files
  (six languages × `errors.ftl` and `common.ftl`) are `draft`, 72 messages per language. English is also a
  draft until the maintainer reviews it; **a native-speaker review of de, fr, es, ja and uk is still open**
  (Principle VII allows machine translation as a draft, but not shipping a release with it unreviewed).
- **Third-party code (Principles V and XII)**: `Cargo.lock` gains `fluent-bundle`, `fluent-syntax`,
  `unic-langid`, `sys-locale` and their dependencies (`fluent-langneg`, `intl-memoizer`, `intl_pluralrules`,
  `unic-langid-impl`, `tinystr`, `zerovec`, `zerofrom`, `displaydoc`, `rustc-hash`, `self_cell`, `smallvec`,
  `type-map`), all MIT, Apache-2.0 or Unicode-3.0 (already allowed in `deny.toml`); nothing reused from
  PulseView, sigrok or Saleae.
- **Constitution follow-up**: `TODO(UI_TOOLKIT_AND_L10N_FORMAT)` and the sentence "the format and its Rust
  library are chosen in the first plan that introduces UI text" are now answered for the localization half
  (Fluent through `fluent-bundle`); updating the constitution needs the maintainer's approval and a version
  bump, so it was not edited (task T058).
- **ICU4X reconsideration triggers** (research R4): month and weekday names or long dates, named time zones,
  currency, collation, a seventh language of a different kind, or right-to-left languages.
- **Platforms**: tests run locally on Windows 11 (and MSRV 1.85); Debian 13 and the MSRV job run in CI; the
  only platform code is inside `sys-locale`.
