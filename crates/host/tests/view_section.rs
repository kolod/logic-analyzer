//! The `VIEW` section codec against `contracts/view-section.md` (spec US4, task T062).
//!
//! The payloads here are built by a small encoder written from the contract text, not by the
//! writer under test, so the reader is checked against the document and not against itself.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_host::error::VIEW_INVALID;
use la_host::markers::{Marker, Markers};
use la_host::presentation::{ChannelStyle, Colour, Presentation};
use la_host::view_section::{Bounds, Decoded, ViewState, decode, encode};

// ---- an encoder written from the contract -----------------------------------------------

fn varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

struct Ch {
    index: u64,
    flags: u8,
    colour: Vec<u8>,
    name: Vec<u8>,
}

struct Mk {
    id: u64,
    position: u64,
    name: Vec<u8>,
}

struct Payload {
    version: u8,
    channels: Vec<Ch>,
    channel_count: Option<u64>,
    markers: Vec<Mk>,
    first: u64,
    second: u64,
    window: (u64, u64),
    follow: u8,
    trailing: Vec<u8>,
}

impl Payload {
    fn empty() -> Self {
        Self {
            version: 1,
            channels: Vec::new(),
            channel_count: None,
            markers: Vec::new(),
            first: 0,
            second: 0,
            window: (0, 0),
            follow: 0,
            trailing: Vec::new(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = vec![self.version];
        varint(
            self.channel_count.unwrap_or(self.channels.len() as u64),
            &mut out,
        );
        for c in &self.channels {
            varint(c.index, &mut out);
            out.push(c.flags);
            out.extend_from_slice(&c.colour);
            varint(c.name.len() as u64, &mut out);
            out.extend_from_slice(&c.name);
        }
        varint(self.markers.len() as u64, &mut out);
        for m in &self.markers {
            varint(m.id, &mut out);
            varint(m.position, &mut out);
            varint(m.name.len() as u64, &mut out);
            out.extend_from_slice(&m.name);
        }
        varint(self.first, &mut out);
        varint(self.second, &mut out);
        varint(self.window.0, &mut out);
        varint(self.window.1, &mut out);
        out.push(self.follow);
        out.extend_from_slice(&self.trailing);
        out
    }
}

const BOUNDS: Bounds = Bounds {
    channels: 3,
    length: 1_000,
};

fn typical() -> Payload {
    Payload {
        channels: vec![
            Ch {
                index: 0,
                flags: 0b01,
                colour: vec![0],
                name: vec![],
            },
            Ch {
                index: 2,
                flags: 0b11,
                colour: vec![0x10, 0x20, 0x30],
                name: b"SDA".to_vec(),
            },
            Ch {
                index: 1,
                flags: 0b00,
                colour: vec![5],
                name: vec![],
            },
        ],
        markers: vec![
            Mk {
                id: 1,
                position: 100,
                name: b"ab".to_vec(),
            },
            Mk {
                id: 5,
                position: 999,
                name: vec![],
            },
        ],
        first: 2,
        second: 6,
        window: (10, 500),
        follow: 1,
        ..Payload::empty()
    }
}

/// 32 channels with 256-byte names and 64 markers with 256-byte names.
fn maximal() -> (Payload, Bounds) {
    let bounds = Bounds {
        channels: 32,
        length: 50_000_000_000,
    };
    let name = "😀".repeat(64).into_bytes();
    assert_eq!(name.len(), 256);
    let channels = (0..32)
        .rev()
        .map(|i| Ch {
            index: i,
            flags: if i % 2 == 0 { 0b11 } else { 0b01 },
            colour: if i % 2 == 0 {
                vec![255, 128, i as u8]
            } else {
                vec![(i % 16) as u8]
            },
            name: name.clone(),
        })
        .collect();
    let markers = (0..64)
        .map(|i| Mk {
            id: 4_000_000_000 - i,
            position: 49_999_999_999 - i * 1_000_003,
            name: name.clone(),
        })
        .collect();
    (
        Payload {
            channels,
            markers,
            first: 4_000_000_000 - 3 + 1,
            second: 4_000_000_000 - 7 + 1,
            window: (1, 49_999_999_999),
            follow: 0,
            ..Payload::empty()
        },
        bounds,
    )
}

fn view(decoded: Decoded) -> ViewState {
    match decoded {
        Decoded::View(v) => v,
        Decoded::Newer => panic!("a newer section"),
    }
}

// ---- the reader against hand-written bytes ----------------------------------------------

#[test]
fn the_default_payload_is_eight_bytes_and_means_nothing_is_stored() {
    let bytes = [1_u8, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(Payload::empty().bytes(), bytes);
    let v = view(decode(&bytes, BOUNDS).unwrap());
    assert_eq!(v.presentation, None);
    assert!(v.markers.list.is_empty());
    assert_eq!(v.markers.selected, [None, None]);
    assert_eq!(v.window, None);
    assert!(!v.follow_latest);
}

#[test]
fn a_typical_payload_decodes_to_the_values_written_in_the_contract() {
    // Written out byte by byte, independently of the encoder helper above.
    let bytes: Vec<u8> = vec![
        1, // version
        3, // channels
        0, 0b01, 0, 0, // index 0: visible, palette 0, no name
        2, 0b11, 0x10, 0x20, 0x30, 3, b'S', b'D', b'A', // index 2: custom colour, "SDA"
        1, 0b00, 5, 0, // index 1: hidden, palette 5
        2, // markers
        1, 100, 2, b'a', b'b', // id 1 at 100 "ab"
        5, 0xE7, 0x07, 0, // id 5 at 999
        2, 6, // selected: id 1 and id 5
        10, 0xF4, 0x03, // window 10 + 500
        1,    // follow
    ];
    assert_eq!(typical().bytes(), bytes);
    let v = view(decode(&bytes, BOUNDS).unwrap());
    let p = v.presentation.unwrap();
    let order: Vec<u8> = p.channels.iter().map(|c| c.index).collect();
    assert_eq!(order, [0, 2, 1]);
    assert_eq!(
        p.channels[1],
        ChannelStyle {
            index: 2,
            name: "SDA".into(),
            colour: Colour::Rgb(0x10, 0x20, 0x30),
            visible: true,
            height: 0,
        }
    );
    assert_eq!(p.channels[2].colour, Colour::Palette(5));
    assert!(!p.channels[2].visible);
    let markers: Vec<(u32, u64, &str)> = v
        .markers
        .list
        .iter()
        .map(|m| (m.id, m.position, m.name.as_str()))
        .collect();
    assert_eq!(markers, [(1, 100, "ab"), (5, 999, "")]);
    assert_eq!(v.markers.selected, [Some(1), Some(5)]);
    assert_eq!(v.window, Some((10, 500)));
    assert!(v.follow_latest);
}

// ---- round trips and determinism ----------------------------------------------------------

fn presentation_of(p: &ViewState) -> Vec<ChannelStyle> {
    p.presentation.clone().unwrap().channels
}

#[test]
fn the_three_states_survive_a_round_trip_and_the_writer_matches_the_contract() {
    let mut cases = vec![
        (Payload::empty().bytes(), BOUNDS),
        (typical().bytes(), BOUNDS),
    ];
    let (max, bounds) = maximal();
    cases.push((max.bytes(), bounds));
    for (bytes, bounds) in cases {
        let state = view(decode(&bytes, bounds).unwrap());
        // The writer produces exactly the bytes the contract describes.
        assert_eq!(encode(&state), bytes);
        let again = view(decode(&encode(&state), bounds).unwrap());
        assert_eq!(again.presentation, state.presentation);
        assert_eq!(again.markers.list, state.markers.list);
        assert_eq!(again.markers.selected, state.markers.selected);
        assert_eq!(again.window, state.window);
        assert_eq!(again.follow_latest, state.follow_latest);
    }
}

#[test]
fn the_writer_is_deterministic_whatever_order_the_state_was_built_in() {
    let mut a = Markers::new();
    let mut b = Markers::new();
    for position in [10, 20, 30] {
        a.place(position);
    }
    for position in [10, 20, 30] {
        b.place(position);
    }
    let make = |markers: Markers, order: Vec<u8>| ViewState {
        presentation: Some(Presentation {
            channels: order
                .iter()
                .map(|&i| ChannelStyle {
                    index: i,
                    name: format!("c{i}"),
                    colour: Colour::Palette(i),
                    visible: true,
                    height: 0,
                })
                .collect(),
        }),
        markers,
        window: Some((1, 100)),
        follow_latest: false,
    };
    let bounds = Bounds {
        channels: 3,
        length: 1_000,
    };
    let first = encode(&make(a, vec![2, 0, 1]));
    for _ in 0..3 {
        assert_eq!(encode(&make(b.clone(), vec![2, 0, 1])), first);
    }
    assert!(decode(&first, bounds).is_ok());
}

// ---- every validation rule fails with host.view_invalid -----------------------------------

fn rejected(bytes: &[u8], bounds: Bounds, what: &str) {
    match decode(bytes, bounds) {
        Err(e) => assert_eq!(e.code(), VIEW_INVALID, "{what}"),
        Ok(_) => panic!("{what}: accepted"),
    }
}

#[test]
fn each_rule_of_the_contract_has_a_failing_fixture() {
    let mut p = typical();
    p.version = 0;
    rejected(&p.bytes(), BOUNDS, "version 0");

    // A non-canonical varint: the count 3 written as 0x83 0x00.
    let mut bytes = typical().bytes();
    bytes.splice(1..2, [0x83, 0x00]);
    rejected(&bytes, BOUNDS, "non-canonical varint");

    // An overlong varint.
    let mut bytes = typical().bytes();
    bytes.splice(1..2, [0x80; 11]);
    rejected(&bytes, BOUNDS, "overlong varint");

    let mut p = typical();
    p.channel_count = Some(33);
    rejected(&p.bytes(), BOUNDS, "more than 32 channels");

    let mut p = typical();
    p.channels.pop();
    rejected(&p.bytes(), BOUNDS, "channel count differs from the capture");

    let mut p = typical();
    p.channels[0].index = 3;
    rejected(&p.bytes(), BOUNDS, "channel index beyond the capture");

    let mut p = typical();
    p.channels[2].index = 0;
    rejected(&p.bytes(), BOUNDS, "duplicate channel index");

    let mut p = typical();
    p.channels[0].flags = 0b101;
    rejected(&p.bytes(), BOUNDS, "reserved flag bit");

    let mut p = typical();
    p.channels[0].colour = vec![16];
    rejected(&p.bytes(), BOUNDS, "palette index 16");

    let mut p = typical();
    p.channels[0].name = vec![b'x'; 257];
    rejected(&p.bytes(), BOUNDS, "name of 257 bytes");

    let mut p = typical();
    p.channels[0].name = vec![0xff, 0xfe];
    rejected(&p.bytes(), BOUNDS, "invalid UTF-8");

    let mut p = typical();
    p.channels[0].name = "a\u{202e}b".as_bytes().to_vec();
    rejected(&p.bytes(), BOUNDS, "a name that is not sanitized");

    let mut p = typical();
    p.channels[0].name = b" padded ".to_vec();
    rejected(&p.bytes(), BOUNDS, "a name with surrounding spaces");

    let mut p = typical();
    p.markers.push(Mk {
        id: 1,
        position: 5,
        name: vec![],
    });
    rejected(&p.bytes(), BOUNDS, "duplicate marker id");

    let mut p = typical();
    p.markers[1].position = 1_000;
    rejected(&p.bytes(), BOUNDS, "marker beyond the capture");

    let mut p = typical();
    p.markers[0].id = u64::from(u32::MAX) + 1;
    p.first = 0;
    p.second = 0;
    rejected(&p.bytes(), BOUNDS, "marker id beyond 32 bits");

    let mut p = typical();
    p.markers = (0..65)
        .map(|i| Mk {
            id: i + 1,
            position: 1,
            name: vec![],
        })
        .collect();
    p.first = 0;
    p.second = 0;
    rejected(&p.bytes(), BOUNDS, "65 markers");

    let mut p = typical();
    p.first = 9;
    rejected(&p.bytes(), BOUNDS, "selection of an unknown marker");

    let mut p = typical();
    p.window = (600, 500);
    rejected(&p.bytes(), BOUNDS, "window beyond the capture");

    let mut p = typical();
    p.window = (5, 0);
    rejected(&p.bytes(), BOUNDS, "window of length 0");

    let mut p = typical();
    p.follow = 2;
    rejected(&p.bytes(), BOUNDS, "follow flag 2");

    let mut p = typical();
    p.trailing = vec![0];
    rejected(&p.bytes(), BOUNDS, "trailing bytes");

    rejected(&[], BOUNDS, "empty payload");
}

#[test]
fn an_empty_capture_accepts_only_markers_at_zero_and_no_window() {
    let empty = Bounds {
        channels: 3,
        length: 0,
    };
    let mut p = Payload::empty();
    p.markers = vec![Mk {
        id: 1,
        position: 0,
        name: vec![],
    }];
    assert!(decode(&p.bytes(), empty).is_ok());
    p.markers[0].position = 1;
    rejected(&p.bytes(), empty, "marker 1 in an empty capture");
    let mut p = Payload::empty();
    p.window = (0, 1);
    rejected(&p.bytes(), empty, "window in an empty capture");
}

#[test]
fn a_newer_section_version_is_not_understood_but_is_not_an_error() {
    for version in [2_u8, 3, 200, 255] {
        let mut p = typical();
        p.version = version;
        // The rest of the payload may be anything a newer writer put there.
        let mut bytes = p.bytes();
        bytes.extend_from_slice(&[9, 9, 9]);
        assert!(matches!(decode(&bytes, BOUNDS).unwrap(), Decoded::Newer));
    }
}

// ---- a mutation fuzzer ----------------------------------------------------------------------

/// Any accepted payload is a state the writer writes back identically.
fn check_accepts_only_canonical(bytes: &[u8], bounds: Bounds) {
    match decode(bytes, bounds) {
        Ok(Decoded::View(state)) => assert_eq!(
            encode(&state),
            bytes,
            "an accepted payload does not re-encode identically"
        ),
        Ok(Decoded::Newer) => assert!(bytes[0] > 1),
        Err(e) => assert_eq!(e.code(), VIEW_INVALID),
    }
}

#[test]
fn no_single_byte_change_truncation_or_extension_panics_or_breaks_the_round_trip() {
    let (max, max_bounds) = maximal();
    for (payload, bounds) in [
        (Payload::empty().bytes(), BOUNDS),
        (typical().bytes(), BOUNDS),
        (max.bytes(), max_bounds),
    ] {
        // Every single-byte change to a handful of values (all 256 for the small payloads).
        let values: Vec<u8> = if payload.len() < 100 {
            (0..=255).collect()
        } else {
            vec![0, 1, 2, 0x7f, 0x80, 0xff]
        };
        let len = payload.len();
        for at in 0..len {
            // The large payload is sampled: its ends in full, the middle every 7th byte.
            if len >= 100 && at >= 64 && at + 64 < len && at % 7 != 0 {
                continue;
            }
            for &value in &values {
                let mut changed = payload.clone();
                changed[at] = value;
                check_accepts_only_canonical(&changed, bounds);
            }
        }
        // Every truncation.
        for cut in 0..payload.len() {
            if payload.len() >= 100 && cut >= 64 && cut + 64 < payload.len() && cut % 7 != 0 {
                continue;
            }
            check_accepts_only_canonical(&payload[..cut], bounds);
        }
        // Extensions.
        for extra in [vec![0], vec![1], vec![0xff; 5], vec![0; 100]] {
            let mut longer = payload.clone();
            longer.extend_from_slice(&extra);
            check_accepts_only_canonical(&longer, bounds);
        }
    }
}

#[test]
fn marker_helpers_agree_with_the_decoded_state() {
    let state = view(decode(&typical().bytes(), BOUNDS).unwrap());
    let restored = Markers::restore(
        vec![
            Marker {
                id: 1,
                position: 100,
                name: "ab".into(),
            },
            Marker {
                id: 5,
                position: 999,
                name: String::new(),
            },
        ],
        [Some(1), Some(5)],
    );
    assert_eq!(state.markers.list, restored.list);
    assert_eq!(presentation_of(&state).len(), 3);
}

/// Random mutations of valid payloads: `LA_VIEW_FUZZ_ITERATIONS` of them (a million in the
/// nightly workflow). Whatever is accepted re-encodes to the same bytes; nothing panics.
#[test]
#[ignore = "a million random mutations; the nightly workflow runs it"]
fn a_million_random_mutations_never_panic_or_break_the_round_trip() {
    let iterations: u64 = std::env::var("LA_VIEW_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let (max, max_bounds) = maximal();
    let seeds = [
        (Payload::empty().bytes(), BOUNDS),
        (typical().bytes(), BOUNDS),
        (max.bytes(), max_bounds),
    ];
    let mut rng = common::Lcg(0x5eed);
    let mut accepted = 0_u64;
    for i in 0..iterations {
        let (payload, bounds) = &seeds[(i % 3) as usize];
        let mut bytes = payload.clone();
        for _ in 0..=rng.below(4) {
            match rng.below(5) {
                0 if !bytes.is_empty() => {
                    let at = rng.below(bytes.len() as u64) as usize;
                    bytes[at] = rng.below(256) as u8;
                }
                1 if !bytes.is_empty() => {
                    let at = rng.below(bytes.len() as u64) as usize;
                    bytes[at] ^= 1 << rng.below(8);
                }
                2 if !bytes.is_empty() => {
                    let at = rng.below(bytes.len() as u64) as usize;
                    bytes.remove(at);
                }
                3 => {
                    let at = rng.below(bytes.len() as u64 + 1) as usize;
                    bytes.insert(at, rng.below(256) as u8);
                }
                _ => {
                    let keep = rng.below(bytes.len() as u64 + 1) as usize;
                    bytes.truncate(keep);
                }
            }
        }
        if decode(&bytes, *bounds).is_ok() {
            accepted += 1;
        }
        check_accepts_only_canonical(&bytes, *bounds);
    }
    assert!(accepted > 0, "no mutation was ever accepted");
}
