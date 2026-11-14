//! The translation parity check (feature `check`).
//!
//! It parses every resource file, compares the six languages with English (the source of truth)
//! and compares English with the places in the source code that use messages. A defect names the
//! file, the line where it is known, the key and the language. See `contracts/resource-format.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fluent_syntax::ast::{
    CallArguments, Entry, Expression, InlineExpression, Message, Pattern, PatternElement,
    VariantKey,
};

use crate::args::Args;
use crate::key::Key;
use crate::language::Language;
use crate::localizer::Localizer;

/// What is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefectKind {
    /// English has the key, the language does not.
    MissingKey,
    /// The language has a key English does not have.
    ExtraKey,
    /// English defines a key that no code uses and that is not marked `# @reserved`.
    UnusedKey,
    /// Code uses a key (or an error code) that English does not define.
    UndefinedKey,
    /// The placeholders of a translation differ from English's.
    PlaceholderMismatch,
    /// A counted message lacks a plural form its language needs.
    MissingPluralCategory,
    /// A key is defined twice in a language.
    DuplicateKey,
    /// A message has no text.
    EmptyMessage,
    /// A resource file does not parse.
    SyntaxError,
    /// A resource file exists but is not registered in the program.
    UnregisteredFile,
    /// A registered resource file does not exist for a language.
    MissingFile,
    /// A resource file has no `### status:` header.
    MissingStatusHeader,
    /// A resource file uses Windows line endings.
    LineEndings,
    /// A counted message shows its count without the number format function.
    RawNumber,
}

/// One problem found by the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    /// What is wrong.
    pub kind: DefectKind,
    /// The language concerned, if any.
    pub language: Option<Language>,
    /// The file concerned (relative to the checked directory when possible).
    pub file: String,
    /// The line, where known (1 is the first line).
    pub line: Option<usize>,
    /// The key concerned (empty if none).
    pub key: String,
    /// A short explanation.
    pub detail: String,
}

/// The result of a check.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Every defect found, in a stable order.
    pub defects: Vec<Defect>,
}

impl Report {
    /// `true` if nothing is wrong.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }

    /// One line per defect: `file:line key kind detail`.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for d in &self.defects {
            let line = d.line.map_or_else(String::new, |l| format!(":{l}"));
            let language = d.language.map_or("-", Language::tag);
            let _ = writeln!(
                out,
                "{}{line} [{language}] {} {:?} {}",
                d.file, d.key, d.kind, d.detail
            );
        }
        out
    }
}

/// What the check knows about one message of one file.
#[derive(Debug, Clone)]
struct MessageInfo {
    file: String,
    line: usize,
    placeholders: BTreeSet<String>,
    /// For every counted selector: the variant names present (plural categories and others).
    selectors: Vec<BTreeSet<String>>,
    reserved: bool,
}

/// All messages of one language, by key (`id` or `id.attribute`).
#[derive(Debug, Default)]
struct LanguageSet {
    messages: BTreeMap<String, MessageInfo>,
}

fn line_of(text: &str, id: &str) -> usize {
    text.lines()
        .position(|l| l.starts_with(id) && l[id.len()..].trim_start().starts_with('='))
        .map_or(1, |p| p.saturating_add(1))
}

fn line_at(text: &str, offset: usize) -> usize {
    text.get(..offset.min(text.len()))
        .map_or(1, |head| head.matches('\n').count().saturating_add(1))
}

