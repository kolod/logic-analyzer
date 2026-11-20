//! The rules of single and pair markers as pure data operations (`specs/015-markers-rework`,
//! `data-model.md`): numbering, ordering, the limit, conversions, what a cut leaves, the time
//! origin and the two window helpers. Expected values are written by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_host::markers::{
    End, Kind, MARKER_COLOURS, MAX_COMMENT_CHARS, MAX_MARKERS, Markers, RemapReport, Shape,
};
use la_host::window::{ViewWindow, WindowLimits};

fn point(markers: &mut Markers, position: u64) -> u32 {
    markers.add(Shape::Point(position)).unwrap()
}

fn pair(markers: &mut Markers, a: u64, b: u64) -> u32 {
    markers.add(Shape::interval(a, b)).unwrap()
}

fn names(markers: &Markers) -> Vec<String> {
    markers
        .list
        .iter()
        .map(la_host::markers::Marker::name)
        .collect()
}

#[test]
fn numbers_are_per_kind_and_never_given_twice() {
    let mut m = Markers::new();
    let t1 = point(&mut m, 5);
    let _t2 = point(&mut m, 6);
    let _p1 = pair(&mut m, 1, 2);
    let _t3 = point(&mut m, 7);
    assert_eq!(names(&m), ["T1", "T2", "P1", "T3"]);
    // Ids are unique across kinds and start at 1.
    let ids: Vec<u32> = m.list.iter().map(|x| x.id).collect();
    assert_eq!(ids, [1, 2, 3, 4]);
    // A removed number is not given again.
    assert!(m.remove(t1));
    let t4 = point(&mut m, 8);
    assert_eq!(m.get(t4).unwrap().name(), "T4");
    // Clearing keeps the numbering too.
    m.clear();
    let t5 = point(&mut m, 9);
    assert_eq!(m.get(t5).unwrap().name(), "T5");
    let p2 = pair(&mut m, 3, 4);
    assert_eq!(m.get(p2).unwrap().name(), "P2");
    assert_eq!(m.get(t5).unwrap().kind(), Kind::Single);
    assert_eq!(m.get(p2).unwrap().kind(), Kind::Pair);
}

#[test]
fn the_limit_is_64_in_all_and_a_pair_counts_one() {
    let mut m = Markers::new();
    for i in 0..32 {
        point(&mut m, i);
        pair(&mut m, i, i + 1);
    }
    assert_eq!(m.list.len(), MAX_MARKERS);
    assert_eq!(m.add(Shape::Point(1)), None);
    assert_eq!(m.add(Shape::interval(1, 2)), None);
    assert!(m.remove(1));
    assert!(m.add(Shape::Point(1)).is_some());
}

#[test]
fn an_interval_is_always_in_order_and_the_first_marker_has_colour_0_and_no_comment() {
    assert_eq!(Shape::interval(9, 3), Shape::Interval { start: 3, end: 9 });
    assert_eq!(Shape::interval(3, 9), Shape::Interval { start: 3, end: 9 });
    let mut m = Markers::new();
    let id = pair(&mut m, 9, 3);
    let marker = m.get(id).unwrap();
    assert_eq!((marker.colour, marker.comment.as_str()), (0, ""));
    assert_eq!((marker.shape.start(), marker.shape.end()), (3, 9));
    assert_eq!(Shape::Point(4).start(), 4);
    assert_eq!(Shape::Point(4).end(), 4);
}

#[test]
fn each_new_marker_gets_the_next_colour_and_the_colours_start_again_after_the_last() {
    let mut m = Markers::new();
    let ids: Vec<u32> = (0..10).map(|i| point(&mut m, i)).collect();
    let colours: Vec<u8> = ids.iter().map(|id| m.get(*id).unwrap().colour).collect();
    assert_eq!(colours, [0, 1, 2, 3, 4, 5, 6, 7, 0, 1]);
    // A removed marker does not give its colour to the next one.
    m.remove(ids[9]);
    assert_eq!(m.next_colour(), 2);
}

#[test]
fn colour_and_comment_are_checked_and_sanitized() {
    let mut m = Markers::new();
    let id = point(&mut m, 1);
    assert!(m.set_colour(id, MARKER_COLOURS - 1));
    assert!(!m.set_colour(id, MARKER_COLOURS));
    assert!(!m.set_colour(99, 0));
    assert_eq!(m.get(id).unwrap().colour, MARKER_COLOURS - 1);

    assert!(m.set_comment(id, "  rising\u{202e}\u{0}edge \n"));
    assert_eq!(m.get(id).unwrap().comment, "risingedge");
    let long = "x".repeat(MAX_COMMENT_CHARS + 1);
    assert!(m.set_comment(id, &long));
    assert_eq!(
        m.get(id).unwrap().comment.chars().count(),
        MAX_COMMENT_CHARS
    );
    assert!(m.set_comment(id, "   \u{200b} "));
    assert_eq!(m.get(id).unwrap().comment, "");
    assert!(!m.set_comment(99, "x"));
}

