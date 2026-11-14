//! The localizer: all six languages, the current language, fallback and diagnostics.

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use unic_langid::LanguageIdentifier;

use crate::args::Args;
use crate::functions;
use crate::key::Key;
use crate::language::Language;
use crate::problems::{Problem, ProblemKind, ProblemLog};
use crate::resources;

/// The extra room (besides four times the text arguments) a formatted message may take.
const OUTPUT_SLACK_BYTES: usize = 64 * 1024;

/// Whether a language's files were reviewed by a native speaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    /// Machine translated or not yet reviewed.
    Draft,
    /// Reviewed.
    Reviewed,
}

/// One language: its bundle, whether it is usable and its review status.
struct LanguageBundle {
    bundle: FluentBundle<FluentResource>,
    /// A file of this language could not be parsed: every lookup resolves from English.
    degraded: bool,
    status: ReviewStatus,
}

struct Shared {
    bundles: Vec<LanguageBundle>,
    current: AtomicU8,
    problems: Arc<ProblemLog>,
}

/// All six languages, the current language and the diagnostics. Cheap to clone; `Send + Sync`.
#[derive(Clone)]
pub struct Localizer {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Localizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Localizer")
            .field("language", &self.language())
            .finish_non_exhaustive()
    }
}

/// The review status a file declares in its header (`### status: reviewed`); a file without a
/// header is a draft.
fn declared_status(text: &str) -> ReviewStatus {
    let reviewed = text
        .lines()
        .take_while(|line| line.starts_with("###"))
        .filter_map(|line| line.strip_prefix("###"))
        .filter_map(|rest| rest.trim().strip_prefix("status:"))
        .any(|value| value.trim() == "reviewed");
    if reviewed {
        ReviewStatus::Reviewed
    } else {
        ReviewStatus::Draft
    }
}

/// Builds the bundle of one language from its files.
fn build(
    language: Language,
    files: &[(String, String)],
    problems: &Arc<ProblemLog>,
) -> LanguageBundle {
    let id = LanguageIdentifier::from_str(language.tag()).unwrap_or_default();
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    bundle.set_use_isolating(false);
    functions::register(&mut bundle, language, problems);
    let mut degraded = false;
    let mut status = ReviewStatus::Reviewed;
    for (name, raw) in files {
        // A byte order mark at the start of a file is not part of the text.
        let text = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
        if declared_status(text) == ReviewStatus::Draft {
            status = ReviewStatus::Draft;
        }
        match FluentResource::try_new(text.to_owned()) {
            Ok(resource) => {
                if bundle.add_resource(resource).is_err() {
                    problems.record(ProblemKind::DamagedResource, language, name);
                }
            }
            Err(_) => {
                degraded = true;
                problems.record(ProblemKind::DamagedResource, language, name);
            }
        }
    }
    if files.is_empty() {
        status = ReviewStatus::Draft;
    }
    LanguageBundle {
        bundle,
        degraded,
        status,
    }
}

/// Formats message `key` with `bundle`, or `None` if the bundle has no such message.
fn format_with(
    bundle: &FluentBundle<FluentResource>,
    language: Language,
    key: &Key,
    args: &Args,
    problems: &ProblemLog,
) -> Option<String> {
    let message = bundle.get_message(key.as_str())?;
    let pattern = message.value()?;
    let fluent_args = args.to_fluent();
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
    if !errors.is_empty() {
        problems.record(ProblemKind::FormatError, language, key.as_str());
    }
    let limit = args
        .text_bytes()
        .saturating_mul(4)
        .saturating_add(OUTPUT_SLACK_BYTES);
    if text.len() > limit {
        problems.record(ProblemKind::FormatError, language, key.as_str());
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        return Some(text.get(..end).unwrap_or_default().to_owned());
    }
    Some(text.into_owned())
}

impl Localizer {
    /// Loads the embedded resources. Never fails: a damaged language degrades to English.
    #[must_use]
    pub fn new(initial: Language) -> Localizer {
        let sources: Vec<(Language, Vec<(String, String)>)> = Language::ALL
            .into_iter()
            .map(|language| {
                let files = resources::files(language.tag())
                    .iter()
                    .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
                    .collect();
                (language, files)
            })
            .collect();
        Localizer::from_sources(initial, sources)
    }

