//! The coding of captured samples (feature 017, T030): the raw packing of 1 to 32 selected channels by hand,
//! the compact coding round trip for constant, sparse, random and worst-case signals with its fall back to raw,
//! and streams that are too long, too short or end inside a record.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_proto::{
    CompactError, bytes_per_sample, decode_compact, encode_compact, pack_raw, unpack_raw,
};

fn pack(words: &[u32], channels: &[u8]) -> Vec<u8> {
    let mut out = vec![0_u8; words.len() * bytes_per_sample(channels.len() as u8)];
    let n = pack_raw(words, channels, &mut out).unwrap();
    assert_eq!(n, out.len());
    out
}

#[test]
fn the_size_of_a_sample_follows_the_number_of_channels() {
    for (channels, bytes) in [
        (1, 1),
        (8, 1),
        (9, 2),
        (16, 2),
        (17, 3),
        (24, 3),
        (25, 4),
        (32, 4),
    ] {
        assert_eq!(bytes_per_sample(channels), bytes, "{channels}");
    }
}

#[test]
fn one_channel_is_one_bit_of_one_byte() {
    // Channel 5 of each word.
    assert_eq!(
        pack(&[0, 1 << 5, 0xFFFF_FFDF, 0xFFFF_FFFF], &[5]),
        [0, 1, 0, 1]
    );
}

#[test]
fn a_gap_in_the_numbers_packs_the_selected_channels_in_the_listed_order() {
    // Channels 30, 2 and 9, in that order: output bit 0 is channel 30, bit 1 channel 2, bit 2 channel 9.
    let words = [1 << 30, 1 << 2, 1 << 9, (1 << 30) | (1 << 9), 0];
    assert_eq!(
        pack(&words, &[30, 2, 9]),
        [0b001, 0b010, 0b100, 0b101, 0b000]
    );
}

#[test]
fn eight_channels_fill_one_byte_with_bit_zero_first() {
    let channels: Vec<u8> = (8..16).collect();
    assert_eq!(
        pack(&[0x0100, 0x8000, 0xFF00, 0x00FF], &channels),
        [0x01, 0x80, 0xFF, 0x00]
    );
}

#[test]
fn nine_channels_take_two_bytes_with_byte_zero_first() {
    let channels: Vec<u8> = (0..9).collect();
    assert_eq!(
        pack(&[0x1FF, 0x100, 0x001], &channels),
        [0xFF, 0x01, 0x00, 0x01, 0x01, 0x00]
    );
}

#[test]
fn all_32_channels_are_the_word_in_little_endian_order() {
    let channels: Vec<u8> = (0..32).collect();
    let words = [0x1234_5678, 0xFFFF_FFFF, 0x8000_0001];
    let raw = pack(&words, &channels);
    let expected: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    assert_eq!(raw, expected);
    // The eight channels above 23 are kept and not mixed with the others.
    let top = pack(&[0xAB00_0000], &channels);
    assert_eq!(top, [0, 0, 0, 0xAB]);
}

#[test]
fn unpacking_puts_the_channels_back_at_their_own_numbers() {
    let channels = [30_u8, 2, 9];
    let words = [
        1_u32 << 30,
        (1 << 2) | (1 << 9),
        0,
        (1 << 30) | (1 << 2) | (1 << 9),
    ];
    let raw = pack(&words, &channels);
    let mut back = vec![0_u32; words.len()];
    unpack_raw(&raw, &channels, &mut back).unwrap();
    assert_eq!(back, words);
    // For every count of channels from 1 to 32 and any bits, only the selected ones come back.
    let mut state = 12345_u64;
    for count in 1..=32_u8 {
        let channels: Vec<u8> = (0..count).map(|i| (i * 7 + 3) % 32).collect::<Vec<_>>();
        let mut unique = channels.clone();
        unique.sort_unstable();
        unique.dedup();
        let words: Vec<u32> = (0..50)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u32
            })
            .collect();
        let raw = pack(&words, &unique);
        let mut back = vec![0_u32; words.len()];
        unpack_raw(&raw, &unique, &mut back).unwrap();
        let mask: u32 = unique.iter().fold(0, |m, &c| m | (1 << c));
        let expected: Vec<u32> = words.iter().map(|w| w & mask).collect();
        assert_eq!(back, expected, "{count}");
    }
}

