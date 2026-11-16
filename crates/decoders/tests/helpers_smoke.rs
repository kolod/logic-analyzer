//! The shared test helpers behave as documented (they are the basis of every other test).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::{Part, append_parts, capture_from_edges, capture_from_parts, words_from_edges};

#[test]
fn words_follow_the_edges() {
    let words = words_from_edges(10, &[(2, 0, true), (5, 1, true), (7, 0, false)]);
    assert_eq!(words, [0, 0, 1, 1, 1, 3, 3, 2, 2, 2]);
}

#[test]
fn a_capture_from_edges_has_those_transitions() {
    let capture = capture_from_edges(2, 1_000_000, 10, &[(2, 0, true), (7, 0, false)]);
    let snapshot = capture.reader().snapshot();
    assert_eq!(snapshot.length(), 10);
    let edges: Vec<(u64, bool)> = snapshot
        .transitions(0, 0, 10)
        .unwrap()
        .map(|e| (e.position, e.rising))
        .collect();
    assert_eq!(edges, [(2, true), (7, false)]);
}

#[test]
fn gaps_and_growth_are_supported() {
    let mut capture = capture_from_parts(
        1,
        1_000,
        &[
            Part::Samples(vec![0, 0]),
            Part::Gap(3),
            Part::Samples(vec![1]),
        ],
    );
    let before = capture.reader().snapshot();
    assert_eq!(before.length(), 6);
    assert_eq!(before.gaps().len(), 1);
    append_parts(&mut capture, &[Part::Samples(vec![1, 0])]);
    assert_eq!(capture.reader().snapshot().length(), 8);
    assert_eq!(before.length(), 6, "a snapshot never changes");
}
