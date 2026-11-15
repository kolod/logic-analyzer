//! The annotation store: window, rank and neighbour queries against a brute-force reference,
//! the shape of the index, snapshots, accounting and limits (spec FR-010, FR-011, SC-003).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::Lcg;
use la_decoders::model::{Annotation, Kind, Shape, Style, Target, Text};
use la_decoders::store::{Budget, Full, Limits, RUNS_MAX, Store, TAIL_MAX};

fn annotation(kind: Kind, error: bool) -> Annotation {
    Annotation {
        kind,
        target: Target::Lane,
        style: Style {
            colour: 1,
            shape: Shape::Dot,
        },
        text: Text::Literal("t".into()),
        error,
    }
}

/// A random annotation of any kind in `0..span`, with overlaps, nesting and equal positions.
fn random_annotation(rng: &mut Lcg, span: u64) -> Annotation {
    let start = rng.below(span);
    let kind = match rng.below(10) {
        0 | 1 => Kind::Point { at: start },
        2 => Kind::Edge {
            at: start,
            rising: rng.below(2) == 0,
        },
        3 => Kind::Range { start, end: start },
        4 => Kind::Range {
            start,
            end: start + rng.below(span / 2 + 1),
        },
        _ => Kind::Range {
            start,
            end: start + 1 + rng.below(20),
        },
    };
    annotation(kind, rng.below(7) == 0)
}

/// A random annotation that is at most 20 positions long.
fn short_annotation(rng: &mut Lcg, span: u64) -> Annotation {
    let start = rng.below(span);
    let kind = match rng.below(3) {
        0 => Kind::Point { at: start },
        1 => Kind::Range { start, end: start },
        _ => Kind::Range {
            start,
            end: start + 1 + rng.below(20),
        },
    };
    annotation(kind, false)
}

fn new_store() -> Store {
    Store::new(Limits::default(), Budget::new(1 << 40))
}

fn start_of(a: &Annotation) -> u64 {
    match a.kind {
        Kind::Range { start, .. } => start,
        Kind::Point { at } | Kind::Edge { at, .. } => at,
    }
}

/// The half-open interval an annotation occupies: points and zero-length ranges take their sample.
fn occupied(a: &Annotation) -> (u64, u64) {
    match a.kind {
        Kind::Range { start, end } => (start, end.max(start + 1)),
        Kind::Point { at } | Kind::Edge { at, .. } => (at, at + 1),
    }
}

fn brute_window(all: &[Annotation], from: u64, to: u64) -> Vec<u64> {
    let mut ids: Vec<u64> = all
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let (s, e) = occupied(a);
            from < to && s < to && e > from
        })
        .map(|(i, _)| i as u64)
        .collect();
    ids.sort_by_key(|id| (start_of(&all[*id as usize]), *id));
    ids
}

/// How the annotations are fed to the store.
#[derive(Debug, Clone, Copy)]
enum Order {
    AsGenerated,
    ByStart,
    ReverseStart,
}

fn ordered(mut all: Vec<Annotation>, order: Order) -> Vec<Annotation> {
    match order {
        Order::AsGenerated => {}
        Order::ByStart => all.sort_by_key(start_of),
        Order::ReverseStart => all.sort_by_key(|a| std::cmp::Reverse(start_of(a))),
    }
    all
}

fn filled(all: &[Annotation]) -> Store {
    let mut store = new_store();
    for (i, a) in all.iter().enumerate() {
        assert_eq!(store.insert(a.clone()).unwrap(), i as u64);
    }
    store
}

