//! The `DANN` section against hand-written bytes of `contracts/deco-section.md` (spec US5, SC-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::model::{Annotation, Item, Kind, Payload, Shape, Style, Target, Text};
use la_decoders::persist::Rejection;
use la_decoders::persist::dann::{Block, Context, decode, encode};

// ---- an encoder of the contract, written for the tests ------------------------------------

fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let low = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(low);
            return out;
        }
        out.push(low | 0x80);
    }
}

fn zigzag(v: i64) -> Vec<u8> {
    varint(((v << 1) ^ (v >> 63)) as u64)
}

fn string(s: &str) -> Vec<u8> {
    let mut out = varint(s.len() as u64);
    out.extend_from_slice(s.as_bytes());
    out
}

fn record(content: &[u8]) -> Vec<u8> {
    let mut out = varint(content.len() as u64);
    out.extend_from_slice(content);
    out
}

const CONTEXT: Context<'static> = Context {
    capture_length: 1_000,
    channels: 4,
    instances: &[1, 2],
};

/// One block by hand: instance 1, covered 1,000, three annotations and two items.
fn block_bytes() -> Vec<u8> {
    let mut b = varint(1);
    b.extend(varint(5)); // setup version
    b.extend(varint(1_000)); // covered
    b.push(0); // flags
    b.extend(varint(3));
    b.extend(varint(2));
    // A range 10..40 in the lane, colour 3, shape Bar (2), message 0 with one argument.
    b.push(0);
    b.extend(varint(10));
    b.extend(varint(30));
    b.extend(varint(0));
    b.push(3 | (2 << 3));
    b.push(1);
    b.extend(varint(0));
    b.extend(varint(1));
    b.extend(string("0x5A"));
    // A rising edge at 25 (delta 15) on channel 2 (target 3), error, literal text.
    b.push(2);
    b.extend(varint(15));
    b.extend(varint(0));
    b.extend(varint(3));
    b.push(1 | (1 << 3) | (1 << 6));
    b.push(0);
    b.extend(string("clk"));
    // A point at 25 in the lane, a message with the same key and no arguments.
    b.push(1);
    b.extend(varint(0));
    b.extend(varint(0));
    b.extend(varint(0));
    b.push(0);
    b.push(1);
    b.extend(varint(0));
    b.extend(varint(0));
    // Two items: 10..40 and an overlapping 30..60 (start is a signed delta from the end 40).
    b.extend(string("byte"));
    b.extend(zigzag(10));
    b.extend(varint(30));
    b.push(0);
    b.push(0);
    b.extend(varint(0x5a));
    b.extend(string("byte"));
    b.extend(zigzag(-10));
    b.extend(varint(30));
    b.push(1);
    b.push(1);
    b.extend(varint(2));
    b.extend([9, 8]);
    b
}

fn payload(blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![1];
    out.extend(varint(1));
    out.extend(string("ann-test-byte"));
    out.extend(varint(blocks.len() as u64));
    for b in blocks {
        out.extend(record(b));
    }
    out
}

fn rejected(bytes: &[u8]) -> String {
    match decode(bytes, &CONTEXT) {
        Err(Rejection::Invalid(why)) => why.0.to_owned(),
        Err(Rejection::Newer) => "newer".to_owned(),
        Ok(_) => panic!("accepted"),
    }
}

/// The annotations of the hand-written block as the model has them.
fn annotation(
    kind: Kind,
    target: Target,
    colour: u8,
    shape: Shape,
    text: Text,
    error: bool,
) -> Annotation {
    Annotation {
        kind,
        target,
        style: Style {
            colour: Some(colour),
            shape,
        },
        text,
        error,
    }
}

#[test]
fn a_hand_written_block_is_read_as_the_contract_says() {
    let blocks = decode(&payload(&[block_bytes()]), &CONTEXT).unwrap();
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(
        (
            block.instance,
            block.setup_version,
            block.covered,
            block.incomplete
        ),
        (1, 5, 1_000, false)
    );
    let message = |args: Vec<String>| Text::Message {
        key: "ann-test-byte",
        args,
    };
    assert_eq!(
        block.annotations,
        vec![
            annotation(
                Kind::Range { start: 10, end: 40 },
                Target::Lane,
                3,
                Shape::Bar,
                message(vec!["0x5A".into()]),
                false
            ),
            annotation(
                Kind::Edge {
                    at: 25,
                    rising: true
                },
                Target::Channel(2),
                1,
                Shape::Arrow,
                Text::Literal("clk".into()),
                true
            ),
            annotation(
                Kind::Point { at: 25 },
                Target::Lane,
                0,
                Shape::Dot,
                message(vec![]),
                false
            ),
        ]
    );
    assert_eq!(
        block.items,
        vec![
            Item {
                kind: "byte",
                start: 10,
                end: 40,
                payload: Payload::Integer(0x5a),
                error: false
            },
            Item {
                kind: "byte",
                start: 30,
                end: 60,
                payload: Payload::Bytes(vec![9, 8]),
                error: true
            },
        ]
    );
    // The writer produces exactly these bytes.
    assert_eq!(encode(&blocks), payload(&[block_bytes()]));
}

