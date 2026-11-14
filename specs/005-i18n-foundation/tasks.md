---

description: "Task list for Localization Foundation"
---

# Tasks: Localization Foundation

**Input**: Design documents from `/specs/005-i18n-foundation/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first): in every phase the test tasks come
first and MUST fail (usually they do not compile yet) before the implementation tasks start. If a
test passes immediately because earlier work already covers it, run a mutation check (temporarily
break the code it guards, see the test fail, restore) before relying on it. Expected values in
tests (plural categories, formatted numbers, language tags) are written by hand from CLDR, never
computed by the code under test.

**Organization**: tasks are grouped by user story (US1–US7, numbered as in spec.md). The language,
key, argument and lookup core and the number formatter are Foundational because every story needs
them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root; the new crate is `crates/i18n` (package
  `la-i18n`), the resources are in `resources/i18n/<lang>/`

## Path Conventions

Source in `crates/i18n/src/`, tests in `crates/i18n/tests/`, benchmarks in `crates/i18n/benches/`,
resources in `resources/i18n/`, the developer walk-through in `docs/localization.md`. The contracts
are `specs/005-i18n-foundation/contracts/i18n-api.md` ("the API contract") and `resource-format.md`
("the resource contract"); the entities and rules are in `data-model.md`.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and
`unsafe` in production code. Use `get`, `checked_*`, `saturating_*`, `u128` intermediates and `?`.
Test code may relax this with the file-level `#![allow(clippy::unwrap_used, ...)]` used by the other
crates' tests. Never commit while clippy fails: a helper that only a later task uses gets
`#[expect(dead_code, reason = "used by T0xx")]`, removed by that task. Never log placeholder values;
log language, key and file names only. All text in files and code is English except the resource
files' translations.

---

## Phase 1: Setup

**Purpose**: the new crate and the resource directory exist, build and are wired into the workspace

- [X] T001 Create `crates/i18n/Cargo.toml` (package `la-i18n`, workspace edition/MSRV/license, `[lints] workspace = true`, dependencies `fluent-bundle`, `fluent-syntax`, `unic-langid`, `sys-locale`, `la-core`, `la-diagnostics`, `tracing` with current stable versions that build on MSRV 1.85 (look them up with `cargo add` / Context7 and pin to versions at least two weeks old); feature `check` (enables the `parity` module); dev-dependencies on itself with `check`, and on `la-storage`, `la-devices`, `la-session` only for the error-code test; `[[bench]] lookup` with `harness = false`) and `crates/i18n/src/lib.rs` with `#![deny(missing_docs)]`, the crate doc (what it is, "never log placeholder values") and empty module declarations; add a stub `crates/i18n/benches/lookup.rs` (`fn main() {}`)
- [X] T002 [P] Create the resource directory skeleton: `resources/i18n/<lang>/common.ftl` and `errors.ftl` for `en`, `de`, `fr`, `es`, `ja`, `uk`, each with the header comment (`### status: draft`) and a single placeholder message for now (the real content comes in later tasks); create `crates/i18n/src/resources.rs` with the registry of embedded files (`include_str!("../../../resources/i18n/<lang>/<file>.ftl")` for every language and file name) and `pub(crate) fn files(language) -> &'static [(&'static str, &'static str)]` (file name, text); unit test: every language has the same file names, every text is non-empty and valid UTF-8
- [X] T003 Run `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo deny --all-features check` (if installed; otherwise rely on CI); confirm `Cargo.lock` gains only `la-i18n` and the third-party crates named in the plan, and that every new license is already allowed in `deny.toml` (add an allow entry with a comment only if a crate needs one)

**Checkpoint**: the empty crate builds and lints clean

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: languages, keys, arguments, the lookup core with fallback and the number formatter

**CRITICAL**: no user story can start until this phase is complete

