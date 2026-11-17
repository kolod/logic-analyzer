//! Pure checkers of the rig's patterns: functions over a captured byte sequence (one byte per
//! sample, bit `i` is channel `i`), the rate and the step. They need no hardware.
//!
//! The Nano's clock is independent of the analyzer's, so a run of equal samples is accepted when
//! its length is within -20 % to +25 % of `step * rate`. The first and the last run of a capture
//! are cut by the start and the end of the capture and are not measured.
#![allow(dead_code)]

/// The runs of equal bytes: `(value, length)`.
pub fn runs(bytes: &[u8]) -> Vec<(u8, usize)> {
    let mut out: Vec<(u8, usize)> = Vec::new();
    for byte in bytes {
        match out.last_mut() {
            Some((value, length)) if *value == *byte => *length += 1,
            _ => out.push((*byte, 1)),
        }
    }
    out
}

/// The run length a step should give, in samples.
fn expected_run(rate: u64, step_us: u64) -> f64 {
    rate as f64 * step_us as f64 / 1_000_000.0
}

fn check_lengths(runs: &[(u8, usize)], rate: u64, step_us: u64) -> Result<(), String> {
    let expected = expected_run(rate, step_us);
    let (low, high) = (expected * 0.8, expected * 1.25);
    // The first and the last run are cut by the capture.
    let interior = runs.get(1..runs.len().saturating_sub(1)).unwrap_or(&[]);
    for (index, (value, length)) in interior.iter().enumerate() {
        let length = *length as f64;
        if length < low || length > high {
            return Err(format!(
                "run {} (value {value:#04x}) is {length} samples long; expected {expected:.1} \
                 (accepted {low:.1} to {high:.1})",
                index + 1
            ));
        }
    }
    Ok(())
}

fn bits(value: u8) -> String {
    format!("{value:#010b}")
}

/// `HOLD`: the captured value is constant and equal to `expected`.
pub fn check_hold(bytes: &[u8], expected: u8) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("the capture is empty".to_owned());
    }
    match bytes.iter().position(|b| *b != expected) {
        None => Ok(()),
        Some(at) => Err(format!(
            "sample {at} is {} but {} is held (bits that differ: {})",
            bits(bytes[at]),
            bits(expected),
            bits(bytes[at] ^ expected)
        )),
    }
}

/// `WALK`: every value has exactly one bit set, the values follow the cycle 01, 02, 04 ... 80, 01,
/// each of the eight channels is seen, and the runs have the length of the step.
pub fn check_walk(bytes: &[u8], rate: u64, step_us: u64) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("the capture is empty".to_owned());
    }
    let runs = runs(bytes);
    let mut seen = 0_u8;
    for (index, (value, _)) in runs.iter().enumerate() {
        if value.count_ones() != 1 {
            return Err(format!(
                "run {index} has the value {}: not one bit set (a stuck or crossed channel)",
                bits(*value)
            ));
        }
        seen |= value;
    }
    for pair in runs.windows(2) {
        let expected = pair[0].0.rotate_left(1);
        if pair[1].0 != expected {
            return Err(format!(
                "the walk went from {} to {} instead of {} (a channel is missing or crossed)",
                bits(pair[0].0),
                bits(pair[1].0),
                bits(expected)
            ));
        }
    }
    if seen != 0xFF {
        let missing: Vec<u8> = (0..8).filter(|i| seen >> i & 1 == 0).collect();
        return Err(format!("channel {missing:?} never seen"));
    }
    check_lengths(&runs, rate, step_us)
}

/// `COUNTER`: consecutive different values differ by +1 modulo 256 and the runs have the length
/// of the step.
pub fn check_counter(bytes: &[u8], rate: u64, step_us: u64) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("the capture is empty".to_owned());
    }
    let runs = runs(bytes);
    for (index, pair) in runs.windows(2).enumerate() {
        let expected = pair[0].0.wrapping_add(1);
        if pair[1].0 != expected {
            return Err(format!(
                "the counter went from {} to {} at run {} instead of {} (a value was skipped, \
                 or a bit is stuck: bits that differ from the expected value are {})",
                pair[0].0,
                pair[1].0,
                index + 1,
                expected,
                bits(pair[1].0 ^ expected)
            ));
        }
    }
    check_lengths(&runs, rate, step_us)
}

/// `SQUARE`: every channel toggles together: the values are 0x00 and 0xFF in turn, and the runs
/// have the length of the step.
pub fn check_square(bytes: &[u8], rate: u64, step_us: u64) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("the capture is empty".to_owned());
    }
    let runs = runs(bytes);
    for (index, (value, _)) in runs.iter().enumerate() {
        if *value != 0x00 && *value != 0xFF {
            return Err(format!(
                "run {index} has the value {}: the channels do not toggle together (bits that \
                 are not at the level of the others: {})",
                bits(*value),
                bits(if value.count_ones() > 4 {
                    !*value
                } else {
                    *value
                })
            ));
        }
    }
    if runs.len() < 2 {
        return Err("the capture never toggled".to_owned());
    }
    check_lengths(&runs, rate, step_us)
}

