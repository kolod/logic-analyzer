//! Message keys.

use std::borrow::Cow;
use std::fmt;

/// The longest key.
pub const MAX_KEY_CHARS: usize = 100;

/// The key of one message, shared by all languages.
///
/// Built from a string literal (with [`key!`](crate::key)) or from an error code. There is
/// deliberately no conversion from an arbitrary string, so user-supplied content can never become a
/// key.
///
/// ```compile_fail
/// // A key cannot be made from a runtime string.
/// let user_text = String::from("anything");
/// let _ = la_i18n::Key::from(user_text);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Key(Cow<'static, str>);

impl Key {
    /// A key from a literal. A key that is not a valid identifier simply finds no message.
    #[must_use]
    pub const fn from_static(id: &'static str) -> Key {
        Key(Cow::Borrowed(id))
    }

    /// The key of the message that describes the error with this stable code:
    /// `session.damaged` is `error-session-damaged`.
    #[must_use]
    pub fn for_error_code(code: &str) -> Key {
        let mut id = String::with_capacity(code.len().saturating_add(6));
        id.push_str("error-");
        id.extend(code.chars().map(|c| if c == '.' { '-' } else { c }));
        Key(Cow::Owned(id))
    }

    /// The key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `id` is a valid key: a Fluent identifier of at most 100 characters (an ASCII letter,
    /// then ASCII letters, digits, `-` or `_`).
    #[must_use]
    pub fn is_valid(id: &str) -> bool {
        let mut chars = id.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && id.chars().count() <= MAX_KEY_CHARS
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key({})", self.0)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A message key from a string literal: `key!("unit-mhz")`.
///
/// This is the only spelling of a literal key the parity check looks for in source files.
#[macro_export]
macro_rules! key {
    ($id:literal) => {
        $crate::Key::from_static($id)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_static_key_keeps_its_text() {
        let key = key!("unit-mhz");
        assert_eq!(key.as_str(), "unit-mhz");
        assert_eq!(key.to_string(), "unit-mhz");
        assert_eq!(key, Key::from_static("unit-mhz"));
    }

    #[test]
    fn error_codes_become_keys() {
        assert_eq!(
            Key::for_error_code("session.damaged").as_str(),
            "error-session-damaged"
        );
        assert_eq!(
            Key::for_error_code("storage.invalid_channel_count").as_str(),
            "error-storage-invalid_channel_count"
        );
        assert_eq!(Key::for_error_code("").as_str(), "error-");
    }

    #[test]
    fn the_mapping_is_injective_for_codes_of_the_documented_shape() {
        // Codes contain lowercase letters, digits and underscores plus one dot; the dot is the
        // only character that changes, so two different codes never give the same key.
        let codes = [
            "a.b_c",
            "a_b.c",
            "ab.c",
            "a.bc",
            "a.b",
            "a1.b",
            "storage.closed",
            "device.busy",
        ];
        let mut keys = std::collections::HashSet::new();
        for code in codes {
            assert!(
                code.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
            );
            assert_eq!(code.matches('.').count(), 1);
            assert!(keys.insert(Key::for_error_code(code)), "{code}");
        }
    }

    #[test]
    fn identifiers_are_validated() {
        for good in [
            "a",
            "unit-mhz",
            "error-session-damaged",
            "a_b-c1",
            &"a".repeat(100),
        ] {
            assert!(Key::is_valid(good), "{good}");
        }
        for bad in [
            "",
            "1a",
            "-a",
            "_a",
            "a b",
            "a.b",
            "\u{e9}",
            "a\u{e9}",
            &"a".repeat(101),
        ] {
            assert!(!Key::is_valid(bad), "{bad:?}");
        }
    }
}
