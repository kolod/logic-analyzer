# Feature Specification: Localization Foundation

**Feature Branch**: `005-i18n-foundation`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "005 (roadmap item 005, `005-i18n-foundation`): choice of the shared resource format and library (Fluent is the leading candidate), six languages, CLDR plurals, locale formatting, runtime language switch, key-parity check in CI."

## Overview

The product speaks six languages — English, German, French, Spanish, Japanese and Ukrainian —
and the constitution makes that a precondition of every feature: no user-facing text in code, one
resource format on every platform, English as the source of truth and the fallback, plural forms
resolved by the mechanism and never by ad-hoc code, locale-correct numbers, and a language the user
picks independently of the system language. This feature builds that foundation before any UI
exists, so that the UI toolkit work (roadmap item 006) and every later feature can add text the
right way from the first string.

Its "users" are the developers of the UI and of every later feature (who add and look up text),
the translators (who edit resource files), the CI (which must refuse a release-blocking gap), and,
indirectly, end users who read the application in their own language and must never see a raw key,
a wrong plural ("1 samples", a wrong Ukrainian form) or a number written the wrong way.

The first real content is the set of user-facing messages for the stable error codes that already
exist in the product (storage, devices, session): the error model deliberately carries only a code
and a technical English detail, and this feature is where the code becomes a message the user can
read.

Terms used in this document:

- **Message**: one piece of user-facing text, identified by a stable **key**, possibly with
  placeholders (a number, a name) and plural forms.
- **Resource file**: the file that holds the messages of one language. English is the source of
  truth for keys; the other five must match it.
- **Language**: one of the six supported languages. **Locale**: a language with its formatting
  conventions (decimal separator, grouping, date order).
- **Fallback**: showing the English message when a translation is missing or unusable.
- **Parity**: every language has exactly the keys, the placeholders and the plural forms its
  English source requires.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every error the user can meet has a message in their language (Priority: P1)

When something fails (a damaged session file, a device that was unplugged, a memory limit), the
application turns the stable error code into a readable message in the selected language, with a
hint of what to do. A developer asks for the message of an error and gets text, never a raw code
or a technical detail, and every error code that exists today has a message in all six languages.

**Why this priority**: the product's error model is built on stable codes precisely so that
messages can be localized; without this the first UI would show codes, and the data-safety promise
of the project ("report the failure with an actionable error") would not reach the user.

**Independent Test**: enumerate every stable error code defined by the product's crates and ask for
its message in each of the six languages; every request returns a non-empty message that is not
the key, and the English message is actionable (says what happened and what to do).

**Acceptance Scenarios**:

1. **Given** the error `session.damaged`, **When** its message is requested in Ukrainian, **Then**
   the user gets a Ukrainian sentence that says the file is damaged and what to do.
2. **Given** every error code defined today, **When** a message is requested in each of the six
   languages, **Then** none is missing, empty or equal to its key.
3. **Given** an error code that has no message (a code added by a later feature without text),
   **When** its message is requested, **Then** the user gets a generic localized "unexpected error"
   message that mentions the code, and the gap is reported as a defect to developers (never a crash
   and never a blank).
4. **Given** the technical detail of an error (English, for the log), **When** a message is shown
   to the user, **Then** the detail is not part of it; the user-facing text depends on the code and
   the language only.

---

### User Story 2 - Plural forms are correct in every language (Priority: P1)

A message that mentions a count ("3 samples were lost", "1 device found") uses the right form for
that number in every language: Ukrainian distinguishes one, few, many and other (1, 2–4, 5–20 and
21, 22–24, 25–30 …), Japanese has a single form, French treats 0 and 1 alike, Spanish and German
differ from English in their own ways. The forms come from the localization mechanism's plural
rules, never from code written in the application.

**Why this priority**: a wrong plural looks careless and, for Ukrainian, is the usual failure of
hand-written plural code; the constitution requires it to be correct.

