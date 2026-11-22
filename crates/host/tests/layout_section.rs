//! The `LAYT` session section against `specs/014-gui-improvements/contracts/layt-section.md`
//! (spec 014, task T006). Payloads are written by hand from the contract, not by the writer under
//! test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::{Path, PathBuf};

use common::{Lcg, TempDir, finished_shell, wait_operation};
use la_host::error::LAYOUT_INVALID;
use la_host::layout_section::{Decoded, decode, encode};
use la_host::notices::NoticeContent;
use la_host::presentation::Presentation;
use la_host::{Command, Outcome, Shell, ShellConfig};
use la_session::{Control, LoadLimits, Section, SectionTag, SessionData};

fn presentation_with(heights: &[(u8, u16)]) -> Presentation {
    let mut presentation = Presentation::defaults(32, u32::MAX);
    for (channel, height) in heights {
        assert!(presentation.set_height(*channel, *height));
    }
    presentation
}

fn heights(channels: Vec<(u8, u16)>, decoders: Vec<(u32, u16)>) -> Decoded {
    Decoded::Heights { channels, decoders }
}

fn heights_of(presentation: &Presentation) -> Vec<(u8, u16)> {
    let mut list: Vec<(u8, u16)> = presentation
        .channels
        .iter()
        .filter(|s| s.height != 0)
        .map(|s| (s.index, s.height))
        .collect();
    list.sort_unstable();
    list
}

// ---- the format ----------------------------------------------------------------------------

#[test]
fn the_payload_is_exactly_the_documented_one() {
    // Channel 1 with 40 and channel 31 with 240: version, count, (channel, height) pairs; 240 is the
    // varint f0 01.
    let payload = encode(&presentation_with(&[(31, 240), (1, 40)])).unwrap();
    assert_eq!(payload, vec![0x01, 0x02, 0x01, 0x28, 0x1f, 0xf0, 0x01]);
    // The same heights always give the same bytes.
    assert_eq!(
        encode(&presentation_with(&[(1, 40), (31, 240)])).unwrap(),
        payload
    );
}

#[test]
fn without_a_custom_height_there_is_no_section() {
    assert_eq!(encode(&presentation_with(&[])), None);
    let mut presentation = presentation_with(&[(4, 50)]);
    presentation.set_height(4, 0);
    assert_eq!(encode(&presentation), None);
}

#[test]
fn a_good_payload_is_read() {
    let decoded = decode(&[0x01, 0x02, 0x01, 0x28, 0x1f, 0xf0, 0x01], 32).unwrap();
    assert_eq!(decoded, heights(vec![(1, 40), (31, 240)], vec![]));
    assert_eq!(
        decode(&[0x01, 0x00], 8).unwrap(),
        heights(Vec::new(), Vec::new())
    );
}

#[test]
fn every_rejection_of_the_contract_is_made() {
    let bad: Vec<(&str, Vec<u8>, u8)> = vec![
        ("empty", vec![], 8),
        ("no count", vec![0x01], 8),
        ("count 33", vec![0x01, 33], 32),
        (
            "a channel not below the channel count",
            vec![0x01, 0x01, 0x08, 0x28],
            8,
        ),
        (
            "a repeated channel",
            vec![0x01, 0x02, 0x01, 0x28, 0x01, 0x30],
            8,
        ),
        (
            "a descending channel",
            vec![0x01, 0x02, 0x03, 0x28, 0x01, 0x30],
            8,
        ),
        ("a height of 23", vec![0x01, 0x01, 0x01, 23], 8),
        ("a height of 241", vec![0x01, 0x01, 0x01, 0xf1, 0x01], 8),
        ("a height of 0", vec![0x01, 0x01, 0x01, 0x00], 8),
        ("a trailing byte", vec![0x01, 0x01, 0x01, 0x28, 0x00], 8),
        ("a missing height", vec![0x01, 0x01, 0x01], 8),
        (
            "a non-canonical varint",
            vec![0x01, 0x01, 0x81, 0x00, 0x28],
            8,
        ),
        (
            "a varint that never ends",
            vec![0x01, 0x01, 0x01, 0xff, 0xff, 0xff],
            8,
        ),
    ];
    for (what, payload, channels) in bad {
        let error = decode(&payload, channels).unwrap_err();
        assert_eq!(error.code(), LAYOUT_INVALID, "{what}");
    }
    // A newer version is not an error: it is kept and not interpreted.
    assert_eq!(decode(&[0x03, 0xff, 0xff], 8).unwrap(), Decoded::Newer);
    assert_eq!(decode(&[0x63], 8).unwrap(), Decoded::Newer);
}

