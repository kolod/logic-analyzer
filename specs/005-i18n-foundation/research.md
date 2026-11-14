# Research: Localization Foundation

Decisions taken while planning. Format: Decision, Rationale, Alternatives.

## R1. The shared resource format: Project Fluent (FTL)

**Decision**: user-facing text is written in Fluent (`.ftl`) files, one set per language, identical
syntax on every platform.

**Rationale**: it is a plain text format translators can edit without tools, with first-class
placeables (`{ $count }`), plural and other selectors resolved by the library (`[one]`, `[few]`,
`*[other]`), message references and attributes, comments that attach to the next message (used for
translator notes, the review status and the "reserved" marker), and a specification with a Rust
implementation that returns errors instead of panicking. The constitution asks for one shared format
for Windows and Debian (replacing Android `strings.xml` / `.resx`); FTL satisfies it, and Fluent was
the roadmap's leading candidate.

**Alternatives**: gettext `.po` (plurals by `Plural-Forms` expressions written by hand per language,
which is the "ad-hoc plural code" the constitution forbids; message identity is the English text, so
key parity is weaker); ICU MessageFormat (powerful, but no mature Rust runtime with the same
safety record and much more syntax); RESX/XLIFF (platform- or tool-bound); custom JSON/TOML tables
(we would write the plural, placeholder and fallback machinery ourselves).

## R2. The library: `fluent-bundle` and friends

**Decision**: `fluent-bundle` (concurrent bundles; plural rules from its `intl_pluralrules`
dependency, which carries the CLDR cardinal rules), `fluent-syntax` (the AST, for the parity check),
`unic-langid` (language identifiers) and `sys-locale` (the system language). All MIT or Apache-2.0.

**Rationale**: `fluent-bundle` is the reference Rust implementation; `FluentResource::try_new` returns
the parsed resource together with its errors instead of failing, `add_resource` reports duplicate ids,
`format_pattern` collects non-fatal resolver errors and bounds the work (a cap on placeables and
recursion detection), and the concurrent variant is `Send + Sync` so one `Localizer` is shared by all
threads. `sys-locale` is a tiny crate that asks Windows and Unix for the user's language, the only
platform code needed (writing it ourselves would need `unsafe` FFI on Windows, which the constitution
asks us to avoid). Four small dependencies, no `unsafe` of our own.

**Alternatives**: `i18n-embed`/`fluent-templates`/`rust-i18n` (convenience layers with their own file
conventions and macros; we need the parity check, the fallback diagnostics and the runtime switch
under our control, which is little code on top of `fluent-bundle`); no language-negotiation crate
(`fluent-langneg`): matching six languages is a few lines and testable by a table.

## R3. Plurals: the library's CLDR rules, categories discovered from the library

**Decision**: plural categories come only from `fluent-bundle`'s rules (`[one]`, `[few]`, `[many]`,
`*[other]` in the resources). For the parity check, the set of categories a language *can* produce is
discovered by asking the library about a sweep of numbers (integers 0 to 10,000,000 at powers and
boundaries, and fractions), not from a table of our own; a hand-written expected table in the tests
(Ukrainian one/few/many, Japanese other only, French 0 and 1 as one, 1,000,000 in Spanish and French,
fractions) pins the behaviour so a library update cannot change it silently.

**Rationale**: FR-011 forbids plural logic in application code; discovering the categories keeps the
check correct when CLDR data changes with a library update, while the expected table makes such a
change visible and reviewable.