#[test]
fn moving_an_end_past_the_other_swaps_the_roles_and_keeps_identity() {
    let mut m = Markers::new();
    let id = pair(&mut m, 10, 20);
    // Moving the start inside keeps the roles.
    assert_eq!(m.move_end(id, End::Start, 15), Some(End::Start));
    assert_eq!(
        m.get(id).unwrap().shape,
        Shape::Interval { start: 15, end: 20 }
    );
    // Moving the start past the end: the moved end is now the end.
    assert_eq!(m.move_end(id, End::Start, 30), Some(End::End));
    assert_eq!(
        m.get(id).unwrap().shape,
        Shape::Interval { start: 20, end: 30 }
    );
    // Moving the end before the start: the moved end is now the start.
    assert_eq!(m.move_end(id, End::End, 5), Some(End::Start));
    assert_eq!(
        m.get(id).unwrap().shape,
        Shape::Interval { start: 5, end: 20 }
    );
    // The same sample for both is allowed.
    assert_eq!(m.move_end(id, End::End, 5), Some(End::End));
    assert_eq!(
        m.get(id).unwrap().shape,
        Shape::Interval { start: 5, end: 5 }
    );
    assert_eq!(m.get(id).unwrap().name(), "P1");
    // The wrong end for the shape, and an unknown marker, change nothing.
    assert_eq!(m.move_end(id, End::Point, 1), None);
    let t = point(&mut m, 4);
    assert_eq!(m.move_end(t, End::Start, 1), None);
    assert_eq!(m.move_end(t, End::Point, 9), Some(End::Point));
    assert_eq!(m.get(t).unwrap().shape, Shape::Point(9));
    assert_eq!(m.move_end(99, End::Point, 1), None);
}

#[test]
fn the_origin_comes_from_a_single_marker_only() {
    let mut m = Markers::new();
    let t = point(&mut m, 40);
    let p = pair(&mut m, 10, 20);
    assert_eq!(m.origin, 0);
    assert!(!m.set_origin(p));
    assert!(!m.set_origin(99));
    assert!(m.set_origin(t));
    assert_eq!(m.origin, 40);
    // The origin is a point of the capture, not of a marker: removing the marker keeps it.
    assert!(m.remove(t));
    assert_eq!(m.origin, 40);
}

#[test]
fn splitting_a_pair_gives_two_new_single_numbers() {
    let mut m = Markers::new();
    point(&mut m, 1);
    point(&mut m, 2);
    let p = pair(&mut m, 10, 20);
    m.set_colour(p, 3);
    m.set_comment(p, "bit 3");
    m.selected = Some(p);
    let (a, b) = m.split(p).unwrap();
    assert_eq!(names(&m), ["T1", "T2", "T3", "T4"]);
    let (first, second) = (m.get(a).unwrap(), m.get(b).unwrap());
    assert_eq!(
        (first.shape, second.shape),
        (Shape::Point(10), Shape::Point(20))
    );
    assert_eq!((first.colour, second.colour), (3, 3));
    assert_eq!(
        (first.comment.as_str(), second.comment.as_str()),
        ("bit 3", "")
    );
    assert_eq!(m.selected, None);
    // The pair's number is not given again.
    let p2 = pair(&mut m, 1, 2);
    assert_eq!(m.get(p2).unwrap().name(), "P2");
    // A single marker cannot be split.
    assert_eq!(m.split(a), None);
    assert_eq!(m.split(99), None);
}

#[test]
fn a_pair_cannot_be_split_when_there_is_no_room_for_the_second_marker() {
    let mut m = Markers::new();
    for i in 0..63 {
        point(&mut m, i);
    }
    let p = pair(&mut m, 1, 2);
    assert_eq!(m.list.len(), 64);
    assert_eq!(m.split(p), None);
    assert_eq!(m.list.len(), 64);
    assert!(m.get(p).is_some());
}