fn collect_inline(
    inline: &InlineExpression<&str>,
    placeholders: &mut BTreeSet<String>,
    selectors: &mut Vec<BTreeSet<String>>,
    raw: &mut Vec<String>,
) {
    match inline {
        InlineExpression::VariableReference { id } => {
            placeholders.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            collect_arguments(arguments, placeholders, selectors, raw);
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => collect_arguments(arguments, placeholders, selectors, raw),
        InlineExpression::Placeable { expression } => {
            collect_expression(expression, placeholders, selectors, raw);
        }
        _ => {}
    }
}

fn collect_arguments(
    arguments: &CallArguments<&str>,
    placeholders: &mut BTreeSet<String>,
    selectors: &mut Vec<BTreeSet<String>>,
    raw: &mut Vec<String>,
) {
    for positional in &arguments.positional {
        collect_inline(positional, placeholders, selectors, raw);
    }
    for named in &arguments.named {
        collect_inline(&named.value, placeholders, selectors, raw);
    }
}

fn collect_expression(
    expression: &Expression<&str>,
    placeholders: &mut BTreeSet<String>,
    selectors: &mut Vec<BTreeSet<String>>,
    raw: &mut Vec<String>,
) {
    match expression {
        Expression::Inline(inline) => collect_inline(inline, placeholders, selectors, raw),
        Expression::Select { selector, variants } => {
            collect_inline(selector, placeholders, selectors, raw);
            let names: BTreeSet<String> = variants
                .iter()
                .map(|v| match &v.key {
                    VariantKey::Identifier { name } => (*name).to_owned(),
                    VariantKey::NumberLiteral { value } => (*value).to_owned(),
                })
                .collect();
            selectors.push(names);
            let selector_variable = match selector {
                InlineExpression::VariableReference { id } => Some(id.name),
                _ => None,
            };
            for variant in variants {
                if let Some(variable) = selector_variable {
                    for element in &variant.value.elements {
                        if let PatternElement::Placeable {
                            expression:
                                Expression::Inline(InlineExpression::VariableReference { id }),
                        } = element
                        {
                            if id.name == variable {
                                raw.push(variable.to_owned());
                            }
                        }
                    }
                }
                collect_pattern(&variant.value, placeholders, selectors, raw);
            }
        }
    }
}

fn collect_pattern(
    pattern: &Pattern<&str>,
    placeholders: &mut BTreeSet<String>,
    selectors: &mut Vec<BTreeSet<String>>,
    raw: &mut Vec<String>,
) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression(expression, placeholders, selectors, raw);
        }
    }
}

/// Whether a pattern has no visible text.
fn pattern_is_empty(pattern: &Pattern<&str>) -> bool {
    pattern.elements.iter().all(|e| match e {
        PatternElement::TextElement { value } => value.trim().is_empty(),
        PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::StringLiteral { value }),
        } => value.trim().is_empty(),
        PatternElement::Placeable { .. } => false,
    })
}

/// Reads one resource file into `set`, reporting syntax and structure defects.
fn read_file(
    language: Language,
    file: &str,
    text: &str,
    set: &mut LanguageSet,
    defects: &mut Vec<Defect>,
) {
    let mut defect = |kind, line, key: &str, detail: String| {
        defects.push(Defect {
            kind,
            language: Some(language),
            file: file.to_owned(),
            line,
            key: key.to_owned(),
            detail,
        });
    };
    if text.contains("\r\n") {
        defect(
            DefectKind::LineEndings,
            None,
            "",
            "the file uses Windows line endings".to_owned(),
        );
    }
    let has_status = text.lines().take_while(|l| l.starts_with("###")).any(|l| {
        l.trim_start_matches('#')
            .trim()
            .strip_prefix("status:")
            .is_some_and(|v| matches!(v.trim(), "draft" | "reviewed"))
    });
    if !has_status {
        defect(
            DefectKind::MissingStatusHeader,
            Some(1),
            "",
            "the file must start with `### status: draft` or `### status: reviewed`".to_owned(),
        );
    }
    let clean = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let resource = match fluent_syntax::parser::parse(clean) {
        Ok(resource) => resource,
        Err((resource, errors)) => {
            for error in errors {
                defect(
                    DefectKind::SyntaxError,
                    Some(line_at(clean, error.pos.start)),
                    "",
                    format!("{}", error.kind),
                );
            }
            resource
        }
    };
    for entry in &resource.body {
        let Entry::Message(message) = entry else {
            continue;
        };
        add_message(file, clean, message, set, &mut defect);
    }
}

