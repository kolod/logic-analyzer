//! Data from the board that is wrong never panics and is typed (spec 013, user story 3, task
//! T020). The expected words are worked out here, not by the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_devices::{AcquisitionOptions, DeviceProvider, EndReason};
use la_driver_weact::protocol::{parse_version, samples_from_bytes};
use la_driver_weact::testing::{Capture, StreamStep};
use la_driver_weact::usb::{BoardKey, UsbInfo};

use common::{config, one_ready_board, outcome, registry_with, wait_until};

#[test]
fn a_version_answer_of_the_wrong_length_is_a_typed_error_from_open() {
    for answer in [vec![], vec![1], vec![1, 4, 0], vec![0; 4_096]] {
        let (usb, key, provider) = one_ready_board();
        usb.set_version(&key, &answer);
        provider.scan().unwrap();
        let error = provider.open(&key.device_id()).err().unwrap();
        assert_eq!(error.code(), "device.transport", "{} bytes", answer.len());
        assert!(parse_version(&answer).is_err());
    }
}

#[test]
fn strange_descriptors_never_make_a_board_of_this_driver_or_panic() {
    let long = "x".repeat(100_000);
    let strings = [
        None,
        Some(String::new()),
        Some(long),
        Some("\u{fffd}\u{0}\u{202e}fx2lafw".to_owned()),
    ];
    for manufacturer in &strings {
        for product in &strings {
            let info = UsbInfo {
                key: BoardKey::new("é\u{0}", vec![255_u8; 255]),
                vendor_id: u16::MAX,
                product_id: u16::MAX,
                manufacturer: manufacturer.clone(),
                product: product.clone(),
                address: 255,
                driver: strings[2].clone(),
            };
            assert!(!info.is_weact());
            assert!(!info.runs_firmware() || product.as_deref() == Some("fx2lafw"));
            let _ = info.key.device_id();
        }
    }
}

/// A deterministic generator written here (the same LCG as the other tests).
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 33
}

#[test]
fn random_bytes_become_exactly_the_masked_words() {
    let mut state = 0x5eed_u64;
    for case in 0..10_000 {
        let length = (lcg(&mut state) % 700) as usize;
        let mask = (lcg(&mut state) & 0xFF) as u32;
        let bytes: Vec<u8> = (0..length).map(|_| lcg(&mut state) as u8).collect();
        let mut out = vec![0xDEAD_BEEF_u32]; // what was there before stays
        samples_from_bytes(&bytes, mask, &mut out);
        assert_eq!(out.len(), 1 + length, "case {case}");
        assert_eq!(out[0], 0xDEAD_BEEF);
        for (i, byte) in bytes.iter().enumerate() {
            assert_eq!(out[1 + i], u32::from(*byte) & mask, "case {case}, byte {i}");
        }
    }
}

#[test]
fn bulk_completions_of_every_odd_size_are_stored_byte_for_byte() {
    let (usb, key, provider) = one_ready_board();
    // Empty, one byte, odd lengths, and one much longer than any buffer of the plan.
    let sizes = [0_usize, 1, 3, 511, 513, 4_097, 300_001];
    let mut state = 77_u64;
    let mut all = Vec::new();
    let mut steps = Vec::new();
    for size in sizes {
        let part: Vec<u8> = (0..size).map(|_| lcg(&mut state) as u8).collect();
        all.extend_from_slice(&part);
        steps.push(if part.is_empty() {
            StreamStep::Empty
        } else {
            StreamStep::Data(part)
        });
    }
    usb.script_capture(&key, Capture::stream(steps));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(std::time::Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    let total = all.len() as u64;
    let acquisition = registry
        .start(
            &id,
            config(0xFF, 1_000_000, Some(total)),
            AcquisitionOptions {
                channel_capacity: 100_000,
                ..AcquisitionOptions::default()
            },
        )
        .unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    common::assert_capture_equals(&acquisition, &all, 0xFF);
}
