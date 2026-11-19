//! Placing markers with the mouse: the state machine of `data-model.md`
//! (`specs/015-markers-rework`, user stories 1 and 2). The pointer arithmetic is written out by
//! hand for three windows.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;

use common::{TempDir, finished_shell, wait_operation};
use la_host::markers::{Kind, MAX_MARKERS, Placement, Pointer, Shape};
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Outcome, Shell, ShellConfig, Unsaved, error,
};

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

/// A pointer over the column that shows `sample` of the whole capture of 1,000 samples on 800
/// pixels: 800 / 1000 pixels per sample, in the middle of the column.
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

fn shapes(shell: &Shell) -> Vec<Shape> {
    shell.state().markers.list.iter().map(|m| m.shape).collect()
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

#[test]
fn starting_a_placement_adds_nothing_and_needs_a_capture() {
    let dir = TempDir::new("place-none");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    ignored(&mut shell, Command::BeginPlacement(Kind::Single));
    assert_eq!(shell.state().placement, None);

    let (_dir, mut shell) = shell_with_capture("place-start");
    let before = shell.state().versions.markers;
    let changes = done(&mut shell, Command::BeginPlacement(Kind::Single));
    assert!(changes.contains(Changes::MARKERS));
    assert!(shell.state().versions.markers > before);
    assert_eq!(
        shell.state().placement,
        Some(Placement {
            kind: Kind::Single,
            first: None,
            colour: 0
        })
    );
    assert!(shell.state().markers.list.is_empty());
}

#[test]
fn the_same_kind_again_cancels_and_the_other_kind_switches() {
    let (_dir, mut shell) = shell_with_capture("place-toggle");
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    assert_eq!(shell.state().placement, None);
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    assert_eq!(shell.state().placement.unwrap().kind, Kind::Pair);
    done(&mut shell, Command::CancelPlacement);
    assert_eq!(shell.state().placement, None);
    ignored(&mut shell, Command::CancelPlacement);
    assert!(shell.state().markers.list.is_empty());
}

#[test]
fn a_click_fixes_a_single_marker_at_the_sample_under_the_pointer() {
    let (_dir, mut shell) = shell_with_capture("place-single");
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    let changes = done(&mut shell, Command::FixPlacement { pointer: over(250) });
    assert!(changes.contains(Changes::MARKERS));
    assert_eq!(shell.state().placement, None, "placement ends");
    assert_eq!(shapes(&shell), [Shape::Point(250)]);
    let marker = &shell.state().markers.list[0];
    assert_eq!((marker.colour, marker.comment.as_str()), (0, ""));
    assert_eq!(marker.name(), "T1");
    assert_eq!(shell.state().markers.selected, Some(marker.id));
    // The next one is T2, whatever happened to T1.
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::FixPlacement { pointer: over(600) });
    let first = shell.state().markers.list[0].id;
    done(&mut shell, Command::RemoveMarker(first));
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::FixPlacement { pointer: over(700) });
    assert_eq!(names(&shell), ["T2", "T3"]);
}

#[test]
fn the_pointer_means_the_sample_of_its_column_in_the_window_now_shown() {
    let (_dir, mut shell) = shell_with_capture("place-arithmetic");
    // The whole capture on 800 pixels: sample = floor(x * 1000 / 800).
    let at = |shell: &mut Shell, x: f32| {
        done(shell, Command::BeginPlacement(Kind::Single));
        done(
            shell,
            Command::FixPlacement {
                pointer: Pointer { x, lane: None },
            },
        );
        match shell.state().markers.list.last().unwrap().shape {
            Shape::Point(p) => p,
            Shape::Interval { .. } => panic!("a pair"),
        }
    };
    assert_eq!(at(&mut shell, 400.0), 500);
    assert_eq!(at(&mut shell, 0.0), 0);
    assert_eq!(at(&mut shell, 799.9), 999);
    // Zoomed to 200..400 on the same 800 pixels: sample = 200 + floor(x * 200 / 800).
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 200,
            end: 400,
        },
    );
    assert_eq!(at(&mut shell, 100.0), 225);
    assert_eq!(at(&mut shell, 799.9), 399);
    // Zoomed to 900..1000: sample = 900 + floor(x * 100 / 800).
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 900,
            end: 1_000,
        },
    );
    assert_eq!(at(&mut shell, 80.0), 910);
}

