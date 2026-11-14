//! The six supported languages and the mapping from system language tags.

/// One of the six languages of the application.
///
/// English is the base language and the fallback of every other one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// English.
    En,
    /// German.
    De,
    /// French.
    Fr,
    /// Spanish.
    Es,
    /// Japanese.
    Ja,
    /// Ukrainian.
    Uk,
}

/// The longest language tag that is looked at (BCP 47 tags are far shorter).
const MAX_TAG_CHARS: usize = 64;

impl Language {
    /// Every supported language: English first, then German, French, Spanish, Japanese and
    /// Ukrainian.
    pub const ALL: [Language; 6] = [
        Language::En,
        Language::De,
        Language::Fr,
        Language::Es,
        Language::Ja,
        Language::Uk,
    ];

    /// The lowercase language tag (`en`, `de`, `fr`, `es`, `ja`, `uk`), the value the settings
    /// store.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::De => "de",
            Language::Fr => "fr",
            Language::Es => "es",
            Language::Ja => "ja",
            Language::Uk => "uk",
        }
    }

    /// Maps a language tag to a supported language by its primary subtag.
    ///
    /// Tolerant of case, of `-` or `_` separators, of a region or script (`de-AT`, `es-419`) and of
    /// the forms some systems report (`de_DE.UTF-8`, `en_US.UTF-8@euro`). Anything else
    /// (including an unsupported language, an empty or over-long string, or a string with control
    /// characters) is `None`.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Language> {
        if tag.is_empty() || tag.chars().count() > MAX_TAG_CHARS {
            return None;
        }
        if !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'))
        {
            return None;
        }
        let primary = tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        Language::ALL.into_iter().find(|l| l.tag() == primary)
    }

    /// The position of the language in [`Language::ALL`].
    pub(crate) const fn index(self) -> usize {
        match self {
            Language::En => 0,
            Language::De => 1,
            Language::Fr => 2,
            Language::Es => 3,
            Language::Ja => 4,
            Language::Uk => 5,
        }
    }

    /// The language at a position of [`Language::ALL`] (English for an out-of-range index).
    pub(crate) fn from_index(index: usize) -> Language {
        Language::ALL.get(index).copied().unwrap_or(Language::En)
    }

    /// The first tag that maps to a supported language, or English if none does.
    #[must_use]
    pub fn negotiate<'a>(tags: impl IntoIterator<Item = &'a str>) -> Language {
        tags.into_iter()
            .find_map(Language::from_tag)
            .unwrap_or(Language::En)
    }

    /// The user's language according to the operating system, or English.
    #[must_use]
    pub fn from_system() -> Language {
        let tags = sys_locale::get_locales().collect::<Vec<String>>();
        Language::negotiate(tags.iter().map(String::as_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_has_six_languages_in_the_documented_order() {
        let tags: Vec<&str> = Language::ALL.iter().map(|l| l.tag()).collect();
        assert_eq!(tags, ["en", "de", "fr", "es", "ja", "uk"]);
    }

    #[test]
    fn tags_round_trip() {
        for language in Language::ALL {
            assert_eq!(Language::from_tag(language.tag()), Some(language));
        }
    }

    #[test]
    fn variants_of_a_supported_language_are_accepted() {
        let table = [
            ("de", Language::De),
            ("DE", Language::De),
            ("de-AT", Language::De),
            ("de_CH", Language::De),
            ("de_DE.UTF-8", Language::De),
            ("es-419", Language::Es),
            ("uk-UA", Language::Uk),
            ("ja-JP", Language::Ja),
            ("fr-CA", Language::Fr),
            ("en-GB", Language::En),
            ("en_US.UTF-8@euro", Language::En),
        ];
        for (tag, expected) in table {
            assert_eq!(Language::from_tag(tag), Some(expected), "{tag}");
        }
    }

    #[test]
    fn everything_else_is_rejected() {
        let long = "e".repeat(1_000);
        for tag in [
            "",
            "-",
            "zh-Hant",
            "pt-BR",
            "i-klingon",
            "deu",
            "d",
            "C",
            "POSIX",
            "xx_YY",
            " de",
            "de ",
            "de\0",
            "de\n",
            "d\u{e9}",
            long.as_str(),
        ] {
            assert_eq!(Language::from_tag(tag), None, "{tag:?}");
        }
        // A tag that is too long is rejected even if it starts with a supported language.
        assert_eq!(Language::from_tag(&format!("de-{}", "x".repeat(100))), None);
    }

    #[test]
    fn negotiation_takes_the_first_supported_tag_or_english() {
        assert_eq!(Language::negotiate(["it", "de-AT"]), Language::De);
        assert_eq!(Language::negotiate(["it", "ru"]), Language::En);
        assert_eq!(Language::negotiate([]), Language::En);
        assert_eq!(Language::negotiate(["", "-", "uk"]), Language::Uk);
    }

    #[test]
    fn the_system_language_is_always_a_supported_one() {
        assert!(Language::ALL.contains(&Language::from_system()));
    }
}
