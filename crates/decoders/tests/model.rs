//! The annotation model and its validation (spec FR-006 to FR-009, data-model.md).
//!
//! Expected outcomes are written by hand from the data model, not computed by the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::model::{
    Annotation, Bounds, COLOURS, Item, Kind, MAX_ARG_BYTES, MAX_ARGS, MAX_PAYLOAD_BYTES,
    MAX_TEXT_BYTES, Payload, Refusal, Shape, Style, Target, Text,
};

const BOUNDS: Bounds = Bounds {
    length: 1_000,
    channels: 4,
};

fn style() -> Style {
    Style {
        colour: Some(2),
        shape: Shape::Dot,
    }
}

fn annotation(kind: Kind, target: Target, text: Text) -> Annotation {
    Annotation {
        kind,
        target,
        style: style(),
        text,
        error: false,
    }
}

fn plain(kind: Kind) -> Annotation {
    annotation(kind, Target::Lane, Text::Literal("x".into()))
}

fn message(args: Vec<String>) -> Text {
    Text::Message {
        key: "ann-test-message",
        args,
    }
}

#[test]
fn a_valid_annotation_of_every_kind_is_accepted() {
    for kind in [
        Kind::Range {
            start: 0,
            end: 1_000,
        },
        Kind::Range { start: 10, end: 10 },
        Kind::Point { at: 999 },
        Kind::Edge {
            at: 0,
            rising: true,
        },
        Kind::Edge {
            at: 500,
            rising: false,
        },
    ] {
        assert_eq!(plain(kind).validate(&BOUNDS), Ok(()), "{kind:?}");
    }
    // Overlays on every existing channel, and on the lane.
    for target in [Target::Lane, Target::Channel(0), Target::Channel(3)] {
        let a = annotation(Kind::Point { at: 5 }, target, Text::Literal("t".into()));
        assert_eq!(a.validate(&BOUNDS), Ok(()), "{target:?}");
    }
}

#[test]
fn positions_must_lie_inside_the_capture() {
    // A range is half open: it may end exactly at the length, not beyond it.
    assert_eq!(
        plain(Kind::Range {
            start: 0,
            end: 1_001
        })
        .validate(&BOUNDS),
        Err(Refusal::Position)
    );
    assert_eq!(
        plain(Kind::Range {
            start: 1_001,
            end: 1_001
        })
        .validate(&BOUNDS),
        Err(Refusal::Position)
    );
    // A point or an edge is a sample: the length itself is already outside.
    assert_eq!(
        plain(Kind::Point { at: 1_000 }).validate(&BOUNDS),
        Err(Refusal::Position)
    );
    assert_eq!(
        plain(Kind::Edge {
            at: u64::MAX,
            rising: true
        })
        .validate(&BOUNDS),
        Err(Refusal::Position)
    );
    // Nothing is valid in an empty capture except a zero-length range at 0.
    let empty = Bounds {
        length: 0,
        channels: 4,
    };
    assert_eq!(
        plain(Kind::Point { at: 0 }).validate(&empty),
        Err(Refusal::Position)
    );
    assert_eq!(
        plain(Kind::Range { start: 0, end: 0 }).validate(&empty),
        Ok(())
    );
}

#[test]
fn a_range_may_not_end_before_it_starts() {
    assert_eq!(
        plain(Kind::Range { start: 20, end: 19 }).validate(&BOUNDS),
        Err(Refusal::Order)
    );
    assert_eq!(
        plain(Kind::Range {
            start: u64::MAX,
            end: 0
        })
        .validate(&BOUNDS),
        Err(Refusal::Order)
    );
}

#[test]
fn an_overlay_must_target_an_existing_channel() {
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Channel(4),
        Text::Literal("t".into()),
    );
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::Target));
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Channel(255),
        Text::Literal("t".into()),
    );
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::Target));
}

#[test]
fn the_style_must_come_from_the_fixed_sets() {
    for colour in 0..COLOURS {
        let mut a = plain(Kind::Point { at: 1 });
        a.style.colour = Some(colour);
        assert_eq!(a.validate(&BOUNDS), Ok(()), "colour {colour}");
    }
    let mut a = plain(Kind::Point { at: 1 });
    a.style.colour = Some(COLOURS);
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::Style));
    a.style.colour = Some(255);
    // No colour is valid: the signal's colour is used.
    a.style.colour = None;
    assert_eq!(a.validate(&BOUNDS), Ok(()));
    a.style.colour = Some(255);
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::Style));
}

#[test]
fn the_text_is_limited() {
    let at_limit = "a".repeat(MAX_TEXT_BYTES);
    let a = annotation(Kind::Point { at: 1 }, Target::Lane, Text::Literal(at_limit));
    assert_eq!(a.validate(&BOUNDS), Ok(()));
    let over = "a".repeat(MAX_TEXT_BYTES + 1);
    let a = annotation(Kind::Point { at: 1 }, Target::Lane, Text::Literal(over));
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::TextLength));
    // Bytes count, not characters: 129 two-byte characters are 258 bytes.
    let wide = "é".repeat(129);
    let a = annotation(Kind::Point { at: 1 }, Target::Lane, Text::Literal(wide));
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::TextLength));
}

