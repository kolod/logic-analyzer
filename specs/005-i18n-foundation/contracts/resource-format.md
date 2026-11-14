# Contract: Resource Files (for developers and translators)

How user-facing text is written. Every rule below is enforced by the parity check
(`cargo test -p la-i18n --test parity`).

## Where

```text
resources/i18n/<lang>/<name>.ftl        <lang> = en | de | fr | es | ja | uk
```

The same file names exist in every language. Today: `errors.ftl` (one message per stable error code)
and `common.ftl` (units, formats, language names, generic messages). A feature adds its own file
(`decoders.ftl`, `settings.ftl`, ...) in all six languages and registers it in
`crates/i18n/src/resources.rs`. English is the source of truth for keys.

## File header

Every file starts with a comment block:

```ftl
### status: draft
### Machine-translated first version; needs review by a native speaker.
```

`status` is `draft` or `reviewed` (a reviewed file may add `### reviewed-by: Name, 2026-11-14`).
The status of a language is the weakest status of its files; a release reports how many files are drafts.

## Messages

```ftl
# Shown when the user opens a session file whose checksum does not match.
# Translator note: "session file" is the file the user saved from this program.
error-session-damaged = The session file is damaged and cannot be opened. If you have a backup copy, open that instead.

count-samples-lost =
    { $count ->
        [one] { FORMAT_NUMBER($count) } sample was lost.
       *[other] { FORMAT_NUMBER($count) } samples were lost.
    }
```

Rules:

1. **Keys** are lowercase words separated by `-` with a prefix per area (see the data model). The key of
   an error message is `error-` plus its code with `.` written as `-`.
2. **Placeholders** are written `{ $name }`. A translation uses exactly the placeholders of the English
   message (order and position are free). User-supplied text is passed by the program as a text argument
   and appears in the output isolated; translators do not need to do anything for it.
3. **Counts** use a selector on the count variable. A translation provides a variant for every plural
   category its language has (`one`, `few`, `many`, `other`) and marks `other` as the default (`*[other]`):

   | Language | Variants required |
   |----------|-------------------|
   | `en`, `de` | `one`, `*other` |
   | `fr`, `es` | `one`, `*other`; also write `many` (exact millions: `1 000 000 de samples`), which the shipped plural rules do not select yet (CLDR 42) but a library update will |
   | `uk` | `one`, `few`, `many`, `*other` |
   | `ja` | `*other` only |

   The check discovers the required set from the library (research R3); the table is what it finds today.
4. **Numbers** are displayed through the functions `FORMAT_NUMBER`, `FORMAT_FREQUENCY`, `FORMAT_SIZE`,
   `FORMAT_DURATION`, `FORMAT_DATE`, `FORMAT_TIME`, `FORMAT_DATETIME`; a bare `{ $count }` would print
   the number without locale separators and is flagged by the check in counted messages. The raw
   variable is used only as a selector.
5. **Units** are messages with one variable, `$value`, already formatted by the function:
   ```ftl
   unit-mhz = { $value }{ " " }MHz
   ```
   The spacing (a no-break space ` ` or a narrow one ` ` between value and unit in most
   languages, none in Japanese) is part of the translation.
6. **Special characters** (`{`, `}`) in literal text are written as placeables: `{ "{" }`. Write a no-break
   space as `{ " " }`, never as a literal character in the file.
7. **Product and unit names** keep their original form (`Logic16`, `MHz`, `MiB`) in every language.
8. **Reserved keys** (defined now, used by a later feature) carry `# @reserved` in their comment; the
   check otherwise reports an English key that no code uses.
9. **Length**: translations may be much longer than English (German by a factor of two or three); the UI
   item lays them out, the text is not shortened.
10. **User content** (channel names, annotation text, decoder output) is never translated and never part of
    a key; the program passes it as a text argument.

## Usage in code

```rust
use la_i18n::{key, Args, Localizer};

let text = localizer.message(&key!("count-samples-lost"), &Args::new().uint("count", lost));
let shown = localizer.error_message(&error);           // by code, never the technical detail
```

Keys in source must be written `key!("literal")` (or `Key::from_static("literal")`) so the check can find
them; a computed key is built only by `Key::for_error_code`.

## Adding a message (checklist)

1. Add the key with its English text (and a comment for the translators) to the right English file.
2. Add it, with the same placeholders and all plural variants, to the other five files; mark a machine
   draft by leaving the file `status: draft`.
3. Use it in code through `key!`.
4. Run `cargo test -p la-i18n`; the parity check names anything missing.

## Adding a language later

Add a variant to `Language`, its row to the locale data table with a hand-written test row, a directory
with every file, and the registration in `resources.rs`; the check then requires full parity for it.
