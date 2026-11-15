//! The fonts the application carries with it.
//!
//! iced bundles Fira Sans, which covers Latin, Latin Extended, Greek and Cyrillic. Japanese needs
//! a font of its own so that the interface is readable on a machine without CJK fonts (a minimal
//! Debian install): a subset of Noto Sans JP, built by `tools/build-font-subsets.py` from the
//! characters of the Japanese resources, kana and the JIS level 1 kanji. The text engine falls
//! back to it, and then to the system fonts, for any character the first font lacks.

/// The Japanese subset (SIL Open Font License 1.1, see `assets/fonts/OFL-NotoSansJP.txt`).
pub const JAPANESE: &[u8] = include_bytes!("../assets/fonts/NotoSansJP-subset.ttf");

/// The toolkit settings with the embedded fonts registered (for the headless renderer of the
/// tests; the application registers the font on its builder).
#[cfg(test)]
#[must_use]
pub fn settings() -> iced::Settings {
    iced::Settings {
        fonts: vec![std::borrow::Cow::Borrowed(JAPANESE)],
        ..iced::Settings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::{FontRef, TableProvider};

    fn font() -> FontRef<'static> {
        FontRef::new(JAPANESE).expect("the embedded font parses")
    }

    #[test]
    fn the_subset_is_small_enough() {
        // Both subsets together are to stay under 1.5 MB (research R5); only one is needed.
        assert!(JAPANESE.len() < 1_500_000, "{} bytes", JAPANESE.len());
        assert!(JAPANESE.len() > 100_000);
    }

    #[test]
    fn every_character_of_the_japanese_resources_is_in_the_font() {
        let cmap = font().cmap().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/i18n/ja");
        let mut missing = std::collections::BTreeSet::new();
        let mut checked = 0;
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap();
            // The characters this font is responsible for; Latin and Cyrillic (the name of
            // Ukrainian, for one) come from the bundled Fira Sans.
            let ours = |c: &char| matches!(u32::from(*c), 0x2000..=0x2fff | 0x3000..=0x9fff | 0xff00..=0xffef);
            for c in text.chars().filter(ours) {
                checked += 1;
                if cmap.map_codepoint(c).is_none() {
                    missing.insert(c);
                }
            }
        }
        assert!(checked > 1_000, "only {checked} characters read");
        assert!(missing.is_empty(), "not in the font: {missing:?}");
    }

    #[test]
    fn common_kana_and_level_one_kanji_are_in_the_font() {
        let cmap = font().cmap().unwrap();
        for c in "あいうえおアイウエオ日本語設定保存開閉表示倍率".chars() {
            assert!(cmap.map_codepoint(c).is_some(), "{c}");
        }
    }
}