- [X] T004 [P] Write the unit tests of `crates/i18n/src/language.rs` first: `Language::ALL` has six entries in the order en, de, fr, es, ja, uk; `tag()` and `from_tag()` round-trip; `from_tag` accepts `de`, `DE`, `de-AT`, `de_CH`, `de_DE.UTF-8`, `es-419`, `uk-UA`, `ja-JP`, `fr-CA`, `en-GB`, `en_US.UTF-8@euro` and rejects `""`, `-`, `zh-Hant`, `pt-BR`, `i-klingon`, `deu`, `d`, a 1,000-character string and a string with control characters
- [X] T005 [P] Write the unit tests of `crates/i18n/src/key.rs` first: `Key::from_static` and `as_str`; `Key::for_error_code("session.damaged")` is `error-session-damaged`, `"storage.invalid_channel_count"` is `error-storage-invalid_channel_count`; the mapping is injective over a set of codes including look-alikes (`a.b-c` cannot occur because codes contain no hyphen, assert that every real code contains only lowercase letters, digits, `_` and one `.`); `Key::is_valid(&str)` accepts Fluent identifiers up to 100 characters and rejects empty, leading digit or hyphen, spaces, dots, non-ASCII and over-long ids; `key!` expands to a `Key`
- [X] T006 [P] Write the unit tests of `crates/i18n/src/args.rs` first: `Args::text` removes every character in U+202A..U+202E and U+2066..U+2069 and wraps the rest in U+2068 … U+2069, including for an empty value and for a value made only of bidi controls; `trusted` inserts as is; `int`, `uint`, `float` (including NaN, infinities, `i64::MIN`, `u64::MAX`) produce numbers; later values with the same name replace earlier ones; at most 64 arguments are kept (extra ones are ignored and counted) and a name longer than 100 characters or not an identifier is ignored; a one-million-character text is accepted without a panic
- [X] T007 [P] Write the unit tests of the locale table and the number formatter first (`crates/i18n/src/locale_data.rs`, `crates/i18n/src/numbers.rs`): the table rows of the data model for decimal, group and minimum grouping; `format_number` for 0, 1, 999, 1,000, 1,234, 12,345, 1,234,567.89, -1,234.5, 0.1, 0.0005 (rounds to 0.001), 0.0004 (0), 1e14, 1e15 and beyond (marker), NaN, ±infinity, `-0.0`, in all six languages against a hand-written table (Spanish `1234` stays ungrouped, `12.345` is grouped; French uses U+202F, Ukrainian U+00A0); the output never contains a trailing `.0`; the function never panics for any `f64` bit pattern (property test over 1,000,000 random bit patterns)
- [X] T008 Implement `crates/i18n/src/language.rs` (`Language`, `ALL`, `tag`, `from_tag`, `negotiate`, `from_system` using `sys-locale::get_locales`/`get_locale` with `negotiate`), `crates/i18n/src/key.rs` (`Key`, `from_static`, `for_error_code`, `is_valid`, `key!`) and `crates/i18n/src/args.rs` (`Args`, the isolation and sanitising rules of research R6, a crate-internal conversion to `FluentArgs`); make T004–T006 pass
- [X] T009 Implement `crates/i18n/src/locale_data.rs` (the table of the data model: separators, grouping, minimum grouping, date and time patterns, clock, AM/PM markers, each row with a comment naming the CLDR locale it comes from) and `crates/i18n/src/numbers.rs` (`format_decimal(value, max_fraction_digits, &LocaleData)` with checked arithmetic and `u128` or string rounding, no allocation beyond the output); make T007 pass
- [X] T010 Write the unit tests of the localizer core first (`crates/i18n/src/localizer.rs`, using small in-memory resources through a crate-internal constructor `Localizer::from_sources`): a found message; English fallback with a `MissingTranslation` problem; unknown key gives `⟦key⟧` and `MissingKey`; a missing argument gives the message with `⟦$name⟧` and `FormatError`; `FORMAT_NUMBER($n)` works inside messages and selectors; user text with `{ $x }` inside is output unchanged; `problems()` is bounded (1,000 distinct) and never contains argument values; the language is read once per message (a message formatted while another thread switches is wholly in one language — a test with two threads and 100,000 iterations); `Localizer` is `Clone + Send + Sync`
- [X] T011 Implement the localizer core in `crates/i18n/src/localizer.rs` and `crates/i18n/src/functions.rs`: build one concurrent Fluent bundle per language from `resources::files` (isolation off, functions registered per bundle that capture that language's `LocaleData`; for now only `FORMAT_NUMBER`), English as the fallback bundle, an atomic current language, the bounded problem list and `tracing` events (language and key only), `new`, `language`, `set_language`, `message`, `message_in`, `problems`; every parse error or duplicate id of an embedded resource is recorded as `DamagedResource`; make T010 pass
- [X] T012 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`; mark T001–T012; commit the phase (Conventional Commits, scope `i18n`, one commit per logical change)

**Checkpoint**: lookup with fallback, arguments and number formatting work; user stories can start

---

## Phase 3: User Story 1 - Every error has a message in the user's language (Priority: P1) MVP

**Goal**: all 44 existing stable error codes have a message in all six languages, and a code
without one still gives readable text.

**Independent Test**: `cargo test -p la-i18n --test error_messages`

### Tests for User Story 1 (write first, must fail)

- [X] T013 [P] [US1] Write `crates/i18n/tests/error_messages.rs`: enumerate the error codes by scanning `crates/*/src/error.rs` and `crates/*/src/**/*error*.rs` for `pub const NAME: &str = "domain.code";` (a helper in `crates/i18n/tests/common/mod.rs` shared with the parity tests, written first; it must find the 44 codes of storage, devices and session today and at least those) and require for each code and each of the six languages a message that is non-empty, not equal to the key, not containing `error-`, and, for English, longer than 20 characters and ending with a full stop; assert that no message contains the technical vocabulary of the `detail` strings it replaces (a short deny list: `invariant`, `bytes of`, `u64`, `panic`) and none contains a raw placeholder marker; `error_message(&Error)` for a real `la_core::Error` built from each code gives the same text as `error_message_for_code`; the technical `detail` of the error never appears in the text (use an error whose detail is a unique marker); an unknown code (`feature.not_yet_defined`) gives the generic `error-unexpected` text containing the code and records `UnknownErrorCode`; codes of every `ErrorKind` are covered; the injectivity of the code-to-key mapping holds for the real codes
- [X] T014 [P] [US1] Write the shared test helpers in `crates/i18n/tests/common/mod.rs`: the error-code scanner (used by T013 and the parity tests), the workspace root locator, a helper that formats every message of every language once, and the table of language names; keep it free of assertions so each test file stays readable

### Implementation for User Story 1

- [X] T015 [US1] Write `resources/i18n/en/errors.ftl`: one message per code, `error-<domain>-<name>` with the dot written as `-`, each saying what happened and what the user can do in plain English (no code names, no jargon), a translator comment above each describing when it appears, plus `error-unexpected = Something unexpected went wrong (code { $code }). Please try again; if it keeps happening, report it with the log file.`; messages are grouped by domain with `###` section comments; keep every message one sentence of cause and one of action
- [X] T016 [US1] Write the other five `errors.ftl` files (`de`, `fr`, `es`, `ja`, `uk`) as machine-translation drafts with the same keys and placeholders (`### status: draft`); keep product and unit names unchanged; a native-speaker review is tracked in the verification record, not here
- [X] T017 [US1] Implement `crates/i18n/src/errors_catalogue.rs` and wire `Localizer::error_message` and `error_message_for_code` (key from `Key::for_error_code`, fallback to `error-unexpected` with the code argument and an `UnknownErrorCode` problem when neither the language nor English has the key; the error's `detail` is never read); make T013 pass
- [X] T018 [US1] Mutation checks (record in the commit message): delete one German message, rename one key in `fr/errors.ftl`, make `error_message` append the detail; confirm `error_messages` fails each time, then restore
- [X] T019 [US1] Run fmt, clippy, tests; mark T013–T019; commit the phase

**Checkpoint**: error codes become readable messages in six languages; US1 independently demonstrable

---

## Phase 4: User Story 3 - A missing or unusable translation never breaks the screen (Priority: P1)

(Numbered after US1 because the lookup core exists; US2 follows.)

**Goal**: every failure of a lookup degrades to readable text plus a diagnostic.

**Independent Test**: `cargo test -p la-i18n --test fallback`

### Tests for User Story 3 (write first)

- [X] T020 [P] [US3] Write `crates/i18n/tests/fallback.rs` (uses `Localizer::from_sources` exposed under a `test-support` feature or a `#[doc(hidden)]` constructor, decide and document): a key missing in German returns the English text and one `MissingTranslation { language: De, key }` problem (not repeated for every call, but every call returns the text); a key missing everywhere returns `⟦key⟧` containing exactly the key and `MissingKey`; a language whose file has a syntax error (a truncated placeable, an unterminated string, a stray `}`) is degraded as a whole: every message of that language resolves from English, one `DamagedResource` problem names the file, the other five languages are untouched; the same for an empty file, a file that is not UTF-8 text (invalid bytes replaced), a file with a byte order mark and one with Windows line endings (these two load fine); a duplicate id in one file keeps the first definition and records the problem; a message with a missing argument still renders (`⟦$name⟧`) with `FormatError`; a selector given a string where a number is expected falls to the default variant with `FormatError`; a self-referencing message and a chain of 1,000 references terminate with a readable result and `FormatError`; a message producing a huge expansion (nested references) is bounded; no problem entry contains an argument value (use a value with a unique marker)
- [X] T021 [P] [US3] Write `crates/i18n/tests/resource_loading.rs`: the six real embedded resource sets load without any problem (`problems()` empty after loading all messages once), every language reports its review status, and `ReviewStatus` follows the header (`### status: reviewed` in a fixture)

### Implementation for User Story 3

- [X] T022 [US3] Complete degradation in `crates/i18n/src/localizer.rs`: per-language `degraded` flag set when a file has syntax errors that remove messages or when the file list is incomplete, the resolver skips a degraded language, `ReviewStatus` parsed from the header comment of each file (weakest wins), duplicate-id handling, bounded problem recording with de-duplication, recursion and size bounds relying on the library's limits plus an explicit output cap (for example 64 KiB per message, longer output is cut and recorded); make T020 and T021 pass
- [X] T023 [US3] Mutation checks: stop recording `MissingTranslation`, return an empty string for a missing key, let a damaged language use its partial content; confirm the tests fail each time, then restore
- [X] T024 [US3] Run fmt, clippy, tests; mark T020–T024; commit the phase

**Checkpoint**: lookups never fail visibly; the damaged-resource path is proven

---

## Phase 5: User Story 2 - Plural forms are correct in every language (Priority: P1)

**Goal**: counted messages use the right form in every language, resolved by the library.

**Independent Test**: `cargo test -p la-i18n --test plurals`

### Tests for User Story 2 (write first, must fail)

- [X] T025 [P] [US2] Write `crates/i18n/tests/plurals.rs`: a hand-written table (from CLDR, not computed) of the expected plural category for counts 0, 1, 2, 3, 4, 5, 11, 12, 14, 21, 22, 25, 100, 101, 102, 111, 1,000,000 and 1.5 for each language (en: other/one/other…; de like en; fr: 0 and 1 and 1.5 are one, 1,000,000 many; es: 1 one, 1,000,000 many, others other; ja: other always; uk: 1 one, 2–4 few, 5–20 many, 21 one, 22–24 few, 25 many, 101 one, 111 many, 1.5 other) and, using the real `count-*` messages of `common.ftl`, the exact expected sentence for each language and count (a table of at least 16 counts × 6 languages written by hand, taking the wording from the reviewed resources); counts beyond `i64` (`u64::MAX`), negative counts, NaN and infinity do not panic and give the `other` form; the categories that the library can produce per language (a sweep over integers and fractions) equal the hand-written set {en,de: one, other; fr,es: one, many, other; uk: one, few, many, other; ja: other} so a library update that changes a rule fails here

### Implementation for User Story 2

- [X] T026 [US2] Add the shared counted messages to `resources/i18n/en/common.ftl` (each marked `# @reserved` until a feature uses it): `count-samples`, `count-channels`, `count-devices`, `count-transitions`, `count-gaps`, each with `one` and `*[other]` variants using `FORMAT_NUMBER($count)`, and a translator comment; add the five translations with all categories required by the table in the resource contract (Ukrainian four forms, French and Spanish `many` for exact millions, Japanese `*[other]` only)
- [X] T027 [US2] Make the plural selection robust in `crates/i18n/src/localizer.rs`: numeric arguments reach Fluent as `FluentValue::Number` with the integer or fraction preserved (no lossy conversion of large integers: `uint` and `int` go through `i64`/`f64` with a documented range, values beyond ±2^53 are passed as `f64` and flagged in the doc), non-numbers fall to the default variant; make T025 pass
- [X] T028 [US2] Mutation checks: remove `few` from the Ukrainian `count-samples`, change a Japanese variant to `[one]`; confirm the table test fails each time, then restore
- [X] T029 [US2] Run fmt, clippy, tests; mark T025–T029; commit the phase

**Checkpoint**: plurals are correct and pinned by a hand-written CLDR table

---

## Phase 6: User Story 4 - The language is chosen by the user at run time (Priority: P2)

**Goal**: initial language from the system, a stored choice that wins, a switch without restart.

**Independent Test**: `cargo test -p la-i18n --test language_choice --test runtime_switch`

### Tests for User Story 4 (write first, must fail)

- [X] T030 [P] [US4] Write `crates/i18n/tests/language_choice.rs`: a hand-written table of at least 20 system language tags (`en`, `en-US`, `en_GB.UTF-8`, `de`, `de-AT`, `de_CH`, `fr-CA`, `fr_BE.UTF-8`, `es-ES`, `es-419`, `es_MX`, `ja`, `ja-JP`, `uk`, `uk-UA`, `it`, `pt-BR`, `zh-Hant`, `ru`, `C`, `POSIX`, empty, `-`, `xx_YY`, a 1,000-character tag) with the expected language; `Language::negotiate` takes the first tag that maps (`["it", "de-AT"]` is German, `["it", "ru"]` is English); a stored tag that is invalid or unsupported falls back to the system language, then English, with an `InvalidLanguageTag`-style problem on the localizer created through `Localizer::from_settings(stored: Option<&str>, system: &[&str])`; a stored valid tag wins over a different system language; `Language::from_system()` returns some supported language without panicking on this machine (do not assert which)
- [X] T031 [P] [US4] Write `crates/i18n/tests/runtime_switch.rs`: `set_language` changes the next message (all six languages in turn, 100 round trips) and `language()`; the switch takes under 10 ms (assert with a generous bound in debug: 50 ms); a clone of the localizer shares the language; four threads format messages in a tight loop (300,000 messages in total) while a fifth switches the language every millisecond: every produced message equals exactly one of the six language versions of that message (compare with precomputed texts), none is a mixture, none is empty, and the whole run finishes within a generous time bound; `message_in` is independent of the current language

### Implementation for User Story 4

- [X] T032 [US4] Implement `Localizer::from_settings`, the problem kind for an invalid stored tag (`InvalidLanguageTag`, added to `ProblemKind` and the API contract), `Language::negotiate` and `from_system` details (lowercase, split at `-`, `_`, `.`, `@`, primary subtag only, length limit before parsing) and verify the atomic language store uses `Acquire`/`Release`; make T030 and T031 pass
- [X] T033 [US4] Mutation checks: let a message read the language twice, make `negotiate` case-sensitive; confirm the tests fail, then restore
- [X] T034 [US4] Run fmt, clippy, tests; mark T030–T034; commit the phase

**Checkpoint**: language choice and run-time switching work and are race-free

---

## Phase 7: User Story 5 - Numbers, frequencies, sizes and dates look right (Priority: P2)

**Goal**: one shared formatter writes every quantity for the current locale, with units from the resources.

**Independent Test**: `cargo test -p la-i18n --test formats`

### Tests for User Story 5 (write first, must fail)

- [ ] T035 [P] [US5] Write `crates/i18n/tests/formats.rs`: hand-written expected strings (from CLDR, with the exact no-break and narrow no-break spaces written as escapes in the test source) for at least 12 values in each of the six languages: numbers 0, 1, 999, 1,000, 1,234,567.89, -1,234.5; frequencies 1 Hz, 999 Hz, 1 kHz, 12.5 MHz, 500 MHz, 1.25 GHz; sizes 0 B, 1,023 B, 1 KiB, 31,981,568 B (30.5 MiB), 5 GiB; durations 1.5 µs, 2.3 ms, 42 s, 125 s (`2 min 5 s`), 3,725 s (`1 h 2 min`); the fixed instant 2026-11-14T16:05:09Z at UTC offsets 0 and +7,200 s and -18,000 s, with the date and time patterns of each language (en `11/14/2026 4:05:09 PM`, de `14.11.2026 16:05:09`, es `14/11/2026 16:05:09`, ja `2026/11/14 16:05:09`, uk `14.11.2026 16:05:09`, fr `14/11/2026 16:05:09`); the day change across midnight with offsets; dates before 1970, 0001-01-01, 9999-12-31 and out of range (marker); leap days 2024-02-29 and 2100-03-01; non-finite and negative durations and sizes give the marker without panic; `FORMAT_*` functions called from a resource give the same text as the `Localizer::format_*` methods; a non-number argument gives `–` and a `FormatError` problem
- [ ] T036 [P] [US5] Unit tests in `crates/i18n/src/datetime.rs` first: days-from-civil and civil-from-days round trip for every day from 0001-01-01 to 9999-12-31 (a loop over about 3.65 million days) against a straightforward reference loop, weekday-free; offsets of ±18 h; checked arithmetic never overflows for any `i64` epoch second and any `i32` offset

### Implementation for User Story 5

- [ ] T037 [US5] Implement `crates/i18n/src/datetime.rs` (civil date from epoch seconds and offset, years 1–9999, patterns from `LocaleData`, 12/24-hour with AM/PM markers) and extend `crates/i18n/src/numbers.rs` with frequency scaling, size scaling, duration components and the rounding rules of the data model; make T036 pass
- [ ] T038 [US5] Register `FORMAT_FREQUENCY`, `FORMAT_SIZE`, `FORMAT_DURATION`, `FORMAT_DATE`, `FORMAT_TIME`, `FORMAT_DATETIME` in `crates/i18n/src/functions.rs`; unit patterns are read at bundle build time from the messages `unit-hz`…`unit-ghz`, `unit-b`…`unit-tib`, `unit-ns`, `unit-us`, `unit-ms`, `unit-s`, `unit-min`, `unit-h` formatted once with a private-use sentinel for `$value` and substituted at call time; add `Localizer::format_number/frequency/size/duration/datetime` and `set_utc_offset_seconds` (an atomic); a missing unit message falls back to English and records a problem
- [ ] T039 [US5] Write the unit and language-name messages to `resources/i18n/<lang>/common.ftl` for all six languages (units with the spacing per language: no-break space `{ " " }` in en/de/es/uk, narrow no-break space `{ " " }` in fr, none in ja; `language-name-en` … `language-name-uk` with the native names; the generic messages the formatter needs); mark unused ones `# @reserved`; make T035 pass
- [ ] T040 [US5] Mutation checks: swap decimal and group separators for German, drop minimum grouping for Spanish, use decimal prefixes for sizes; confirm `formats` fails each time, then restore
- [ ] T041 [US5] Run fmt, clippy, tests; mark T035–T041; commit the phase

**Checkpoint**: every quantity is written the locale's way by one facility

---

## Phase 8: User Story 6 - CI refuses a gap in the translations (Priority: P2)

**Goal**: a test-run check that names every parity defect with file and key.

**Independent Test**: `cargo test -p la-i18n --test parity`

### Tests for User Story 6 (write first, must fail)

- [ ] T042 [P] [US6] Create the fixtures under `crates/i18n/tests/fixtures/parity/`: a small valid set (`good/resources/i18n/<lang>/*.ftl` for six languages with one counted and one plain message, and `good/src/lib.rs` using both keys through `key!`) and one broken copy per defect class, each a directory that differs from `good` in exactly one way: `missing_key_ja`, `extra_key_fr`, `unused_key`, `undefined_key` (code uses a key that is defined nowhere), `placeholder_renamed_de`, `missing_plural_few_uk`, `duplicate_key`, `empty_message`, `syntax_error`, `unregistered_file` (an `.ftl` file in `en/` that is not in the registry file `registry.txt` that the fixtures use in place of `resources.rs`), `missing_file_es`, `missing_status_header`, plus `reserved_ok` (an unused key marked `# @reserved` passes)
- [ ] T043 [P] [US6] Write `crates/i18n/tests/parity.rs`: the real workspace resources and sources pass (`check(resources/i18n, workspace root)` is clean, with the full report rendered into the assertion message); each fixture directory produces exactly the expected defect kinds with the right language, file, key and, for syntax errors, line (assert on the full set of defects for the fixture, not just containment); the clean fixture and `reserved_ok` pass; the check on the real data finishes in under 10 seconds (debug build: 30); running it twice gives the same report; a file with Windows line endings is reported as the defect `line-endings` (the parser accepts such a file, the check flags it so the repository stays uniform)

### Implementation for User Story 6

- [ ] T044 [US6] Implement `crates/i18n/src/parity.rs` (feature `check`) per research R11: parse files with `fluent-syntax` (errors with line and column), collect keys, variables (including inside selectors, attributes and message references; a referenced message's variables count for the referencing message), plural variants and the default marker, comments for `@reserved` and the status header; discover the required plural categories per language from the library (research R3); scan `.rs` files under the workspace (skipping `target/`, `tests/` and the `i18n` fixtures) for `key!("…")`, `Key::from_static("…")` and for error-code constants (`pub const NAME: &str = "a.b"` in `error.rs` files, yielding `error-a-b`) as used keys; produce `Report`/`Defect` with `render()` as `file:line key kind detail`; make T043 pass
- [ ] T045 [US6] Register the check so it runs with `cargo test --workspace`: the `parity` test enables the `check` feature through the dev-dependency, and mention in `specs/001-workspace-ci-foundation/contracts/ci-checks.md` that the `test` job includes the localization parity check (no new CI job, no change to `scripts/protection.json`)
- [ ] T046 [US6] Mutation checks: stop counting variables inside selectors, stop reporting unused keys, accept a missing default variant; confirm the fixture tests fail each time, then restore
- [ ] T047 [US6] Run fmt, clippy, tests; mark T042–T047; commit the phase

**Checkpoint**: a translation gap or an unknown key fails CI with a precise report

---

## Phase 9: User Story 7 - Adding text is easy and user content is safe (Priority: P3)

**Goal**: user content is never interpreted; the walk-through lets a developer add a message in 10 minutes.

**Independent Test**: `cargo test -p la-i18n --test user_content --test stress`

### Tests for User Story 7 (write first)

- [ ] T048 [P] [US7] Write `crates/i18n/tests/user_content.rs`: text arguments equal to `{ $count } samples`, `{ NUMBER($count) }`, `{ -term }`, `{ "\u0000" }`, `{`, `}`, `\`, a very long string of `{`, strings with U+202E (right-to-left override), U+2066..U+2069, U+200E/U+200F, NUL, U+FEFF and a lone surrogate-like replacement character: the output contains the (sanitised) value exactly once, the surrounding message text is byte-identical to the message with an ordinary value, isolates are balanced (as many U+2068 as U+2069 and each U+2069 after its U+2068), a one-million-character value is produced in under 200 ms in release (2 s in debug) without a panic; a user text equal to an existing key (`unit-mhz`) is shown as text, not looked up; `Key` cannot be built from an `Args` value (compile-fail doctest on `Key`)
- [ ] T049 [P] [US7] Write `crates/i18n/tests/stress.rs` (SC-006): a deterministic generator drives 1,000,000 operations over 4 threads while a fifth switches the language: lookups of existing keys, missing keys, malformed key strings (empty, huge, non-ASCII, with dots and spaces), `from_tag` and `negotiate` on random strings, formatting of random `f64` bit patterns, sizes, durations and instants, and arguments of every kind including syntax look-alikes, bidi controls and megabyte strings; no panic, no hang (a watchdog aborts after 60 s), every message is one of the precomputed language versions or an English fallback or a `⟦…⟧` placeholder; run with 100,000 operations in the normal suite and the full million in an `#[ignore]`d test run by the weekly workflow

### Implementation for User Story 7

- [ ] T050 [US7] Fix whatever T048 and T049 expose in `args.rs` and `localizer.rs`; add the `compile_fail` doctest on `Key` (no `From<String>`/`From<&str>` conversion exists); add the million-operation stress test to `.github/workflows/fuzz.yml` as a step (`cargo test -p la-i18n --release --test stress -- --ignored`, informational) and document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md`
- [ ] T051 [US7] Write `docs/localization.md`: the walk-through of the resource contract with the full example of adding `count-samples-lost` in all six languages, the plural table, the `status` header, `@reserved`, how to run the check, how to add a language and what a machine draft is; review it by following it literally in a scratch branch (add a message, see the check fail, fix it, see it pass) and note the time it took in the verification record (SC-010)
- [ ] T052 [US7] Run fmt, clippy, tests; mark T048–T052; commit the phase

**Checkpoint**: all user stories (US1–US7) are independently functional

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: timing evidence, documentation, constitution gates

- [ ] T053 [P] Implement `crates/i18n/benches/lookup.rs` (`harness = false`, same style as the other benchmarks): a plain lookup, a counted message, a formatted frequency, a Ukrainian counted message and a language switch, reporting nanoseconds per operation and writing `target/la-i18n-bench/lookup.json` (resolve the workspace `target` from `CARGO_MANIFEST_DIR` as the other benchmarks do) with a `--write-baseline` section in `crates/i18n/benches/baseline.json`; fail if a lookup exceeds 100 µs or a switch 10 ms (SC-005) or a result is more than 5x slower than the baseline
- [ ] T054 Run the benchmark on the maintainer machine, commit `crates/i18n/benches/baseline.json` and record the SC-005 result in `specs/005-i18n-foundation/checklists/verification.md`; extend `.github/workflows/bench.yml` with a step `cargo bench -p la-i18n --bench lookup` (non-blocking, artifact path `target/la-i18n-bench/*.json`) and document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md`
- [ ] T055 [P] Review the public API against `specs/005-i18n-foundation/contracts/*.md` and update the contracts where the implementation differs (names, `from_settings`, `InvalidLanguageTag`, the test constructor, the `line-endings` defect, number range notes); ensure rustdoc exists for every public item and that `RUSTDOCFLAGS="-D warnings" cargo doc -p la-i18n --no-deps --all-features` passes
- [ ] T056 [P] Data-safety review (constitution Principle I): search `crates/i18n/src` for `unwrap(`, `expect(`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow`, `#[expect`, `as` casts and indexing; matches only in `#[cfg(test)]` modules; also check that no log call receives an argument value; record the result in `specs/005-i18n-foundation/checklists/verification.md`
- [ ] T057 [P] Record the remaining constitution gates in the same checklist: localization (this feature is the mechanism; the draft status of each language's files and how many messages are drafts), third-party code (`Cargo.lock` additions listed with licenses; nothing reused from PulseView/sigrok/Saleae), platforms (Windows locally; Debian 13 and MSRV 1.85 in CI; the only platform code is inside `sys-locale`), the ICU4X reconsideration triggers
- [ ] T058 Ask the maintainer whether to update the constitution's open follow-up note `TODO(UI_TOOLKIT_AND_L10N_FORMAT)` (the localization half is now decided: Fluent through `fluent-bundle`, recorded in `specs/005-i18n-foundation/research.md`) and the sentence "the format and its Rust library are chosen in the first plan that introduces UI text" in "Localization Requirements"; a constitution change needs a version bump and the maintainer's approval, so do not edit `.specify/memory/constitution.md` without it
- [ ] T059 Run the whole quickstart (`specs/005-i18n-foundation/quickstart.md`, steps 1–9) on Windows; fix any gap and update the quickstart where reality differs
- [ ] T060 Update `roadmap.md` (mark item 005 implemented) and the `README.md` status line (the localization foundation: six languages, error messages, locale formatting; still no UI and no real drivers)
- [ ] T061 Final local gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo +1.85 test --workspace`, then push the branch, open a pull request and wait for all required checks on both operating systems; fix any CI-only failure (wait for states instead of asserting them instantly, and keep timing bounds generous in debug builds)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup — blocks all stories; T004–T007 (tests) are independent of each other, T008 and T009 implement them, T010–T011 need `Language`, `Key`, `Args` and the number formatter
- **US1 (Phase 3)**: depends on Foundational; also needs the lookup fallback for `error-unexpected`
- **US3 (Phase 4)**: depends on Foundational (the lookup core); independent of US1 apart from the shared test helper
- **US2 (Phase 5)**: depends on Foundational (`FORMAT_NUMBER`) and on the resources of US1's file layout
- **US4 (Phase 6)**: depends on Foundational; independent of US1–US3
- **US5 (Phase 7)**: depends on Foundational (number formatting); extends `common.ftl` from US2
- **US6 (Phase 8)**: depends on US1 (real resources and codes to check), US2 and US5 (their messages), and on the resource contract; the fixtures can be written earlier
- **US7 (Phase 9)**: depends on Foundational and US6 (docs refer to the check)
- **Polish (Phase 10)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Resources (English first, then the five translations) before the code that needs them
- Run clippy with `-D warnings` at the end of every phase and never commit a failing gate
- The parity test (US6) is the final judge: a resource added by an earlier phase must already satisfy
  it when US6 lands, so run `cargo test -p la-i18n` after every phase from US6 on

## Parallel Execution Examples

- Foundational: T004 + T005 + T006 + T007 (different files, tests first)
- US1: T013 can be written while T015/T016 (the resources) are drafted
- US4 and US5 can proceed side by side after the Foundational phase (different source files)
- US6 fixtures (T042) can be prepared during any earlier phase

## Implementation Strategy

**MVP**: Phases 1–3 (Setup, Foundational, US1): every existing error has a readable message in six
languages. Then US3 and US2 (fallback and plurals, both P1), then US4–US6, then US7 and Polish. The
parity check lands in US6 but every phase before it already keeps the resources consistent by hand and
by its own tests, so US6 should find nothing to fix; if it does, that is a defect to fix in the phase
that introduced it. Commit after every phase and keep the gate green at every commit.