#[test]
fn two_singles_become_one_pair_with_the_look_of_the_earlier() {
    let mut m = Markers::new();
    let a = point(&mut m, 30);
    let b = point(&mut m, 12);
    m.set_colour(a, 5);
    m.set_comment(a, "late");
    m.set_colour(b, 2);
    m.set_comment(b, "early");
    let id = m.join(a, b).unwrap();
    assert_eq!(names(&m), ["P1"]);
    let pair = m.get(id).unwrap();
    assert_eq!(pair.shape, Shape::Interval { start: 12, end: 30 });
    assert_eq!((pair.colour, pair.comment.as_str()), (2, "early"));
    assert_eq!(m.selected, Some(id));
    // The numbers of the two singles are not given again.
    let t = point(&mut m, 1);
    assert_eq!(m.get(t).unwrap().name(), "T3");
}

#[test]
fn only_two_different_singles_can_be_joined() {
    let mut m = Markers::new();
    let a = point(&mut m, 1);
    let b = point(&mut m, 2);
    let p = pair(&mut m, 3, 4);
    assert_eq!(m.join(a, a), None);
    assert_eq!(m.join(a, p), None);
    assert_eq!(m.join(p, b), None);
    assert_eq!(m.join(a, 99), None);
    assert_eq!(m.list.len(), 3);
}

#[test]
fn a_cut_keeps_the_inside_shifts_it_and_drops_half_pairs() {
    let mut m = Markers::new();
    let before = point(&mut m, 5);
    let on_edge = point(&mut m, 10);
    let inside = point(&mut m, 25);
    let after = point(&mut m, 40);
    let whole = pair(&mut m, 12, 30);
    let half_left = pair(&mut m, 2, 15);
    let half_right = pair(&mut m, 20, 50);
    let outside = pair(&mut m, 1, 8);
    m.origin = 25;
    m.selected = Some(before);
    let report = m.remap(10, 40);
    assert_eq!(
        report,
        RemapReport {
            removed: 5,
            split_pairs: 2
        }
    );
    assert!(m.get(before).is_none() && m.get(after).is_none());
    assert!(m.get(half_left).is_none() && m.get(half_right).is_none() && m.get(outside).is_none());
    assert_eq!(m.get(on_edge).unwrap().shape, Shape::Point(0));
    assert_eq!(m.get(inside).unwrap().shape, Shape::Point(15));
    assert_eq!(
        m.get(whole).unwrap().shape,
        Shape::Interval { start: 2, end: 20 }
    );
    assert_eq!(m.origin, 15);
    assert_eq!(m.selected, None);
    // The numbering goes on.
    let t = point(&mut m, 3);
    assert_eq!(m.get(t).unwrap().name(), "T5");
}

#[test]
fn the_origin_goes_back_to_the_start_when_it_is_cut_away() {
    let mut m = Markers::new();
    m.origin = 3;
    m.remap(10, 20);
    assert_eq!(m.origin, 0);
    m.origin = 20;
    m.remap(10, 20);
    assert_eq!(m.origin, 0, "the end of the range is not inside it");
    m.origin = 12;
    m.remap(10, 20);
    assert_eq!(m.origin, 2);
}

#[test]
fn markers_are_ordered_by_their_start_then_by_id() {
    let mut m = Markers::new();
    let a = point(&mut m, 50);
    let b = pair(&mut m, 10, 90);
    let c = point(&mut m, 10);
    let d = point(&mut m, 30);
    let order: Vec<u32> = m.ordered().iter().map(|x| x.id).collect();
    assert_eq!(order, [b, c, d, a]);
}

#[test]
fn jumping_goes_to_the_next_and_previous_start_and_wraps_round() {
    let mut m = Markers::new();
    assert_eq!(m.jump_target(5, true), None);
    let a = point(&mut m, 10);
    let b = pair(&mut m, 20, 80);
    let c = point(&mut m, 50);
    assert_eq!(m.jump_target(0, true), Some(a));
    assert_eq!(m.jump_target(10, true), Some(b), "strictly after");
    assert_eq!(
        m.jump_target(20, true),
        Some(c),
        "a pair counts at its start"
    );
    assert_eq!(m.jump_target(50, true), Some(a), "wraps to the first");
    assert_eq!(m.jump_target(60, false), Some(c));
    assert_eq!(m.jump_target(50, false), Some(b));
    assert_eq!(m.jump_target(10, false), Some(c), "wraps to the last");
}

#[test]
fn selecting_checks_the_marker_exists() {
    let mut m = Markers::new();
    let a = point(&mut m, 1);
    assert!(m.select(Some(a)));
    assert_eq!(m.selected, Some(a));
    assert!(!m.select(Some(99)));
    assert_eq!(m.selected, Some(a));
    assert!(m.select(None));
    assert_eq!(m.selected, None);
    m.select(Some(a));
    m.remove(a);
    assert_eq!(m.selected, None);
}