#[test]
fn packing_refuses_what_cannot_be_packed() {
    let mut out = [0_u8; 8];
    assert_eq!(
        pack_raw(&[1], &[32], &mut out),
        Err(CompactError::BadChannel)
    );
    let many: Vec<u8> = (0..33).map(|i| i % 32).collect();
    assert_eq!(pack_raw(&[1], &many, &mut out), Err(CompactError::BadSize));
    assert_eq!(
        pack_raw(&[1, 2, 3], &[0], &mut out[..2]),
        Err(CompactError::OutputTooSmall)
    );
    let mut words = [0_u32; 2];
    assert_eq!(
        unpack_raw(&[1, 2, 3], &[0, 1, 2, 3, 4, 5, 6, 7, 8], &mut words),
        Err(CompactError::BadSize)
    );
    assert_eq!(
        unpack_raw(&[1, 2], &[0], &mut words[..1]),
        Err(CompactError::BadSize)
    );
    assert_eq!(
        unpack_raw(&[1], &[], &mut words[..1]),
        Err(CompactError::BadSize)
    );
}

// ---- the compact coding ---------------------------------------------------------------------

fn round_trip(raw: &[u8], per: usize) -> usize {
    let mut coded = vec![0_u8; raw.len() * 3 + 16];
    let n = encode_compact(raw, per, &mut coded).unwrap();
    let mut back = vec![0_u8; raw.len()];
    decode_compact(&coded[..n], per, raw.len() / per, &mut back).unwrap();
    assert_eq!(back, raw);
    n
}

#[test]
fn a_constant_signal_is_the_first_sample_only() {
    let per = 2;
    let constant: Vec<u8> = std::iter::repeat_n([0xA5, 0x5A], 1000).flatten().collect();
    assert_eq!(round_trip(&constant, per), per);
    assert_eq!(round_trip(&[7], 1), 1);
    assert_eq!(round_trip(&[], 1), 0);
}

#[test]
fn a_change_is_a_distance_and_a_mask() {
    // Samples: 00 00 00 01 01 03 -> first 00; change at 3 by 01; change at 5 by 02.
    let raw = [0, 0, 0, 1, 1, 3];
    let mut coded = [0_u8; 32];
    let n = encode_compact(&raw, 1, &mut coded).unwrap();
    assert_eq!(&coded[..n], &[0x00, 3, 0x01, 2, 0x02]);
    // A distance of 300 is two bytes: 0xAC 0x02.
    let mut raw = vec![0_u8; 301];
    raw[300] = 4;
    let n = encode_compact(&raw, 1, &mut coded).unwrap();
    assert_eq!(&coded[..n], &[0x00, 0xAC, 0x02, 0x04]);
}

#[test]
fn sparse_signals_round_trip_and_get_smaller() {
    let mut raw = vec![0_u8; 4 * 10_000];
    for i in (0..10_000).step_by(997) {
        for b in 0..4 {
            raw[i * 4 + b] = (i % 251) as u8 + b as u8;
        }
    }
    // Everything after a changed sample stays at its value until the next change.
    let mut level = [0_u8; 4];
    for i in 0..10_000 {
        if i % 997 == 0 {
            level.copy_from_slice(&raw[i * 4..i * 4 + 4]);
        }
        raw[i * 4..i * 4 + 4].copy_from_slice(&level);
    }
    let n = round_trip(&raw, 4);
    assert!(n < raw.len() / 100, "{n} of {}", raw.len());
}

#[test]
fn random_and_worst_case_signals_round_trip_and_the_caller_falls_back_to_raw() {
    let mut state = 99_u64;
    for per in 1..=4 {
        for flips in [0_u64, 1, 4, 50, 1000] {
            let samples = 3_000;
            let mut raw = Vec::with_capacity(samples * per);
            let mut level = vec![0_u8; per];
            for _ in 0..samples {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                if flips > 0 && state % 1000 < flips {
                    for b in &mut level {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        *b ^= state as u8;
                    }
                }
                raw.extend_from_slice(&level);
            }
            round_trip(&raw, per);
        }
        // The worst case: every sample differs from the one before in every bit.
        let samples = 500;
        let raw: Vec<u8> = (0..samples)
            .flat_map(|i| std::iter::repeat_n(if i % 2 == 0 { 0x00 } else { 0xFF }, per))
            .collect();
        let n = round_trip(&raw, per);
        assert!(
            n > raw.len(),
            "compact is larger here, so the sender uses raw: {n} vs {}",
            raw.len()
        );
    }
}

