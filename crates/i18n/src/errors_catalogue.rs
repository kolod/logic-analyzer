//! The user-facing message of an error: it depends on the error's stable code and the language
//! only. The technical detail of the error is for the log and never reaches the user.

use la_core::Error;

use crate::args::Args;
use crate::key::Key;
use crate::localizer::Localizer;
use crate::problems::ProblemKind;

/// The key of the message used for codes that have none.
const GENERIC_KEY: Key = Key::from_static("error-unexpected");

impl Localizer {
    /// The user-facing message of `error`: it depends on the error's code and the current language
    /// only (never on its technical detail).
    #[must_use]
    pub fn error_message(&self, error: &Error) -> String {
        self.error_message_for_code(error.code())
    }

    /// The user-facing message of the error with this stable code, in the current language.
    ///
    /// A code without a message gives the generic message that mentions the code and records an
    /// `UnknownErrorCode` problem.
    #[must_use]
    pub fn error_message_for_code(&self, code: &str) -> String {
        let key = Key::for_error_code(code);
        if self.has_message(&key) {
            return self.message(&key, &Args::new());
        }
        self.problem_log()
            .record(ProblemKind::UnknownErrorCode, self.language(), code);
        self.message(&GENERIC_KEY, &Args::new().text("code", code))
    }
}
