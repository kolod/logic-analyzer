//! The `VIEW` section codec, payload version 2, against
//! `specs/015-markers-rework/contracts/view-section-2.md`.
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
use la_host::markers::{Marker, Markers, Shape};
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
    flags: u8,
    number: u64,
    colour: u8,
    a: u64,
    /// Present only for a pair.
    b: Option<u64>,
    comment: Vec<u8>,
}

struct Payload {
    version: u8,
    channels: Vec<Ch>,
    channel_count: Option<u64>,
    markers: Vec<Mk>,
    last_id: u64,
    next_single: u64,
    next_pair: u64,
    origin: u64,
    selected: u64,
    window: (u64, u64),
    follow: u8,
    trailing: Vec<u8>,
}

impl Payload {
    fn empty() -> Self {
        Self {
            version: 2,
            channels: Vec::new(),
            channel_count: None,
            markers: Vec::new(),
            last_id: 0,
            next_single: 1,
            next_pair: 1,
            origin: 0,
            selected: 0,
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
            out.push(m.flags);
            varint(m.number, &mut out);
            out.push(m.colour);
            varint(m.a, &mut out);
            if let Some(b) = m.b {
                varint(b, &mut out);
            }
            varint(m.comment.len() as u64, &mut out);
            out.extend_from_slice(&m.comment);
        }
        varint(self.last_id, &mut out);
        varint(self.next_single, &mut out);
        varint(self.next_pair, &mut out);
        varint(self.origin, &mut out);
        varint(self.selected, &mut out);
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

fn single(id: u64, number: u64, a: u64) -> Mk {
    Mk {
        id,
        flags: 0,
        number,
        colour: 0,
        a,
        b: None,
        comment: vec![],
    }
}

fn pair(id: u64, number: u64, a: u64, b: u64) -> Mk {
    Mk {
        id,
        flags: 1,
        number,
        colour: 0,
        a,
        b: Some(b),
        comment: vec![],
    }
}

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
                comment: b"ab".to_vec(),
                ..single(1, 1, 100)
            },
            Mk {
                colour: 3,
                ..pair(5, 1, 200, 999)
            },
        ],
        last_id: 7,
        next_single: 3,
        next_pair: 2,
        origin: 100,
        selected: 6,
        window: (10, 500),
        follow: 1,
        ..Payload::empty()
    }
}

