//! The read-only signal reader over a storage snapshot (spec FR-005, FR-016).
//!
//! Expected edges and levels come from a brute-force pass over the sample words, not from the
//! storage queries the reader uses.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{Lcg, Part, capture_from_parts};
use la_decoders::input::{Edge, Level, SignalReader};
use la_storage::Gap;

const ROLES: [(&str, u8); 3] = [("clock", 0), ("data", 2), ("enable", 5)];

fn random_words(seed: u64, length: usize) -> Vec<u32> {
    let mut rng = Lcg(seed);
    let mut word = 0_u32;
    (0..length)
        .map(|_| {
            // Mostly quiet, with bursts: every channel flips with its own small probability.
            for channel in 0_u32..8 {
                if rng.below(6 + u64::from(channel)) == 0 {
                    word ^= 1 << channel;
                }
            }
            word
        })
        .collect()
}

/// The edges of the assigned roles in `[start, end)` by brute force.
fn brute_edges(words: &[u32], start: u64, end: u64) -> Vec<Edge> {
    let mut edges = Vec::new();
    for position in start.max(1)..end.min(words.len() as u64) {
        let (before, after) = (words[position as usize - 1], words[position as usize]);
        for (role, channel) in ROLES {
            let bit = 1_u32 << channel;
            if before & bit != after & bit {
                edges.push(Edge {
                    position,
                    role,
                    rising: after & bit != 0,
                });
            }
        }
    }
    edges
}

fn reader_for(words: Vec<u32>) -> (SignalReader, Vec<u32>) {
    let mut capture = capture_from_parts(8, 1_000_000, &[Part::Samples(words.clone())]);
    capture.close().unwrap();
    let reader = SignalReader::new(capture.reader().snapshot(), ROLES.to_vec());
    (reader, words)
}

#[test]
fn merged_edges_are_in_position_order_and_match_brute_force() {
    for seed in 1..=6 {
        let (reader, words) = reader_for(random_words(seed, 3_000));
        let all: Vec<Edge> = reader.edges(0, 3_000).collect();
        assert_eq!(all, brute_edges(&words, 0, 3_000), "seed {seed}");
        assert!(!all.is_empty());
        // Simultaneous edges come in the order of the roles (the brute force uses the same).
        assert!(all.windows(2).all(|w| w[0].position <= w[1].position));
    }
}

#[test]
fn an_edge_on_a_chunk_boundary_belongs_to_exactly_one_chunk() {
    let (reader, words) = reader_for(random_words(9, 2_000));
    for width in [1_u64, 2, 7, 64, 333, 2_000] {
        let mut seen = Vec::new();
        let mut start = 0;
        while start < 2_000 {
            let end = (start + width).min(2_000);
            let chunk: Vec<Edge> = reader.edges(start, end).collect();
            assert!(
                chunk
                    .iter()
                    .all(|e| e.position >= start && e.position < end)
            );
            seen.extend(chunk);
            start = end;
        }
        assert_eq!(seen, brute_edges(&words, 0, 2_000), "width {width}");
    }
}

#[test]
fn empty_and_reversed_ranges_give_no_edges() {
    let (reader, _) = reader_for(random_words(3, 500));
    assert_eq!(reader.edges(10, 10).count(), 0);
    assert_eq!(reader.edges(20, 10).count(), 0);
    assert_eq!(reader.edges(0, 0).count(), 0);
    assert_eq!(reader.edges(0, 1).count(), 0, "position 0 has no edge");
    assert_eq!(reader.edges(10_000, 20_000).count(), 0, "beyond the end");
}

#[test]
fn only_assigned_roles_are_reported() {
    let (reader, words) = reader_for(random_words(4, 1_000));
    let only_data = SignalReader::new(reader.snapshot().clone(), vec![("data", 2)]);
    let edges: Vec<Edge> = only_data.edges(0, 1_000).collect();
    assert!(edges.iter().all(|e| e.role == "data"));
    let expected: Vec<Edge> = brute_edges(&words, 0, 1_000)
        .into_iter()
        .filter(|e| e.role == "data")
        .collect();
    assert_eq!(edges, expected);
    assert!(only_data.role_assigned("data"));
    assert!(!only_data.role_assigned("clock"));
}

#[test]
fn levels_follow_the_samples_and_are_missing_where_there_is_no_data() {
    let words = random_words(5, 800);
    let (reader, words) = reader_for(words);
    for position in (0..800).step_by(13) {
        for (role, channel) in ROLES {
            let expected = if words[position] & (1 << channel) != 0 {
                Level::High
            } else {
                Level::Low
            };
            assert_eq!(
                reader.level(role, position as u64),
                expected,
                "{role} {position}"
            );
        }
    }
    // Beyond the snapshot, and a role nobody assigned.
    assert_eq!(reader.level("clock", 800), Level::Missing);
    assert_eq!(reader.level("clock", u64::MAX), Level::Missing);
    assert_eq!(reader.level("nothing", 5), Level::Missing);
}

#[test]
fn a_gap_is_missing_and_listed_per_chunk() {
    let mut rng = Lcg(2);
    let a: Vec<u32> = (0..100).map(|_| rng.below(256) as u32).collect();
    let b: Vec<u32> = (0..100).map(|_| rng.below(256) as u32).collect();
    let mut capture = capture_from_parts(
        8,
        1_000_000,
        &[Part::Samples(a), Part::Gap(50), Part::Samples(b)],
    );
    capture.close().unwrap();
    let reader = SignalReader::new(capture.reader().snapshot(), ROLES.to_vec());
    assert_eq!(reader.length(), 250);
    // Inside the gap: no data. Just before and after it: data.
    assert_eq!(reader.level("clock", 100), Level::Missing);
    assert_eq!(reader.level("clock", 149), Level::Missing);
    assert_ne!(reader.level("clock", 99), Level::Missing);
    assert_ne!(reader.level("clock", 150), Level::Missing);
    // The gap is listed for every chunk that intersects it, and only for those.
    let gap = Gap {
        start: 100,
        length: 50,
    };
    assert_eq!(reader.gaps_in(0, 100), vec![]);
    assert_eq!(reader.gaps_in(0, 101), vec![gap]);
    assert_eq!(reader.gaps_in(120, 130), vec![gap]);
    assert_eq!(reader.gaps_in(149, 250), vec![gap]);
    assert_eq!(reader.gaps_in(150, 250), vec![]);
}

#[test]
fn a_reader_sees_the_snapshot_it_was_made_from_not_later_data() {
    let first: Vec<u32> = (0..100).map(|i| u32::from(i % 2 == 0)).collect();
    let mut capture = capture_from_parts(8, 1_000_000, &[Part::Samples(first)]);
    let reader = SignalReader::new(capture.reader().snapshot(), vec![("clock", 0)]);
    assert_eq!(reader.length(), 100);
    common::append_parts(&mut capture, &[Part::Samples(vec![1; 100])]);
    // The old reader is unchanged; a new one sees the growth.
    assert_eq!(reader.length(), 100);
    assert_eq!(reader.level("clock", 150), Level::Missing);
    let newer = SignalReader::new(capture.reader().snapshot(), vec![("clock", 0)]);
    assert_eq!(newer.length(), 200);
    assert_eq!(newer.level("clock", 150), Level::High);
}

#[test]
fn a_channel_the_capture_does_not_have_is_missing_and_never_an_error() {
    let (reader, _) = reader_for(random_words(8, 100));
    let odd = SignalReader::new(reader.snapshot().clone(), vec![("clock", 20)]);
    assert_eq!(odd.level("clock", 5), Level::Missing);
    assert_eq!(odd.edges(0, 100).count(), 0);
}