    /// Builds a localizer from in-memory resource files (for tests). A language that is not listed
    /// has no messages.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn from_sources(
        initial: Language,
        sources: Vec<(Language, Vec<(String, String)>)>,
    ) -> Localizer {
        Localizer::build_from(initial, sources)
    }

    #[cfg(not(any(test, feature = "test-support")))]
    fn from_sources(
        initial: Language,
        sources: Vec<(Language, Vec<(String, String)>)>,
    ) -> Localizer {
        Localizer::build_from(initial, sources)
    }

    fn build_from(initial: Language, sources: Vec<(Language, Vec<(String, String)>)>) -> Localizer {
        let problems = Arc::new(ProblemLog::default());
        let bundles = Language::ALL
            .into_iter()
            .map(|language| {
                let files: &[(String, String)] = sources
                    .iter()
                    .find(|(l, _)| *l == language)
                    .map_or(&[], |(_, files)| files.as_slice());
                build(language, files, &problems)
            })
            .collect();
        Localizer {
            shared: Arc::new(Shared {
                bundles,
                current: AtomicU8::new(u8::try_from(initial.index()).unwrap_or(0)),
                problems,
            }),
        }
    }

    /// A localizer whose language is chosen at start: the stored choice of the settings if there is
    /// a valid one, otherwise the first supported language among the system's preferred
    /// `system` tags, otherwise English. A stored value that is not a supported language is
    /// reported as a problem and ignored; it never prevents startup.
    #[must_use]
    pub fn from_settings(stored: Option<&str>, system: &[&str]) -> Localizer {
        let (initial, invalid_stored) = match stored {
            Some(tag) => match Language::from_tag(tag) {
                Some(language) => (language, false),
                None => (Language::negotiate(system.iter().copied()), true),
            },
            None => (Language::negotiate(system.iter().copied()), false),
        };
        let localizer = Localizer::new(initial);
        if invalid_stored {
            localizer.problem_log().record(
                ProblemKind::InvalidLanguageTag,
                initial,
                "stored-language",
            );
        }
        localizer
    }

    /// The current language.
    #[must_use]
    pub fn language(&self) -> Language {
        Language::from_index(usize::from(self.shared.current.load(Ordering::Acquire)))
    }

    /// Makes `language` the current one. Every message requested afterwards is in it; messages
    /// being formatted by other threads finish in the language they started with.
    pub fn set_language(&self, language: Language) {
        let index = u8::try_from(language.index()).unwrap_or(0);
        self.shared.current.store(index, Ordering::Release);
    }

    fn bundle(&self, language: Language) -> Option<&LanguageBundle> {
        self.shared.bundles.get(language.index())
    }

    /// The message for `key` in the current language, with English fallback. Always text.
    #[must_use]
    pub fn message(&self, key: &Key, args: &Args) -> String {
        self.message_in(self.language(), key, args)
    }

    /// The message for `key` in `language`, with English fallback. Always text.
    #[must_use]
    pub fn message_in(&self, language: Language, key: &Key, args: &Args) -> String {
        let problems = &self.shared.problems;
        if !Key::is_valid(key.as_str()) {
            problems.record(ProblemKind::MissingKey, language, "<invalid key>");
            return placeholder(key.as_str());
        }
        let mut missing_in_language = false;
        if language != Language::En {
            if let Some(entry) = self.bundle(language).filter(|b| !b.degraded) {
                if let Some(text) = format_with(&entry.bundle, language, key, args, problems) {
                    return text;
                }
                missing_in_language = true;
            }
        }
        if let Some(entry) = self.bundle(Language::En) {
            if let Some(text) = format_with(&entry.bundle, Language::En, key, args, problems) {
                if missing_in_language {
                    problems.record(ProblemKind::MissingTranslation, language, key.as_str());
                }
                return text;
            }
        }
        problems.record(ProblemKind::MissingKey, language, key.as_str());
        placeholder(key.as_str())
    }

    /// Whether English (the source of truth) has a message for `key`.
    pub(crate) fn has_message(&self, key: &Key) -> bool {
        Key::is_valid(key.as_str())
            && self
                .bundle(Language::En)
                .is_some_and(|b| b.bundle.has_message(key.as_str()))
    }

    /// The review status of a language (the weakest status of its files).
    #[must_use]
    pub fn review_status(&self, language: Language) -> ReviewStatus {
        self.bundle(language)
            .map_or(ReviewStatus::Draft, |b| b.status)
    }

    /// The problems found so far (fallbacks, damaged resources), without placeholder values.
    #[must_use]
    pub fn problems(&self) -> Vec<Problem> {
        self.shared.problems.snapshot()
    }

    pub(crate) fn problem_log(&self) -> &ProblemLog {
        &self.shared.problems
    }
}

