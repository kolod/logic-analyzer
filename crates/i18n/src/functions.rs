//! The functions resources can call: `FORMAT_NUMBER`, `FORMAT_FREQUENCY`, `FORMAT_SIZE`,
//! `FORMAT_DURATION`, `FORMAT_DATE`, `FORMAT_TIME` and `FORMAT_DATETIME`. Each takes one number and
//! returns text in the language of the bundle; anything else gives the marker and a problem.

use std::borrow::Cow;
use std::sync::Arc;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentResource, FluentValue};

use crate::formatter::Formatter;
use crate::language::Language;
use crate::numbers::MARKER;
use crate::problems::{ProblemKind, ProblemLog};

/// The result of a format function for its first argument: the formatted text, or the marker and
/// a problem if the argument is not a number.
fn number_argument<'a>(
    positional: &[FluentValue<'a>],
    function: &str,
    language: Language,
    problems: &ProblemLog,
    format: impl Fn(f64) -> String,
) -> FluentValue<'a> {
    match positional.first() {
        Some(FluentValue::Number(n)) => FluentValue::String(Cow::Owned(format(n.value))),
        _ => {
            problems.record(ProblemKind::FormatError, language, function);
            FluentValue::String(Cow::Borrowed(MARKER))
        }
    }
}

/// Registers one format function.
macro_rules! register_function {
    ($bundle:expr, $name:literal, $formatter:expr, $problems:expr, $method:ident) => {{
        let formatter = Arc::clone($formatter);
        let log = Arc::clone($problems);
        let language = $formatter.language();
        let added = $bundle.add_function($name, move |positional, _named| {
            number_argument(positional, $name, language, &log, |value| {
                formatter.$method(value)
            })
        });
        if added.is_err() {
            $problems.record(ProblemKind::FormatError, language, $name);
        }
    }};
}

/// Registers the format functions of one language on `bundle`.
pub(crate) fn register(
    bundle: &mut FluentBundle<FluentResource>,
    formatter: &Arc<Formatter>,
    problems: &Arc<ProblemLog>,
) {
    register_function!(bundle, "FORMAT_NUMBER", formatter, problems, number);
    register_function!(bundle, "FORMAT_FREQUENCY", formatter, problems, frequency);
    register_function!(bundle, "FORMAT_SIZE", formatter, problems, size);
    register_function!(bundle, "FORMAT_DURATION", formatter, problems, duration);
    register_function!(bundle, "FORMAT_DATE", formatter, problems, date);
    register_function!(bundle, "FORMAT_TIME", formatter, problems, time);
    register_function!(bundle, "FORMAT_DATETIME", formatter, problems, datetime);
}