**Independent Test**: a table of counts (0, 1, 2, 3, 4, 5, 11, 12, 14, 21, 22, 25, 100, 101, 102,
111, 1,000,000 and fractions such as 1.5) is formatted in each language for a test message and the
result is compared with a hand-written table of the expected CLDR category and wording.

**Acceptance Scenarios**:

1. **Given** the count 1, 2, 5 and 21 in Ukrainian, **When** a counted message is formatted,
   **Then** the forms are "one", "few", "many" and "one" respectively, with the matching wording.
2. **Given** any count in Japanese, **When** a counted message is formatted, **Then** the single
   form is used.
3. **Given** the counts 0 and 1 in French, **When** a counted message is formatted, **Then** both
   use the singular form; and in English and German, 0 uses the plural form.
4. **Given** a language-specific plural category that a translation did not provide, **When** the
   message is formatted for a count in that category, **Then** it is treated as a missing
   translation (fallback), not a crash and not a wrong form.

---

### User Story 3 - A missing or unusable translation never breaks the screen (Priority: P1)

If a message has no translation in the selected language, the user sees the English message. If a
resource is damaged, a placeholder cannot be filled, or a key does not exist at all, the user still
sees readable text and the problem is reported to the developers through the log, not to the user
as a crash.

**Why this priority**: Principle I forbids crashing on any external input, and resource text is
edited by people; the fallback rule is also a constitutional requirement.

**Independent Test**: remove a translation, damage a resource, and ask for an unknown key; in every
case the lookup returns readable text and a logged diagnostic, and never panics.

**Acceptance Scenarios**:

1. **Given** a key that exists in English but not in German, **When** the message is requested in
   German, **Then** the English message is returned and a "missing translation" diagnostic names the
   language and the key.
2. **Given** a key that exists in no language, **When** it is requested, **Then** the result is a
   visible placeholder that contains the key, and a diagnostic is logged.
3. **Given** a resource file that cannot be read or parsed for one language, **When** that language
   is selected, **Then** every message falls back to English, a diagnostic names the file and the
   problem, and the application keeps running.
4. **Given** a message whose placeholder value is missing at the call site, **When** it is
   formatted, **Then** the message is still produced (the placeholder shown as an empty marker) and
   a diagnostic is logged.

---

### User Story 4 - The language is chosen by the user, at runtime (Priority: P2)

The application starts in a language derived from the system language (the closest supported one,
English if none matches), and the user can switch to any of the six languages at any time,
independently of the system language, without restarting. After the switch, every message requested
afterwards is in the new language; the choice is a plain value the settings can store.

**Why this priority**: required by the constitution and by users who run an operating system in one
language and want the tool in another; it is a P2 because it builds on the lookup of US1–US3.

**Independent Test**: start with a system language of each supported language, of an unsupported one
and of a regional variant (for example Austrian German, Latin-American Spanish, Canadian French);
switch languages repeatedly while messages are being formatted from several threads; check each
result.

**Acceptance Scenarios**:

1. **Given** a system language of `de-AT`, **When** the application starts without a stored choice,
   **Then** the language is German.
2. **Given** a system language that is not supported (for example Italian), **When** the
   application starts, **Then** the language is English.
3. **Given** the language is English, **When** the user switches to Japanese, **Then** the next
   message requested is Japanese, with no restart and no stale text.
4. **Given** a stored choice and a different system language, **When** the application starts,
   **Then** the stored choice wins.
5. **Given** messages being formatted concurrently on several threads, **When** the language is
   switched, **Then** every message is wholly in the old or wholly in the new language, never a
   mixture, and nothing is lost or blocked noticeably.
6. **Given** an invalid stored choice (an unknown or malformed language tag), **When** the
   application starts, **Then** it falls back to the system language or English and reports the
   problem in the log.

---

### User Story 5 - Numbers, frequencies, sizes and dates look right in each locale (Priority: P2)

