//! Placing, snapping, moving and removing markers (spec US3).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell};
use la_host::markers::MAX_MARKERS;
use la_host::window::ViewSize;
use la_host::{Changes, Command, ConfigEdit, Outcome, Shell, ShellConfig, SignalProfile, error};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

fn positions(shell: &Shell) -> Vec<u64> {
    shell
        .state()
        .markers
        .list
        .iter()
        .map(|m| m.position)
        .collect()
}

fn set_snap(shell: &mut Shell, on: bool) {
    done(shell, Command::SetSnap(on));
}

#[test]
fn markers_need_a_capture() {
    let dir = TempDir::new("marker-none");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(
        shell.apply(Command::PlaceMarker {
            position: 5,
            channel: 0
        }),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
}

#[test]
fn placement_clamps_to_the_capture_and_ids_are_unique_and_never_reused() {
    let dir = TempDir::new("marker-place");
    let mut shell = finished_shell(&dir, 0b1, 1_000, 1);
    set_snap(&mut shell, false);
    let before = shell.state().versions;
    let changes = done(
        &mut shell,
        Command::PlaceMarker {
            position: 400,
            channel: 0,
        },
    );
    assert_eq!(changes, Changes::MARKERS);
    done(
        &mut shell,
        Command::PlaceMarker {
            position: 5_000,
            channel: 0,
        },
    );
    assert_eq!(positions(&shell), [400, 999]);
    let ids: Vec<u32> = shell.state().markers.list.iter().map(|m| m.id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(shell.state().versions.markers, before.markers + 2);

    done(&mut shell, Command::RemoveMarker(ids[0]));
    done(
        &mut shell,
        Command::PlaceMarker {
            position: 10,
            channel: 0,
        },
    );
    let new_id = shell.state().markers.list.last().unwrap().id;
    assert!(!ids.contains(&new_id), "an id is never reused");
}

#[test]
fn at_most_64_markers_can_be_placed() {
    let dir = TempDir::new("marker-limit");
    let mut shell = finished_shell(&dir, 0b1, 1_000, 2);
    set_snap(&mut shell, false);
    assert_eq!(MAX_MARKERS, 64);
    for i in 0..64 {
        done(
            &mut shell,
            Command::PlaceMarker {
                position: i,
                channel: 0,
            },
        );
    }
    assert_eq!(
        shell.apply(Command::PlaceMarker {
            position: 99,
            channel: 0
        }),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
    assert_eq!(shell.state().markers.list.len(), 64);
}

#[test]
fn snapping_moves_a_marker_to_the_nearest_transition_of_the_channel() {
    let dir = TempDir::new("marker-snap");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(0b1),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(200_000),
        ConfigEdit::Seed(Some(21)),
        ConfigEdit::Profile(SignalProfile::Sparse),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    shell.apply(Command::StartCapture);
    while shell.state().activity != la_host::Activity::Idle {
        shell.poll(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let snapshot = shell.state().capture.as_ref().unwrap().snapshot.clone();
    let transitions: Vec<u64> = snapshot
        .transitions(0, 0, snapshot.length())
        .unwrap()
        .map(|e| e.position)
        .collect();
    assert!(transitions.len() > 10, "{}", transitions.len());
    // The whole capture is shown at 800 px: 250 samples per pixel, so the snap radius covers
    // about 3,000 samples. Pick a transition whose neighbours are more than twice that away.
    let (i, p) = transitions
        .iter()
        .enumerate()
        .skip(1)
        .find(|(i, p)| {
            let prev = transitions[*i - 1];
            let next = transitions.get(*i + 1).copied().unwrap_or(u64::MAX);
            **p - prev > 7_000 && next - **p > 7_000
        })
        .map(|(i, p)| (i, *p))
        .expect("an isolated transition");
    let _ = i;

    set_snap(&mut shell, true);
    done(
        &mut shell,
        Command::PlaceMarker {
            position: p + 700,
            channel: 0,
        },
    );
    assert_eq!(positions(&shell), [p], "snapped to the nearest transition");
    done(
        &mut shell,
        Command::PlaceMarker {
            position: p.saturating_sub(900),
            channel: 0,
        },
    );
    assert_eq!(positions(&shell)[1], p, "from either side");

    // Beyond the radius nothing is near: the marker stays where it was dropped.
    done(
        &mut shell,
        Command::PlaceMarker {
            position: p + 3_500,
            channel: 0,
        },
    );
    assert_eq!(positions(&shell)[2], p + 3_500);

    // With snapping off the position is kept exactly.
    set_snap(&mut shell, false);
    done(
        &mut shell,
        Command::PlaceMarker {
            position: p + 3,
            channel: 0,
        },
    );
    assert_eq!(positions(&shell)[3], p + 3);

    // Moving a marker snaps too.
    set_snap(&mut shell, true);
    let id = shell.state().markers.list[3].id;
    done(
        &mut shell,
        Command::MoveMarker {
            id,
            position: p + 20,
            channel: 0,
        },
    );
    assert_eq!(positions(&shell)[3], p);
}

#[test]
fn rename_remove_and_select() {
    let dir = TempDir::new("marker-edit");
    let mut shell = finished_shell(&dir, 0b1, 1_000, 3);
    set_snap(&mut shell, false);
    for position in [100, 200, 300] {
        done(
            &mut shell,
            Command::PlaceMarker {
                position,
                channel: 0,
            },
        );
    }
    let ids: Vec<u32> = shell.state().markers.list.iter().map(|m| m.id).collect();
    // By default the two most recently placed markers are measured.
    assert_eq!(shell.state().markers.selected, [Some(ids[1]), Some(ids[2])]);

    done(
        &mut shell,
        Command::RenameMarker {
            id: ids[0],
            name: " \u{202e}edge\u{0} ".into(),
        },
    );
    assert_eq!(shell.state().markers.list[0].name, "edge");
    assert_eq!(
        shell.apply(Command::RenameMarker {
            id: 9_999,
            name: "x".into()
        }),
        Outcome::Ignored(error::INVALID_COMMAND)
    );

    done(
        &mut shell,
        Command::SelectMarkers {
            first: Some(ids[0]),
            second: Some(ids[2]),
        },
    );
    assert_eq!(shell.state().markers.selected, [Some(ids[0]), Some(ids[2])]);
    assert_eq!(
        shell.apply(Command::SelectMarkers {
            first: Some(8_888),
            second: None
        }),
        Outcome::Ignored(error::INVALID_COMMAND)
    );

    // Removing a selected marker clears it from the selection.
    done(&mut shell, Command::RemoveMarker(ids[2]));
    assert_eq!(shell.state().markers.selected, [Some(ids[0]), None]);
    done(&mut shell, Command::RemoveAllMarkers);
    assert!(shell.state().markers.list.is_empty());
    assert_eq!(shell.state().markers.selected, [None, None]);
}

#[test]
fn go_to_marker_reveals_it_without_changing_the_zoom() {
    let dir = TempDir::new("marker-goto");
    let mut shell = finished_shell(&dir, 0b1, 10_000, 4);
    set_snap(&mut shell, false);
    done(
        &mut shell,
        Command::PlaceMarker {
            position: 8_000,
            channel: 0,
        },
    );
    let id = shell.state().markers.list[0].id;
    done(&mut shell, Command::ZoomToRange { start: 0, end: 500 });
    done(&mut shell, Command::GoToMarker(id));
    let w = shell.state().window;
    assert_eq!(w.length, 500);
    assert!(w.start <= 8_000 && 8_000 < w.end());
}

#[test]
fn a_marker_stays_attached_to_its_sample_while_zooming_and_panning() {
    let dir = TempDir::new("marker-attached");
    let mut shell = finished_shell(&dir, 0b1, 10_000, 5);
    set_snap(&mut shell, false);
    done(
        &mut shell,
        Command::PlaceMarker {
            position: 2_500,
            channel: 0,
        },
    );
    let size = ViewSize {
        pixels: 800,
        scale: 1.0,
    };
    // Whole capture: sample 2,500 of 10,000 is a quarter of the way: x = 200.
    let plan = shell.frame_plan(size);
    assert_eq!(plan.markers.len(), 1);
    assert!((plan.markers[0].x - 200.0).abs() < 1e-9);
    assert_eq!(plan.markers[0].position, 2_500);

    done(
        &mut shell,
        Command::ZoomToRange {
            start: 2_000,
            end: 3_000,
        },
    );
    let plan = shell.frame_plan(size);
    // 500 samples into a window of 1,000 over 800 px: x = 400.
    assert!((plan.markers[0].x - 400.0).abs() < 1e-9);

    // Panned out of the window, the marker is not part of the plan.
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 6_000,
            end: 7_000,
        },
    );
    assert!(shell.frame_plan(size).markers.is_empty());
}