fn add_message(
    file: &str,
    text: &str,
    message: &Message<&str>,
    set: &mut LanguageSet,
    defect: &mut impl FnMut(DefectKind, Option<usize>, &str, String),
) {
    let id = message.id.name;
    let line = line_of(text, id);
    let reserved = message
        .comment
        .as_ref()
        .is_some_and(|c| c.content.iter().any(|l| l.contains("@reserved")));
    let mut parts: Vec<(String, &Pattern<&str>)> = Vec::new();
    if let Some(value) = &message.value {
        parts.push((id.to_owned(), value));
    }
    for attribute in &message.attributes {
        parts.push((format!("{id}.{}", attribute.id.name), &attribute.value));
    }
    for (key, pattern) in parts {
        let (mut placeholders, mut selectors, mut raw) = (BTreeSet::new(), Vec::new(), Vec::new());
        collect_pattern(pattern, &mut placeholders, &mut selectors, &mut raw);
        if pattern_is_empty(pattern) {
            defect(
                DefectKind::EmptyMessage,
                Some(line),
                &key,
                "the message has no text".to_owned(),
            );
        }
        for variable in raw {
            defect(
                DefectKind::RawNumber,
                Some(line),
                &key,
                format!("`{{ ${variable} }}` shows the count without FORMAT_NUMBER"),
            );
        }
        if set
            .messages
            .insert(
                key.clone(),
                MessageInfo {
                    file: file.to_owned(),
                    line,
                    placeholders,
                    selectors,
                    reserved,
                },
            )
            .is_some()
        {
            defect(
                DefectKind::DuplicateKey,
                Some(line),
                &key,
                "the key is defined twice".to_owned(),
            );
        }
    }
}

/// The plural categories a language produces, discovered from the library (research R3).
fn plural_categories(language: Language) -> BTreeSet<String> {
    let probe = "k = { $n ->\n    [zero] zero\n    [one] one\n    [two] two\n    [few] few\n    [many] many\n   *[other] other\n}\n";
    let localizer = Localizer::from_sources(
        language,
        vec![(language, vec![("probe.ftl".to_owned(), probe.to_owned())])],
    );
    let key = Key::from_static("k");
    let mut samples: Vec<f64> = (0..2_000).map(f64::from).collect();
    samples.extend([
        1_000_000.0,
        2_000_000.0,
        10_000_000.0,
        0.5,
        1.5,
        2.5,
        10.5,
        21.5,
    ]);
    samples
        .into_iter()
        .map(|n| localizer.message(&key, &Args::new().float("n", n)))
        .collect()
}

/// Every string literal in the non-comment lines of a Rust file, with its line number.
fn literals(text: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let mut chars = line.chars().peekable();
        let mut current: Option<String> = None;
        while let Some(c) = chars.next() {
            match (&mut current, c) {
                (None, '"') => current = Some(String::new()),
                (Some(s), '\\') => {
                    if let Some(next) = chars.next() {
                        s.push(next);
                    }
                }
                (Some(_), '"') => {
                    if let Some(s) = current.take() {
                        found.push((s, index.saturating_add(1)));
                    }
                }
                (Some(s), other) => s.push(other),
                (None, _) => {}
            }
        }
    }
    found
}

/// What the sources say about keys.
#[derive(Debug, Default)]
struct SourceUse {
    /// Every string literal anywhere in the sources (evidence that a key is used).
    literals: BTreeSet<String>,
    /// Keys written with `key!("…")` or `Key::from_static("…")`, with file and line.
    keys: Vec<(String, String, usize)>,
    /// Error codes defined by `pub const NAME: &str = "domain.code";` in `error.rs` files.
    codes: Vec<(String, String, usize)>,
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), "target" | "tests" | "fixtures" | ".git") {
                collect_rust_files(&path, out);
            }
        } else if name.ends_with(".rs") && name != "parity.rs" {
            out.push(path);
        }
    }
}

