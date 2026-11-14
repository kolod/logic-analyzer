//! Locale-aware formatting of numbers.
//!
//! Fluent's own `NUMBER` does not insert the separators of the language, so numbers shown to the
//! user go through [`format_decimal`]. Everything here is panic-free for any input: values that
//! cannot be shown (not finite, beyond ±10^15) give the marker [`MARKER`].

use crate::locale_data::LocaleData;

/// What is shown instead of a value that cannot be displayed.
pub(crate) const MARKER: &str = "\u{2013}";

/// The largest absolute value that is shown (10^15).
const LIMIT: f64 = 1.0e15;

/// Digits kept when the value is first written out, before rounding to the requested fraction.
const WORKING_FRACTION_DIGITS: usize = 12;

/// Formats `value` with at most `max_fraction_digits` fraction digits (rounded half up on the
/// decimal digits, trailing zeros removed), grouping the integer digits and using the separators of
/// `data`.
pub(crate) fn format_decimal(value: f64, max_fraction_digits: usize, data: &LocaleData) -> String {
    if !value.is_finite() || value.abs() >= LIMIT {
        return MARKER.to_owned();
    }
    let keep = max_fraction_digits.min(WORKING_FRACTION_DIGITS);
    let written = format!("{:.*}", WORKING_FRACTION_DIGITS, value.abs());
    let (int_part, frac_part) = written.split_once('.').unwrap_or((written.as_str(), ""));
    let mut digits: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes().take(keep))
        .collect();
    let int_len = int_part.len();
    let round_up = frac_part.as_bytes().get(keep).is_some_and(|d| *d >= b'5');
    let mut int_len = int_len;
    if round_up {
        let mut carry = true;
        for digit in digits.iter_mut().rev() {
            if !carry {
                break;
            }
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit = digit.saturating_add(1);
                carry = false;
            }
        }
        if carry {
            digits.insert(0, b'1');
            int_len = int_len.saturating_add(1);
        }
    }
    let (int_digits, frac_digits) = digits.split_at(int_len.min(digits.len()));
    let mut fraction: Vec<u8> = frac_digits.to_vec();
    while fraction.last() == Some(&b'0') {
        fraction.pop();
    }
    let is_zero = int_digits.iter().all(|d| *d == b'0') && fraction.is_empty();

    let mut out = String::with_capacity(digits.len().saturating_add(16));
    if value.is_sign_negative() && !is_zero {
        out.push('-');
    }
    push_grouped(&mut out, int_digits, data);
    if !fraction.is_empty() {
        out.push_str(data.decimal);
        out.extend(fraction.iter().map(|d| char::from(*d)));
    }
    out
}