#[test]
fn an_empty_and_a_maximal_state_round_trip() {
    assert_eq!(decode(&[1, 0, 0], &CONTEXT).unwrap(), vec![]);
    assert_eq!(encode(&[]), vec![1, 0, 0]);
    // Two instances, one with a thousand annotations and a few items.
    let annotations: Vec<Annotation> = (0..1_000_u64)
        .map(|i| {
            annotation(
                Kind::Point { at: i },
                Target::Lane,
                (i % 8) as u8,
                Shape::Dot,
                Text::Literal(String::new()),
                i % 97 == 0,
            )
        })
        .collect();
    let items: Vec<Item> = (0..50_u64)
        .map(|i| Item {
            kind: "byte",
            start: i * 10,
            end: i * 10 + 9,
            payload: Payload::Text("x".into()),
            error: false,
        })
        .collect();
    let blocks = vec![
        Block {
            instance: 1,
            setup_version: 1,
            covered: 1_000,
            incomplete: true,
            annotations,
            items,
        },
        Block {
            instance: 2,
            setup_version: 9,
            covered: 0,
            incomplete: false,
            annotations: vec![],
            items: vec![],
        },
    ];
    let bytes = encode(&blocks);
    assert_eq!(decode(&bytes, &CONTEXT).unwrap(), blocks);
    assert_eq!(
        encode(&decode(&bytes, &CONTEXT).unwrap()),
        bytes,
        "deterministic"
    );
}

#[test]
fn every_rule_of_the_contract_has_a_failing_fixture() {
    let with = |change: &dyn Fn(&mut Vec<u8>)| {
        let mut b = block_bytes();
        change(&mut b);
        payload(&[b])
    };
    // The version, framing, trailing bytes.
    assert_eq!(rejected(&[0]), "the version is not 1");
    assert_eq!(rejected(&[2]), "newer");
    let mut trailing = payload(&[block_bytes()]);
    trailing.push(0);
    assert_eq!(rejected(&trailing), "bytes remain after the last block");
    // A block for an instance DECO does not define; the same instance twice.
    assert_eq!(
        rejected(&with(&|b| b[0] = 9)),
        "a block names an instance DECO does not define"
    );
    assert_eq!(
        rejected(&payload(&[block_bytes(), block_bytes()])),
        "a block repeats an instance"
    );
    // A reserved bit in the flags (the byte after the three varints of the header).
    assert_eq!(rejected(&with(&|b| b[4] = 2)), "a reserved bit is set");
    // Counts larger than the block, before anything is allocated.
    assert_eq!(
        rejected(&with(&|b| {
            let bigger = [vec![1, 5, 0xe8, 0x07, 0], varint(4_000_000), varint(0)].concat();
            b.splice(0..8, bigger);
        })),
        "the counts are larger than the block"
    );
    // A string index out of the table; a reserved bit in the style; an unknown shape and kind.
    let message_at = block_bytes().iter().position(|x| *x == 1).unwrap();
    let _ = message_at;
    let broken_index = {
        let mut p = payload(&[block_bytes()]);
        let at = p.len() - block_bytes().len();
        let off = block_bytes()
            .windows(3)
            .position(|w| w == [1, 0, 1])
            .unwrap();
        p[at + off + 1] = 5;
        p
    };
    assert_eq!(rejected(&broken_index), "a string index is out of range");
    assert_eq!(
        rejected(&with(&|b| b[11] = 0x80 | 3)),
        "a reserved bit is set"
    );
    assert_eq!(rejected(&with(&|b| b[11] = 7 << 3)), "a shape is unknown");
    assert_eq!(
        rejected(&with(&|b| b[7] = 9)),
        "an annotation kind is unknown"
    );
    // Beyond the capture: a range ending past it, an edge on a channel the capture lacks.
    assert_eq!(
        rejected(&with(&|b| {
            b.splice(9..10, [0xe8, 0x07]);
        })),
        "an annotation does not fit the capture"
    );
    // A literal that is not in its sanitized form.
    let unsanitized = payload(&[{
        let mut b = block_bytes();
        let off = b.windows(3).position(|w| w == b"clk").unwrap();
        b[off] = 0x07;
        b
    }]);
    assert_eq!(
        rejected(&unsanitized),
        "a text is not in its sanitized form"
    );
    // The string table: a repeat, an entry never used.
    let mut repeated = vec![1];
    repeated.extend(varint(2));
    repeated.extend(string("a"));
    repeated.extend(string("a"));
    repeated.extend(varint(0));
    assert_eq!(rejected(&repeated), "a string repeats in the table");
    let mut unused = vec![1];
    unused.extend(varint(1));
    unused.extend(string("never-used"));
    unused.extend(varint(0));
    assert_eq!(
        rejected(&unused),
        "the string table is not in order of first use"
    );
    // The table longer than the payload.
    assert_eq!(
        rejected(&[1, 0xe8, 0x07]),
        "the string table is longer than the payload"
    );
    // A varint that is not canonical.
    assert_eq!(rejected(&[1, 0x80, 0x00]), "varint is not canonical");
}