Quantities are written the way the user's locale writes them: the decimal and grouping separators,
the digit grouping, the space before a unit, the unit symbols, the date and time order and the
24-hour or 12-hour clock. A frequency such as 12.5 MHz, a sample count such as 1 234 567, a file size
such as 30.5 MiB, a duration, and a date and time are formatted by one shared facility so that
no screen writes its own.

**Why this priority**: constitutionally required; wrong separators in a measurement tool are
misreadings of data (1,234 versus 1.234), which matters more here than in most products.

**Independent Test**: a table of values (0, 1, 999, 1,000, 1,234,567.89, 12.5 MHz, 1 Hz, 500 MHz,
30.5 MiB, 1 KiB, 0 bytes, a duration of 1.5 µs, 2.3 ms and 1 h 2 min, and one fixed date and time) is
formatted in each language and compared with a hand-written table of the expected text.

**Acceptance Scenarios**:

1. **Given** 1,234,567.89 formatted for English, German, French, Spanish, Japanese and Ukrainian,
   **When** compared with the expected table, **Then** each uses its own separators and grouping.
2. **Given** a frequency of 12,500,000 Hz, **When** formatted in each language, **Then** the value
   is scaled to a sensible unit (12.5 MHz) and written with the locale's decimal separator.
3. **Given** a file size of 31,981,568 bytes, **When** formatted, **Then** it is written in binary
   units with the locale's separators (about 30.5 MiB).
4. **Given** one fixed point in time, **When** formatted in each language, **Then** date and time
   follow that locale's order and clock convention, in the UTC offset supplied by the caller (the
   application passes the system's).
5. **Given** a number that is not finite or is out of the displayable range, **When** formatted,
   **Then** a readable marker is produced, never a panic.

---

### User Story 6 - CI refuses a gap in the translations (Priority: P2)

A check that runs with the normal tests and in CI compares all resource files and the places in the
code that use messages, and fails with a clear report if anything breaks parity: a key missing in
a language, a key a translation has but English does not, a key in English that nothing uses, a
placeholder that differs from the English one, a missing plural category, a resource that does not
parse, or a key the code uses that does not exist. The report names the file and key, so a
translator or developer can fix it without guessing.

**Why this priority**: the constitution makes the key-parity check a CI requirement and forbids
shipping a string without all six translations; it is P2 because it guards what US1–US5 deliver.

**Independent Test**: starting from a clean set of resources, introduce each class of defect in a
copy and confirm the check fails and names the defect; then confirm the clean set passes.

**Acceptance Scenarios**:

1. **Given** a key present in English and absent in Japanese, **When** the check runs, **Then** it
   fails and reports the key and the language.
2. **Given** a placeholder renamed in a translation, **When** the check runs, **Then** it fails and
   reports the key, the language and the two placeholder sets.
3. **Given** a Ukrainian message that provides only "one" and "other" for a counted message, **When**
   the check runs, **Then** it fails because "few" and "many" are required.
4. **Given** a key defined in English that no code uses, **When** the check runs, **Then** it fails
   (an unused key), with an explicit way to mark a key as intentionally reserved.
5. **Given** code that asks for a key that exists in no resource, **When** the check runs, **Then**
   it fails and names the file and line.
6. **Given** a resource file with a syntax error, **When** the check runs, **Then** it fails with the
   file, line and problem.
7. **Given** the complete, correct resources, **When** the check runs, **Then** it passes in under
   10 seconds.

---

### User Story 7 - Adding text is easy, and user content is never mistaken for a message (Priority: P3)

A developer adds a message by adding its key and English text, and the translations, to the
resource files, and asks for it by key; nothing else is needed. User- or third-party-supplied
content (channel names, labels, annotation text, output of decoders and scripts) is inserted into a
message verbatim as data and is never interpreted as message syntax, never translated, and never
able to change the message's structure.

**Why this priority**: it keeps the system maintainable and safe from the "my channel is named
`{ $count }`" class of bug, but the system is useful before it is polished.

**Independent Test**: insert strings that look like message syntax, contain right-to-left and
control characters, and are extremely long, as placeholder values; the output contains them
unchanged and the message around them is unaffected.

