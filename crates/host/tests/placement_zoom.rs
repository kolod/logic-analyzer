//! The automatic zoom of a crowded snapping zone and the way the view comes back
//! (`specs/015-markers-rework`, research R4, user story 9, FR-034). Expected values of the hand
//! case are worked out by hand from the rule in the contract.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{Lcg, TempDir, shell_showing, snapshot_with_edges};
use la_host::markers::{End, Kind, Pointer, Shape, ZoomTarget};
use la_host::{Command, Outcome, Shell, error};

/// 12,000 samples on 800 pixels: 15 samples to a pixel; the snapping distance is 180 samples.
const LENGTH: u64 = 12_000;

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn ignored(shell: &mut Shell, command: Command) {
    assert_eq!(
        shell.apply(command),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
}

fn hand_case(name: &str) -> (TempDir, Shell) {
    let dir = TempDir::new(name);
    let snapshot = snapshot_with_edges(
        &[false, false],
        &[vec![1_000, 1_002, 1_005, 1_006, 1_010], vec![]],
        LENGTH,
    );
    let shell = shell_showing(&dir, snapshot, 2);
    (dir, shell)
}

fn pointer(x: f32, lane: Option<u8>) -> Pointer {
    Pointer { x, lane }
}

fn whole_view(shell: &Shell) {
    let state = shell.state();
    assert!(state.auto_fit, "the whole capture is shown again");
    assert_eq!(state.snap_zoom, None);
}

#[test]
fn the_target_for_a_crowded_zone_is_worked_out_by_hand() {
    let (_dir, shell) = hand_case("zoom-hand");
    // Pointer at x = 67: sample floor(67 * 15) = 1,005. The nearest edges are 1,000, 1,002, 1,005
    // and 1,006; the smallest gap is 1, so the transitions are 24 pixels apart when 800 pixels show
    // ceil(1 * 800 / 24) = 34 samples. The window would start at 1,005 - floor(67 * 34 / 800) =
    // 1,003, but the pointer's column (15 samples, 7 either side of 1,005) must fit: the start is
    // held to at most 1,005 - 7 - 1 = 997 (and at least 1,005 + 7 + 1 - 34 = 979).
    let target = shell.crowded_zoom_target(pointer(67.0, Some(0)));
    assert_eq!(
        target,
        Some(ZoomTarget {
            start: 997,
            length: 34
        })
    );
    // The same zone is crowded for the host.
    assert!(shell.resolve_pointer(pointer(67.0, Some(0))).crowded);
}

#[test]
fn there_is_no_target_when_the_zone_is_not_crowded_or_nothing_snaps() {
    let (_dir, mut shell) = hand_case("zoom-none");
    // Another lane: no edges.
    assert_eq!(shell.crowded_zoom_target(pointer(67.0, Some(1))), None);
    // No lane (the ruler, the legend, a decoder lane).
    assert_eq!(shell.crowded_zoom_target(pointer(67.0, None)), None);
    // Far from every edge.
    assert_eq!(shell.crowded_zoom_target(pointer(400.0, Some(0))), None);
    // Snapping off.
    done(&mut shell, Command::SetSnap(false));
    assert_eq!(shell.crowded_zoom_target(pointer(67.0, Some(0))), None);
}

#[test]
fn the_view_only_ever_zooms_in() {
    let (_dir, mut shell) = hand_case("zoom-in-only");
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 990,
            end: 1_020,
        },
    );
    // 30 samples are shown, and the target would be 34: already closer.
    assert_eq!(shell.crowded_zoom_target(pointer(400.0, Some(0))), None);
}

fn random_clock(rng: &mut Lcg) -> Vec<u64> {
    let mut at = 1_000;
    let mut edges = Vec::new();
    while at < 7_000 {
        edges.push(at);
        at += 1 + rng.below(3);
    }
    edges
}

#[test]
fn any_of_a_thousand_edges_one_to_three_samples_apart_can_be_picked_exactly_and_the_view_comes_back()
 {
    let mut rng = Lcg(0xBEEF);
    let edges = random_clock(&mut rng);
    let dir = TempDir::new("zoom-thousand");
    let snapshot = snapshot_with_edges(&[false], std::slice::from_ref(&edges), LENGTH);
    let mut shell = shell_showing(&dir, snapshot, 1);
    for round in 0..1_000 {
        let wanted = edges[1 + usize::try_from(rng.below(edges.len() as u64 - 2)).unwrap()];
        // The pointer is over the column that holds the edge (15 samples to a pixel).
        let column = wanted * 800 / LENGTH;
        let over = pointer(column as f32 + 0.5, Some(0));
        assert!(
            shell.resolve_pointer(over).crowded,
            "round {round}: {wanted}"
        );
        let target = shell
            .crowded_zoom_target(over)
            .unwrap_or_else(|| panic!("round {round}: no target for {wanted}"));
        done(&mut shell, Command::BeginPlacement(Kind::Single));
        done(
            &mut shell,
            Command::PlacementZoomTo {
                start: target.start,
                length: target.length,
            },
        );
        let window = shell.state().window;
        assert!(
            window.start <= wanted && wanted < window.start + window.length,
            "round {round}: {wanted} is not in {window:?}"
        );
        // The pointer is moved onto the wanted edge (to the nearest pixel) and the click fixes it.
        let x = ((wanted - window.start) as f64 + 0.5) * 800.0 / window.length as f64;
        done(
            &mut shell,
            Command::FixPlacement {
                pointer: pointer(x.round() as f32, Some(0)),
            },
        );
        let marker = shell.state().markers.list.last().unwrap().shape;
        assert_eq!(marker, Shape::Point(wanted), "round {round}");
        whole_view(&shell);
        done(&mut shell, Command::RemoveAllMarkers);
    }
}

