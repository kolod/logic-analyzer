//! Display sanitizing of text that comes from outside the application (literal annotation text,
//! message arguments, decoder-supplied names).
//!
//! The text is shown as given, never translated, but it must not be able to break the layout or the
//! order of the surrounding text: control characters, bidirectional controls and invisible
//! characters are removed and the ends are trimmed. The length is limited by the validation of the
//! model (a refusal), not by cutting here. The rules are those of `sanitize_text` in `la-host` (a
//! test in the host checks that the two agree).

/// Whether `c` is removed from outside text.
fn is_removed(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            // Line and paragraph separators.
            '\u{2028}' | '\u{2029}'
            // Zero-width characters and the byte order mark.
            | '\u{200b}'..='\u{200d}' | '\u{2060}' | '\u{feff}'
            // Directional marks, embeddings, overrides and isolates.
            | '\u{200e}' | '\u{200f}' | '\u{061c}'
            | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

/// `text` without control, bidirectional and invisible characters, trimmed at both ends.
/// Applying it twice gives the same result as applying it once.
#[must_use]
pub fn sanitize(text: &str) -> String {
    let filtered: String = text.chars().filter(|c| !is_removed(*c)).collect();
    filtered.trim().to_owned()
}