#[test]
fn window_rank_and_neighbours_match_brute_force_for_every_insertion_order() {
    let span = 50_000;
    for (seed, count, order) in [
        (1, 300, Order::AsGenerated),
        (2, 9_000, Order::AsGenerated),
        (3, 9_000, Order::ByStart),
        (4, 9_000, Order::ReverseStart),
        (5, 25_000, Order::AsGenerated),
    ] {
        let mut rng = Lcg(seed);
        let generated: Vec<Annotation> = (0..count)
            .map(|_| random_annotation(&mut rng, span))
            .collect();
        let all = ordered(generated, order);
        let mut store = filled(&all);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.len(), count);
        for _ in 0..120 {
            let a = rng.below(span + 100);
            let width = if rng.below(2) == 0 { 30 } else { span };
            let b = a + rng.below(width);
            let got: Vec<u64> = snapshot.window(a, b).iter().map(|e| e.id).collect();
            assert_eq!(got, brute_window(&all, a, b), "{order:?} window [{a},{b})");
            // Rank.
            let position = rng.below(span + 100);
            let rank = snapshot.rank(position, &mut 0);
            let below: Vec<&Annotation> = all.iter().filter(|x| start_of(x) < position).collect();
            assert_eq!(rank.count, below.len() as u64, "{order:?} rank {position}");
            assert_eq!(
                rank.errors,
                below.iter().filter(|x| x.error).count() as u64,
                "{order:?} errors {position}"
            );
            // Neighbours by start, ties by id.
            let next = all
                .iter()
                .enumerate()
                .filter(|(_, x)| start_of(x) > position)
                .min_by_key(|(i, x)| (start_of(x), *i))
                .map(|(i, _)| i as u64);
            assert_eq!(snapshot.next_after(position, &mut 0).map(|e| e.id), next);
            let prev = all
                .iter()
                .enumerate()
                .filter(|(_, x)| start_of(x) < position)
                .max_by_key(|(i, x)| (start_of(x), *i))
                .map(|(i, _)| i as u64);
            assert_eq!(snapshot.prev_before(position, &mut 0).map(|e| e.id), prev);
        }
        assert_eq!(
            snapshot.errors(),
            all.iter().filter(|a| a.error).count() as u64
        );
    }
}

#[test]
fn empty_windows_and_empty_stores_answer_nothing() {
    let mut store = new_store();
    let empty = store.snapshot();
    assert!(empty.is_empty());
    assert!(empty.window(0, 100).is_empty());
    assert_eq!(empty.rank(50, &mut 0).count, 0);
    assert!(empty.next_after(0, &mut 0).is_none());
    assert!(empty.prev_before(100, &mut 0).is_none());
    store
        .insert(annotation(Kind::Range { start: 1, end: 10 }, false))
        .unwrap();
    let snapshot = store.snapshot();
    assert!(
        snapshot.window(5, 5).is_empty(),
        "an empty window holds nothing"
    );
    assert!(
        snapshot.window(7, 3).is_empty(),
        "a reversed window holds nothing"
    );
    assert_eq!(snapshot.window(9, 10).len(), 1);
    assert!(snapshot.window(10, 11).is_empty(), "the end is not inside");
    assert!(
        snapshot.window(0, 1).is_empty(),
        "a window ending at the start does not reach it"
    );
}

#[test]
fn the_index_keeps_its_shape_and_compaction_gives_one_run() {
    let mut rng = Lcg(11);
    let mut store = new_store();
    for i in 0..120_000_u64 {
        store
            .insert(random_annotation(&mut rng, 1_000_000))
            .unwrap();
        assert!(store.tail_len() <= TAIL_MAX, "tail at {i}");
        assert!(store.run_count() <= RUNS_MAX, "runs at {i}");
    }
    assert!(
        store.run_count() > 1,
        "this many annotations need several runs"
    );
    let before = store.snapshot();
    let window_before: Vec<u64> = before
        .window(400_000, 410_000)
        .iter()
        .map(|e| e.id)
        .collect();
    store.compact();
    assert_eq!(store.run_count(), 1);
    assert_eq!(store.tail_len(), 0);
    // Compaction changes nothing a query can see.
    let after = store.snapshot();
    let window_after: Vec<u64> = after
        .window(400_000, 410_000)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(window_before, window_after);
    assert_eq!(before.len(), after.len());
    // An empty store compacts to nothing.
    let mut empty = new_store();
    empty.compact();
    assert_eq!(empty.run_count(), 0);
}