/// The visible text shown when a key has no message anywhere.
fn placeholder(key: &str) -> String {
    format!("\u{27E6}{key}\u{27E7}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(text: &str) -> Vec<(String, String)> {
        vec![("test.ftl".to_owned(), text.to_owned())]
    }

    fn sample() -> Localizer {
        Localizer::from_sources(
            Language::De,
            vec![
                (
                    Language::En,
                    files(
                        "### status: reviewed\n\
                         hello = Hello\n\
                         only-english = Only English\n\
                         greet = Hi { $name }!\n\
                         count = { $n ->\n    [one] { FORMAT_NUMBER($n) } thing\n   *[other] { FORMAT_NUMBER($n) } things\n}\n",
                    ),
                ),
                (
                    Language::De,
                    files(
                        "### status: reviewed\n\
                         hello = Hallo\n\
                         greet = Hallo { $name }!\n\
                         count = { $n ->\n    [one] { FORMAT_NUMBER($n) } Ding\n   *[other] { FORMAT_NUMBER($n) } Dinge\n}\n",
                    ),
                ),
            ],
        )
    }

    #[test]
    fn a_message_is_found_in_the_current_language() {
        let l = sample();
        assert_eq!(l.language(), Language::De);
        assert_eq!(l.message(&Key::from_static("hello"), &Args::new()), "Hallo");
        assert!(l.problems().is_empty());
    }

    #[test]
    fn a_missing_translation_falls_back_to_english_and_is_recorded() {
        let l = sample();
        let key = Key::from_static("only-english");
        assert_eq!(l.message(&key, &Args::new()), "Only English");
        assert_eq!(l.message(&key, &Args::new()), "Only English");
        let problems = l.problems();
        assert_eq!(problems.len(), 1, "recorded once");
        assert_eq!(problems[0].kind, ProblemKind::MissingTranslation);
        assert_eq!(problems[0].language, Language::De);
        assert_eq!(problems[0].subject, "only-english");
    }

    #[test]
    fn an_unknown_key_shows_a_visible_placeholder() {
        let l = sample();
        assert_eq!(
            l.message(&Key::from_static("nowhere"), &Args::new()),
            "\u{27E6}nowhere\u{27E7}"
        );
        assert_eq!(l.problems()[0].kind, ProblemKind::MissingKey);
        for bad in ["", "a b", "a.b", "1abc", "\u{e9}"] {
            let text = l.message(&Key::from_static(bad), &Args::new());
            assert!(text.contains(bad) || bad.is_empty(), "{text}");
            assert!(!text.is_empty());
        }
    }

    #[test]
    fn a_missing_argument_still_renders_the_message() {
        let l = sample();
        let text = l.message(&Key::from_static("greet"), &Args::new());
        assert!(text.starts_with("Hallo ") && text.ends_with('!'), "{text}");
        assert!(
            l.problems()
                .iter()
                .any(|p| p.kind == ProblemKind::FormatError)
        );
    }

    #[test]
    fn plurals_and_the_number_function_work_inside_messages() {
        let l = sample();
        let key = Key::from_static("count");
        assert_eq!(l.message(&key, &Args::new().uint("n", 1)), "1 Ding");
        assert_eq!(
            l.message(&key, &Args::new().uint("n", 1_234)),
            "1.234 Dinge"
        );
        l.set_language(Language::En);
        assert_eq!(
            l.message(&key, &Args::new().uint("n", 1_234)),
            "1,234 things"
        );
        assert_eq!(l.message(&key, &Args::new().uint("n", 0)), "0 things");
    }

    #[test]
    fn user_text_is_inserted_as_data_and_never_evaluated() {
        let l = sample();
        let key = Key::from_static("greet");
        let text = l.message(
            &key,
            &Args::new().text("name", "{ $name } { NUMBER($n) } {"),
        );
        assert_eq!(text, "Hallo \u{2068}{ $name } { NUMBER($n) } {\u{2069}!");
        assert!(l.problems().is_empty());
    }

    #[test]
    fn problems_never_contain_argument_values() {
        let l = sample();
        let secret = "unique-secret-marker";
        let _ = l.message(
            &Key::from_static("missing-key"),
            &Args::new().text("x", secret),
        );
        let _ = l.message(
            &Key::from_static("greet"),
            &Args::new().text("other", secret),
        );
        for problem in l.problems() {
            assert!(!problem.subject.contains(secret));
        }
    }

    #[test]
    fn the_review_status_follows_the_headers() {
        let l = sample();
        assert_eq!(l.review_status(Language::En), ReviewStatus::Reviewed);
        assert_eq!(l.review_status(Language::De), ReviewStatus::Reviewed);
        assert_eq!(
            l.review_status(Language::Fr),
            ReviewStatus::Draft,
            "no files"
        );
        assert_eq!(
            declared_status("### status: draft\na = b\n"),
            ReviewStatus::Draft
        );
        assert_eq!(declared_status("a = b\n"), ReviewStatus::Draft);
        assert_eq!(
            declared_status("### status: reviewed\n"),
            ReviewStatus::Reviewed
        );
    }

    #[test]
    fn a_message_is_wholly_in_one_language_while_another_thread_switches() {
        let l = sample();
        let key = Key::from_static("hello");
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let switcher = {
            let (l, stop) = (l.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut i = 0;
                while !stop.load(Ordering::Acquire) {
                    l.set_language(if i % 2 == 0 {
                        Language::En
                    } else {
                        Language::De
                    });
                    i += 1;
                }
            })
        };
        let readers: Vec<_> = (0..3)
            .map(|_| {
                let (l, key) = (l.clone(), key.clone());
                std::thread::spawn(move || {
                    for _ in 0..100_000 {
                        let text = l.message(&key, &Args::new());
                        assert!(text == "Hello" || text == "Hallo", "{text}");
                    }
                })
            })
            .collect();
        for reader in readers {
            reader.join().unwrap();
        }
        stop.store(true, Ordering::Release);
        switcher.join().unwrap();
    }

    #[test]
    fn the_localizer_can_be_shared_between_threads() {
        fn assert_traits<T: Clone + Send + Sync>() {}
        assert_traits::<Localizer>();
    }
}
