# Contract: API of `la-i18n` (signatures only)

What the UI and every later feature use. A non-final sketch: bodies, helpers and lifetimes are decided
during implementation; changes follow semver. No function panics on any input and none blocks on I/O.

```rust
// ---- languages -----------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language { En, De, Fr, Es, Ja, Uk }
impl Language {
    pub const ALL: [Language; 6];
    pub fn tag(self) -> &'static str;                       // "en", "de", ...
    pub fn from_tag(tag: &str) -> Option<Language>;         // tolerant: case, `-`/`_`, region, ".UTF-8"
    /// First tag that maps to a supported language, otherwise English.
    pub fn negotiate<'a>(tags: impl IntoIterator<Item = &'a str>) -> Language;
    /// The user's language from the operating system (via `sys-locale`), otherwise English.
    pub fn from_system() -> Language;
}

// ---- keys and arguments --------------------------------------------------------
/// A message key. Built from literals (`key!`) or from error codes; never from user content.
pub struct Key(/* Cow<'static, str> */);
impl Key {
    pub const fn from_static(id: &'static str) -> Key;
    pub fn for_error_code(code: &str) -> Key;               // "session.damaged" -> "error-session-damaged"
    pub fn as_str(&self) -> &str;
}
/// `key!("unit-mhz")`: the only spelling the parity check recognises in source files.
#[macro_export] macro_rules! key { ($id:literal) => { $crate::Key::from_static($id) }; }

pub struct Args { /* named values */ }
impl Args {
    pub fn new() -> Args;
    /// User or third-party content: stripped of bidi controls and isolated, never interpreted.
    pub fn text(self, name: &str, value: &str) -> Args;
    /// Text written by the program itself, inserted as is.
    pub fn trusted(self, name: &str, value: &str) -> Args;
    pub fn int(self, name: &str, value: i64) -> Args;
    pub fn uint(self, name: &str, value: u64) -> Args;
    pub fn float(self, name: &str, value: f64) -> Args;     // any f64, including NaN and infinities
}

// ---- the localizer ------------------------------------------------------------------
/// All six languages, the current language and the diagnostics. Cheap to clone; Send + Sync.
#[derive(Clone)]
pub struct Localizer { /* Arc */ }
impl Localizer {
    /// Loads the embedded resources. Never fails: a damaged language degrades to English.
    pub fn new(initial: Language) -> Localizer;
    pub fn language(&self) -> Language;
    /// Takes effect for every message requested afterwards; safe while other threads format.
    pub fn set_language(&self, language: Language);
    /// The UTC offset used by the date and time formats, in seconds (default 0).
    pub fn set_utc_offset_seconds(&self, seconds: i32);

    /// The message for `key` in the current language, with English fallback. Always text.
    pub fn message(&self, key: &Key, args: &Args) -> String;
    pub fn message_in(&self, language: Language, key: &Key, args: &Args) -> String;
    /// The user-facing message of an error: depends on its code only (never on its detail).
    pub fn error_message(&self, error: &la_core::Error) -> String;
    pub fn error_message_for_code(&self, code: &str) -> String;
    /// The name of a language for the chooser (native name).
    pub fn language_name(&self, language: Language) -> String;

    // Formatting through the same code the resources use.
    pub fn format_number(&self, value: f64) -> String;
    pub fn format_frequency(&self, hertz: f64) -> String;
    pub fn format_size(&self, bytes: u64) -> String;
    pub fn format_duration(&self, seconds: f64) -> String;
    pub fn format_datetime(&self, epoch_seconds: i64) -> String;

    pub fn review_status(&self, language: Language) -> ReviewStatus;
    /// Problems found so far (fallbacks, damaged resources), without placeholder values.
    pub fn problems(&self) -> Vec<Problem>;
}

pub enum ReviewStatus { Draft, Reviewed }
pub struct Problem { pub kind: ProblemKind, pub language: Language, pub subject: String }
pub enum ProblemKind { MissingTranslation, MissingKey, DamagedResource, FormatError, UnknownErrorCode }

// ---- the parity check (feature `check`) ---------------------------------------------
pub mod parity {
    pub struct Report { pub defects: Vec<Defect> }
    pub struct Defect { pub kind: DefectKind, pub language: Option<Language>, pub file: String,
                        pub line: Option<usize>, pub key: String, pub detail: String }
    pub enum DefectKind { MissingKey, ExtraKey, UnusedKey, UndefinedKey, PlaceholderMismatch,
        MissingPluralCategory, DuplicateKey, EmptyMessage, SyntaxError, UnregisteredFile,
        MissingFile, MissingStatusHeader }
    /// Checks the resources under `resources_dir` against the sources under `workspace_dir`.
    pub fn check(resources_dir: &Path, workspace_dir: &Path) -> Report;
    impl Report { pub fn is_clean(&self) -> bool; pub fn render(&self) -> String; }   // `file:line key kind`
}
```

## Fluent functions available to resources

`FORMAT_NUMBER($n)`, `FORMAT_FREQUENCY($hz)`, `FORMAT_SIZE($bytes)`, `FORMAT_DURATION($seconds)`,
`FORMAT_DATE($epoch)`, `FORMAT_TIME($epoch)`, `FORMAT_DATETIME($epoch)`. Each takes one number and
returns text in the bundle's language; a non-number argument yields the marker `–` and a `format-error`
problem. (Fluent's built-in `NUMBER` is not used for display; it stays available for selectors.)

## Behaviour guarantees

- `message` and `error_message` always return text: the found message, English, or the placeholder
  `⟦key⟧`; they never panic, block on I/O, or return an empty string for a non-empty English message.
- A language switch is one atomic store (under 10 ms including nothing but that); a message reads the
  language once, so it is wholly in one language.
- Placeholder values are data: `{ $name }` inside a value is output as it is; bidi controls are removed
  from text arguments and the value is isolated.
- `problems()` and the log contain language, key and file only, never placeholder values.
- The embedded resources never change at run time; no file on the user's disk is read.
- `error_message` for a code with no message returns `error-unexpected` with the code as an argument and
  records `unknown-error-code`.