fn scan_sources(source_dirs: &[PathBuf]) -> SourceUse {
    let mut files = Vec::new();
    for dir in source_dirs {
        collect_rust_files(dir, &mut files);
    }
    let mut used = SourceUse::default();
    for path in files {
        let Ok(full) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Test modules at the end of a file are not production code.
        let text = full.split("\n#[cfg(test)]").next().unwrap_or(&full);
        let name = path.display().to_string();
        let is_error_file = path.file_name().is_some_and(|n| n == "error.rs");
        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for marker in ["key!(", "Key::from_static("] {
                if let Some(at) = line.find(marker) {
                    let rest = line
                        .get(at.saturating_add(marker.len())..)
                        .unwrap_or("")
                        .trim_start();
                    if let Some(body) = rest.strip_prefix('"') {
                        if let Some(end) = body.find('"') {
                            let key = body.get(..end).unwrap_or("");
                            used.keys
                                .push((key.to_owned(), name.clone(), index.saturating_add(1)));
                        }
                    }
                }
            }
            if is_error_file {
                if let Some(rest) = trimmed.strip_prefix("pub const ") {
                    if let Some((_, value)) = rest.split_once(": &str = \"") {
                        if let Some(code) = value.strip_suffix("\";") {
                            if code.contains('.') {
                                used.codes.push((
                                    code.to_owned(),
                                    name.clone(),
                                    index.saturating_add(1),
                                ));
                            }
                        }
                    }
                }
            }
        }
        for (literal, _) in literals(text) {
            used.literals.insert(literal);
        }
    }
    used
}

fn english_name(language: Language) -> &'static str {
    language.tag()
}

/// Checks the resources under `resources_dir` (one directory per language) against each other and
/// against the sources under `source_dirs`; `registered` lists the file names the program embeds.
#[must_use]
pub fn check(resources_dir: &Path, source_dirs: &[PathBuf], registered: &[String]) -> Report {
    let mut defects = Vec::new();
    let mut sets: BTreeMap<usize, LanguageSet> = BTreeMap::new();

    for language in Language::ALL {
        let dir = resources_dir.join(english_name(language));
        let mut on_disk = BTreeSet::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.ends_with(".ftl") {
                    on_disk.insert(name);
                }
            }
        }
        for name in &on_disk {
            if !registered.contains(name) {
                defects.push(Defect {
                    kind: DefectKind::UnregisteredFile,
                    language: Some(language),
                    file: format!("{}/{name}", language.tag()),
                    line: None,
                    key: String::new(),
                    detail: "the file is not registered in resources.rs".to_owned(),
                });
            }
        }
        let mut set = LanguageSet::default();
        for name in registered {
            let file = format!("{}/{name}", language.tag());
            match std::fs::read_to_string(dir.join(name)) {
                Ok(text) => read_file(language, &file, &text, &mut set, &mut defects),
                Err(_) => defects.push(Defect {
                    kind: DefectKind::MissingFile,
                    language: Some(language),
                    file,
                    line: None,
                    key: String::new(),
                    detail: "a registered file does not exist for this language".to_owned(),
                }),
            }
        }
        sets.insert(language.index(), set);
    }

    let english = sets.remove(&Language::En.index()).unwrap_or_default();
    check_languages(&english, &sets, &mut defects);
    check_sources(&english, &scan_sources(source_dirs), &mut defects);

    defects.sort_by(|a, b| {
        (
            a.language.map(Language::index),
            &a.file,
            a.line,
            a.kind,
            &a.key,
        )
            .cmp(&(
                b.language.map(Language::index),
                &b.file,
                b.line,
                b.kind,
                &b.key,
            ))
    });
    Report { defects }
}

