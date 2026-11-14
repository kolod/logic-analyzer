//! Placeholder values passed to messages.
//!
//! Values are data: they are handed to the message resolver as values and never concatenated into
//! message text, so message syntax inside a value is never evaluated. Text that comes from users
//! or third parties (channel names, annotations, decoder output) is additionally stripped of
//! bidirectional control characters and wrapped in an isolate, so it cannot reorder the text around
//! it.

use fluent_bundle::{FluentArgs, FluentValue};

use crate::key::Key;

/// The largest number of arguments one message call keeps.
pub const MAX_ARGS: usize = 64;

/// First strong isolate: starts an isolated run of text.
pub(crate) const FSI: char = '\u{2068}';
/// Pop directional isolate: ends it.
pub(crate) const PDI: char = '\u{2069}';

/// One value of a call.
#[derive(Debug, Clone)]
enum Value {
    Text(String),
    Number(f64),
}

/// The named values of one message call.
#[derive(Debug, Clone, Default)]
pub struct Args {
    values: Vec<(String, Value)>,
    dropped: usize,
}

/// Whether `c` is a bidirectional embedding, override or isolate control.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

impl Args {
    /// No arguments.
    #[must_use]
    pub fn new() -> Args {
        Args::default()
    }

    fn put(mut self, name: &str, value: Value) -> Args {
        if !Key::is_valid(name) {
            self.dropped = self.dropped.saturating_add(1);
            return self;
        }
        if let Some(slot) = self.values.iter_mut().find(|(n, _)| n == name) {
            slot.1 = value;
        } else if self.values.len() >= MAX_ARGS {
            self.dropped = self.dropped.saturating_add(1);
        } else {
            self.values.push((name.to_owned(), value));
        }
        self
    }

    /// User or third-party content (a channel name, an annotation, decoder output).
    ///
    /// Bidirectional controls are removed and the rest is isolated; the value is never interpreted
    /// as message syntax and never translated.
    #[must_use]
    pub fn text(self, name: &str, value: &str) -> Args {
        let mut isolated = String::with_capacity(value.len().saturating_add(6));
        isolated.push(FSI);
        isolated.extend(value.chars().filter(|c| !is_bidi_control(*c)));
        isolated.push(PDI);
        self.put(name, Value::Text(isolated))
    }

    /// Text written by the program itself (for example the name of a unit), inserted as is.
    #[must_use]
    pub fn trusted(self, name: &str, value: &str) -> Args {
        self.put(name, Value::Text(value.to_owned()))
    }