**Alternatives**: a hand-maintained table of categories per language (a second source of truth that
can disagree with the library); requiring every plural message to carry every CLDR category of every
language (impossible to know without the library's data).

## R4. Numbers, sizes, durations and dates: a small table-driven formatter, not ICU4X

**Decision**: `fluent-bundle`'s `NUMBER` does not insert locale grouping or decimal separators, so the
crate has its own formatter for the six languages, driven by a table derived from CLDR (decimal and
grouping separators, minimum grouping digits, short date and time patterns, 12/24-hour clock) and
checked against hand-written expected strings. It is offered to the resources as Fluent functions
(`FORMAT_NUMBER`, `FORMAT_FREQUENCY`, `FORMAT_SIZE`, `FORMAT_DURATION`, `FORMAT_DATETIME`), and the
unit symbols and the spacing between value and unit live in the resources (`common.ftl`), not in code.
ICU4X is evaluated and **not** adopted now.

**Rationale**: the product needs six fixed languages and a handful of value kinds (counts,
frequencies, sizes, durations, one numeric date-time form); a table of about a dozen facts per language
plus a few hundred lines of formatting code, verified against CLDR by tests, is smaller and easier to
audit than ICU4X, which would add roughly forty crates, large compiled data and a licence-file update
for functionality we would use a fraction of (Principle V). The code is panic-free by construction
(checked arithmetic, bounded output).

**Reconsideration triggers** (documented so the decision is not permanent): month and weekday names or
long date formats, named time zones, currency, collation, a seventh language whose rules differ in kind
(for example grouping that is not in threes), or right-to-left languages. Then ICU4X replaces
`locale_data`, `numbers` and `datetime` behind the same functions.

**Alternatives**: ICU4X (above); `num-format`/`chrono`/`time` with locale crates (several crates, none
covers all six locales' date patterns and units, and `chrono`'s local time handling is an
`unsafe`-adjacent area); Fluent's `NUMBER` alone (wrong separators in German, French, Spanish and
Ukrainian: a data-misreading bug in a measurement tool).

## R5. Runtime language switch

**Decision**: a `Localizer` is a cheaply cloneable handle (`Arc`) holding a bundle per language, built
once, and an atomic holding the current language. A lookup reads the language once, then formats with
that language's bundle (and with English on fallback), so each message is wholly in one language even
while another thread switches; the switch is a single atomic store. The language is the plain enum
`Language`, parsed from and printed to its tag (`en`, `de`, `fr`, `es`, `ja`, `uk`) for the settings.

**Rationale**: FR-015 and SC-006 (no mixture, no blocking); no locks on the lookup path.

**Alternatives**: rebuilding bundles on a switch (milliseconds of work and a window where nothing is
loaded); a global static (hidden state that tests cannot isolate; an explicit handle is passed to the
UI instead).

## R6. User-supplied text: isolate, never interpret

**Decision**: Fluent's automatic isolation is turned off (`set_use_isolating(false)`) because it would
also wrap numbers and units that we format ourselves. User-supplied text arguments are wrapped by the
crate in U+2068 (first strong isolate) and U+2069 (pop directional isolate), after every occurrence of
the bidirectional controls U+202A to U+202E and U+2066 to U+2069 has been removed from the value, so a
value cannot close the isolate and reorder the text around it. Placeholder values are passed to Fluent
as values (`FluentValue::String`), never concatenated into the message, so message syntax inside a
value is never parsed. Keys are a separate type (`Key`) that cannot be built from a placeholder.

**Rationale**: FR-021 and FR-022 and the edge case of a channel named `{ $count } samples`.

**Alternatives**: leaving Fluent's isolation on (isolates numbers too and makes expected strings
noisy); trusting callers to sanitize (the point of the foundation is that they cannot forget).

## R7. Error messages: key derived from the stable code

**Decision**: the message key of an error is `error-` followed by the code with `.` replaced by `-`
(`session.damaged` → `error-session-damaged`; codes use only lowercase letters, digits and underscores
apart from the dot, so the mapping is injective, which a test enforces). An error without a message
gets the generic key `error-unexpected` with the code as an argument. The localized text depends on the
code and the language only; `Error::detail` is never shown.

**Rationale**: the error model already guarantees stable codes "mapped to user-facing text by the
localization layer"; deriving the key keeps the catalogue mechanical and checkable (SC-001: a test
scans the crates' sources for code constants and requires a key for each).

**Alternatives**: an explicit table from code to key in Rust (a second list to keep in sync); using the
kind alone (too coarse to be actionable).

## R8. Resources: layout, embedding and the review status

**Decision**: `resources/i18n/<lang>/<name>.ftl` at the repository root, compiled in with
`include_str!` through a list in `resources.rs` (the parity check fails if a file exists that the list
does not register, or the reverse). Each file starts with a header comment block that records
`### status: draft` or `### status: reviewed` (and who reviewed it, free text); a `# @reserved` comment
on a message marks a key that is intentionally unused so far; other comments are translator notes.