#[test]
fn restore_raises_the_counters_above_what_is_in_use() {
    let mut m = Markers::new();
    point(&mut m, 1);
    pair(&mut m, 2, 3);
    point(&mut m, 4);
    let list = m.list.clone();
    // A section that stored counters that are too low cannot make a number repeat.
    let mut restored = Markers::restore(list, None, 0, 0, [1, 1]);
    assert_eq!(restored.last_id(), 3);
    assert_eq!(restored.next_numbers(), [3, 2]);
    let t = restored.add(Shape::Point(9)).unwrap();
    assert_eq!(restored.get(t).unwrap().name(), "T3");
    assert_eq!(t, 4);
}

#[test]
fn check_accepts_what_the_rules_make_and_names_each_broken_invariant() {
    let mut m = Markers::new();
    point(&mut m, 5);
    pair(&mut m, 1, 9);
    m.origin = 3;
    m.selected = Some(1);
    assert_eq!(m.check(10), Ok(()));
    assert_eq!(
        m.check(0),
        Err("T1 is beyond the capture".to_owned()),
        "length 0 allows position 0 only"
    );

    let mut broken = m.clone();
    broken.list[1].id = 1;
    assert!(broken.check(10).unwrap_err().contains("id 1 repeats"));

    let mut broken = m.clone();
    broken.list[1].number = 1;
    broken.list[1].shape = Shape::Point(2);
    broken.list.push(broken.list[0].clone());
    broken.list[2].id = 2;
    assert!(broken.check(10).is_err());

    let mut broken = m.clone();
    broken.list[0].shape = Shape::Point(10);
    assert!(broken.check(10).unwrap_err().contains("beyond"));

    let mut broken = m.clone();
    broken.list[1].shape = Shape::Interval { start: 8, end: 2 };
    assert!(broken.check(10).unwrap_err().contains("in order"));

    let mut broken = m.clone();
    broken.origin = 10;
    assert!(broken.check(10).unwrap_err().contains("origin"));

    let mut broken = m.clone();
    broken.selected = Some(77);
    assert!(broken.check(10).unwrap_err().contains("selected"));

    let mut broken = m.clone();
    broken.list[0].colour = MARKER_COLOURS;
    assert!(broken.check(10).unwrap_err().contains("colour"));
}

#[test]
fn centring_a_window_is_clamped_to_the_capture() {
    let limits = WindowLimits {
        capture_length: 1000,
        pixels: 800,
    };
    let window = ViewWindow {
        start: 0,
        length: 100,
    };
    assert_eq!(
        window.centre_on(&limits, 500),
        ViewWindow {
            start: 450,
            length: 100
        }
    );
    assert_eq!(
        window.centre_on(&limits, 10),
        ViewWindow {
            start: 0,
            length: 100
        }
    );
    assert_eq!(
        window.centre_on(&limits, 999),
        ViewWindow {
            start: 900,
            length: 100
        }
    );
    assert_eq!(
        window.centre_on(&limits, 50),
        ViewWindow {
            start: 0,
            length: 100
        }
    );
}

#[test]
fn fitting_a_range_leaves_the_margin_in_pixels_on_both_sides() {
    let limits = WindowLimits {
        capture_length: 100_000,
        pixels: 800,
    };
    // 1000 samples between the marks, a margin of 16 px at 800 px: the window is
    // ceil(1000 * 800 / 768) = 1042 samples, 21 of them on each side (16.1 px).
    let window = ViewWindow::fit_range(&limits, 1000, 2000, 16);
    assert_eq!(
        window,
        ViewWindow {
            start: 979,
            length: 1042
        }
    );
    // The ends are given in either order.
    assert_eq!(ViewWindow::fit_range(&limits, 2000, 1000, 16), window);
    // At the start of the capture the left margin is what the capture has.
    let at_start = ViewWindow::fit_range(&limits, 5, 1005, 16);
    assert_eq!(
        at_start,
        ViewWindow {
            start: 0,
            length: 1042
        }
    );
    // At the end of the capture the window is moved back inside it.
    let at_end = ViewWindow::fit_range(&limits, 99_000, 99_999, 16);
    assert_eq!(at_end.end(), 100_000);
    assert!(at_end.is_valid(&limits));
    // A zero-length range and a margin that leaves no room fall back to the zoom limit.
    let zero = ViewWindow::fit_range(&limits, 500, 500, 16);
    assert_eq!(zero.length, limits.min_length());
    assert!(zero.is_valid(&limits));
    let no_room = ViewWindow::fit_range(&limits, 1000, 2000, 400);
    assert_eq!(no_room, ViewWindow::zoom_to(&limits, 1000, 2000));
}
