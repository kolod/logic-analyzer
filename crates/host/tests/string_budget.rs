//! Interface texts must fit their controls in every language (spec SC-008, task T077).
//!
//! The headless renderer cannot tell whether a translation overflows a button, so the texts that
//! sit in fixed-size places (menu entries, buttons, labels, titles) have a length budget:
//! relative to the English text, and an absolute maximum for the class. Lengths count a wide
//! (CJK) character as two, as it takes about two letters of width. A text over its budget
//! fails the test with the longest German and Japanese texts listed, so a translator sees what
//! is acceptable.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::BTreeMap;

const LANGUAGES: [&str; 6] = ["en", "de", "fr", "es", "ja", "uk"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Menu,
    Button,
    Label,
    Title,
}

impl Class {
    /// The class of a message, from its key (the naming convention of `ui.ftl`).
    fn of(key: &str) -> Option<Class> {
        if key.ends_with("-title") {
            Some(Class::Title)
        } else if key.starts_with("menu-") {
            Some(Class::Menu)
        } else if key.starts_with("toolbar-") {
            Some(Class::Label)
        } else if key.starts_with("dialog-") && !key.ends_with("-body") {
            Some(Class::Button)
        } else if matches!(
            key,
            "decoders-add"
                | "decoder-run"
                | "decoder-cancel"
                | "decoder-lane-show"
                | "decoder-source"
                | "decoder-source-capture"
                | "decoder-channel-none"
                | "panel-decoders"
        ) || key.starts_with("decoder-state-")
        {
            // The controls of the Decoders panel sit in a column 280 pixels wide.
            Some(Class::Label)
        } else {
            None
        }
    }

    /// The most width units a text of this class may have in any language.
    fn absolute_max(self) -> usize {
        match self {
            Class::Menu | Class::Button | Class::Label => 34,
            Class::Title => 40,
        }
    }
}

/// The width of a text in letter units: a wide character counts two.
fn width(text: &str) -> usize {
    text.chars()
        .map(|c| match u32::from(c) {
            0x3000..=0x9fff | 0xff00..=0xffef => 2,
            _ => 1,
        })
        .sum()
}

/// What a translation may take, given the English text.
fn allowed(english_width: usize, class: Class) -> usize {
    let relative = (english_width * 16).div_ceil(10).max(english_width + 16);
    relative.min(class.absolute_max())
}

/// The single-line messages of a resource file with their placeholders replaced by `00`.
fn messages(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            continue;
        }
        let mut plain = String::new();
        let mut depth = 0_u32;
        for c in value.chars() {
            match c {
                '{' => {
                    if depth == 0 {
                        plain.push_str("00");
                    }
                    depth += 1;
                }
                '}' => depth = depth.saturating_sub(1),
                _ if depth == 0 => plain.push(c),
                _ => {}
            }
        }
        out.insert(key.to_owned(), plain);
    }
    out
}

/// A text that is over its budget.
#[derive(Debug)]
struct Violation {
    language: String,
    key: String,
    text: String,
    width: usize,
    allowed: usize,
}

/// Every text of `languages` over its budget; `english` is the reference.
fn violations(
    english: &BTreeMap<String, String>,
    languages: &BTreeMap<String, BTreeMap<String, String>>,
) -> Vec<Violation> {
    let mut found = Vec::new();
    for (key, en) in english {
        let Some(class) = Class::of(key) else {
            continue;
        };
        let budget = allowed(width(en), class);
        for (language, texts) in languages {
            let Some(text) = texts.get(key) else {
                continue;
            };
            let w = width(text);
            if w > budget {
                found.push(Violation {
                    language: language.clone(),
                    key: key.clone(),
                    text: text.clone(),
                    width: w,
                    allowed: budget,
                });
            }
        }
    }
    found
}

fn load_all() -> BTreeMap<String, BTreeMap<String, String>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/i18n");
    LANGUAGES
        .iter()
        .map(|l| {
            let text = std::fs::read_to_string(root.join(l).join("ui.ftl")).unwrap();
            ((*l).to_owned(), messages(&text))
        })
        .collect()
}

/// The longest German and Japanese texts of the budgeted classes, for the failure message.
fn longest(languages: &BTreeMap<String, BTreeMap<String, String>>) -> String {
    let mut lines = Vec::new();
    for language in ["de", "ja"] {
        let mut texts: Vec<(usize, &String, &String)> = languages[language]
            .iter()
            .filter(|(k, _)| Class::of(k).is_some())
            .map(|(k, v)| (width(v), k, v))
            .collect();
        texts.sort_by_key(|t| std::cmp::Reverse(t.0));
        for (w, key, text) in texts.into_iter().take(3) {
            lines.push(format!("  {language} {w:>2}  {key}: {text}"));
        }
    }
    lines.join("\n")
}

#[test]
fn every_budgeted_text_fits_in_every_language() {
    let languages = load_all();
    let english = languages["en"].clone();
    assert!(english.len() > 80, "only {} messages read", english.len());
    let over = violations(&english, &languages);
    assert!(
        over.is_empty(),
        "texts over their budget: {over:#?}\nthe longest German and Japanese texts:\n{}",
        longest(&languages)
    );
}

#[test]
fn a_deliberately_over_long_text_is_caught() {
    let mut languages = load_all();
    let english = languages["en"].clone();
    // A button that no German translator should write.
    languages.get_mut("de").unwrap().insert(
        "dialog-save".into(),
        "Die aufgenommenen Signale jetzt in einer Sitzungsdatei speichern".into(),
    );
    // A Japanese menu entry far too long for its place.
    languages.get_mut("ja").unwrap().insert(
        "menu-file-new".into(),
        "新しいキャプチャを開始する前に現在のキャプチャを保存するかどうかを確認します".into(),
    );
    let over = violations(&english, &languages);
    let keys: Vec<(&str, &str)> = over
        .iter()
        .map(|v| (v.language.as_str(), v.key.as_str()))
        .collect();
    assert!(keys.contains(&("de", "dialog-save")), "{keys:?}");
    assert!(keys.contains(&("ja", "menu-file-new")), "{keys:?}");
    assert!(
        over.iter()
            .all(|v| v.width > v.allowed && !v.text.is_empty())
    );
}

#[test]
fn the_width_of_wide_characters_counts_double() {
    assert_eq!(width("Save"), 4);
    assert_eq!(width("保存"), 4);
    assert_eq!(width("Сохранить"), 9);
}

#[test]
fn the_budget_rule_is_the_larger_of_one_and_six_tenths_or_sixteen_more() {
    assert_eq!(allowed(4, Class::Button), 20);
    assert_eq!(
        allowed(30, Class::Button),
        34,
        "capped by the class maximum"
    );
    assert_eq!(allowed(12, Class::Menu), 28);
}