**Acceptance Scenarios**:

1. **Given** a channel name equal to `{ $count } samples`, **When** it is inserted into a message,
   **Then** the output contains exactly that text, unevaluated.
2. **Given** a placeholder value with control characters or text in a right-to-left script, **When**
   it is inserted, **Then** it is shown unchanged and the message structure around it is intact
   (the value is isolated so it cannot reorder the surrounding text).
3. **Given** a placeholder value of one million characters, **When** it is inserted, **Then** the
   message is produced without a crash or a noticeable delay.
4. **Given** a new key with English text and five translations, **When** it is requested, **Then**
   it is returned in every language with no other change to the code.

---

### Edge Cases

- A language tag with a script or region (`zh-Hant`, `pt-BR`, `de-CH`, `es-419`, `uk-UA`) or in
  a different case or with underscores (`de_DE.UTF-8` as some systems report it): mapped to the
  closest supported language or to English.
- A message whose translation is longer than English by a factor of three (German): the foundation
  returns the text; layout is the UI item's concern, and a test fixture with long text exists for it.
- A count that is negative, zero, fractional, very large (`u64::MAX`), or not a number.
- A plural message used with a placeholder that is not a number.
- Messages that are the same in every language (a unit symbol): they are still keys with a value in
  each file, so parity stays mechanical.
- Placeholder names that differ only in case, a key that contains characters the format treats as
  special, duplicate keys in one file, an empty message, a message of only whitespace.
- A resource file that is not valid UTF-8, has a byte order mark, uses Windows line endings, or is
  empty.
- Two threads switching the language at the same moment.
- Time zones and daylight-saving changes in formatted dates; dates before 1970 and far in the
  future.
- A system that reports no language, or a malformed one.
- Text that must keep the original capitalization (product name, unit symbols such as MHz and MiB)
  in every language.

## Requirements *(mandatory)*

### Functional Requirements

**Resources and lookup**

