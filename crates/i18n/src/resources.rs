//! The resource files compiled into the program.
//!
//! Every language has the same list of files. A new file is added here for all six languages
//! (the parity check fails if a file exists in `resources/i18n` but is not registered, or the
//! reverse).

/// A file name and its text.
pub(crate) type SourceFile = (&'static str, &'static str);

macro_rules! language_files {
    ($lang:literal) => {
        &[
            (
                "common.ftl",
                include_str!(concat!("../../../resources/i18n/", $lang, "/common.ftl")),
            ),
            (
                "errors.ftl",
                include_str!(concat!("../../../resources/i18n/", $lang, "/errors.ftl")),
            ),
        ]
    };
}

const EN: &[SourceFile] = language_files!("en");
const DE: &[SourceFile] = language_files!("de");
const FR: &[SourceFile] = language_files!("fr");
const ES: &[SourceFile] = language_files!("es");
const JA: &[SourceFile] = language_files!("ja");
const UK: &[SourceFile] = language_files!("uk");

/// The embedded files of the language with this tag (empty for an unknown tag).
pub(crate) fn files(tag: &str) -> &'static [SourceFile] {
    match tag {
        "en" => EN,
        "de" => DE,
        "fr" => FR,
        "es" => ES,
        "ja" => JA,
        "uk" => UK,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAGS: [&str; 6] = ["en", "de", "fr", "es", "ja", "uk"];

    #[test]
    fn every_language_has_the_same_file_names_and_non_empty_text() {
        let names = |tag: &str| -> Vec<&'static str> { files(tag).iter().map(|f| f.0).collect() };
        let english = names("en");
        assert!(!english.is_empty());
        for tag in TAGS {
            assert_eq!(names(tag), english, "{tag}");
            for (name, text) in files(tag) {
                assert!(!text.trim().is_empty(), "{tag}/{name} is empty");
                assert!(
                    text.starts_with("###"),
                    "{tag}/{name} must start with its header"
                );
            }
        }
        assert!(files("xx").is_empty());
    }
}