#[test]
fn a_snapshot_never_changes_when_more_is_inserted() {
    let mut rng = Lcg(21);
    let mut store = new_store();
    for _ in 0..5_000 {
        store.insert(random_annotation(&mut rng, 10_000)).unwrap();
    }
    let snapshot = store.snapshot();
    let ids: Vec<u64> = snapshot.window(0, 10_000).iter().map(|e| e.id).collect();
    let (len, bytes, errors) = (snapshot.len(), snapshot.bytes(), snapshot.errors());
    for _ in 0..30_000 {
        store.insert(random_annotation(&mut rng, 10_000)).unwrap();
    }
    store.compact();
    assert_eq!(
        snapshot
            .window(0, 10_000)
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        (snapshot.len(), snapshot.bytes(), snapshot.errors()),
        (len, bytes, errors)
    );
    // Asking twice without a change hands out the same snapshot.
    let a = store.snapshot();
    let b = store.snapshot();
    assert!(std::sync::Arc::ptr_eq(&a, &b));
}

#[test]
fn counting_and_bytes_follow_what_is_stored_and_the_budget_is_released() {
    let budget = Budget::new(1 << 30);
    let mut store = Store::new(Limits::default(), std::sync::Arc::clone(&budget));
    let mut previous = 0;
    for i in 0..50 {
        store
            .insert(annotation(Kind::Point { at: i }, i % 5 == 0))
            .unwrap();
        assert!(store.bytes() > previous, "every annotation costs bytes");
        previous = store.bytes();
    }
    assert_eq!(store.len(), 50);
    assert_eq!(budget.used(), store.bytes());
    // A longer text costs more.
    let mut long = annotation(Kind::Point { at: 1 }, false);
    long.text = Text::Literal("x".repeat(200));
    let before = store.bytes();
    store.insert(long).unwrap();
    assert!(store.bytes() - before > 200);
    drop(store);
    assert_eq!(budget.used(), 0, "dropping a store gives its memory back");
}

#[test]
fn the_annotation_count_limit_refuses_and_stores_nothing_more() {
    let budget = Budget::new(1 << 30);
    let mut store = Store::new(
        Limits {
            max_annotations: 10,
        },
        std::sync::Arc::clone(&budget),
    );
    for i in 0..10 {
        store
            .insert(annotation(Kind::Point { at: i }, false))
            .unwrap();
    }
    let bytes = store.bytes();
    assert_eq!(
        store.insert(annotation(Kind::Point { at: 11 }, false)),
        Err(Full::Count)
    );
    assert_eq!(
        (store.len(), store.bytes(), budget.used()),
        (10, bytes, bytes)
    );
    assert_eq!(store.snapshot().len(), 10);
}

#[test]
fn the_memory_budget_is_shared_and_refuses_at_the_limit() {
    let one = {
        let mut probe = new_store();
        probe
            .insert(annotation(Kind::Point { at: 0 }, false))
            .unwrap();
        probe.bytes()
    };
    // Room for exactly five annotations between two stores.
    let budget = Budget::new(one * 5);
    let mut a = Store::new(Limits::default(), std::sync::Arc::clone(&budget));
    let mut b = Store::new(Limits::default(), std::sync::Arc::clone(&budget));
    for i in 0..3 {
        a.insert(annotation(Kind::Point { at: i }, false)).unwrap();
    }
    for i in 0..2 {
        b.insert(annotation(Kind::Point { at: i }, false)).unwrap();
    }
    assert_eq!(
        a.insert(annotation(Kind::Point { at: 9 }, false)),
        Err(Full::Memory)
    );
    assert_eq!(
        b.insert(annotation(Kind::Point { at: 9 }, false)),
        Err(Full::Memory)
    );
    assert_eq!((a.len(), b.len()), (3, 2));
    // Raising the limit lets one carry on; releasing one store frees room for the other.
    drop(b);
    a.insert(annotation(Kind::Point { at: 9 }, false)).unwrap();
    budget.set_limit(one * 100);
    a.insert(annotation(Kind::Point { at: 10 }, false)).unwrap();
}