#[test]
fn message_arguments_are_limited_in_number_and_size() {
    let args = |n: usize, len: usize| (0..n).map(|_| "b".repeat(len)).collect::<Vec<_>>();
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Lane,
        message(args(MAX_ARGS, MAX_ARG_BYTES)),
    );
    assert_eq!(a.validate(&BOUNDS), Ok(()));
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Lane,
        message(args(MAX_ARGS + 1, 1)),
    );
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::Arguments));
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Lane,
        message(args(1, MAX_ARG_BYTES + 1)),
    );
    assert_eq!(a.validate(&BOUNDS), Err(Refusal::ArgumentLength));
}

#[test]
fn control_and_bidirectional_characters_are_removed_and_the_ends_trimmed() {
    let hostile = "  \u{202e}ev\u{0}il\u{202c}\n  ";
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Lane,
        Text::Literal(hostile.into()),
    )
    .sanitized();
    assert_eq!(a.text, Text::Literal("evil".into()));
    // Message arguments are outside text too.
    let a = annotation(
        Kind::Point { at: 1 },
        Target::Lane,
        message(vec!["\u{200b}0x5A\t".into()]),
    )
    .sanitized();
    assert_eq!(a.text, message(vec!["0x5A".into()]));
    // Sanitizing twice changes nothing.
    assert_eq!(a.clone().sanitized(), a);
}

#[test]
fn items_must_be_in_range_ordered_by_end_and_small() {
    let item = |start, end, payload| Item {
        kind: "byte",
        start,
        end,
        payload,
        error: false,
    };
    // A valid item and the previous end it is checked against.
    assert_eq!(item(0, 8, Payload::Integer(5)).validate(&BOUNDS, 0), Ok(()));
    assert_eq!(
        item(10, 10, Payload::Integer(0)).validate(&BOUNDS, 10),
        Ok(())
    );
    // An item ends where the capture ends at the latest.
    assert_eq!(
        item(0, 1_001, Payload::Integer(0)).validate(&BOUNDS, 0),
        Err(Refusal::Position)
    );
    assert_eq!(
        item(5, 4, Payload::Integer(0)).validate(&BOUNDS, 0),
        Err(Refusal::Order)
    );
    // The end never goes backwards (this is what gives an upper decoder its watermark).
    assert_eq!(
        item(0, 9, Payload::Integer(0)).validate(&BOUNDS, 10),
        Err(Refusal::ItemOrder)
    );
    // The payload is small.
    let big = Payload::Bytes(vec![0; MAX_PAYLOAD_BYTES + 1]);
    assert_eq!(item(0, 1, big).validate(&BOUNDS, 0), Err(Refusal::Payload));
    let ok = Payload::Bytes(vec![0; MAX_PAYLOAD_BYTES]);
    assert_eq!(item(0, 1, ok).validate(&BOUNDS, 0), Ok(()));
    let long_text = Payload::Text("t".repeat(MAX_PAYLOAD_BYTES + 1));
    assert_eq!(
        item(0, 1, long_text).validate(&BOUNDS, 0),
        Err(Refusal::Payload)
    );
}

#[test]
fn start_and_end_are_the_span_of_every_kind() {
    assert_eq!(plain(Kind::Range { start: 3, end: 9 }).span(), (3, 9));
    assert_eq!(plain(Kind::Point { at: 7 }).span(), (7, 7));
    assert_eq!(
        plain(Kind::Edge {
            at: 7,
            rising: false
        })
        .span(),
        (7, 7)
    );
}

#[test]
fn refusals_have_a_short_name_for_the_log() {
    // The names are logged (never the text of an annotation); they are stable identifiers.
    let names: Vec<&str> = [
        Refusal::Position,
        Refusal::Order,
        Refusal::Target,
        Refusal::Style,
        Refusal::TextLength,
        Refusal::Arguments,
        Refusal::ArgumentLength,
        Refusal::Payload,
        Refusal::ItemOrder,
    ]
    .iter()
    .map(|r| r.name())
    .collect();
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "{names:?}");
}

/// The block shapes of the lane bands (decoder API 1.2) are for ranges only.
#[test]
fn the_block_shapes_are_valid_on_a_range_and_refused_on_a_point_or_an_edge() {
    for shape in [Shape::Part, Shape::Whole] {
        let mut range = plain(Kind::Range { start: 2, end: 8 });
        range.style.shape = shape;
        assert_eq!(range.validate(&BOUNDS), Ok(()), "{shape:?}");
        let mut point = plain(Kind::Point { at: 1 });
        point.style.shape = shape;
        assert_eq!(point.validate(&BOUNDS), Err(Refusal::Style), "{shape:?}");
        let mut edge = plain(Kind::Edge {
            at: 1,
            rising: true,
        });
        edge.style.shape = shape;
        assert_eq!(edge.validate(&BOUNDS), Err(Refusal::Style), "{shape:?}");
        // A zero-length range is drawn as a point: it is accepted (the planner draws it as a mark).
        let mut empty = plain(Kind::Range { start: 3, end: 3 });
        empty.style.shape = shape;
        assert_eq!(empty.validate(&BOUNDS), Ok(()));
    }
}
