//! The commands that change a marker once it exists: dragging an end, selecting, going to a
//! marker, colour, comment, the time origin and the conversions
//! (`specs/015-markers-rework/contracts/host-api-2.0.md`). Expected values are written by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell, place_pair, place_single, wait_operation};
use la_host::markers::{End, MARKER_COLOURS, MAX_COMMENT_CHARS, Pointer, Shape};
use la_host::{Changes, Command, Outcome, Shell, Unsaved, error};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

fn ignored(shell: &mut Shell, command: Command) {
    assert_eq!(
        shell.apply(command),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
}

/// A pointer over `sample` of the whole capture of 1,000 samples on 800 pixels.
fn over(sample: u64) -> Pointer {
    Pointer {
        x: ((sample as f64 + 0.5) * 0.8) as f32,
        lane: None,
    }
}

fn shell_with_capture(name: &str) -> (TempDir, Shell) {
    let dir = TempDir::new(name);
    let shell = finished_shell(&dir, 0b1, 1_000, 1);
    (dir, shell)
}

fn shape_of(shell: &Shell, id: u32) -> Shape {
    shell.state().markers.get(id).unwrap().shape
}

fn names(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .markers
        .list
        .iter()
        .map(la_host::markers::Marker::name)
        .collect()
}

fn check(shell: &Shell) {
    let length = shell.state().capture.as_ref().unwrap().snapshot.length();
    shell.state().markers.check(length).unwrap();
}

// ---- dragging ---------------------------------------------------------------------------------

#[test]
fn dragging_a_single_marker_moves_it_and_the_end_of_the_drag_is_remembered() {
    let (dir, mut shell) = shell_with_capture("cmd-drag");
    let id = place_single(&mut shell, 100);
    done(
        &mut shell,
        Command::SaveSessionAs(dir.path().join("a.lasession")),
    );
    wait_operation(&mut shell);
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
    let before = shell.state().versions.markers;
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::Point,
            pointer: over(600),
        },
    );
    assert_eq!(shape_of(&shell, id), Shape::Point(600));
    assert_eq!(shell.state().drag, Some((id, End::Point)));
    assert!(shell.state().versions.markers > before);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    // Beyond the capture it is held at the end.
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::Point,
            pointer: Pointer {
                x: 99_999.0,
                lane: None,
            },
        },
    );
    assert_eq!(shape_of(&shell, id), Shape::Point(999));
    done(&mut shell, Command::EndMarkerDrag);
    assert_eq!(shell.state().drag, None);
    ignored(&mut shell, Command::EndMarkerDrag);
    check(&shell);
}

#[test]
fn a_drag_names_a_marker_and_an_end_that_exist() {
    let (_dir, mut shell) = shell_with_capture("cmd-drag-bad");
    let single = place_single(&mut shell, 100);
    let pair = place_pair(&mut shell, 200, 300);
    let pointer = over(500);
    for command in [
        Command::MoveMarkerEnd {
            id: 99,
            end: End::Point,
            pointer,
        },
        Command::MoveMarkerEnd {
            id: single,
            end: End::Start,
            pointer,
        },
        Command::MoveMarkerEnd {
            id: single,
            end: End::End,
            pointer,
        },
        Command::MoveMarkerEnd {
            id: pair,
            end: End::Point,
            pointer,
        },
    ] {
        ignored(&mut shell, command);
    }
    assert_eq!(shape_of(&shell, single), Shape::Point(100));
    assert_eq!(
        shape_of(&shell, pair),
        Shape::Interval {
            start: 200,
            end: 300
        }
    );
    assert_eq!(shell.state().drag, None);
}

#[test]
fn dragging_an_end_of_a_pair_past_the_other_swaps_the_roles_and_the_host_says_which() {
    let (_dir, mut shell) = shell_with_capture("cmd-drag-pair");
    let id = place_pair(&mut shell, 300, 700);
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::End,
            pointer: over(500),
        },
    );
    assert_eq!(
        shape_of(&shell, id),
        Shape::Interval {
            start: 300,
            end: 500
        }
    );
    assert_eq!(shell.state().drag, Some((id, End::End)));
    // The end that was dragged crosses the start: it is now the start.
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::End,
            pointer: over(100),
        },
    );
    assert_eq!(
        shape_of(&shell, id),
        Shape::Interval {
            start: 100,
            end: 300
        }
    );
    assert_eq!(shell.state().drag, Some((id, End::Start)));
    // The interface goes on dragging the end it now has.
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::Start,
            pointer: over(200),
        },
    );
    assert_eq!(
        shape_of(&shell, id),
        Shape::Interval {
            start: 200,
            end: 300
        }
    );
    assert_eq!(shell.state().markers.get(id).unwrap().name(), "P1");
    check(&shell);
}

