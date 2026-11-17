//! The documentation of the driver says what a person needs (spec 013, user story 5, task T030).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::path::PathBuf;

use la_driver_weact::error;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist: {e}"))
}

#[test]
fn the_driver_page_names_every_code_and_the_facts_a_user_needs() {
    let page = read("docs/drivers/weact.md");
    for code in [
        error::ACCESS_DENIED,
        error::DRIVER_MISSING,
        error::IN_USE,
        error::FIRMWARE_REJECTED,
        error::NOT_RENUMERATED,
        error::PREPARING,
        error::NO_DATA,
        error::STREAM_STOPPED,
        error::FIRMWARE_INVALID,
        "device.disconnected",
        "device.transport",
        "device.not_found",
    ] {
        assert!(page.contains(code), "weact.md must explain `{code}`");
    }
    for word in [
        "WinUSB",
        "Zadig",
        "udev",
        "24 MHz",
        "5.5 V",
        "no trigger",
        "GPL",
        "sigrok",
    ] {
        assert!(page.contains(word), "weact.md must mention {word}");
    }
}

#[test]
fn the_udev_rule_matches_the_board_and_gives_the_logged_in_user_access() {
    let rule = read("packaging/linux/60-logic-analyzer-weact.rules");
    for word in ["1d50", "608c", "uaccess"] {
        assert!(rule.contains(word), "the rule must contain {word}");
    }
}

#[test]
fn the_windows_notes_say_the_board_needs_winusb() {
    let notes = read("packaging/windows/README.md");
    assert!(notes.contains("WeAct"));
    assert!(notes.contains("WinUSB"));
}

#[test]
fn the_verification_procedure_has_its_record_template() {
    let page = read("docs/drivers/weact-verification.md");
    assert!(page.contains("Record template"));
    assert!(page.contains("Date:"));
    assert!(page.contains("Operating system"));
}

#[test]
fn the_checksum_in_the_source_notes_is_a_sha256() {
    let notes = read("crates/driver-weact/firmware/SOURCE.md");
    let line = notes
        .lines()
        .find(|l| l.contains("SHA-256 of the file"))
        .unwrap();
    let digest = line.split('`').nth(1).unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn the_user_guide_points_to_the_driver_page() {
    let guide = read("docs/user-guide.md");
    assert!(guide.contains("WeAct"));
    assert!(guide.contains("docs/drivers/weact.md"));
}