#[test]
fn random_bytes_never_panic_and_what_is_accepted_is_canonical() {
    let mut random = Lcg(14);
    let mut accepted = 0;
    for round in 0..20_000_u32 {
        let mut payload: Vec<u8> = if round % 3 == 0 {
            // Pure noise.
            let length = random.below(12) as usize;
            (0..length).map(|_| random.below(256) as u8).collect()
        } else {
            // A well-formed payload that may then be damaged by one change.
            let count = random.below(5) as usize;
            let mut bytes = vec![1, count as u8];
            let mut channel = random.below(6);
            for _ in 0..count {
                bytes.push(channel as u8);
                let height = 24 + random.below(217);
                if height >= 128 {
                    bytes.push((height & 0x7f) as u8 | 0x80);
                    bytes.push((height >> 7) as u8);
                } else {
                    bytes.push(height as u8);
                }
                channel += 1 + random.below(5);
            }
            if round % 3 == 2 && !bytes.is_empty() {
                let at = random.below(bytes.len() as u64) as usize;
                bytes[at] = random.below(256) as u8;
            }
            bytes
        };
        if round % 7 == 0 {
            payload.push(random.below(256) as u8);
        }
        if let Ok(Decoded::Heights {
            channels: heights, ..
        }) = decode(&payload, 32)
        {
            accepted += 1;
            let mut presentation = Presentation::defaults(32, u32::MAX);
            for (channel, height) in &heights {
                assert!(presentation.set_height(*channel, *height));
            }
            let again = encode(&presentation);
            if heights.is_empty() {
                assert_eq!(again, None);
                assert_eq!(payload, vec![1, 0]);
            } else {
                assert_eq!(again.as_deref(), Some(payload.as_slice()));
            }
        }
    }
    assert!(accepted > 3_000, "{accepted}");
}

// ---- version 2: the lanes of decoder instances --------------------------------------------

#[test]
fn decoder_lane_heights_make_a_version_two_payload_and_none_keep_version_one() {
    // Channel 1 with 40 and the decoder instances 2 and 300 with 60 and 240: version 2, the
    // channels as before, then the count and the (instance, height) pairs; 300 is the varint ac 02.
    let mut presentation = presentation_with(&[(1, 40)]);
    presentation.set_decoder_height(300, 240);
    presentation.set_decoder_height(2, 60);
    let payload = encode(&presentation).unwrap();
    assert_eq!(
        payload,
        vec![
            0x02, 0x01, 0x01, 0x28, 0x02, 0x02, 0x3c, 0xac, 0x02, 0xf0, 0x01
        ]
    );
    assert_eq!(
        decode(&payload, 8).unwrap(),
        heights(vec![(1, 40)], vec![(2, 60), (300, 240)])
    );
    // Only decoder lanes: the count of channels is zero.
    let mut only = Presentation::defaults(8, 0xff);
    only.set_decoder_height(7, 24);
    let payload = encode(&only).unwrap();
    assert_eq!(payload, vec![0x02, 0x00, 0x01, 0x07, 0x18]);
    // Without a decoder height the payload is version 1, as before.
    assert_eq!(encode(&presentation_with(&[(1, 40)])).unwrap()[0], 0x01);
    // A height of 0 is automatic: no entry.
    let mut automatic = Presentation::defaults(8, 0xff);
    automatic.set_decoder_height(7, 60);
    automatic.set_decoder_height(7, 0);
    assert_eq!(encode(&automatic), None);
    // The limits of a height are those of a channel.
    let mut limits = Presentation::defaults(8, 0xff);
    limits.set_decoder_height(1, 5);
    limits.set_decoder_height(2, 900);
    assert_eq!(limits.decoder_heights[&1], 24);
    assert_eq!(limits.decoder_heights[&2], 240);
}