// ---- removing and selecting -------------------------------------------------------------------

#[test]
fn removing_keeps_the_numbering_and_the_rest() {
    let (_dir, mut shell) = shell_with_capture("cmd-remove");
    let a = place_single(&mut shell, 100);
    place_pair(&mut shell, 200, 300);
    done(&mut shell, Command::RemoveMarker(a));
    assert_eq!(names(&shell), ["P1"]);
    ignored(&mut shell, Command::RemoveMarker(a));
    done(&mut shell, Command::RemoveAllMarkers);
    assert!(shell.state().markers.list.is_empty());
    assert_eq!(shell.state().markers.selected, None);
    place_single(&mut shell, 400);
    place_pair(&mut shell, 500, 600);
    assert_eq!(names(&shell), ["T2", "P2"]);
}

#[test]
fn removing_a_pair_removes_both_ends_at_once() {
    let (_dir, mut shell) = shell_with_capture("cmd-remove-pair");
    let id = place_pair(&mut shell, 200, 300);
    done(&mut shell, Command::RemoveMarker(id));
    assert!(shell.state().markers.list.is_empty());
}

#[test]
fn selecting_a_marker_checks_it_exists() {
    let (_dir, mut shell) = shell_with_capture("cmd-select");
    let a = place_single(&mut shell, 100);
    let b = place_single(&mut shell, 200);
    assert_eq!(
        shell.state().markers.selected,
        Some(b),
        "the new one is selected"
    );
    let changes = done(&mut shell, Command::SelectMarker(Some(a)));
    assert!(changes.contains(Changes::MARKERS));
    assert_eq!(shell.state().markers.selected, Some(a));
    ignored(&mut shell, Command::SelectMarker(Some(99)));
    assert_eq!(shell.state().markers.selected, Some(a));
    done(&mut shell, Command::SelectMarker(None));
    assert_eq!(shell.state().markers.selected, None);
}

// ---- scrolling to a marker --------------------------------------------------------------------

#[test]
fn scrolling_to_a_marker_centres_it_in_a_window_of_the_same_length() {
    let (_dir, mut shell) = shell_with_capture("cmd-scroll");
    let middle = place_single(&mut shell, 500);
    let near_start = place_single(&mut shell, 10);
    let near_end = place_single(&mut shell, 995);
    let pair = place_pair(&mut shell, 600, 800);
    let inside = place_single(&mut shell, 560);
    done(&mut shell, Command::ZoomToRange { start: 0, end: 100 });
    done(&mut shell, Command::SetFollowLatest(true));
    let changes = done(&mut shell, Command::ScrollToMarker(middle));
    assert!(changes.contains(Changes::WINDOW));
    let state = shell.state();
    assert_eq!((state.window.start, state.window.length), (450, 100));
    assert!(!state.auto_fit);
    assert!(!state.follow_latest);
    // Near the ends the window is held inside the capture.
    done(&mut shell, Command::ScrollToMarker(near_start));
    assert_eq!(shell.state().window.start, 0);
    done(&mut shell, Command::ScrollToMarker(near_end));
    assert_eq!(shell.state().window.start, 900);
    // A pair is centred on its start.
    done(&mut shell, Command::ScrollToMarker(pair));
    assert_eq!(shell.state().window.start, 550);
    assert_eq!(shell.state().window.length, 100, "the zoom does not change");
    // A marker that is already visible but off centre is centred too (not just revealed).
    done(&mut shell, Command::ScrollToMarker(inside));
    assert_eq!(shell.state().window.start, 510);
    ignored(&mut shell, Command::ScrollToMarker(99));
}

// ---- colour and comment -----------------------------------------------------------------------

#[test]
fn colour_and_comment_are_checked_and_never_translated() {
    let (_dir, mut shell) = shell_with_capture("cmd-look");
    let id = place_single(&mut shell, 100);
    done(
        &mut shell,
        Command::SetMarkerColour {
            id,
            colour: MARKER_COLOURS - 1,
        },
    );
    assert_eq!(
        shell.state().markers.get(id).unwrap().colour,
        MARKER_COLOURS - 1
    );
    ignored(
        &mut shell,
        Command::SetMarkerColour {
            id,
            colour: MARKER_COLOURS,
        },
    );
    ignored(&mut shell, Command::SetMarkerColour { id: 99, colour: 0 });
    done(
        &mut shell,
        Command::SetMarkerComment {
            id,
            comment: "  rise\u{202e}\u{0}d \n".into(),
        },
    );
    assert_eq!(shell.state().markers.get(id).unwrap().comment, "rised");
    done(
        &mut shell,
        Command::SetMarkerComment {
            id,
            comment: "x".repeat(MAX_COMMENT_CHARS + 20),
        },
    );
    assert_eq!(
        shell
            .state()
            .markers
            .get(id)
            .unwrap()
            .comment
            .chars()
            .count(),
        MAX_COMMENT_CHARS
    );
    ignored(
        &mut shell,
        Command::SetMarkerComment {
            id: 99,
            comment: "x".into(),
        },
    );
    // The name does not follow the language.
    let name = shell.state().markers.get(id).unwrap().name();
    for language in la_i18n::Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        assert_eq!(shell.state().markers.get(id).unwrap().name(), name);
        assert_eq!(shell.marker_texts()[0].name, "T1");
    }
}