- **FR-001**: The system MUST provide one shared resource format for all user-facing text, used
  identically on Windows and Debian, with one resource file per language for each of the six
  languages (`en`, `de`, `fr`, `es`, `ja`, `uk`); the format and its library are chosen in the plan
  and recorded with the reasons (closing the constitution's open localization-format item).
- **FR-002**: The English resource file MUST be the single source of truth for keys: the set of
  keys, their placeholders and their plural categories are defined there.
- **FR-003**: The system MUST return a message by key for the current language, filling its
  placeholders with caller-supplied values.
- **FR-004**: A missing key or translation MUST fall back to English automatically; a key missing
  everywhere MUST produce a visible placeholder containing the key; neither may panic or return an
  empty string, and each MUST be reported through the log with the language and key.
- **FR-005**: A resource that cannot be read or parsed MUST make that language fall back to English
  as a whole, with a diagnostic naming the file and the problem; the other languages MUST be
  unaffected.
- **FR-006**: The resources MUST be part of the application itself (no resource file is looked up
  on the user's disk at run time in this version), so a missing file on the user's machine cannot
  remove a language.
- **FR-007**: Looking up a message MUST NOT panic, block on I/O, or allocate without bound for any
  key, language tag or placeholder value.

**Error messages**

- **FR-008**: Every stable error code defined by the product's crates (those of the storage, device
  and session crates, and any defined by the core and diagnostics crates) MUST have a message
  in all six languages, keyed by the code, that says what happened and what the user can do, written
  without technical jargon and without repeating the English technical detail.
- **FR-009**: The system MUST offer a function that turns an error (its kind and code) into a
  localized message, with a generic localized "unexpected error" message that mentions the code for
  codes without a message; the technical detail MUST NOT be part of the user-facing text.
- **FR-010**: A test MUST enumerate the error codes defined in the product's crates and fail if a
  code has no message, so a later feature cannot add a code without text.

**Plurals**

- **FR-011**: Plural forms MUST be selected by the localization mechanism using the Unicode CLDR
  plural rules of each language (cardinal numbers), including Ukrainian (one, few, many, other),
  Japanese (other only), French (one covers 0 and 1), Spanish, German and English; application code
  MUST NOT contain plural logic.
- **FR-012**: A counted message MUST provide every plural category its language requires; a missing
  category MUST be a parity failure in CI and a fallback to English at run time.

**Language selection**

- **FR-013**: The system MUST represent the current language as a plain value (a supported-language
  identifier) that can be stored and restored by the application settings.
- **FR-014**: The system MUST derive the initial language from the system language when no choice is
  stored, mapping variants (region, script, case, underscore form, encoding suffix) to the closest
  supported language and falling back to English.
- **FR-015**: The language MUST be changeable at run time, independently of the system language,
  without restarting; messages requested after the change MUST be in the new language, and a switch
  MUST be safe while other threads format messages (each message wholly in one language).
- **FR-016**: An invalid stored language value MUST NOT prevent startup: the system uses the system
  language or English and logs the problem.

**Locale formatting**

- **FR-017**: The system MUST format integers and decimals, frequencies (with automatic scaling
  from Hz to GHz), sample counts, durations (from nanoseconds to hours), file sizes in binary units,
  and dates and times according to the current language's locale conventions, through one shared
  facility; screens MUST NOT format these themselves.
- **FR-018**: Unit symbols (Hz, kHz, MHz, GHz, B, KiB, MiB, GiB, ns, µs, ms, s) MUST be written
  per locale conventions (spacing, and localized names where a language requires them) from the
  resources, not from code.
- **FR-019**: Formatting of a value that cannot be displayed (not finite, out of range) MUST produce
  a readable marker and MUST NOT panic.
- **FR-020**: Clock conventions MUST follow the locale, and the UTC offset MUST be supplied by the
  caller (the application passes the system's offset; named time zones and daylight-saving rules are
  not part of this item), so that formatted times are testable and reproducible.

**Safety of inserted content**

- **FR-021**: Placeholder values supplied by callers (including user- and third-party-supplied
  content) MUST be inserted verbatim as data: never evaluated as message syntax, never translated,
  never able to alter the structure of the message; text in right-to-left scripts MUST be isolated
  so it cannot reorder the surrounding text.
- **FR-022**: User- and third-party-supplied content MUST NOT be a message key or part of one.

**Parity check and process**

- **FR-023**: A check MUST run with the normal test suite and in CI and fail, naming the file and
  the key (and line where known), for: a key missing in a language; a key in a language that English
  lacks; an English key that no code uses (with an explicit marker for intentionally reserved keys);
  a key used by code that exists in no resource; a placeholder set that differs from English; a
  missing plural category; duplicate keys; an empty message; and a resource that does not parse.
- **FR-024**: The check MUST be deterministic, independent of the operating system, and complete in
  under 10 seconds for the full resource set.
- **FR-025**: The documentation MUST describe, for developers and translators: how to add or change
  a message, the key naming rules, how plurals and placeholders are written, how to mark a key as
  reserved, how machine-translated drafts are marked and reviewed, and how to add a language later.
- **FR-026**: Machine-translated drafts are acceptable (constitution); each resource MUST record
  whether its translation is a draft or reviewed, so a release can report how much text is still
  unreviewed.
- **FR-027**: Log output about missing keys and damaged resources MUST contain language, key and
  file names only, never the content of user-supplied placeholder values.

### Key Entities

- **Language**: one of the six supported languages, with its identifier, its English name and its
  own name (used by the language chooser in the settings).
- **Message key**: a stable, dotted, lowercase identifier of one piece of text; shared by all
  languages; the English resource defines it.
- **Message**: the text of one key in one language, with placeholders and, if counted, plural forms.
- **Resource file**: the messages of one language; carries a review status (draft or reviewed).
- **Catalogue of error messages**: the messages keyed by the stable error codes of the product.
- **Locale formatter**: the shared facility that writes numbers, frequencies, sizes, durations,
  dates and times for the current language.
- **Current language setting**: the value the settings store and the application changes at run time.
- **Parity report**: the output of the check, listing every defect with file, key and kind.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the stable error codes defined by the product's crates have a message in all
  six languages (checked by a test that enumerates the codes, today about 44), and none of the
  messages is empty or equal to its key.
- **SC-002**: For a table of at least 16 counts (including 0, 1, 2, 3, 4, 5, 11, 12, 14, 21, 22, 25,
  101, 111, 1,000,000 and 1.5) in each of the six languages, the selected plural category and wording
  equal a hand-written CLDR-based table in 100% of cases.
- **SC-003**: For a table of at least 12 values (numbers, frequencies, sizes, durations, one fixed
  date and time) in each of the six languages, the formatted text equals a hand-written table in
  100% of cases.
- **SC-004**: With any one of these defects introduced into a copy of the resources — a missing
  key, an extra key, an unused key, a changed placeholder, a missing plural category, a duplicate
  key, an empty message, a syntax error, a key used by code but undefined — the parity check fails
  and names the defect, in 100% of the nine cases; the correct resources pass.
- **SC-005**: Looking up and formatting any message takes under 100 microseconds on the reference
  machine, so a list of thousands of messages never delays a screen; switching the language takes
  under 10 milliseconds.
- **SC-006**: A stress run of 1,000,000 lookups with random keys (existing, missing, malformed),
  random language tags and placeholder values (including message-syntax look-alikes, control
  characters, right-to-left text and megabyte-long strings), on several threads while the language
  is switched, produces no panic, no hang and no message in a mixture of languages.
- **SC-007**: With a language's resource removed or damaged, 100% of the messages of that language
  appear in English and the application keeps running; the diagnostic names the file.
- **SC-008**: The initial language is correct for a table of at least 20 system language tags
  (supported, regional variants, scripts, underscore and encoding forms, unsupported, empty,
  malformed) in 100% of cases.
- **SC-009**: The full parity check finishes in under 10 seconds and gives the same result on
  Windows and Debian.
- **SC-010**: A developer can add a new message in all six languages by editing resource files only
  and requesting it by key, demonstrated by a documented walk-through that a reviewer follows in
  under 10 minutes without reading the implementation.

## Assumptions

- Only the foundation and the error-message catalogue are in scope. The rest of the UI text is
  written by the features that need it (starting with item 006); the foundation is built to receive
  it, and the check keeps that text complete from then on.
- The error model carries only a code and a technical English detail; messages for errors are
  therefore fixed text per code, without values from the failure. Carrying structured values from an
  error into its message (for example a path or a count) is a later extension of the error model and
  is not required here.
- Machine translation is acceptable as a draft (constitution); the resources record whether they are
  drafts or reviewed. Professional review of translations is outside this item.
- The user-facing UI, fonts and layout (including correct Japanese rendering and room for long
  German strings) belong to roadmap item 006; this item provides text and formatting, not drawing.
  A fixture with long and CJK text is provided so that item can test layout.
- Resources are compiled into the application (no external language packs in this version); adding
  a language later means adding a resource file and registering the language.
- Right-to-left languages are not supported in this version; the inserted-text isolation is
  required so that right-to-left user content does not disturb the surrounding text.
- Only cardinal plural rules are needed now (counts); ordinal and range forms are not.
- The date and time conventions follow the language's locale; a region-specific variant of a
  language (for example Swiss German number formatting) is treated as the base language.
- The choice of the resource format and the Rust library is made in the plan (Fluent is the leading
  candidate named in the roadmap) against the requirements above: CLDR plurals, placeholders,
  fallback, run-time language switching, availability on both operating systems, and no panics on
  malformed input. This closes the constitution's follow-up item for the localization format; the UI
  toolkit choice remains with item 006.
- Depends on roadmap item 001 (workspace, CI, error model, logging); the error catalogue also reads
  the error codes of items 002–004 and 003's session crate, which already exist.