**Rationale**: the project layout in the plan template; embedding satisfies FR-006 (no lookup on the
user's disk, no missing-file failure); comments are the part of Fluent that the AST preserves with the
message they precede, so the markers need no extra file.

**Alternatives**: runtime loading from an install directory (a damaged or missing file would remove a
language; also a second failure path to handle); a status table in code (separated from the text it
describes).

## R9. Time: the UTC offset is supplied by the caller

**Decision**: date-time formatting takes an instant (seconds and nanoseconds since the Unix epoch) and
a UTC offset in seconds; the civil date is computed with the days-from-civil algorithm, with checked
arithmetic and a range limit (years 1 to 9999). The application passes the system's current offset;
named time zones and daylight-saving rules are not part of this item (the spec was adjusted to say so).

**Rationale**: getting the local offset portably needs either `unsafe` FFI or a crate with known
soundness caveats on Unix; the offset is a settings or shell concern, and a pure function of
(instant, offset, language) is testable and reproducible (FR-020).

**Alternatives**: `chrono`/`time` local offset (soundness caveats in multi-threaded programs); a tz
database crate (large, and not needed until the UI shows zone names).

## R10. System language

**Decision**: `sys-locale` supplies the user's preferred language tags; negotiation maps each tag,
case-insensitively and accepting `-` or `_` separators and an encoding suffix such as `.UTF-8`, to a
supported language by its primary subtag (`de-AT`, `de_CH.UTF-8` → `de`; `es-419` → `es`; `uk-UA` →
`uk`; `zh-Hant` → none, so English), taking the first tag that matches and English otherwise. A stored
setting wins over the system language; an invalid stored value is reported and ignored.

**Rationale**: FR-014 and FR-016; matching on the primary subtag is exactly the rule the six
languages need (spec assumption: regional variants use the base language).

**Alternatives**: `fluent-langneg` (a full negotiation algorithm for a problem of six primary subtags).

## R11. The parity check

**Decision**: the check is library code in `la-i18n` (feature `check`) with a test that runs it on the
real resources and on one fixture per defect class. It (1) parses every registered file with
`fluent-syntax` and reports syntax errors with file and line, (2) takes English as the source of keys,
placeholders (variable references, including those inside selectors and attributes) and plural
variants, (3) for each other language reports missing keys, extra keys, placeholder sets that differ,
and counted messages that lack a required category (R3), (4) reports duplicate ids and empty messages,
(5) scans every `.rs` file of the workspace (excluding `tests/` and `target/`) for `key!("...")`
and `Key::new("...")` uses and for error-code constants, and reports keys used but undefined and
English keys never used and not reserved, and (6) checks the registered file list against the
directory. The report lists `file:line key kind` per defect and the test fails if it is not empty.

**Rationale**: FR-023 and FR-024; a test runs on Windows and Debian with the normal suite and needs no
extra CI job; sources are scanned as text with a small matcher (no regex dependency), which is enough
because keys are written as string literals inside one macro.

**Alternatives**: a separate binary run by a CI step (a second entry point to keep working locally and
in CI; a test is both); a procedural macro that checks keys at compile time (a heavier build and
cannot report unused keys).

## R12. Performance

**Decision**: formatting uses prebuilt concurrent bundles (their memoizers cache plural rules per
language) and our own allocation-light number code; nothing is parsed at lookup time. A benchmark
(`benches/lookup.rs`) records the lookup, a counted message, a formatted frequency and a language
switch for information; the tests assert generous bounds (a lookup under 100 µs on the reference
machine in release builds, the switch under 10 ms).

**Rationale**: SC-005 only needs "never noticeable"; the design has no I/O or parsing on the hot path.

## R3 addendum (found while implementing)

The plural rules of the shipped `intl_pluralrules` predate CLDR 42: French and Spanish have only `one`
and `other` there, not `many` for exact millions. The tests pin what the library does today (so a
library update that adds `many` is noticed and reviewed), the parity check requires exactly the
categories the library produces and **allows** a `many` variant in French and Spanish, and the
resources already carry those `many` forms (`d'échantillons`, `de muestras`) so that they start to be
used when the library is updated.