/// Appends the integer digits, with the group separator every three digits when the number is long
/// enough for the language's minimum grouping.
fn push_grouped(out: &mut String, digits: &[u8], data: &LocaleData) {
    let count = digits.len();
    let grouped = count >= 3_usize.saturating_add(data.min_grouping);
    for (index, digit) in digits.iter().enumerate() {
        if grouped && index > 0 {
            let remaining = count.saturating_sub(index);
            if remaining % 3 == 0 {
                out.push_str(data.group);
            }
        }
        out.push(char::from(*digit));
    }
    if digits.is_empty() {
        out.push('0');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Language;

    fn data(language: Language) -> &'static LocaleData {
        LocaleData::for_language(language)
    }

    /// Expected outputs, written by hand, in the order en, de, fr, es, ja, uk.
    #[test]
    fn numbers_match_the_hand_written_table() {
        let fr = "\u{202F}";
        let uk = "\u{A0}";
        let table: Vec<(f64, [String; 6])> = vec![
            (0.0, ["0", "0", "0", "0", "0", "0"].map(String::from)),
            (1.0, ["1", "1", "1", "1", "1", "1"].map(String::from)),
            (
                999.0,
                ["999", "999", "999", "999", "999", "999"].map(String::from),
            ),
            (
                1_000.0,
                [
                    "1,000".to_owned(),
                    "1.000".to_owned(),
                    format!("1{fr}000"),
                    "1000".to_owned(),
                    "1,000".to_owned(),
                    format!("1{uk}000"),
                ],
            ),
            (
                1_234.0,
                [
                    "1,234".to_owned(),
                    "1.234".to_owned(),
                    format!("1{fr}234"),
                    "1234".to_owned(),
                    "1,234".to_owned(),
                    format!("1{uk}234"),
                ],
            ),
            (
                12_345.0,
                [
                    "12,345".to_owned(),
                    "12.345".to_owned(),
                    format!("12{fr}345"),
                    "12.345".to_owned(),
                    "12,345".to_owned(),
                    format!("12{uk}345"),
                ],
            ),
            (
                1_234_567.89,
                [
                    "1,234,567.89".to_owned(),
                    "1.234.567,89".to_owned(),
                    format!("1{fr}234{fr}567,89"),
                    "1.234.567,89".to_owned(),
                    "1,234,567.89".to_owned(),
                    format!("1{uk}234{uk}567,89"),
                ],
            ),
            (
                -1_234.5,
                [
                    "-1,234.5".to_owned(),
                    "-1.234,5".to_owned(),
                    format!("-1{fr}234,5"),
                    "-1234,5".to_owned(),
                    "-1,234.5".to_owned(),
                    format!("-1{uk}234,5"),
                ],
            ),
            (
                0.1,
                ["0.1", "0,1", "0,1", "0,1", "0.1", "0,1"].map(String::from),
            ),
            (
                0.0005,
                ["0.001", "0,001", "0,001", "0,001", "0.001", "0,001"].map(String::from),
            ),
            (0.0004, ["0", "0", "0", "0", "0", "0"].map(String::from)),
            (-0.0004, ["0", "0", "0", "0", "0", "0"].map(String::from)),
            (-0.0, ["0", "0", "0", "0", "0", "0"].map(String::from)),
            (1.9996, ["2", "2", "2", "2", "2", "2"].map(String::from)),
            (
                999.9996,
                [
                    "1,000",
                    "1.000",
                    "1\u{202F}000",
                    "1000",
                    "1,000",
                    "1\u{A0}000",
                ]
                .map(String::from),
            ),
            (
                100_000_000_000_000.0,
                [
                    "100,000,000,000,000".to_owned(),
                    "100.000.000.000.000".to_owned(),
                    format!("100{fr}000{fr}000{fr}000{fr}000"),
                    "100.000.000.000.000".to_owned(),
                    "100,000,000,000,000".to_owned(),
                    format!("100{uk}000{uk}000{uk}000{uk}000"),
                ],
            ),
        ];
        for (value, expected) in table {
            for (language, want) in Language::ALL.into_iter().zip(expected) {
                assert_eq!(
                    format_decimal(value, 3, data(language)),
                    want,
                    "{value} in {language:?}"
                );
            }
        }
    }

    #[test]
    fn values_that_cannot_be_shown_give_the_marker() {
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1.0e15,
            -1.0e15,
            1.0e300,
            f64::MAX,
            f64::MIN,
        ] {
            for language in Language::ALL {
                assert_eq!(format_decimal(value, 3, data(language)), MARKER, "{value}");
            }
        }
        // Just below the limit is still shown.
        assert_ne!(
            format_decimal(999_999_999_999_999.0, 3, data(Language::En)),
            MARKER
        );
    }

    #[test]
    fn the_number_of_fraction_digits_is_respected() {
        let en = data(Language::En);
        assert_eq!(format_decimal(1.23456, 0, en), "1");
        assert_eq!(format_decimal(1.5, 0, en), "2");
        assert_eq!(format_decimal(1.23456, 1, en), "1.2");
        assert_eq!(format_decimal(1.25, 1, en), "1.3");
        assert_eq!(format_decimal(30.4999, 1, en), "30.5");
        assert_eq!(format_decimal(1.0, 3, en), "1");
        assert_eq!(format_decimal(2.5, 100, en), "2.5");
    }

    #[test]
    fn no_bit_pattern_panics_and_the_output_is_bounded() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        for _ in 0..1_000_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let value = f64::from_bits(state);
            let out = format_decimal(value, 3, data(Language::Fr));
            assert!(!out.is_empty() && out.len() < 64, "{value:e} gave {out:?}");
        }
    }
}
