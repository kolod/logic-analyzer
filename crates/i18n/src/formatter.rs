//! The shared formatter of one language: numbers, frequencies, sizes, durations, dates and times.
//!
//! The arithmetic is in `numbers`, `quantity` and `datetime`; the symbol of each unit and the
//! space between a value and its symbol come from the language's resources (`unit-*` messages),
//! read once when the language is loaded.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};

use crate::datetime::{self, Civil};
use crate::language::Language;
use crate::locale_data::LocaleData;
use crate::numbers::{MARKER, format_decimal};
use crate::problems::{ProblemKind, ProblemLog};
use crate::quantity::{self, Quantity, Unit};

/// Stands for the value inside a unit pattern while the pattern is extracted.
const VALUE_MARK: &str = "\u{E000}";
/// Stand for the two parts of a duration inside the pattern that joins them.
const FIRST_MARK: &str = "\u{E001}";
const SECOND_MARK: &str = "\u{E002}";

/// Fraction digits of a plain number.
const NUMBER_FRACTION_DIGITS: usize = 3;

/// The patterns read from the resources.
#[derive(Debug, Default)]
struct Patterns {
    units: HashMap<&'static str, String>,
    parts: Option<String>,
}

/// The formatter of one language.
pub(crate) struct Formatter {
    language: Language,
    data: &'static LocaleData,
    patterns: OnceLock<Patterns>,
    offset_seconds: Arc<AtomicI32>,
    problems: Arc<ProblemLog>,
}

/// Formats message `id` of `bundle` with the given arguments, or `None` if it does not exist.
fn format_message(
    bundle: &FluentBundle<FluentResource>,
    id: &str,
    args: &FluentArgs<'_>,
) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, Some(args), &mut errors);
    errors.is_empty().then(|| text.into_owned())
}

impl Formatter {
    /// The language this formatter writes.
    pub(crate) fn language(&self) -> Language {
        self.language
    }

    pub(crate) fn new(
        language: Language,
        offset_seconds: Arc<AtomicI32>,
        problems: Arc<ProblemLog>,
    ) -> Formatter {
        Formatter {
            language,
            data: LocaleData::for_language(language),
            patterns: OnceLock::new(),
            offset_seconds,
            problems,
        }
    }

    /// Reads the unit patterns from the loaded resources. Called once, after the files of the
    /// language were added to `bundle`; a missing message is recorded and replaced by an English
    /// symbol with a no-break space.
    pub(crate) fn load_patterns(&self, bundle: &FluentBundle<FluentResource>) {
        let mut patterns = Patterns::default();
        let mut value_args = FluentArgs::new();
        value_args.set("value", FluentValue::from(VALUE_MARK));
        for unit in Unit::ALL {
            match format_message(bundle, unit.key(), &value_args) {
                Some(text) if text.contains(VALUE_MARK) => {
                    patterns.units.insert(unit.key(), text);
                }
                // A unit without a message is reported when it is first needed.
                _ => {}
            }
        }
        let mut parts_args = FluentArgs::new();
        parts_args.set("first", FluentValue::from(FIRST_MARK));
        parts_args.set("second", FluentValue::from(SECOND_MARK));
        match format_message(bundle, "format-duration-parts", &parts_args) {
            Some(text) if text.contains(FIRST_MARK) && text.contains(SECOND_MARK) => {
                patterns.parts = Some(text);
            }
            _ => {}
        }
        // A second call has no effect: the patterns are read once.
        let _ = self.patterns.set(patterns);
    }

    fn with_unit(&self, text: &str, unit: Unit) -> String {
        match self.patterns.get().and_then(|p| p.units.get(unit.key())) {
            Some(pattern) => pattern.replace(VALUE_MARK, text),
            None => {
                self.problems
                    .record(ProblemKind::MissingTranslation, self.language, unit.key());
                format!("{text}\u{A0}{}", unit.english_symbol())
            }
        }
    }

    fn quantity(&self, quantity: Option<Quantity>) -> String {
        match quantity {
            None => MARKER.to_owned(),
            Some(Quantity::Single {
                value,
                fraction,
                unit,
            }) => self.with_unit(&format_decimal(value, fraction, self.data), unit),
            Some(Quantity::Parts(parts)) => {
                let mut texts = parts.iter().map(|(n, unit)| {
                    #[allow(
                        clippy::cast_precision_loss,
                        reason = "durations are far below 2^53 seconds"
                    )]
                    let number = *n as f64;
                    self.with_unit(&format_decimal(number, 0, self.data), *unit)
                });
                let first = texts.next().unwrap_or_default();
                match texts.next() {
                    None => first,
                    Some(second) => match self.patterns.get().and_then(|p| p.parts.as_ref()) {
                        Some(pattern) => pattern
                            .replace(FIRST_MARK, &first)
                            .replace(SECOND_MARK, &second),
                        None => {
                            self.problems.record(
                                ProblemKind::MissingTranslation,
                                self.language,
                                "format-duration-parts",
                            );
                            format!("{first} {second}")
                        }
                    },
                }
            }
        }
    }

    pub(crate) fn number(&self, value: f64) -> String {
        format_decimal(value, NUMBER_FRACTION_DIGITS, self.data)
    }

    pub(crate) fn frequency(&self, hertz: f64) -> String {
        self.quantity(quantity::frequency(hertz))
    }

    pub(crate) fn size(&self, bytes: f64) -> String {
        self.quantity(quantity::size(bytes))
    }

    pub(crate) fn duration(&self, seconds: f64) -> String {
        // A time before a point of reference (the axis counts from a marker) is negative: the
        // sign is written in front of the text of its size. A negative zero has no sign.
        if seconds.is_finite() && seconds < 0.0 {
            let size = self.quantity(quantity::duration(-seconds));
            return if size == MARKER {
                size
            } else {
                format!("-{size}")
            };
        }
        self.quantity(quantity::duration(seconds))
    }

    /// The civil time of `epoch_seconds` (a number that may not be a whole second or even finite).
    fn civil(&self, epoch_seconds: f64) -> Option<Civil> {
        if !epoch_seconds.is_finite() {
            return None;
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "out-of-range values are rejected by civil_from_epoch"
        )]
        let epoch = epoch_seconds.floor() as i64;
        datetime::civil_from_epoch(epoch, self.offset_seconds.load(Ordering::Acquire))
    }

    pub(crate) fn date(&self, epoch_seconds: f64) -> String {
        self.civil(epoch_seconds).map_or_else(
            || MARKER.to_owned(),
            |c| datetime::format_date(&c, self.data),
        )
    }

    pub(crate) fn time(&self, epoch_seconds: f64) -> String {
        self.civil(epoch_seconds).map_or_else(
            || MARKER.to_owned(),
            |c| datetime::format_time(&c, self.data),
        )
    }

    pub(crate) fn datetime(&self, epoch_seconds: f64) -> String {
        self.civil(epoch_seconds).map_or_else(
            || MARKER.to_owned(),
            |c| datetime::format_datetime(&c, self.data),
        )
    }
}