/// 32 channels with 256-byte names and 64 markers (32 singles, 32 pairs) with 320-byte comments.
fn maximal() -> (Payload, Bounds) {
    let bounds = Bounds {
        channels: 32,
        length: 50_000_000_000,
    };
    let name = "😀".repeat(64).into_bytes();
    assert_eq!(name.len(), 256);
    let comment = "😀".repeat(80).into_bytes();
    assert_eq!(comment.len(), 320);
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
    let markers = (0..64_u64)
        .map(|i| {
            let id = 4_000_000_000 - i;
            let number = i / 2 + 1;
            let a = 49_999_999_999 - 2 * i * 1_000_003;
            let mut m = if i % 2 == 0 {
                single(id, number, a)
            } else {
                pair(id, number, a - 1_000, a)
            };
            m.colour = (i % 8) as u8;
            m.comment = comment.clone();
            m
        })
        .collect();
    (
        Payload {
            channels,
            markers,
            last_id: 4_000_000_000,
            next_single: 33,
            next_pair: 33,
            origin: 1_000_003,
            selected: 4_000_000_000 - 3 + 1,
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
fn the_default_payload_is_eleven_bytes_and_means_nothing_is_stored() {
    let bytes = [2_u8, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0];
    assert_eq!(Payload::empty().bytes(), bytes);
    let v = view(decode(&bytes, BOUNDS).unwrap());
    assert_eq!(v.presentation, None);
    assert!(v.markers.list.is_empty());
    assert_eq!(v.markers.selected, None);
    assert_eq!(v.markers.origin, 0);
    assert_eq!(v.markers.next_numbers(), [1, 1]);
    assert_eq!(v.window, None);
    assert!(!v.follow_latest);
}

#[test]
fn a_typical_payload_decodes_to_the_values_written_in_the_contract() {
    // Written out byte by byte, independently of the encoder helper above.
    let bytes: Vec<u8> = vec![
        2, // version
        3, // channels
        0, 0b01, 0, 0, // index 0: visible, palette 0, no name
        2, 0b11, 0x10, 0x20, 0x30, 3, b'S', b'D', b'A', // index 2: custom colour, "SDA"
        1, 0b00, 5, 0, // index 1: hidden, palette 5
        2, // markers
        1, 0, 1, 0, 100, 2, b'a', b'b', // id 1, single, number 1, colour 0, at 100, "ab"
        5, 1, 1, 3, 0xC8, 0x01, 0xE7, 0x07,
        0,   // id 5, pair, number 1, colour 3, 200..999, no comment
        7,   // last id
        3,   // next single number
        2,   // next pair number
        100, // origin
        6,   // selected: id 5
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
    assert_eq!(
        v.markers.list,
        vec![
            Marker {
                id: 1,
                number: 1,
                colour: 0,
                comment: "ab".into(),
                shape: Shape::Point(100),
            },
            Marker {
                id: 5,
                number: 1,
                colour: 3,
                comment: String::new(),
                shape: Shape::Interval {
                    start: 200,
                    end: 999
                },
            },
        ]
    );
    assert_eq!(v.markers.selected, Some(5));
    assert_eq!(v.markers.origin, 100);
    assert_eq!(v.markers.last_id(), 7);
    assert_eq!(v.markers.next_numbers(), [3, 2]);
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
        assert_eq!(again.markers, state.markers);
        assert_eq!(again.window, state.window);
        assert_eq!(again.follow_latest, state.follow_latest);
    }
}

#[test]
fn the_writer_is_deterministic_whatever_order_the_state_was_built_in() {
    let build = || {
        let mut m = Markers::new();
        for position in [10, 20, 30] {
            m.add(Shape::Point(position));
        }
        m.add(Shape::interval(5, 50));
        m.set_colour(2, 4);
        m.set_comment(2, "x");
        m.origin = 20;
        m.selected = Some(2);
        m
    };
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
            ..Presentation::default()
        }),
        markers,
        window: Some((1, 100)),
        follow_latest: false,
    };
    let bounds = Bounds {
        channels: 3,
        length: 1_000,
    };
    let first = encode(&make(build(), vec![2, 0, 1]));
    for _ in 0..3 {
        assert_eq!(encode(&make(build(), vec![2, 0, 1])), first);
    }
    let decoded = view(decode(&first, bounds).unwrap());
    assert_eq!(decoded.markers, build());
}

