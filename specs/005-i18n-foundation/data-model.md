# Data Model: Localization Foundation

Entities, fields, rules and states. Rust signatures are in [contracts/i18n-api.md](contracts/i18n-api.md),
the files translators write in [contracts/resource-format.md](contracts/resource-format.md).

## Language

One of `en`, `de`, `fr`, `es`, `ja`, `uk` (a plain enum, `Copy`, stored in settings as its tag).

| Field | Rule |
|-------|------|
| tag | the lowercase primary language subtag: `en`, `de`, `fr`, `es`, `ja`, `uk` |
| English name, native name | for the language chooser (`English`, `Deutsch`, `Français`, `Español`, `日本語`, `Українська`); they are resources too (`language-name-<tag>` in `common.ftl`) so the chooser is localized like everything else, and the native names are the same in every language |
| fallback | always `en`; `en` has no fallback |

Parsing a tag (from settings or the system): lowercase, split at `-`, `_` or `.`, take the primary
subtag; accepted only if it is one of the six. `zh-Hant`, `pt-BR`, empty and malformed input are not
languages (negotiation then tries the next tag, then English).

## Key

A stable identifier of one message, shared by all languages.

| Rule | |
|------|--|
| syntax | a Fluent identifier: starts with an ASCII letter, then ASCII letters, digits, `-` or `_`; at most 100 characters |
| style | lowercase words separated by `-` (`error-session-damaged`, `unit-mhz`, `count-samples-lost`); a prefix per area (`error-`, `unit-`, `format-`, `language-`, then the feature's own) |
| definition | the English resource defines it; the other languages must match |
| from code | error keys are `error-` plus the error code with `.` replaced by `-` (injective: codes contain only lowercase letters, digits and `_` besides the dot) |
| never from users | a `Key` is built from a string literal or from a validated developer-side string; no API accepts a placeholder value as a key |

## Message

The text of one key in one language: a Fluent pattern with literal text, placeables
(`{ $count }`, `{ FORMAT_FREQUENCY($hz) }`, references to other messages) and selectors.

| Rule | |
|------|--|
| placeables | the set of variable names a message uses (including those in selectors and in referenced messages) must equal English's for every language |
| plurals | a selector on a count must have a variant for every plural category the language produces (research R3) and a default (`*[other]`) |
| non-empty | a message is not empty and not only whitespace |
| attributes | a message may have attributes (`.title`, `.hint`); they follow the same rules, key `message.attribute` |
| comment | the comment directly above a message is its translator note; `# @reserved` marks a key unused so far |

## Resource file

`resources/i18n/<tag>/<name>.ftl`, UTF-8 without byte order mark, `\n` line endings (a `\r\n` file is
accepted by the parser and flagged by the check). It begins with a header comment block containing
`### status: draft` or `### status: reviewed`. The set of file names is the same in every language and
is registered in `resources.rs`.

| State | Meaning |
|-------|---------|
| loaded | parsed without errors; its messages are in the bundle |
| damaged | a syntax error, a duplicate id or an unreadable header at load time: the language is degraded (see below), a diagnostic is recorded |
| draft / reviewed | the review status from the header, reported by the localizer |

## Language bundle and degradation

At start the localizer builds a bundle per language. If any file of a language has a syntax error that
the parser cannot recover from, or the language's resources are unusable, that language is marked
**degraded**: every lookup in it resolves from English, with one diagnostic per file. A language whose
files only lack some messages is not degraded: those messages fall back one by one.

## Lookup states of a message

1. found in the current language → its text;
2. not in the current language but in English → English text, diagnostic `missing-translation`;
3. in neither → the placeholder `⟦key⟧` (visible, contains the key), diagnostic `missing-key`;
4. a placeable cannot be resolved (missing argument, wrong type) → the message with that placeable
   shown as `⟦$name⟧`, diagnostic `format-error`;
5. a counted message lacks the needed plural category → the default variant is used by Fluent; the
   parity check catches it before release.

## Args (placeholder values)

| Kind | Meaning |
|------|---------|
| text (user content) | a string that is data: bidi controls U+202A–U+202E and U+2066–U+2069 are removed, the rest is wrapped in U+2068 … U+2069; never parsed |
| trusted text | a string from the program itself (a unit name) inserted as is |
| number | an integer or a finite or non-finite float; used for plural selection and for the format functions |

## Problem (diagnostic record)

`kind` (`missing-translation`, `missing-key`, `damaged-resource`, `format-error`,
`unknown-error-code`, `invalid-language-tag`), `language`, `key` or `file`. Recorded in a bounded list
(the first 1,000 distinct ones) and logged; never contains a placeholder value.

## Locale data (the formatter's table)

Per language: decimal separator, grouping separator, grouping size (3), minimum grouping digits (2 for
Spanish, 1 otherwise), the numeric short date pattern, the time pattern with seconds, the clock
convention (12 or 24 hour) and the AM and PM markers where used. Derived from CLDR (Unicode-3.0
license); each row has a test row written by hand.

| Language | Decimal | Group | Min grouping | Date | Time |
|----------|---------|-------|--------------|------|------|
| en | `.` | `,` | 1 | `M/d/yyyy` | `h:mm:ss a` |
| de | `,` | `.` | 1 | `dd.MM.yyyy` | `HH:mm:ss` |
| fr | `,` | U+202F | 1 | `dd/MM/yyyy` | `HH:mm:ss` |
| es | `,` | `.` | 2 | `d/M/yyyy` | `H:mm:ss` |
| ja | `.` | `,` | 1 | `yyyy/MM/dd` | `H:mm:ss` |
| uk | `,` | U+00A0 | 1 | `dd.MM.yyyy` | `HH:mm:ss` |

## Formatting rules

- **Number** (`FORMAT_NUMBER`): integers without fraction; others with at most 3 fraction digits,
  trailing zeros removed, rounded half up on the decimal digits;
  grouping per the table; a negative sign `-`; not finite or beyond ±1e15 → the marker `–` (en dash).
- **Frequency** (`FORMAT_FREQUENCY`, input hertz): the largest of Hz, kHz, MHz, GHz with value ≥ 1 (below
  1 Hz stays in Hz), at most 3 fraction digits; the unit and its spacing from the resource messages
  `unit-hz`, `unit-khz`, `unit-mhz`, `unit-ghz`.
- **Size** (`FORMAT_SIZE`, input bytes): B, KiB, MiB, GiB, TiB (powers of 1024), at most 1 fraction digit,
  B without fraction; units from `unit-b` … `unit-tib`.
- **Duration** (`FORMAT_DURATION`, input seconds as a number): below 60 s a single unit from ns, µs, ms,
  s with at most 3 fraction digits; 60 s and more as up to two components of h, min, s (`1 h 2 min`,
  `2 min 5 s`), values rounded down to the second; units from `unit-ns` … `unit-h`.
- **Date and time** (`FORMAT_DATE`, `FORMAT_TIME`, `FORMAT_DATETIME`, input seconds since the Unix epoch
  as a number): the civil date at the localizer's UTC offset (range years 1 to 9999, otherwise the
  marker), patterns from the table, date and time joined by one space.
- Unit messages take one variable, `$value`, already formatted; the function substitutes it.

## Parity defects

`missing-key`, `extra-key`, `unused-key`, `undefined-key` (used by code, defined nowhere),
`placeholder-mismatch`, `missing-plural-category`, `duplicate-key`, `empty-message`, `syntax-error`,
`unregistered-file`, `missing-file`, `missing-status-header`. Each defect carries file, line (where
known), key and the language.
