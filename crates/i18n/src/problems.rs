//! Diagnostics about missing or unusable text.
//!
//! A problem names the kind, the language and a subject (a key, a file or a function name). It
//! never contains a placeholder value: values can be user content.

use std::collections::HashSet;
use std::sync::Mutex;

use la_diagnostics::lock_recover;

use crate::language::Language;

/// How many distinct problems are kept.
pub const MAX_PROBLEMS: usize = 1_000;

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProblemKind {
    /// The key exists in English but not in the requested language.
    MissingTranslation,
    /// The key exists in no language (or is not a valid key).
    MissingKey,
    /// A resource file could not be parsed or has a duplicate message.
    DamagedResource,
    /// A message could not be formatted completely (a missing argument, a bad value, a too large
    /// result).
    FormatError,
    /// An error code has no message.
    UnknownErrorCode,
    /// A stored language tag is not a supported language.
    InvalidLanguageTag,
}

/// One diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Problem {
    /// What went wrong.
    pub kind: ProblemKind,
    /// The language concerned.
    pub language: Language,
    /// The key, file name or function name concerned; never a placeholder value.
    pub subject: String,
}

#[derive(Debug, Default)]
struct Inner {
    list: Vec<Problem>,
    seen: HashSet<Problem>,
}

/// The bounded, de-duplicated list of problems of one localizer.
#[derive(Debug, Default)]
pub(crate) struct ProblemLog {
    inner: Mutex<Inner>,
}

impl ProblemLog {
    /// Records a problem the first time it happens; logs it once.
    pub(crate) fn record(&self, kind: ProblemKind, language: Language, subject: &str) {
        let problem = Problem {
            kind,
            language,
            subject: subject.to_owned(),
        };
        let mut inner = lock_recover(&self.inner);
        if inner.list.len() >= MAX_PROBLEMS || !inner.seen.insert(problem.clone()) {
            return;
        }
        tracing::warn!(
            target: "la_i18n",
            kind = ?problem.kind,
            language = problem.language.tag(),
            subject = %problem.subject,
            "localization problem"
        );
        inner.list.push(problem);
    }

    /// A copy of the problems so far, in the order they happened.
    pub(crate) fn snapshot(&self) -> Vec<Problem> {
        lock_recover(&self.inner).list.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_is_recorded_once_and_the_list_is_bounded() {
        let log = ProblemLog::default();
        for _ in 0..5 {
            log.record(ProblemKind::MissingKey, Language::De, "a-key");
        }
        assert_eq!(log.snapshot().len(), 1);
        for i in 0..2_000 {
            log.record(ProblemKind::MissingKey, Language::De, &format!("key-{i}"));
        }
        assert_eq!(log.snapshot().len(), MAX_PROBLEMS);
        assert_eq!(log.snapshot()[0].subject, "a-key");
    }
}
