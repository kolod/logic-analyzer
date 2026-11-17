//! The wire protocol of the board against `specs/013-driver-weact/contracts/usb-protocol.md`
//! (spec 013, task T004). Every expected value is written by hand from the contract.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_driver_weact::protocol::{
    Clock, MANUFACTURER, PRODUCT, PRODUCT_ID, SAMPLE_RATES, VENDOR_ID, Version, buffer_plan,
    clock_and_delay, parse_version, samples_from_bytes, start_payload,
};

#[test]
fn the_identity_is_the_one_sigrok_lists_for_the_board() {
    assert_eq!((VENDOR_ID, PRODUCT_ID), (0x1d50, 0x608c));
    assert_eq!((MANUFACTURER, PRODUCT), ("sigrok", "fx2lafw"));
}

#[test]
fn the_sixteen_rates_are_the_ones_of_the_contract_in_increasing_order() {
    assert_eq!(
        SAMPLE_RATES,
        [
            20_000, 25_000, 50_000, 100_000, 200_000, 250_000, 500_000, 1_000_000, 2_000_000,
            3_000_000, 4_000_000, 6_000_000, 8_000_000, 12_000_000, 16_000_000, 24_000_000,
        ]
    );
    assert!(SAMPLE_RATES.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn the_start_payload_of_every_rate_is_the_table_of_the_contract() {
    // (rate, flags, delay_hi, delay_lo), from the table in the USB contract.
    let table: [(u64, [u8; 3]); 16] = [
        (20_000, [0x00, 0x05, 0xDB]),
        (25_000, [0x00, 0x04, 0xAF]),
        (50_000, [0x40, 0x03, 0xBF]),
        (100_000, [0x40, 0x01, 0xDF]),
        (200_000, [0x40, 0x00, 0xEF]),
        (250_000, [0x40, 0x00, 0xBF]),
        (500_000, [0x40, 0x00, 0x5F]),
        (1_000_000, [0x40, 0x00, 0x2F]),
        (2_000_000, [0x40, 0x00, 0x17]),
        (3_000_000, [0x40, 0x00, 0x0F]),
        (4_000_000, [0x40, 0x00, 0x0B]),
        (6_000_000, [0x40, 0x00, 0x07]),
        (8_000_000, [0x40, 0x00, 0x05]),
        (12_000_000, [0x40, 0x00, 0x03]),
        (16_000_000, [0x40, 0x00, 0x02]),
        (24_000_000, [0x40, 0x00, 0x01]),
    ];
    for (rate, payload) in table {
        assert_eq!(start_payload(rate).unwrap(), payload, "rate {rate}");
    }
}

#[test]
fn a_rate_that_is_not_in_the_list_is_an_error_and_never_a_panic() {
    for rate in [
        0,
        1,
        19_999,
        20_001,
        31_250,
        3_000_001,
        24_000_001,
        48_000_000,
        u64::MAX,
    ] {
        let error = start_payload(rate).unwrap_err();
        assert_eq!(
            error.code(),
            "device.unsupported_sample_rate",
            "rate {rate}"
        );
    }
    // Ten thousand other values from a fixed seed: only the listed rates succeed.
    let mut state = 0x013_u64;
    for _ in 0..10_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let rate = state >> 24;
        assert_eq!(
            start_payload(rate).is_ok(),
            SAMPLE_RATES.contains(&rate),
            "rate {rate}"
        );
    }
}

#[test]
fn the_clock_rule_prefers_48_mhz_and_falls_back_when_the_delay_is_too_long() {
    // The delay must not exceed 6 * 256 = 1536.
    assert_eq!(clock_and_delay(24_000_000), Some((Clock::Mhz48, 1)));
    assert_eq!(clock_and_delay(48_000_000), Some((Clock::Mhz48, 0)));
    // 48,000,000 / 31,250 = 1,536: the delay 1,535 fits.
    assert_eq!(clock_and_delay(31_250), Some((Clock::Mhz48, 1_535)));
    // 48,000,000 / 30,000 = 1,600: the delay 1,599 is too long, so the 30 MHz clock is used:
    // 30,000,000 / 30,000 = 1,000, delay 999.
    assert_eq!(clock_and_delay(30_000), Some((Clock::Mhz30, 999)));
    assert_eq!(clock_and_delay(20_000), Some((Clock::Mhz30, 1_499)));
    assert_eq!(clock_and_delay(25_000), Some((Clock::Mhz30, 1_199)));
    // A rate that divides neither clock has no delay; zero and the largest value are safe.
    assert_eq!(clock_and_delay(7), None);
    assert_eq!(clock_and_delay(3_000_001), None);
    assert_eq!(clock_and_delay(0), None);
    assert_eq!(clock_and_delay(u64::MAX), None);
}

#[test]
fn the_version_answer_is_exactly_two_bytes() {
    assert_eq!(
        parse_version(&[1, 4]).unwrap(),
        Version { major: 1, minor: 4 }
    );
    assert_eq!(
        parse_version(&[2, 0]).unwrap(),
        Version { major: 2, minor: 0 }
    );
    for bad in [&[][..], &[1][..], &[1, 4, 0][..], &[0_u8; 64][..]] {
        let error = parse_version(bad).unwrap_err();
        assert_eq!(error.code(), "device.transport", "length {}", bad.len());
    }
}

#[test]
fn the_buffers_follow_the_plan_of_the_contract() {
    // size = 10 ms of bytes rounded up to a multiple of 512 (at least 512);
    // count = 500 ms of bytes / size, at least 4 and at most 32. Worked by hand.
    let cases = [
        (20_000_u64, 512_usize, 19_usize),
        (25_000, 512, 24),
        (50_000, 512, 32),
        (1_000_000, 10_240, 32),
        (24_000_000, 240_128, 32),
    ];
    for (rate, size, count) in cases {
        let plan = buffer_plan(rate).unwrap();
        assert_eq!((plan.size, plan.count), (size, count), "rate {rate}");
    }
    for rate in SAMPLE_RATES {
        let plan = buffer_plan(rate).unwrap();
        assert_eq!(plan.size % 512, 0, "rate {rate}");
        assert!(plan.size >= 512, "rate {rate}");
        assert!((4..=32).contains(&plan.count), "rate {rate}");
    }
    assert!(buffer_plan(0).is_err());
    assert!(buffer_plan(48_000_000).is_err());
}

#[test]
fn bytes_become_words_with_bit_i_as_channel_i_and_disabled_channels_zero() {
    let mut out = Vec::new();
    samples_from_bytes(&[0x00, 0x01, 0x80, 0xFF, 0xA5], 0xFF, &mut out);
    assert_eq!(out, vec![0x00, 0x01, 0x80, 0xFF, 0xA5]);
    // Channels 0 and 7 only.
    out.clear();
    samples_from_bytes(&[0x00, 0x01, 0x80, 0xFF, 0xA5], 0x81, &mut out);
    assert_eq!(out, vec![0x00, 0x01, 0x80, 0x81, 0x81]);
    // A mask beyond the eight channels of the board cannot set a bit that does not exist.
    out.clear();
    samples_from_bytes(&[0xFF], u32::MAX, &mut out);
    assert_eq!(out, vec![0xFF]);
    // An empty input and an empty mask are fine.
    out.clear();
    samples_from_bytes(&[], 0xFF, &mut out);
    assert!(out.is_empty());
    samples_from_bytes(&[0xFF, 0x12], 0, &mut out);
    assert_eq!(out, vec![0, 0]);
}

#[test]
fn a_long_input_is_converted_without_trouble() {
    let bytes: Vec<u8> = (0..1_000_000_u32).map(|i| (i % 251) as u8).collect();
    let mut out = Vec::new();
    samples_from_bytes(&bytes, 0x0F, &mut out);
    assert_eq!(out.len(), bytes.len());
    assert_eq!(out[250], 250 & 0x0F);
    assert_eq!(out[251], 0);
}