#[test]
fn a_mutation_fuzzer_finds_nothing_and_what_is_accepted_writes_back_unchanged() {
    let bytes = payload(&[block_bytes()]);
    let check = |mutated: &[u8]| {
        if let Ok(blocks) = decode(mutated, &CONTEXT) {
            assert_eq!(
                encode(&blocks),
                mutated,
                "an accepted payload does not write back"
            );
        }
    };
    for at in 0..bytes.len() {
        for value in [
            0_u8,
            1,
            2,
            7,
            0x7f,
            0x80,
            0xff,
            bytes[at].wrapping_add(1),
            bytes[at] ^ 0x20,
        ] {
            let mut mutated = bytes.clone();
            mutated[at] = value;
            check(&mutated);
        }
    }
    for cut in 0..bytes.len() {
        check(&bytes[..cut]);
    }
    for extra in [vec![0], vec![0xff; 5]] {
        let mut longer = bytes.clone();
        longer.extend(extra);
        check(&longer);
    }
}

#[test]
#[ignore = "randomized: set LA_DECO_FUZZ_ITERATIONS (a million in the nightly run)"]
fn a_randomized_fuzzer_finds_nothing() {
    let iterations: u64 = std::env::var("LA_DECO_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);
    let base = payload(&[block_bytes()]);
    let mut state = 7_u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 16
    };
    for _ in 0..iterations {
        let mut bytes = base.clone();
        for _ in 0..=(next() % 4) {
            let at = (next() % bytes.len() as u64) as usize;
            bytes[at] = next() as u8;
        }
        if let Ok(blocks) = decode(&bytes, &CONTEXT) {
            assert_eq!(encode(&blocks), bytes);
        }
    }
}

#[test]
fn an_annotation_without_a_colour_round_trips_and_is_written_with_bit_seven() {
    let mut a = annotation(
        Kind::Point { at: 5 },
        Target::Channel(1),
        0,
        Shape::Dot,
        Text::Literal("x".into()),
        false,
    );
    a.style.colour = None;
    let block = Block {
        instance: 1,
        setup_version: 1,
        covered: 10,
        incomplete: false,
        annotations: vec![a.clone()],
        items: vec![],
    };
    let bytes = encode(std::slice::from_ref(&block));
    assert_eq!(decode(&bytes, &CONTEXT).unwrap(), vec![block]);
    // Bit 7 of the style byte means "the colour of the signal"; the colour bits are then zero.
    assert!(bytes.contains(&0x80), "{bytes:?}");
}

#[test]
fn the_block_shapes_and_the_error_flag_round_trip_and_shape_seven_is_refused() {
    let part = annotation(
        Kind::Range { start: 2, end: 9 },
        Target::Lane,
        4,
        Shape::Part,
        Text::Literal("a".into()),
        false,
    );
    let whole = annotation(
        Kind::Range { start: 1, end: 12 },
        Target::Lane,
        0,
        Shape::Whole,
        Text::Literal("b".into()),
        true,
    );
    let block = Block {
        instance: 1,
        setup_version: 1,
        covered: 20,
        incomplete: false,
        annotations: vec![whole, part],
        items: vec![],
    };
    let bytes = encode(std::slice::from_ref(&block));
    assert_eq!(decode(&bytes, &CONTEXT).unwrap(), vec![block.clone()]);
    // The two new shapes have the codes 5 and 6 in bits 3 to 5 of the style byte; code 7 is unknown.
    // The style byte is the one byte in which an encoding with a bar differs from one with a part.
    let mut with_bar = block.clone();
    with_bar.annotations[1].style.shape = Shape::Bar;
    let bar_bytes = encode(std::slice::from_ref(&with_bar));
    let part_bytes = encode(std::slice::from_ref(&block));
    let differing: Vec<usize> = (0..bar_bytes.len())
        .filter(|i| bar_bytes[*i] != part_bytes[*i])
        .collect();
    assert_eq!(differing.len(), 1, "{differing:?}");
    assert_eq!((part_bytes[differing[0]] >> 3) & 7, 5);
    let mut unknown = bar_bytes.clone();
    unknown[differing[0]] = (unknown[differing[0]] & !0x38) | (7 << 3);
    assert!(decode(&unknown, &CONTEXT).is_err());
}
