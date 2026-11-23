//! Restarting the board, normally and into its bootloader (feature 017, T054), so that the firmware can be
//! updated and a stuck board recovered without a programmer. The recordings are synthetic (`tests/fixtures/`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;

use la_devices::{DeviceAction, DeviceId, DeviceProvider};
use la_driver_rp2350b::error::{PROTOCOL, REQUEST_REFUSED, TIMEOUT};
use la_driver_rp2350b::link::PortInfo;
use la_driver_rp2350b::testing::{ScriptedPorts, Step, parse_recording};
use la_driver_rp2350b::{PROVIDER_NAME, Rp2350bProvider, USB_PID, USB_VID};

const SERIAL: &str = "S1";

fn fixture(name: &str) -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    parse_recording(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A board whose sessions are the information reply, then `then` in order.
fn board(then: &[&str]) -> (Rp2350bProvider, Arc<ScriptedPorts>, DeviceId) {
    let ports = Arc::new(ScriptedPorts::new());
    let mut scripts = vec![fixture("info_32ch.rec")];
    scripts.extend(then.iter().map(|n| fixture(n)));
    ports.add_port(
        PortInfo {
            name: "COM7".to_owned(),
            vid: USB_VID,
            pid: USB_PID,
            serial: Some(SERIAL.to_owned()),
        },
        scripts,
    );
    let provider = Rp2350bProvider::with_backend(ports.clone());
    let id = provider.scan().unwrap().remove(0).id;
    (provider, ports, id)
}

#[test]
fn the_board_offers_both_restarts_when_its_firmware_has_them() {
    let (provider, _, _) = board(&[]);
    let actions = provider.scan().unwrap().remove(0).capabilities.actions;
    assert_eq!(
        actions,
        vec![DeviceAction::Restart, DeviceAction::RestartIntoBootloader]
    );
}

#[test]
fn a_restart_sends_command_8_and_waits_for_the_answer() {
    let (provider, ports, id) = board(&["restart_ok.rec"]);
    provider.perform(&id, DeviceAction::Restart).unwrap();
    let writes = ports.writes("COM7");
    assert_eq!(writes[1], [0x55, 0xAA, 0x08, 0xAA, 0x55]);
}

#[test]
fn a_restart_into_the_bootloader_sends_command_4() {
    let (provider, ports, id) = board(&["bootloader_ok.rec"]);
    provider
        .perform(&id, DeviceAction::RestartIntoBootloader)
        .unwrap();
    assert_eq!(ports.writes("COM7")[1], [0x55, 0xAA, 0x04, 0xAA, 0x55]);
}

#[test]
fn after_a_restart_the_board_is_asked_again_when_it_comes_back() {
    let (provider, ports, id) = board(&["restart_ok.rec", "info_32ch.rec"]);
    provider.perform(&id, DeviceAction::Restart).unwrap();
    // The board is not listed from the old answer any more: the next listing asks it again, and finds it.
    assert_eq!(ports.opens("COM7"), 2);
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), format!("{PROVIDER_NAME}:{SERIAL}"));
    assert_eq!(ports.opens("COM7"), 3);
}

#[test]
fn a_board_that_refuses_or_does_not_answer_is_a_typed_error() {
    for (recording, code) in [
        ("err_busy.rec", REQUEST_REFUSED),
        ("no_answer.rec", TIMEOUT),
        ("capture_32ch.rec", PROTOCOL),
    ] {
        let (provider, _, id) = board(&[recording]);
        let error = provider.perform(&id, DeviceAction::Restart).unwrap_err();
        assert_eq!(error.code(), code, "{recording}");
        // A board that did not restart stays listed with its answer.
        assert_eq!(provider.scan().unwrap().len(), 1);
    }
}

#[test]
fn an_unknown_board_is_not_found() {
    let (provider, _, _) = board(&[]);
    let error = provider
        .perform(
            &DeviceId::new(PROVIDER_NAME, "other"),
            DeviceAction::Restart,
        )
        .unwrap_err();
    assert_eq!(error.code(), la_devices::error::NOT_FOUND);
}