    /// A signed integer. Values beyond 2^53 lose their low digits (the value is carried as a
    /// double, which is what the plural rules and the number formats use).
    #[must_use]
    pub fn int(self, name: &str, value: i64) -> Args {
        #[allow(
            clippy::cast_precision_loss,
            reason = "documented: values beyond 2^53 are approximate"
        )]
        let number = value as f64;
        self.put(name, Value::Number(number))
    }

    /// An unsigned integer (a count). Values beyond 2^53 lose their low digits.
    #[must_use]
    pub fn uint(self, name: &str, value: u64) -> Args {
        #[allow(
            clippy::cast_precision_loss,
            reason = "documented: values beyond 2^53 are approximate"
        )]
        let number = value as f64;
        self.put(name, Value::Number(number))
    }

    /// Any double, including NaN and the infinities.
    #[must_use]
    pub fn float(self, name: &str, value: f64) -> Args {
        self.put(name, Value::Number(value))
    }

    /// The total size of the text arguments in bytes (bounds the size of a formatted message).
    pub(crate) fn text_bytes(&self) -> usize {
        self.values
            .iter()
            .map(|(_, v)| match v {
                Value::Text(t) => t.len(),
                Value::Number(_) => 0,
            })
            .fold(0_usize, usize::saturating_add)
    }

    /// How many arguments were ignored (an invalid name, or more than 64 arguments).
    #[must_use]
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// The arguments in the form the resolver takes.
    pub(crate) fn to_fluent(&self) -> FluentArgs<'_> {
        let mut args = FluentArgs::with_capacity(self.values.len());
        for (name, value) in &self.values {
            match value {
                Value::Text(text) => args.set(name.as_str(), FluentValue::from(text.as_str())),
                Value::Number(number) => args.set(name.as_str(), FluentValue::from(*number)),
            }
        }
        args
    }

    /// The text value of an argument (for tests and diagnostics).
    #[cfg(test)]
    fn text_of(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| match v {
                Value::Text(t) => Some(t.as_str()),
                Value::Number(_) => None,
            })
    }

    /// The number value of an argument (for tests).
    #[cfg(test)]
    fn number_of(&self, name: &str) -> Option<f64> {
        self.values
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| match v {
                Value::Number(n) => Some(*n),
                Value::Text(_) => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_text_is_stripped_of_bidi_controls_and_isolated() {
        let all_controls: String = ('\u{202A}'..='\u{202E}')
            .chain('\u{2066}'..='\u{2069}')
            .collect();
        let args = Args::new()
            .text("plain", "channel 1")
            .text("empty", "")
            .text("only_controls", &all_controls)
            .text("mixed", "a\u{202E}b\u{2069}c\u{2066}d");
        assert_eq!(args.text_of("plain"), Some("\u{2068}channel 1\u{2069}"));
        assert_eq!(args.text_of("empty"), Some("\u{2068}\u{2069}"));
        assert_eq!(args.text_of("only_controls"), Some("\u{2068}\u{2069}"));
        assert_eq!(args.text_of("mixed"), Some("\u{2068}abcd\u{2069}"));
        // Other invisible characters are the user's content and stay.
        let kept = Args::new().text("x", "a\u{200F}b\u{200E}\u{FEFF}\0");
        assert_eq!(
            kept.text_of("x"),
            Some("\u{2068}a\u{200F}b\u{200E}\u{FEFF}\0\u{2069}")
        );
    }

    #[test]
    fn trusted_text_is_inserted_as_is() {
        let args = Args::new().trusted("unit", "MHz");
        assert_eq!(args.text_of("unit"), Some("MHz"));
    }

    #[test]
    fn numbers_cover_the_whole_range_without_panicking() {
        let args = Args::new()
            .int("min", i64::MIN)
            .uint("max", u64::MAX)
            .float("nan", f64::NAN)
            .float("inf", f64::INFINITY)
            .float("neg_inf", f64::NEG_INFINITY)
            .uint("small", 3)
            .int("negative", -7);
        assert_eq!(args.number_of("small"), Some(3.0));
        assert_eq!(args.number_of("negative"), Some(-7.0));
        assert!(args.number_of("nan").is_some_and(f64::is_nan));
        assert_eq!(args.number_of("inf"), Some(f64::INFINITY));
        assert!(args.number_of("min").is_some_and(|v| v < -9.0e18));
        assert!(args.number_of("max").is_some_and(|v| v > 1.8e19));
        let _ = args.to_fluent();
    }

    #[test]
    fn a_later_value_replaces_an_earlier_one_with_the_same_name() {
        let args = Args::new().uint("n", 1).uint("n", 2);
        assert_eq!(args.number_of("n"), Some(2.0));
        assert_eq!(args.dropped(), 0);
    }

    #[test]
    fn invalid_names_and_too_many_arguments_are_ignored_and_counted() {
        let mut args = Args::new()
            .uint("", 1)
            .uint("1a", 1)
            .uint("a b", 1)
            .uint(&"n".repeat(101), 1);
        assert_eq!(args.dropped(), 4);
        for i in 0..70 {
            args = args.uint(&format!("n{i}"), 1);
        }
        assert_eq!(args.values.len(), MAX_ARGS);
        assert_eq!(args.dropped(), 4 + 6);
        // Replacing an existing name still works when the list is full.
        args = args.uint("n0", 9);
        assert_eq!(args.number_of("n0"), Some(9.0));
    }

    #[test]
    fn a_huge_text_is_accepted() {
        let big = "x".repeat(1_000_000);
        let args = Args::new().text("big", &big);
        assert_eq!(args.text_of("big").map(str::len), Some(1_000_000 + 6));
    }
}