// ---- the saved view ---------------------------------------------------------------------------

#[test]
fn a_frame_of_the_zoom_needs_a_placement_or_a_drag_and_saves_the_view_once() {
    let (_dir, mut shell) = hand_case("zoom-save");
    ignored(
        &mut shell,
        Command::PlacementZoomTo {
            start: 997,
            length: 34,
        },
    );
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(
        &mut shell,
        Command::PlacementZoomTo {
            start: 900,
            length: 400,
        },
    );
    let state = shell.state();
    assert_eq!((state.window.start, state.window.length), (900, 400));
    assert!(!state.auto_fit);
    assert!(!state.follow_latest);
    let saved = state.snap_zoom.expect("the view is saved");
    // A second frame does not save the first frame's window over the original.
    done(
        &mut shell,
        Command::PlacementZoomTo {
            start: 997,
            length: 34,
        },
    );
    assert_eq!(shell.state().snap_zoom, Some(saved));
    // The frames are clamped like any window: nothing is shorter than the zoom limit allows.
    done(
        &mut shell,
        Command::PlacementZoomTo {
            start: 5,
            length: 1,
        },
    );
    assert!(shell.state().window.length >= 800_u64.div_ceil(64));
}

#[test]
fn fixing_cancelling_starting_again_and_ending_a_drag_each_bring_the_view_back() {
    let zoomed = |shell: &mut Shell| {
        done(
            shell,
            Command::PlacementZoomTo {
                start: 997,
                length: 34,
            },
        );
        assert_ne!(shell.state().window.length, LENGTH);
    };
    // A click.
    let (_dir, mut shell) = hand_case("zoom-fix");
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    zoomed(&mut shell);
    done(
        &mut shell,
        Command::FixPlacement {
            pointer: pointer(100.0, None),
        },
    );
    whole_view(&shell);
    assert_eq!(shell.state().window.length, LENGTH);
    // Escape.
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    zoomed(&mut shell);
    done(&mut shell, Command::CancelPlacement);
    whole_view(&shell);
    // The same button again, and the other one.
    for next in [Kind::Single, Kind::Pair] {
        done(&mut shell, Command::BeginPlacement(Kind::Single));
        zoomed(&mut shell);
        done(&mut shell, Command::BeginPlacement(next));
        whole_view(&shell);
        if shell.state().placement.is_some() {
            done(&mut shell, Command::CancelPlacement);
        }
    }
    // A drag of a marker that is already fixed.
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(
        &mut shell,
        Command::FixPlacement {
            pointer: pointer(100.0, None),
        },
    );
    let id = shell.state().markers.list[0].id;
    done(
        &mut shell,
        Command::MoveMarkerEnd {
            id,
            end: End::Point,
            pointer: pointer(120.0, Some(0)),
        },
    );
    zoomed(&mut shell);
    done(&mut shell, Command::EndMarkerDrag);
    whole_view(&shell);
    assert_eq!(shell.state().window.length, LENGTH);
}

#[test]
fn a_view_that_was_not_the_whole_capture_comes_back_as_it_was() {
    let (_dir, mut shell) = hand_case("zoom-back");
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 2_000,
            end: 6_000,
        },
    );
    let before = shell.state().window;
    done(&mut shell, Command::BeginPlacement(Kind::Single));
    done(
        &mut shell,
        Command::PlacementZoomTo {
            start: 997,
            length: 34,
        },
    );
    done(&mut shell, Command::CancelPlacement);
    let state = shell.state();
    assert_eq!(state.window, before);
    assert!(!state.auto_fit);
    assert_eq!(state.snap_zoom, None);
}

#[test]
fn a_zoom_or_scroll_by_hand_takes_the_view_over_and_it_is_not_put_back() {
    for by_hand in [
        Command::GoToEnd,
        Command::GoToStart,
        Command::FitAll,
        Command::ZoomToRange {
            start: 3_000,
            end: 4_000,
        },
        Command::Pan { pixels: 50 },
        Command::PanStep(la_host::PanStep::Right),
    ] {
        let name =
            format!("zoom-hand-{by_hand:?}").replace(|c: char| !c.is_ascii_alphanumeric(), "");
        let (_dir, mut shell) = hand_case(&name);
        done(&mut shell, Command::BeginPlacement(Kind::Single));
        done(
            &mut shell,
            Command::PlacementZoomTo {
                start: 997,
                length: 34,
            },
        );
        shell.apply(by_hand.clone());
        assert_eq!(shell.state().snap_zoom, None, "{by_hand:?}");
        let by_hand_window = shell.state().window;
        done(
            &mut shell,
            Command::FixPlacement {
                pointer: pointer(100.0, None),
            },
        );
        assert_eq!(shell.state().window, by_hand_window, "{by_hand:?}");
    }
}

#[test]
fn a_pair_gets_its_view_back_after_each_click() {
    let (_dir, mut shell) = hand_case("zoom-pair");
    done(&mut shell, Command::BeginPlacement(Kind::Pair));
    for _ in 0..2 {
        done(
            &mut shell,
            Command::PlacementZoomTo {
                start: 997,
                length: 34,
            },
        );
        assert!(shell.state().snap_zoom.is_some());
        done(
            &mut shell,
            Command::FixPlacement {
                pointer: pointer(50.0, None),
            },
        );
        whole_view(&shell);
    }
    assert_eq!(shell.state().markers.list.len(), 1);
    assert_eq!(shell.state().placement, None);
}
