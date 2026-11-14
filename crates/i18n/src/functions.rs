//! The functions resources can call: `FORMAT_NUMBER` and, in later tasks, the other formats.

use std::borrow::Cow;
use std::sync::Arc;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentResource, FluentValue};

use crate::language::Language;
use crate::locale_data::LocaleData;
use crate::numbers::{MARKER, format_decimal};
use crate::problems::{ProblemKind, ProblemLog};

/// Fraction digits of a plain number.
const NUMBER_FRACTION_DIGITS: usize = 3;

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

/// Registers the format functions of `language` on `bundle`.
pub(crate) fn register(
    bundle: &mut FluentBundle<FluentResource>,
    language: Language,
    problems: &Arc<ProblemLog>,
) {
    let data = LocaleData::for_language(language);
    let log = Arc::clone(problems);
    let added = bundle.add_function("FORMAT_NUMBER", move |positional, _named| {
        number_argument(positional, "FORMAT_NUMBER", language, &log, |value| {
            format_decimal(value, NUMBER_FRACTION_DIGITS, data)
        })
    });
    if added.is_err() {
        problems.record(ProblemKind::FormatError, language, "FORMAT_NUMBER");
    }
}