fn check_languages(
    english: &LanguageSet,
    others: &BTreeMap<usize, LanguageSet>,
    defects: &mut Vec<Defect>,
) {
    for language in Language::ALL {
        let required = plural_categories(language);
        let set = if language == Language::En {
            Some(english)
        } else {
            others.get(&language.index())
        };
        let Some(set) = set else { continue };
        for (key, info) in &set.messages {
            for variants in &info.selectors {
                let plural_keys: BTreeSet<&str> = variants
                    .iter()
                    .map(String::as_str)
                    .filter(|v| matches!(*v, "zero" | "one" | "two" | "few" | "many"))
                    .collect();
                if plural_keys.is_empty() {
                    continue;
                }
                let missing: Vec<&str> = required
                    .iter()
                    .map(String::as_str)
                    .filter(|c| *c != "other" && !variants.contains(*c))
                    .collect();
                if !missing.is_empty() {
                    defects.push(Defect {
                        kind: DefectKind::MissingPluralCategory,
                        language: Some(language),
                        file: info.file.clone(),
                        line: Some(info.line),
                        key: key.clone(),
                        detail: format!("missing plural forms: {}", missing.join(", ")),
                    });
                }
            }
        }
        if language == Language::En {
            continue;
        }
        for (key, source) in &english.messages {
            match set.messages.get(key) {
                None => defects.push(Defect {
                    kind: DefectKind::MissingKey,
                    language: Some(language),
                    file: source.file.replace("en/", &format!("{}/", language.tag())),
                    line: None,
                    key: key.clone(),
                    detail: "the key exists in English but not in this language".to_owned(),
                }),
                Some(info) if info.placeholders != source.placeholders => defects.push(Defect {
                    kind: DefectKind::PlaceholderMismatch,
                    language: Some(language),
                    file: info.file.clone(),
                    line: Some(info.line),
                    key: key.clone(),
                    detail: format!(
                        "placeholders {:?} differ from English {:?}",
                        info.placeholders, source.placeholders
                    ),
                }),
                Some(_) => {}
            }
        }
        for (key, info) in &set.messages {
            if !english.messages.contains_key(key) {
                defects.push(Defect {
                    kind: DefectKind::ExtraKey,
                    language: Some(language),
                    file: info.file.clone(),
                    line: Some(info.line),
                    key: key.clone(),
                    detail: "the key does not exist in English".to_owned(),
                });
            }
        }
    }
}

fn check_sources(english: &LanguageSet, used: &SourceUse, defects: &mut Vec<Defect>) {
    let code_keys: BTreeSet<String> = used
        .codes
        .iter()
        .map(|(code, _, _)| Key::for_error_code(code).as_str().to_owned())
        .collect();
    for (key, file, line) in &used.keys {
        if !english.messages.contains_key(key) {
            defects.push(Defect {
                kind: DefectKind::UndefinedKey,
                language: None,
                file: file.clone(),
                line: Some(*line),
                key: key.clone(),
                detail: "code uses a key that English does not define".to_owned(),
            });
        }
    }
    for (code, file, line) in &used.codes {
        let key = Key::for_error_code(code);
        if !english.messages.contains_key(key.as_str()) {
            defects.push(Defect {
                kind: DefectKind::UndefinedKey,
                language: None,
                file: file.clone(),
                line: Some(*line),
                key: key.as_str().to_owned(),
                detail: format!("the error code `{code}` has no message"),
            });
        }
    }
    for (key, info) in &english.messages {
        let used_by_code = used.literals.contains(key) || code_keys.contains(key);
        if !used_by_code && !info.reserved && !key.contains('.') {
            defects.push(Defect {
                kind: DefectKind::UnusedKey,
                language: Some(Language::En),
                file: info.file.clone(),
                line: Some(info.line),
                key: key.clone(),
                detail: "no code uses this key (mark it `# @reserved` if it is for later)"
                    .to_owned(),
            });
        }
    }
}

/// The file names the program embeds (the same for every language).
#[must_use]
pub fn registered_files() -> Vec<String> {
    crate::resources::files(Language::En.tag())
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect()
}