#[test]
fn every_rejection_of_version_two_is_made() {
    let bad: Vec<(&str, Vec<u8>)> = vec![
        ("no decoder count", vec![0x02, 0x00]),
        ("a decoder count of 0", vec![0x02, 0x00, 0x00]),
        ("a decoder count of 65", vec![0x02, 0x00, 65]),
        (
            "a repeated instance",
            vec![0x02, 0x00, 0x02, 0x03, 0x28, 0x03, 0x30],
        ),
        (
            "a descending instance",
            vec![0x02, 0x00, 0x02, 0x04, 0x28, 0x03, 0x30],
        ),
        ("a height of 23", vec![0x02, 0x00, 0x01, 0x03, 23]),
        ("a height of 241", vec![0x02, 0x00, 0x01, 0x03, 0xf1, 0x01]),
        ("a missing height", vec![0x02, 0x00, 0x01, 0x03]),
        ("a trailing byte", vec![0x02, 0x00, 0x01, 0x03, 0x28, 0x00]),
        (
            "an instance too large",
            vec![0x02, 0x00, 0x01, 0xff, 0xff, 0xff, 0xff, 0x7f, 0x28],
        ),
    ];
    for (what, payload) in bad {
        let error = decode(&payload, 8).unwrap_err();
        assert_eq!(error.code(), LAYOUT_INVALID, "{what}");
    }
}

// ---- in a session --------------------------------------------------------------------------

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .map(|n| match &n.content {
            NoticeContent::Error { code } => code.clone(),
            NoticeContent::Info { key, .. } => (*key).to_owned(),
        })
        .collect()
}

fn open(dir: &TempDir, path: &Path) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let outcome = shell.apply(Command::OpenSession(path.to_path_buf()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut shell);
    shell
}