#[test]
fn a_pointer_beyond_the_view_or_not_a_number_fixes_at_the_nearest_end() {
    let (_dir, mut shell) = shell_with_capture("place-clamp");
    for (x, expected) in [
        (-50.0, 0),
        (f32::NAN, 0),
        (f32::NEG_INFINITY, 0),
        (5_000.0, 999),
        (f32::INFINITY, 999),
        (f32::MAX, 999),
    ] {
        done(&mut shell, Command::BeginPlacement(Kind::Single));
        done(
            &mut shell,
            Command::FixPlacement {
                pointer: Pointer { x, lane: None },
            },
        );
        assert_eq!(
            shell.state().markers.list.last().unwrap().shape,
            Shape::Point(expected),
            "x = {x}"
        );
        done(&mut shell, Command::RemoveAllMarkers);
    }
}

#[test]
fn a_click_without_a_placement_does_nothing() {
    let (_dir, mut shell) = shell_with_capture("place-stray");
    ignored(&mut shell, Command::FixPlacement { pointer: over(10) });
    assert!(shell.state().markers.list.is_empty());
}

#[test]
fn fixing_a_marker_makes_a_saved_capture_unsaved() {
    let (dir, mut shell) = shell_with_capture("place-dirty");
    let path = dir.path().join("a.lasession");
    done(&mut shell, Command::SaveSessionAs(path));
    wait_operation(&mut shell);
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    assert_eq!(
        shell.state().unsaved,
        Unsaved::Clean,
        "starting changes nothing worth saving"
    );
    done(&mut shell, Command::FixPlacement { pointer: over(10) });
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
}

#[test]
fn nothing_can_be_started_at_the_limit_or_while_saving() {
    let (dir, mut shell) = shell_with_capture("place-refuse");
    for i in 0..MAX_MARKERS {
        done(&mut shell, Command::BeginPlacement(Kind::Single));
        done(
            &mut shell,
            Command::FixPlacement {
                pointer: over(i as u64 * 10),
            },
        );
    }
    assert_eq!(shell.state().markers.list.len(), MAX_MARKERS);
    ignored(&mut shell, Command::BeginPlacement(Kind::Single));
    ignored(&mut shell, Command::BeginPlacement(Kind::Pair));
    assert_eq!(shell.state().placement, None);
    done(&mut shell, Command::RemoveAllMarkers);

    // While a save runs the capture cannot change, and neither can its markers.
    shell.set_session_fs(Arc::new(common::SlowFs));
    done(
        &mut shell,
        Command::SaveSessionAs(dir.path().join("slow.lasession")),
    );
    assert!(shell.state().operation.is_some(), "the save is running");
    assert!(matches!(
        shell.apply(Command::BeginPlacement(Kind::Single)),
        Outcome::Ignored(_)
    ));
    assert_eq!(shell.state().placement, None);
    wait_operation(&mut shell);
    done(&mut shell, Command::BeginPlacement(Kind::Single));
}