/// Parses the answer line of the Nano: `Ok(())` for `OK`, the protocol version for `LA-RIG <n>`.
pub fn parse_answer(line: &str) -> Result<Answer, String> {
    let line = line.trim();
    if line == "OK" {
        return Ok(Answer::Ok);
    }
    if line == "ERR" {
        return Ok(Answer::Err);
    }
    if let Some(version) = line.strip_prefix("LA-RIG ") {
        return version
            .parse::<u32>()
            .map(Answer::Greeting)
            .map_err(|_| format!("`{line}` has no version number"));
    }
    Err(format!("`{line}` is not an answer of the rig"))
}

/// What the Nano says.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Ok,
    Err,
    Greeting(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u64 = 1_000_000;
    const STEP: u64 = 100;

    /// `count` steps of `STEP` µs at `RATE`, one value per step.
    fn steps(values: impl IntoIterator<Item = u8>) -> Vec<u8> {
        values
            .into_iter()
            .flat_map(|v| std::iter::repeat_n(v, 100))
            .collect()
    }

    fn walk(count: usize) -> Vec<u8> {
        steps((0..count).map(|i| 1_u8 << (i % 8)))
    }

    #[test]
    fn correct_captures_pass() {
        check_walk(&walk(30), RATE, STEP).unwrap();
        check_counter(&steps((0..600).map(|i| (i % 256) as u8)), RATE, STEP).unwrap();
        check_hold(&[0x55; 100], 0x55).unwrap();
        check_square(
            &steps((0..20).map(|i| if i % 2 == 0 { 0 } else { 0xFF })),
            RATE,
            STEP,
        )
        .unwrap();
        // 20 % short and 25 % long are inside the tolerance.
        let cycle: Vec<u8> = (0..16)
            .zip([100_usize, 80, 125].iter().cycle())
            .flat_map(|(i, n)| std::iter::repeat_n(1_u8 << (i % 8), *n))
            .collect();
        check_walk(&cycle, RATE, STEP).unwrap();
    }

    #[test]
    fn a_missing_channel_is_named() {
        // Channel 5 never goes high: the walk jumps from 0x10 to 0x40.
        let values = [0x01, 0x02, 0x04, 0x08, 0x10, 0x40, 0x80, 0x01, 0x02];
        let message = check_walk(&steps(values), RATE, STEP).unwrap_err();
        assert!(message.contains("0b00010000"), "{message}");
        assert!(message.contains("0b00100000"), "{message}");
        let message = check_walk(&steps([1, 2, 4, 8, 16, 32, 64]), RATE, STEP).unwrap_err();
        assert!(message.contains("[7]"), "{message}");
    }

    #[test]
    fn a_skipped_counter_value_is_named() {
        let values = (0..50_u32).map(|i| if i < 20 { i as u8 } else { (i + 1) as u8 });
        let message = check_counter(&steps(values), RATE, STEP).unwrap_err();
        assert!(message.contains("went from 19 to 21"), "{message}");
    }

    #[test]
    fn a_stuck_bit_is_named() {
        // Bit 2 is stuck high in a walk.
        let values: Vec<u8> = (0..16).map(|i| (1_u8 << (i % 8)) | 0x04).collect();
        let message = check_walk(&steps(values), RATE, STEP).unwrap_err();
        assert!(message.contains("not one bit set"), "{message}");
        // Bit 3 is stuck low in a hold of 0xFF.
        let message = check_hold(&[0xFF, 0xFF, 0xF7], 0xFF).unwrap_err();
        assert!(message.contains("0b00001000"), "{message}");
        // One channel does not toggle with the others in a square wave.
        let bytes = steps([0x00, 0xFB, 0x00, 0xFB]);
        let message = check_square(&bytes, RATE, STEP).unwrap_err();
        assert!(message.contains("0b00000100"), "{message}");
    }

    #[test]
    fn runs_twice_as_long_or_too_short_are_rejected() {
        let long: Vec<u8> = (0..16)
            .flat_map(|i| std::iter::repeat_n(1_u8 << (i % 8), 200))
            .collect();
        let message = check_walk(&long, RATE, STEP).unwrap_err();
        assert!(message.contains("200"), "{message}");
        let short: Vec<u8> = (0..16)
            .flat_map(|i| std::iter::repeat_n(1_u8 << (i % 8), 70))
            .collect();
        assert!(check_walk(&short, RATE, STEP).is_err());
    }

    #[test]
    fn an_empty_capture_fails_everywhere() {
        for result in [
            check_walk(&[], RATE, STEP),
            check_counter(&[], RATE, STEP),
            check_hold(&[], 0),
            check_square(&[], RATE, STEP),
        ] {
            assert_eq!(result.unwrap_err(), "the capture is empty");
        }
    }

    #[test]
    fn the_answers_of_the_nano_are_parsed() {
        assert_eq!(parse_answer("OK\r\n").unwrap(), Answer::Ok);
        assert_eq!(parse_answer("ERR").unwrap(), Answer::Err);
        assert_eq!(parse_answer("LA-RIG 1").unwrap(), Answer::Greeting(1));
        assert!(parse_answer("LA-RIG x").is_err());
        assert!(parse_answer("").is_err());
        assert!(parse_answer("garbage \u{0}").is_err());
    }
}
