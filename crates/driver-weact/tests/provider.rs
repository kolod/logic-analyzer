//! Listing, identity and what the provider ignores (spec 013, user story 1, task T010).
//!
//! Expected identities and names are written by hand from the data model.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::Instant;

use la_devices::{DeviceId, DeviceProvider, SampleRates};
use la_driver_weact::WeActProvider;
use la_driver_weact::protocol::SAMPLE_RATES;
use la_driver_weact::testing::{BoardSpec, Fail, RecordedUsb, Request};

use common::one_ready_board;

#[test]
fn a_ready_board_is_listed_once_with_its_identity_name_and_capabilities() {
    let (_usb, _key, provider) = one_ready_board();
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    let board = &list[0];
    assert_eq!(board.id.as_str(), "weact:001-1");
    assert_eq!(board.name, "WeAct LogicAnalyzer V1");
    assert!(!board.simulated);
    assert_eq!(board.capabilities.channels, 8);
    assert_eq!(
        board.capabilities.sample_rates,
        SampleRates::List(SAMPLE_RATES.to_vec())
    );
    assert!(board.capabilities.triggers.is_empty(), "no trigger");
    assert_eq!(board.capabilities.buffer_depth, None, "it streams");
    board.validate().unwrap();
    // Asking again gives the same board with the same identity.
    let again = provider.scan().unwrap();
    assert_eq!(again, list);
}

#[test]
fn two_boards_have_two_identities_and_numbered_names_a_single_board_has_no_number() {
    let usb = RecordedUsb::new();
    usb.plug(BoardSpec::running("001", &[2]));
    usb.plug(BoardSpec::running("001", &[1]));
    let provider = WeActProvider::with_backend(Arc::new(usb.clone()), usb.timer());
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 2);
    let mut found: Vec<(String, String)> = list
        .iter()
        .map(|d| (d.id.as_str().to_owned(), d.name.clone()))
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            (
                "weact:001-1".to_owned(),
                "WeAct LogicAnalyzer V1 (1)".to_owned()
            ),
            (
                "weact:001-2".to_owned(),
                "WeAct LogicAnalyzer V1 (2)".to_owned()
            ),
        ],
        "numbered by position, whatever the order the bus lists them in"
    );
    // One goes away: the other loses its number.
    usb.unplug(&la_driver_weact::usb::BoardKey::new("001", [2]));
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "WeAct LogicAnalyzer V1");
    assert_eq!(list[0].id.as_str(), "weact:001-1");
}

#[test]
fn a_device_with_another_identity_is_never_listed_or_opened() {
    let usb = RecordedUsb::new();
    usb.plug(BoardSpec::running("001", &[1]));
    // The blank Cypress FX2 identity, another vendor's board, and an unrelated device.
    for (ports, vendor, product) in [
        (&[2_u8][..], 0x04b4, 0x8613),
        (&[3][..], 0x1d50, 0x608d),
        (&[4][..], 0x0403, 0x6001),
    ] {
        let mut spec = BoardSpec::running("001", ports);
        spec.vendor_id = vendor;
        spec.product_id = product;
        usb.plug(spec);
    }
    let provider = WeActProvider::with_backend(Arc::new(usb.clone()), usb.timer());
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), "weact:001-1");
    assert!(
        !usb.log().iter().any(|r| matches!(r, Request::Open(_))),
        "nothing was opened: {:?}",
        usb.log()
    );
}

#[test]
fn a_bus_that_cannot_be_read_does_not_fail_the_application() {
    let (usb, _key, provider) = one_ready_board();
    // Nothing known yet and the bus cannot be read: a typed error.
    usb.fail_list(Some(Fail::Transport));
    let error = provider.scan().unwrap_err();
    assert_eq!(error.code(), "device.transport");
    // Once the board is known, a failing read keeps what is known.
    usb.fail_list(None);
    assert_eq!(provider.scan().unwrap().len(), 1);
    usb.fail_list(Some(Fail::Transport));
    let kept = provider.scan().unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id.as_str(), "weact:001-1");
    usb.fail_list(None);
    assert_eq!(provider.scan().unwrap().len(), 1);
}

#[test]
fn scanning_is_quick() {
    let (_usb, _key, provider) = one_ready_board();
    let started = Instant::now();
    for _ in 0..20 {
        provider.scan().unwrap();
    }
    assert!(
        started.elapsed().as_millis() < 200 * 20,
        "a scan took too long: {:?}",
        started.elapsed()
    );
}

#[test]
fn opening_an_identity_that_is_not_a_board_of_this_driver_is_not_found() {
    let (_usb, _key, provider) = one_ready_board();
    provider.scan().unwrap();
    for id in [
        DeviceId::new("weact", "001-9"),
        DeviceId::new("weact", ""),
        DeviceId::new("virtual", "0"),
        DeviceId::new("weact", "x".repeat(10_000).as_str()),
    ] {
        let error = provider.open(&id).err().unwrap();
        assert_eq!(error.code(), "device.not_found", "{id:?}");
    }
}

#[test]
fn the_provider_is_called_weact() {
    let (_usb, _key, provider) = one_ready_board();
    assert_eq!(provider.name(), "weact");
}
