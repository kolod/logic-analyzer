//! Display sanitizing of user-supplied text (channel names, marker names, descriptions).
//!
//! The text is shown as typed, never translated, but it must not be able to break the layout or
//! the order of the surrounding text: control characters, bidirectional controls and invisible
//! characters are removed, the ends are trimmed and the length is limited.

/// The longest channel or marker name the interface keeps, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// Whether `c` is removed from user text.
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

/// `text` without control, bidirectional and invisible characters, trimmed, and cut to at most
/// `max_chars` characters. Applying it twice gives the same result as applying it once.
#[must_use]
pub fn sanitize_text(text: &str, max_chars: usize) -> String {
    let filtered: String = text.chars().filter(|c| !is_removed(*c)).collect();
    let cut: String = filtered.trim().chars().take(max_chars).collect();
    cut.trim_end().to_owned()
}
