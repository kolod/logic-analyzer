//! A board whose firmware the driver cannot use (feature 017, T050): it is listed, so that it can be restarted
//! into its bootloader and updated, and a capture on it is refused with `rp2350b.firmware_unsupported` naming
//! what the board reported; an older firmware (without capture request 2) is told apart from one the driver does
//! not know. The recordings are synthetic (`tests/fixtures/`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;

use la_devices::{DeviceAction, DeviceId, DeviceProvider};
use la_driver_rp2350b::error::FIRMWARE_UNSUPPORTED;
use la_driver_rp2350b::link::PortInfo;
use la_driver_rp2350b::testing::{ScriptedPorts, Step, parse_recording};
use la_driver_rp2350b::{PROVIDER_NAME, Rp2350bProvider, USB_PID, USB_VID};

fn fixture(name: &str) -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    parse_recording(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn provider(sessions: &[&str]) -> (Rp2350bProvider, Arc<ScriptedPorts>) {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        PortInfo {
            name: "COM7".to_owned(),
            vid: USB_VID,
            pid: USB_PID,
            serial: Some("S1".to_owned()),
        },
        sessions.iter().map(|n| fixture(n)).collect(),
    );
    (Rp2350bProvider::with_backend(ports.clone()), ports)
}

#[test]
fn a_firmware_the_driver_does_not_know_is_listed_and_refused_with_what_it_reported() {
    let (provider, _) = provider(&["info_future_firmware.rec"]);
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1, "the board is listed");
    list[0].validate().unwrap();
    assert_eq!(
        list[0].capabilities.actions,
        vec![DeviceAction::RestartIntoBootloader],
        "the only thing to do with it is to update it"
    );
    let error = provider.open(&list[0].id).err().unwrap();
    assert_eq!(error.code(), FIRMWARE_UNSUPPORTED);
    assert!(error.detail().contains("9.0.0"), "{}", error.detail());
    assert!(
        error.detail().contains("protocol level 3"),
        "{}",
        error.detail()
    );
}

#[test]
fn an_older_firmware_is_told_apart_from_one_the_driver_does_not_know() {
    let (provider, _) = provider(&["info_old_firmware.rec"]);
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    // Its own number of channels is kept (24, as the original), it is not made a 32-channel device.
    assert_eq!(list[0].capabilities.channels, 24);
    let error = provider.open(&list[0].id).err().unwrap();
    assert_eq!(error.code(), FIRMWARE_UNSUPPORTED);
    assert!(
        error
            .detail()
            .contains("older firmware without capture request 2"),
        "{}",
        error.detail()
    );
}

#[test]
fn an_unsupported_board_can_be_restarted_into_its_bootloader_and_is_asked_again_after() {
    let (provider, ports) = provider(&[
        "info_old_firmware.rec",
        "bootloader_ok.rec",
        "info_32ch.rec",
        "capture_32ch.rec",
    ]);
    let id = provider.scan().unwrap().remove(0).id;
    provider
        .perform(&id, DeviceAction::RestartIntoBootloader)
        .unwrap();
    assert_eq!(ports.writes("COM7")[1], [0x55, 0xAA, 0x04, 0xAA, 0x55]);
    // The board comes back with the new firmware: it is asked again and now it can be used.
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    assert!(
        list[0]
            .capabilities
            .actions
            .contains(&DeviceAction::Restart)
    );
    assert!(provider.open(&list[0].id).is_ok());
}

#[test]
fn the_normal_restart_is_not_offered_for_a_board_that_cannot_capture() {
    let (provider, _) = provider(&["info_future_firmware.rec"]);
    let actions = provider.scan().unwrap().remove(0).capabilities.actions;
    assert!(!actions.contains(&DeviceAction::Restart));
}

#[test]
fn something_that_does_not_answer_as_an_analyzer_is_not_listed() {
    let (provider, _) = provider(&["info_not_an_analyzer.rec"]);
    assert!(provider.scan().unwrap().is_empty());
}

#[test]
fn a_supported_board_is_not_affected() {
    let (provider, _) = provider(&["info_32ch.rec", "capture_32ch.rec"]);
    let list = provider.scan().unwrap();
    assert_eq!(list[0].id, DeviceId::new(PROVIDER_NAME, "S1"));
    assert!(provider.open(&list[0].id).is_ok());
}