#[test]
fn the_numbering_state_survives_even_when_no_marker_is_left() {
    let mut m = Markers::new();
    let a = m.add(Shape::Point(5)).unwrap();
    m.add(Shape::interval(1, 2)).unwrap();
    m.remove(a);
    m.clear();
    let state = ViewState {
        presentation: None,
        markers: m,
        window: None,
        follow_latest: false,
    };
    let back = view(decode(&encode(&state), BOUNDS).unwrap());
    assert!(back.markers.list.is_empty());
    assert_eq!(back.markers.last_id(), 2);
    assert_eq!(back.markers.next_numbers(), [2, 2]);
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
    for version in [0_u8, 1] {
        let mut p = typical();
        p.version = version;
        rejected(&p.bytes(), BOUNDS, "an older version is not read");
    }

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

    // Markers.
    let mut p = typical();
    p.markers.push(single(1, 2, 5));
    p.next_single = 4;
    rejected(&p.bytes(), BOUNDS, "duplicate marker id");

    let mut p = typical();
    p.markers.push(single(6, 1, 5));
    rejected(
        &p.bytes(),
        BOUNDS,
        "duplicate number within the single kind",
    );

    let mut p = typical();
    p.markers.push(pair(6, 1, 5, 6));
    p.last_id = 7;
    rejected(&p.bytes(), BOUNDS, "duplicate number within the pair kind");

    let mut p = typical();
    p.markers.push(single(6, 0, 5));
    rejected(&p.bytes(), BOUNDS, "number 0");

    let mut p = typical();
    p.markers[0].flags = 0b10;
    rejected(&p.bytes(), BOUNDS, "reserved marker flag bit");

    let mut p = typical();
    p.markers[0].colour = 8;
    rejected(&p.bytes(), BOUNDS, "colour 8");

    let mut p = typical();
    p.markers[0].a = 1_000;
    rejected(&p.bytes(), BOUNDS, "single marker beyond the capture");

    let mut p = typical();
    p.markers[1].b = Some(1_000);
    rejected(&p.bytes(), BOUNDS, "pair end beyond the capture");

    let mut p = typical();
    p.markers[1].a = 500;
    p.markers[1].b = Some(499);
    rejected(&p.bytes(), BOUNDS, "pair end before its start");

    let mut p = typical();
    p.markers[0].comment = vec![0xff];
    rejected(&p.bytes(), BOUNDS, "a comment that is not UTF-8");

    let mut p = typical();
    p.markers[0].comment = "a\u{202e}b".as_bytes().to_vec();
    rejected(&p.bytes(), BOUNDS, "a comment that is not sanitized");

    let mut p = typical();
    p.markers[0].comment = "x".repeat(81).into_bytes();
    rejected(&p.bytes(), BOUNDS, "a comment of 81 characters");

    let mut p = typical();
    p.markers[0].comment = vec![b'x'; 321];
    rejected(&p.bytes(), BOUNDS, "a comment of 321 bytes");

    let mut p = typical();
    p.markers[0].id = u64::from(u32::MAX) + 1;
    p.selected = 0;
    rejected(&p.bytes(), BOUNDS, "marker id beyond 32 bits");

    let mut p = typical();
    p.markers = (0..65).map(|i| single(i + 1, i + 1, 1)).collect();
    p.last_id = 65;
    p.next_single = 66;
    p.next_pair = 1;
    p.selected = 0;
    rejected(&p.bytes(), BOUNDS, "65 markers");

    let mut p = typical();
    p.last_id = 4;
    rejected(&p.bytes(), BOUNDS, "last id below an id in use");

    let mut p = typical();
    p.last_id = u64::from(u32::MAX) + 1;
    rejected(&p.bytes(), BOUNDS, "last id beyond 32 bits");

    let mut p = typical();
    p.next_single = 1;
    rejected(
        &p.bytes(),
        BOUNDS,
        "next single number not above the ones in use",
    );

    let mut p = typical();
    p.next_pair = 1;
    rejected(
        &p.bytes(),
        BOUNDS,
        "next pair number not above the ones in use",
    );

    let mut p = typical();
    p.next_single = 0;
    rejected(&p.bytes(), BOUNDS, "next single number 0");

    let mut p = typical();
    p.origin = 1_000;
    rejected(&p.bytes(), BOUNDS, "origin beyond the capture");

    let mut p = typical();
    p.selected = 9;
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
    p.markers = vec![single(1, 1, 0), pair(2, 1, 0, 0)];
    p.last_id = 2;
    p.next_single = 2;
    p.next_pair = 2;
    assert!(decode(&p.bytes(), empty).is_ok());
    p.markers[0].a = 1;
    rejected(&p.bytes(), empty, "a marker at 1 in an empty capture");
    let mut p = Payload::empty();
    p.origin = 1;
    rejected(&p.bytes(), empty, "an origin in an empty capture");
    let mut p = Payload::empty();
    p.window = (0, 1);
    rejected(&p.bytes(), empty, "window in an empty capture");
}

#[test]
fn a_newer_section_version_is_not_understood_but_is_not_an_error() {
    for version in [3_u8, 4, 200, 255] {
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
        Ok(Decoded::Newer) => assert!(bytes[0] > 2),
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
                number: 1,
                colour: 0,
                comment: "ab".into(),
                shape: Shape::Point(100),
            },
            Marker {
                id: 5,
                number: 1,
                colour: 3,
                comment: String::new(),
                shape: Shape::Interval {
                    start: 200,
                    end: 999,
                },
            },
        ],
        Some(5),
        100,
        7,
        [3, 2],
    );
    assert_eq!(state.markers, restored);
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
