//! The framing of host-to-device messages (feature 017, T028): frames spelled out byte by byte, the parser in
//! chunks of every size, damaged frames dropped without losing the next, and a fuzz run that never panics.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_proto::{Event, FrameError, FrameParser, MAX_BODY_LEN, MAX_FRAME_LEN, encode_frame};

#[derive(Debug, PartialEq, Eq, Clone)]
enum Seen {
    Frame(Vec<u8>),
    Overflow,
    Damaged,
}

fn parse_in_chunks(bytes: &[u8], chunk: usize) -> Vec<Seen> {
    let mut parser = FrameParser::new();
    let mut seen = Vec::new();
    for part in bytes.chunks(chunk.max(1)) {
        parser.feed(part, |event| {
            seen.push(match event {
                Event::Frame(body) => Seen::Frame(body.to_vec()),
                Event::Overflow => Seen::Overflow,
                Event::Damaged => Seen::Damaged,
            });
        });
    }
    seen
}

fn frame_of(body: &[u8]) -> Vec<u8> {
    let mut out = vec![0_u8; 4 + 2 * body.len()];
    let n = encode_frame(body, &mut out).unwrap();
    out.truncate(n);
    out
}

#[test]
fn the_device_information_request_is_five_bytes() {
    assert_eq!(frame_of(&[0]), [0x55, 0xAA, 0x00, 0xAA, 0x55]);
}

#[test]
fn aa_and_f0_are_escaped_in_the_body() {
    assert_eq!(
        frame_of(&[0x07, 0xAA, 0xF0, 0x55, 0x00]),
        [
            0x55, 0xAA, 0x07, 0xF0, 0x5A, 0xF0, 0x00, 0x55, 0x00, 0xAA, 0x55
        ]
    );
    // And the parser takes them back.
    let wire = frame_of(&[0x07, 0xAA, 0xF0, 0x55, 0x00]);
    assert_eq!(
        parse_in_chunks(&wire, 64),
        vec![Seen::Frame(vec![0x07, 0xAA, 0xF0, 0x55, 0x00])]
    );
}

#[test]
fn a_body_that_ends_in_55_is_not_taken_for_a_new_start() {
    let wire = frame_of(&[0x01, 0x55]);
    assert_eq!(wire, [0x55, 0xAA, 0x01, 0x55, 0xAA, 0x55]);
    assert_eq!(
        parse_in_chunks(&wire, 1),
        vec![Seen::Frame(vec![0x01, 0x55])]
    );
}

#[test]
fn the_longest_frame_passes_and_one_byte_more_is_an_overflow() {
    assert_eq!(MAX_BODY_LEN, 252);
    let longest: Vec<u8> = (0..MAX_BODY_LEN).map(|i| (i % 200) as u8 + 1).collect();
    assert_eq!(
        parse_in_chunks(&frame_of(&longest), 7),
        vec![Seen::Frame(longest.clone())]
    );
    // The builder refuses a body that is too long ...
    let mut out = vec![0_u8; 1024];
    let too_long = vec![1_u8; MAX_BODY_LEN + 1];
    assert_eq!(
        encode_frame(&too_long, &mut out),
        Err(FrameError::BodyTooLong)
    );
    // ... and the parser drops one that arrives, then goes on with the next frame.
    let mut wire = vec![0x55, 0xAA];
    wire.extend_from_slice(&too_long);
    wire.extend_from_slice(&[0xAA, 0x55]);
    wire.extend_from_slice(&frame_of(&[0]));
    assert_eq!(
        parse_in_chunks(&wire, 5),
        vec![Seen::Overflow, Seen::Frame(vec![0])]
    );
    assert_eq!(MAX_FRAME_LEN, 256);
}

#[test]
fn a_small_output_buffer_is_refused() {
    let mut out = [0_u8; 4];
    assert_eq!(
        encode_frame(&[0], &mut out),
        Err(FrameError::OutputTooSmall)
    );
}

#[test]
fn bytes_outside_a_frame_are_ignored() {
    let mut wire = vec![0x00, 0x13, 0x55, 0x55, 0x01, 0xAA];
    wire.extend_from_slice(&frame_of(&[5]));
    assert_eq!(parse_in_chunks(&wire, 3), vec![Seen::Frame(vec![5])]);
}

#[test]
fn every_chunking_gives_the_same_events() {
    let mut wire = Vec::new();
    for body in [&[0_u8][..], &[7, 0xAA, 0xF0, 1, 2, 3], &[4], &[1, 0x55]] {
        wire.extend_from_slice(&frame_of(body));
        wire.extend_from_slice(&[0x99, 0x55]); // noise between frames
    }
    let whole = parse_in_chunks(&wire, wire.len());
    assert_eq!(whole.len(), 4);
    for chunk in 1..=wire.len() {
        assert_eq!(parse_in_chunks(&wire, chunk), whole, "chunk {chunk}");
    }
}

#[test]
fn a_bad_escape_drops_that_frame_and_not_the_next() {
    let mut wire = vec![0x55, 0xAA, 0x01, 0xF0, 0x33, 0x02, 0xAA, 0x55];
    wire.extend_from_slice(&frame_of(&[9]));
    assert_eq!(
        parse_in_chunks(&wire, 2),
        vec![Seen::Damaged, Seen::Frame(vec![9])]
    );
}

#[test]
fn a_frame_that_never_ends_is_dropped_when_the_next_one_starts() {
    // `55 AA 01 02` (no end), then a whole frame.
    let mut wire = vec![0x55, 0xAA, 0x01, 0x02];
    wire.extend_from_slice(&frame_of(&[8]));
    assert_eq!(
        parse_in_chunks(&wire, 1),
        vec![Seen::Damaged, Seen::Frame(vec![8])]
    );
}

#[test]
fn an_end_that_does_not_follow_is_damaged() {
    let mut wire = vec![0x55, 0xAA, 0x01, 0xAA, 0x77];
    wire.extend_from_slice(&frame_of(&[3]));
    assert_eq!(
        parse_in_chunks(&wire, 4),
        vec![Seen::Damaged, Seen::Frame(vec![3])]
    );
}

#[test]
fn random_bytes_never_panic_and_a_frame_after_them_is_found() {
    let iterations: u64 = std::env::var("LA_PROTO_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_000);
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..iterations {
        let len = (next() % 600) as usize;
        // Bytes biased towards the ones that matter to the parser.
        let bytes: Vec<u8> = (0..len)
            .map(|_| match next() % 6 {
                0 => 0x55,
                1 => 0xAA,
                2 => 0xF0,
                3 => 0x5A,
                _ => next() as u8,
            })
            .collect();
        let chunk = (next() % 40 + 1) as usize;
        let _ = parse_in_chunks(&bytes, chunk);
        // After the noise, a damaged frame is closed and a good one comes through: feed a closing marker
        // and then the frame (the noise may have left the parser inside a body).
        let mut tail = bytes.clone();
        tail.extend_from_slice(&[0xAA, 0x55, 0xAA, 0x55]);
        tail.extend_from_slice(&frame_of(&[0x2A, 1, 2]));
        let seen = parse_in_chunks(&tail, chunk);
        assert_eq!(
            seen.last(),
            Some(&Seen::Frame(vec![0x2A, 1, 2])),
            "{bytes:02X?}"
        );
    }
}