#[test]
fn a_marker_can_be_placed_while_a_capture_runs() {
    let dir = TempDir::new("place-running");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Channels(0b1),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(5_000_000),
        ConfigEdit::Seed(Some(3)),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    assert_eq!(shell.state().activity, Activity::Capturing);
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::FixPlacement { pointer: over(0) });
    assert_eq!(shell.state().markers.list.len(), 1);
    done(&mut shell, Command::StopCapture);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.state().activity != Activity::Idle {
        shell.poll(std::time::Instant::now());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn a_new_capture_or_an_opened_file_drops_a_placement_and_adds_nothing() {
    let (dir, mut shell) = shell_with_capture("place-drop");
    let path = dir.path().join("a.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(100) });
    assert!(shell.state().placement.is_some());
    done(&mut shell, Command::OpenSession(path));
    wait_operation(&mut shell);
    assert_eq!(shell.state().placement, None);
    assert!(shell.state().markers.list.is_empty());

    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(&mut shell, Command::NewCapture);
    if shell.state().pending_decision.is_some() {
        done(&mut shell, Command::Decide(Decision::Discard));
    }
    assert_eq!(shell.state().placement, None);
    assert!(shell.state().markers.list.is_empty());
}

// ---- pairs ------------------------------------------------------------------------------------

#[test]
fn the_first_click_of_a_pair_fixes_a_half_and_adds_nothing() {
    let (_dir, mut shell) = shell_with_capture("pair-first");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(300) });
    assert_eq!(
        shell.state().placement,
        Some(Placement {
            kind: Kind::Pair,
            first: Some(300),
            colour: 0
        })
    );
    assert!(shell.state().markers.list.is_empty());
    done(&mut shell, Command::FixPlacement { pointer: over(700) });
    assert_eq!(shell.state().placement, None);
    assert_eq!(
        shapes(&shell),
        [Shape::Interval {
            start: 300,
            end: 700
        }]
    );
    assert_eq!(names(&shell), ["P1"]);
    assert_eq!(
        shell.state().markers.selected,
        Some(shell.state().markers.list[0].id)
    );
}

#[test]
fn the_left_half_is_the_start_whichever_was_fixed_first() {
    let (_dir, mut shell) = shell_with_capture("pair-order");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(700) });
    done(&mut shell, Command::FixPlacement { pointer: over(300) });
    assert_eq!(
        shapes(&shell),
        [Shape::Interval {
            start: 300,
            end: 700
        }]
    );
}

#[test]
fn two_clicks_on_the_same_sample_make_a_pair_of_zero_duration() {
    let (_dir, mut shell) = shell_with_capture("pair-same");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(400) });
    done(&mut shell, Command::FixPlacement { pointer: over(400) });
    assert_eq!(
        shapes(&shell),
        [Shape::Interval {
            start: 400,
            end: 400
        }]
    );
    let texts = shell.marker_texts();
    assert!(texts[0].duration.is_some());
    assert_eq!(
        texts[0].frequency, None,
        "no frequency for a duration of zero"
    );
}

#[test]
fn cancelling_after_the_first_click_drops_it_and_adds_nothing() {
    let (_dir, mut shell) = shell_with_capture("pair-cancel");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(300) });
    done(&mut shell, Command::CancelPlacement);
    assert_eq!(shell.state().placement, None);
    assert!(shell.state().markers.list.is_empty());
    // The same kind again, after a first click, also cancels.
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(300) });
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    assert_eq!(shell.state().placement, None);
    assert!(shell.state().markers.list.is_empty());
}

#[test]
fn switching_kind_after_the_first_click_drops_the_half() {
    let (_dir, mut shell) = shell_with_capture("pair-switch");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    done(&mut shell, Command::FixPlacement { pointer: over(300) });
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    assert_eq!(
        shell.state().placement,
        Some(Placement {
            kind: Kind::Single,
            first: None,
            colour: 0
        })
    );
    done(&mut shell, Command::FixPlacement { pointer: over(500) });
    assert_eq!(shapes(&shell), [Shape::Point(500)]);
}

#[test]
fn pairs_and_singles_are_numbered_separately() {
    let (_dir, mut shell) = shell_with_capture("pair-numbers");
    for (kind, clicks) in [
        (Kind::Single, vec![100]),
        (Kind::Pair, vec![200, 300]),
        (Kind::Single, vec![400]),
        (Kind::Pair, vec![500, 600]),
    ] {
        done(&mut shell, Command::BeginPlacement(kind));
        for sample in clicks {
            done(
                &mut shell,
                Command::FixPlacement {
                    pointer: over(sample),
                },
            );
        }
    }
    assert_eq!(names(&shell), ["T1", "P1", "T2", "P2"]);
}