fn save(shell: &mut Shell, path: &Path) {
    let outcome = shell.apply(Command::SaveSessionAs(path.to_path_buf()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(shell);
}

fn extras(path: &Path) -> Vec<(String, Vec<u8>)> {
    let session = la_session::load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap();
    session
        .extras()
        .iter()
        .map(|s| (s.tag.as_str().to_owned(), s.payload.clone()))
        .collect()
}

/// A file like `source` with `extra` added as an optional section.
fn with_section(source: &Path, target: &Path, tag: &str, payload: Vec<u8>) {
    let session =
        la_session::load_blocking(source, &LoadLimits::default(), &Control::new()).unwrap();
    let mut data = SessionData::new(session.description().clone(), session.snapshot());
    for section in session.extras() {
        if section.tag.as_str() != tag {
            data = data.with_section(section.clone()).unwrap();
        }
    }
    data = data
        .with_section(Section {
            tag: SectionTag::new(tag).unwrap(),
            critical: false,
            payload,
        })
        .unwrap();
    la_session::save_blocking(target, &data, &Control::new()).unwrap();
}

fn set(shell: &mut Shell, channel: u8, height: u16) {
    let outcome = shell.apply(Command::SetChannelHeight { channel, height });
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

#[test]
fn heights_are_saved_with_the_session_and_come_back() {
    let dir = TempDir::new("layt-roundtrip");
    let mut shell = finished_shell(&dir, 0b1111, 5_000, 3);
    set(&mut shell, 1, 40);
    set(&mut shell, 3, 200);
    let path = dir.path().join("heights.lasession");
    save(&mut shell, &path);
    let sections = extras(&path);
    let layt = sections
        .iter()
        .find(|(tag, _)| tag == "LAYT")
        .expect("LAYT");
    assert_eq!(layt.1, vec![0x01, 0x02, 0x01, 0x28, 0x03, 0xc8, 0x01]);

    let reopened = open(&dir, &path);
    assert_eq!(
        heights_of(&reopened.state().presentation),
        vec![(1, 40), (3, 200)]
    );
    assert!(codes(&reopened).iter().all(|c| !c.contains("layout")));
}

#[test]
fn a_session_with_no_custom_height_has_no_section() {
    let dir = TempDir::new("layt-none");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 1);
    let path = dir.path().join("plain.lasession");
    save(&mut shell, &path);
    assert!(extras(&path).iter().all(|(tag, _)| tag != "LAYT"));
    // Setting a height and setting it back to automatic leaves none either.
    set(&mut shell, 0, 80);
    set(&mut shell, 0, 0);
    save(&mut shell, &path);
    assert!(extras(&path).iter().all(|(tag, _)| tag != "LAYT"));
}

#[test]
fn a_file_of_an_earlier_version_opens_with_automatic_heights_and_no_notice() {
    let dir = TempDir::new("layt-old");
    for name in [
        "feature_006_view.lasession",
        "feature_008_decoder.lasession",
    ] {
        let shell = open(&dir, &fixture(name));
        assert!(heights_of(&shell.state().presentation).is_empty(), "{name}");
        assert!(
            codes(&shell).iter().all(|c| !c.contains("layout")),
            "{name}: {:?}",
            codes(&shell)
        );
    }
}

#[test]
fn a_damaged_section_is_dropped_with_a_warning_and_the_capture_opens_with_automatic_heights() {
    let dir = TempDir::new("layt-bad");
    let bad = dir.path().join("bad.lasession");
    // Channel 9 does not exist in a capture of four channels... the fixture has 32, so use a
    // height out of range instead.
    with_section(
        &fixture("feature_006_view.lasession"),
        &bad,
        "LAYT",
        vec![0x01, 0x01, 0x01, 0x05],
    );
    let mut shell = open(&dir, &bad);
    assert!(codes(&shell).contains(&LAYOUT_INVALID.to_owned()));
    assert!(heights_of(&shell.state().presentation).is_empty());
    // The invalid section is not written back.
    let again = dir.path().join("again.lasession");
    save(&mut shell, &again);
    assert!(extras(&again).iter().all(|(tag, _)| tag != "LAYT"));
}

#[test]
fn a_newer_section_is_kept_untouched_with_an_info_notice() {
    let dir = TempDir::new("layt-newer");
    let newer = dir.path().join("newer.lasession");
    with_section(
        &fixture("feature_006_view.lasession"),
        &newer,
        "LAYT",
        vec![0x03, 0xaa, 0xbb],
    );
    let mut shell = open(&dir, &newer);
    assert!(codes(&shell).contains(&"notice-layout-newer".to_owned()));
    assert!(heights_of(&shell.state().presentation).is_empty());
    let again = dir.path().join("again.lasession");
    save(&mut shell, &again);
    let layt = extras(&again)
        .into_iter()
        .find(|(tag, _)| tag == "LAYT")
        .expect("kept");
    assert_eq!(layt.1, vec![0x03, 0xaa, 0xbb]);
}

#[test]
fn a_reader_that_does_not_know_the_section_keeps_it_byte_for_byte() {
    // What a build without feature 014 does: it reads the file with the session library, keeps the
    // optional sections it does not know and writes them back.
    let dir = TempDir::new("layt-unknown");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 2);
    set(&mut shell, 0, 77);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let before = extras(&path);
    let copy = dir.path().join("b.lasession");
    let session =
        la_session::load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    let mut data = SessionData::new(session.description().clone(), session.snapshot());
    for section in session.extras() {
        data = data.with_section(section.clone()).unwrap();
    }
    la_session::save_blocking(&copy, &data, &Control::new()).unwrap();
    assert_eq!(extras(&copy), before);
    assert!(before.iter().any(|(tag, _)| tag == "LAYT"));
}
