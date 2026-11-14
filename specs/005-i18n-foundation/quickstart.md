# Quickstart: Validate the Localization Foundation

Prerequisite: the workspace builds (`cargo build --workspace`). All commands run from the repository
root on Windows 10/11 and Debian 13.

## 1. Error messages (User Story 1)

```text
cargo test -p la-i18n --test error_messages
```

Expected: every stable error code defined in the storage, device and session crates (found by scanning
their sources) has a message in each of the six languages, none empty or equal to its key; the English
text says what happened and what to do; a code without a message gives the generic localized message
with the code and records `unknown-error-code`; the technical detail never appears.

## 2. Plurals (User Story 2)

```text
cargo test -p la-i18n --test plurals
```

Expected: for 16 or more counts in each language the category and wording equal the hand-written CLDR
table (Ukrainian one/few/many/other, Japanese single form, French 0 and 1 as one, exact millions in
Spanish and French, fractions).

## 3. Fallback (User Story 3)

```text
cargo test -p la-i18n --test fallback
```

Expected: a missing translation shows English and records `missing-translation`; an unknown key shows
`⟦key⟧`; a damaged resource degrades that language to English as a whole with a diagnostic naming the
file and leaves the other languages alone; a missing argument still produces the message.

## 4. Language choice and runtime switch (User Story 4)

```text
cargo test -p la-i18n --test language_choice --test runtime_switch
```

Expected: 20 or more system language tags map as in the table (variants, scripts, `_` and `.UTF-8`
forms, unsupported, empty, malformed); a stored choice wins; a switch is visible in the next message
without restart; threads that format while another switches see each message wholly in one language.

## 5. Locale formatting (User Story 5)

```text
cargo test -p la-i18n --test formats
```

Expected: at least 12 values (numbers, frequencies, sizes, durations, one fixed date and time) formatted
in each language equal the hand-written table (for example 1,234,567.89 → `1,234,567.89`,
`1.234.567,89`, `1 234 567,89` with a narrow no-break space, `1.234.567,89`, `1,234,567.89`,
`1 234 567,89` with a no-break space).

## 6. The parity check (User Story 6)

```text
cargo test -p la-i18n --test parity
```

Expected: the real resources pass in under 10 seconds; for each of the defect classes (missing key, extra
key, unused key, changed placeholder, missing plural category, duplicate key, empty message, syntax
error, key used but undefined, unregistered file) a small broken copy fails and the report names file,
key and kind.

## 7. User content and stress (User Story 7, SC-006)

```text
cargo test -p la-i18n --test user_content --test stress
```

Expected: text that looks like message syntax, right-to-left text, control characters and megabyte
strings are inserted unchanged and isolated; one million lookups with random keys, tags and values on
several threads while the language is switched produce no panic, no hang and no mixed-language message.

## 8. Timing (informational)

```text
cargo bench -p la-i18n --bench lookup
```

Expected: a lookup and format well under 100 µs; a language switch well under 10 ms.

## 9. Whole gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny --all-features check
```

The same checks run in CI on both operating systems. `docs/localization.md` is the walk-through a new
developer follows to add a message in all six languages (SC-010).