// ---- the time origin --------------------------------------------------------------------------

#[test]
fn time_counts_from_a_single_marker_and_is_negative_before_it() {
    let (_dir, mut shell) = shell_with_capture("cmd-origin");
    let early = place_single(&mut shell, 100);
    let late = place_single(&mut shell, 500);
    let pair = place_pair(&mut shell, 300, 400);
    // The sample rate is 1 MHz: one sample is one microsecond.
    let texts = shell.marker_texts();
    assert!(texts[1].time_a.starts_with("500"), "{}", texts[1].time_a);
    ignored(&mut shell, Command::SetTimeOrigin(pair));
    ignored(&mut shell, Command::SetTimeOrigin(99));
    ignored(&mut shell, Command::ResetTimeOrigin);
    let before = shell.state().versions.capture;
    done(&mut shell, Command::SetTimeOrigin(early));
    assert_eq!(shell.state().markers.origin, 100);
    assert_eq!(shell.state().versions.capture, before, "no data changes");
    let texts = shell.marker_texts();
    let time = |id: u32| texts.iter().find(|t| t.id == id).unwrap().clone();
    assert!(
        time(early).time_a.starts_with('0'),
        "{}",
        time(early).time_a
    );
    assert!(
        time(late).time_a.starts_with("400"),
        "{}",
        time(late).time_a
    );
    // The duration of a pair does not depend on the origin.
    assert!(time(pair).duration.unwrap().starts_with("100"));
    assert!(
        time(pair).time_a.starts_with("200"),
        "{}",
        time(pair).time_a
    );
    // Before the origin the time is negative.
    done(&mut shell, Command::SetTimeOrigin(late));
    let texts = shell.marker_texts();
    let early_text = &texts.iter().find(|t| t.id == early).unwrap().time_a;
    assert!(early_text.starts_with("-400"), "{early_text}");
    done(&mut shell, Command::ResetTimeOrigin);
    assert_eq!(shell.state().markers.origin, 0);
    ignored(&mut shell, Command::ResetTimeOrigin);
}

// ---- the conversions --------------------------------------------------------------------------

#[test]
fn a_pair_is_split_into_two_singles_and_two_singles_become_a_pair() {
    let (_dir, mut shell) = shell_with_capture("cmd-convert");
    let a = place_single(&mut shell, 700);
    let b = place_single(&mut shell, 200);
    let pair = place_pair(&mut shell, 400, 500);
    done(
        &mut shell,
        Command::SetMarkerColour {
            id: pair,
            colour: 4,
        },
    );
    done(
        &mut shell,
        Command::SetMarkerComment {
            id: pair,
            comment: "bit".into(),
        },
    );
    done(&mut shell, Command::SplitPair(pair));
    assert_eq!(names(&shell), ["T1", "T2", "T3", "T4"]);
    let list = &shell.state().markers.list;
    assert_eq!(
        (list[2].shape, list[3].shape),
        (Shape::Point(400), Shape::Point(500))
    );
    assert_eq!((list[2].colour, list[2].comment.as_str()), (4, "bit"));
    ignored(&mut shell, Command::SplitPair(a));
    ignored(&mut shell, Command::SplitPair(99));

    let joined = {
        done(
            &mut shell,
            Command::MakePair {
                first: a,
                second: b,
            },
        );
        shell.state().markers.selected.unwrap()
    };
    assert_eq!(
        shape_of(&shell, joined),
        Shape::Interval {
            start: 200,
            end: 700
        }
    );
    assert_eq!(shell.state().markers.get(joined).unwrap().name(), "P2");
    assert_eq!(shell.state().markers.get(a), None);
    ignored(
        &mut shell,
        Command::MakePair {
            first: joined,
            second: 3,
        },
    );
    ignored(
        &mut shell,
        Command::MakePair {
            first: 3,
            second: 3,
        },
    );
    check(&shell);
}
