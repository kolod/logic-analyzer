# Localization: adding and changing text

All user-facing text lives in Project Fluent (`.ftl`) files under `resources/i18n/<language>/` and is
compiled into the program. The program speaks six languages: English (`en`, the source of truth and the
fallback), German (`de`), French (`fr`), Spanish (`es`), Japanese (`ja`) and Ukrainian (`uk`). The rules
below are the short version of `specs/005-i18n-foundation/contracts/resource-format.md`; the parity check
(`cargo test -p la-i18n --test parity`) enforces them and names the file and key of anything that is
wrong.

## The files

```text
resources/i18n/en/errors.ftl     one message per stable error code
resources/i18n/en/common.ftl     counted messages, units, formats, language names
resources/i18n/<lang>/...        the same files in the other five languages
```

Every file starts with a status header:

```ftl
### status: draft
```

Use `draft` for a machine translation and `reviewed` once a native speaker has read the file. A release
reports how many files are drafts.

## Add a message: a worked example

We want to show "3 samples were lost" (or "1 sample was lost") when a device overrun is recorded.

**1. English first.** Add the key to `resources/i18n/en/common.ftl` (or the file of your feature, see "Add
a file" below). A counted message selects its form with the language's plural rules and writes the number
with `FORMAT_NUMBER`, so every language uses its own separators:

```ftl
# Shown in the capture summary when an overrun lost samples. $count is the number of lost samples.
count-samples-lost =
    { $count ->
        [one] { FORMAT_NUMBER($count) } sample was lost.
       *[other] { FORMAT_NUMBER($count) } samples were lost.
    }
```

The comment above the message is a note for translators: say where it appears and what the variables are.

**2. The other five languages.** Add the same key with the same variables, and every plural form the
language needs:

| Language | Forms to write | Example ending |
|----------|----------------|----------------|
| `de` | `one`, `*other` | `Sample ging verloren.` / `Samples gingen verloren.` |
| `fr` | `one`, `*other` (and `many`, for exact millions: `d'échantillons`) | `échantillon a été perdu.` |
| `es` | `one`, `*other` (and `many`: `de muestras`) | `muestra se perdió.` |
| `ja` | `*other` only | `サンプルが失われました。` |
| `uk` | `one`, `few`, `many`, `*other` | `відлік утрачено.` / `відліки утрачено.` / `відліків утрачено.` |

Ukrainian needs four forms: one (1, 21, 101), few (2–4, 22–24), many (0, 5–20, 25–30) and other (fractions).
Use `*[other]` for the default. If a language needs more forms than you write, the check says which one is
missing. Leave the file's `### status: draft` as it is.

**3. Use it in code.** Write the key as a literal inside `key!` so the parity check can find it:

```rust
use la_i18n::{key, Args};

let text = localizer.message(&key!("count-samples-lost"), &Args::new().uint("count", lost));
```

User-supplied text (a channel name, an annotation, decoder output) is passed with `Args::text`, never
concatenated into a message and never used as a key:

```rust
let text = localizer.message(&key!("channel-renamed"), &Args::new().text("name", &channel_name));
```

**4. Run the check.**

```text
cargo test -p la-i18n
```

If you forgot a language, the report says so, for example:

```text
ja/common.ftl [ja] count-samples-lost MissingKey the key exists in English but not in this language
```

A key that no code uses yet is reported too; mark it `# @reserved` in English if it is for a later
feature.

## Error messages

The message of an error is keyed by its stable code: `session.damaged` is `error-session-damaged` in
`errors.ftl`. When you add an error code to a crate's `error.rs`
(`pub const NAME: &str = "domain.code";`), add its message to `errors.ftl` in all six languages in the same
change; the check fails otherwise. An error message says what happened and what the user can do. It never
contains technical details: those stay in the error's `detail`, which goes to the log only.

## Numbers, sizes, times

Never format a number yourself. In a message use the functions `FORMAT_NUMBER`, `FORMAT_FREQUENCY`
(hertz), `FORMAT_SIZE` (bytes), `FORMAT_DURATION` (seconds), `FORMAT_DATE`, `FORMAT_TIME`,
`FORMAT_DATETIME` (seconds since the Unix epoch). In code use the same facility through
`localizer.format_frequency(hz)` and friends. The symbol of a unit and the space before it are part of the
translation (the `unit-*` messages in `common.ftl`).

## Add a file

Create `resources/i18n/<lang>/<name>.ftl` for all six languages (with the status header), then register the
name in `crates/i18n/src/resources.rs` (one line per language in the `language_files!` macro). The check
fails if a file exists that is not registered, or the reverse.

## Add a language later

Add a variant to `Language` in `crates/i18n/src/language.rs`, a row to `crates/i18n/src/locale_data.rs` with
a hand-written test row in `numbers.rs` and `datetime.rs`, a directory with every file, and the language's
files to `resources.rs`. The parity check then requires full parity for it.

## What not to do

- Do not put user-visible text in code, in `format!` strings or in the error `detail`.
- Do not write plural logic (`if n == 1`) in code; use a selector in the message.
- Do not translate user content, product names (`Logic16`) or unit symbols that are the same everywhere.
- Do not edit another language's file to "fix" English: change English, then update the translations.