#[test]
fn encoding_refuses_a_buffer_that_is_too_small_or_a_ragged_input() {
    let raw = [0_u8, 1, 0, 1, 0, 1];
    let mut small = [0_u8; 3];
    assert_eq!(
        encode_compact(&raw, 1, &mut small),
        Err(CompactError::OutputTooSmall)
    );
    let mut out = [0_u8; 32];
    assert_eq!(
        encode_compact(&[1, 2, 3], 2, &mut out),
        Err(CompactError::BadSize)
    );
    assert_eq!(
        encode_compact(&[1], 0, &mut out),
        Err(CompactError::BadSize)
    );
}

fn decode(stream: &[u8], per: usize, samples: usize) -> Result<Vec<u8>, CompactError> {
    let mut out = vec![0_u8; samples * per];
    decode_compact(stream, per, samples, &mut out).map(|()| out)
}

#[test]
fn a_stream_that_does_not_add_up_is_refused() {
    // Good: 6 samples of the example above.
    assert_eq!(
        decode(&[0x00, 3, 0x01, 2, 0x02], 1, 6).unwrap(),
        [0, 0, 0, 1, 1, 3]
    );
    // No first sample.
    assert_eq!(decode(&[], 1, 3), Err(CompactError::Truncated));
    // Ends inside a record: a distance without its mask, a varint that never ends.
    assert_eq!(decode(&[0, 3], 1, 6), Err(CompactError::Truncated));
    assert_eq!(decode(&[0, 0x80], 1, 6), Err(CompactError::Truncated));
    // Too long: a change at or beyond the last sample, a distance of zero, a mask that changes nothing.
    assert_eq!(decode(&[0, 6, 1], 1, 6), Err(CompactError::TooLong));
    assert_eq!(decode(&[0, 5, 1, 1, 1], 1, 6), Err(CompactError::TooLong));
    assert_eq!(decode(&[0, 0, 1], 1, 6), Err(CompactError::TooLong));
    assert_eq!(decode(&[0, 2, 0], 1, 6), Err(CompactError::TooLong));
    // Nothing is expected, but something came.
    assert_eq!(decode(&[1], 1, 0), Err(CompactError::TooLong));
    assert_eq!(decode(&[], 1, 0).unwrap(), Vec::<u8>::new());
    // A distance over 32 bits.
    assert_eq!(
        decode(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 1], 1, 6),
        Err(CompactError::BadNumber)
    );
    // A wrong output buffer.
    let mut out = [0_u8; 5];
    assert_eq!(
        decode_compact(&[0], 1, 6, &mut out),
        Err(CompactError::BadSize)
    );
    assert_eq!(
        decode_compact(&[0], 0, 0, &mut []),
        Err(CompactError::BadSize)
    );
}

#[test]
fn a_huge_declared_count_is_refused_by_the_buffer_the_caller_can_give() {
    // The decoder never allocates: it fills the buffer it is given, whatever the stream says. A stream that
    // claims a change far beyond the buffer is refused, not followed.
    let mut out = [0_u8; 8];
    assert_eq!(
        decode_compact(&[0, 0xFF, 0xFF, 0xFF, 0x0F, 1], 1, 8, &mut out),
        Err(CompactError::TooLong)
    );
}

#[test]
fn random_streams_never_panic() {
    let mut state = 7_u64;
    for _ in 0..5_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let len = (state % 40) as usize;
        let stream: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let per = (state % 4) as usize + 1;
        let samples = (state >> 8) as usize % 50;
        let mut out = vec![0_u8; samples * per];
        let _ = decode_compact(&stream, per, samples, &mut out);
    }
}
