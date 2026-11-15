//! The display sanitizer for user text (data-model "Channel presentation").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

use la_host::sanitize::{MAX_NAME_CHARS, sanitize_text};

#[test]
fn plain_text_passes_through() {
    assert_eq!(sanitize_text("SDA", 64), "SDA");
    assert_eq!(sanitize_text("Takt 1 (CLK)", 64), "Takt 1 (CLK)");
}

#[test]
fn international_text_is_kept() {
    assert_eq!(sanitize_text("シグナル", 64), "シグナル");
    assert_eq!(sanitize_text("Сигнал №3", 64), "Сигнал №3");
    assert_eq!(sanitize_text("Größe", 64), "Größe");
    // A base letter with a combining accent stays together.
    assert_eq!(sanitize_text("e\u{301}", 64), "e\u{301}");
    assert_eq!(sanitize_text("🔌 bus", 64), "🔌 bus");
}

#[test]
fn control_characters_are_removed() {
    assert_eq!(sanitize_text("a\u{0}b\u{7}c\u{1b}d", 64), "abcd");
    assert_eq!(sanitize_text("a\nb\rc\td", 64), "abcd");
    assert_eq!(sanitize_text("a\u{7f}b\u{85}c\u{9f}d", 64), "abcd");
    assert_eq!(sanitize_text("a\u{2028}b\u{2029}c", 64), "abc");
}

#[test]
fn bidirectional_and_invisible_controls_are_removed() {
    // LRE RLE PDF LRO RLO and the isolates.
    assert_eq!(
        sanitize_text("a\u{202a}b\u{202b}c\u{202c}d\u{202d}e\u{202e}f", 64),
        "abcdef"
    );
    assert_eq!(
        sanitize_text("a\u{2066}b\u{2067}c\u{2068}d\u{2069}e", 64),
        "abcde"
    );
    assert_eq!(sanitize_text("a\u{200e}b\u{200f}c\u{61c}d", 64), "abcd");
    assert_eq!(
        sanitize_text("a\u{200b}b\u{200c}c\u{200d}d\u{2060}e\u{feff}f", 64),
        "abcdef"
    );
}

#[test]
fn right_to_left_letters_are_kept() {
    assert_eq!(sanitize_text("שלום", 64), "שלום");
    assert_eq!(sanitize_text("مرحبا", 64), "مرحبا");
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(sanitize_text("  SCL  ", 64), "SCL");
    assert_eq!(sanitize_text("   ", 64), "");
    assert_eq!(sanitize_text("\u{0}\u{1}\u{2}", 64), "");
    assert_eq!(sanitize_text("", 64), "");
}

#[test]
fn long_text_is_cut_at_a_character_boundary() {
    let long = "シ".repeat(10_000);
    let cut = sanitize_text(&long, MAX_NAME_CHARS);
    assert_eq!(cut.chars().count(), MAX_NAME_CHARS);
    assert_eq!(sanitize_text("abcdef", 3), "abc");
    // Trailing whitespace left by the cut is trimmed.
    assert_eq!(sanitize_text("ab   cd", 4), "ab");
    // A zero limit gives an empty string.
    assert_eq!(sanitize_text("abc", 0), "");
}

/// A tiny deterministic generator (the corpus must not depend on a random crate).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

#[test]
fn sanitizing_twice_changes_nothing_over_a_seeded_corpus() {
    let pool: Vec<char> = "aZ09 _-\u{0}\u{7}\n\t\u{7f}\u{85}\u{200b}\u{200e}\u{202e}\u{2066}\u{2069}\u{feff}éシ界Ж😀\u{301}\u{2028}"
        .chars()
        .collect();
    let mut rng = Lcg(0x5eed);
    for _ in 0..10_000 {
        let length = usize::try_from(rng.next() % 90).unwrap();
        let text: String = (0..length)
            .map(|_| pool[usize::try_from(rng.next()).unwrap() % pool.len()])
            .collect();
        let once = sanitize_text(&text, MAX_NAME_CHARS);
        assert_eq!(sanitize_text(&once, MAX_NAME_CHARS), once, "input {text:?}");
        assert!(once.chars().count() <= MAX_NAME_CHARS);
        assert!(!once.chars().any(char::is_control));
        assert_eq!(once.trim(), once);
    }
}