#[test]
fn a_window_query_costs_what_it_returns_not_what_the_store_holds() {
    // Two stores with the same ten annotations in the window and very different amounts
    // elsewhere: the number of probes must not grow with the rest.
    let cost = |others: u64| {
        let mut rng = Lcg(5);
        let mut store = new_store();
        for _ in 0..others {
            // Everything else lies far below or far above the window.
            let a = short_annotation(&mut rng, 900_000);
            store.insert(a).unwrap();
        }
        for i in 0..10 {
            store
                .insert(annotation(
                    Kind::Range {
                        start: 1_000_000 + i * 5,
                        end: 1_000_003 + i * 5,
                    },
                    false,
                ))
                .unwrap();
        }
        for _ in 0..others {
            let a = short_annotation(&mut rng, 900_000);
            let shifted = match a.kind {
                Kind::Range { start, end } => Kind::Range {
                    start: start + 2_000_000,
                    end: end + 2_000_000,
                },
                Kind::Point { at } => Kind::Point { at: at + 2_000_000 },
                Kind::Edge { at, rising } => Kind::Edge {
                    at: at + 2_000_000,
                    rising,
                },
            };
            store.insert(annotation(shifted, false)).unwrap();
        }
        let snapshot = store.snapshot();
        let mut probes = 0;
        let (found, truncated) =
            snapshot.window_limited(1_000_000, 1_000_100, usize::MAX, &mut probes);
        assert_eq!((found.len(), truncated), (10, false), "others {others}");
        probes
    };
    let small = cost(2_000);
    let large = cost(100_000);
    // Linear scanning would cost 200,000; the tree costs a few hundred at most. The larger
    // store may have a few more runs but never anything like 50 times the probes.
    // (The unindexed tail of at most 4,096 entries is scanned once per window query.)
    assert!(
        large < TAIL_MAX as u64 + 1_500,
        "{large} probes for 100,000 others"
    );
    assert!(large < small + 2_000, "small {small}, large {large}");
}

#[test]
fn rank_counts_cost_a_bounded_number_of_probes() {
    let mut rng = Lcg(8);
    let mut store = new_store();
    for _ in 0..150_000 {
        store
            .insert(random_annotation(&mut rng, 1_000_000))
            .unwrap();
    }
    let snapshot = store.snapshot();
    let mut probes = 0;
    let rank = snapshot.rank(500_000, &mut probes);
    assert!(rank.count > 50_000);
    // Every run and the sorted tail cost one binary search.
    assert!(probes < 16 * 24 + 32, "{probes}");
}

#[test]
fn a_limited_window_stops_and_says_so() {
    let mut store = new_store();
    for i in 0..10_000_u64 {
        store
            .insert(annotation(Kind::Point { at: i }, false))
            .unwrap();
    }
    let snapshot = store.snapshot();
    let (found, truncated) = snapshot.window_limited(0, 10_000, 100, &mut 0);
    assert_eq!(found.len(), 100);
    assert!(truncated);
    let (found, truncated) = snapshot.window_limited(0, 100, 100, &mut 0);
    assert_eq!((found.len(), truncated), (100, false));
}

#[test]
fn coverage_is_part_of_the_snapshot() {
    use la_decoders::store::Coverage;
    let mut store = new_store();
    let coverage = Coverage {
        covered: 500,
        incomplete: true,
        refused: 3,
    };
    store.set_coverage(coverage);
    assert_eq!(store.coverage(), coverage);
    assert_eq!(store.snapshot().coverage(), coverage);
    store.set_coverage(Coverage {
        covered: 900,
        ..coverage
    });
    assert_eq!(store.snapshot().coverage().covered, 900);
}
