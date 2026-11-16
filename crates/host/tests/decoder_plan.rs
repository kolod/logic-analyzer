//! The lanes and overlays of the frame plan, and moving the view to annotations (spec US1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell};
use la_decoders::decoders::clocked_bits::ID;
use la_decoders::lanes::{self, Cell, LANE_ROWS, OVERLAY_ROWS};
use la_decoders::model::Target;
use la_host::decoders::{AnnotationRef, InstanceState};
use la_host::window::ViewSize;
use la_host::{AnnotationTarget, Command, Outcome, Shell};

const SIZE: ViewSize = ViewSize {
    pixels: 800,
    scale: 1.0,
};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn shell_with_decoder(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 30_000, 11);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    for (role, channel) in [("clock", 0), ("data", 1)] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: role.into(),
                channel: Some(channel),
            },
        );
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while shell.state().decoders.instances[0].state == InstanceState::Running {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    (shell, id)
}

#[test]
fn the_plan_has_a_lane_and_overlays_equal_to_the_stored_results() {
    let dir = TempDir::new("plan-lanes");
    let (mut shell, id) = shell_with_decoder(&dir);
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 0,
            end: 5_000,
        },
    );
    let plan = shell.frame_plan(SIZE);
    assert_eq!(plan.lanes.len(), 1);
    let window = plan.window;
    let view = lanes::Window {
        start: window.start,
        end: window.end(),
        columns: SIZE.pixels,
    };
    let results = shell.decoder_results(id).unwrap().clone();
    // The lane is the plan of the stored annotations, nothing more and nothing less.
    assert_eq!(
        plan.lanes[0].plan,
        lanes::plan(&results.lane, &view, LANE_ROWS)
    );
    assert_eq!(plan.lanes[0].instance, id);
    assert_eq!(plan.lanes[0].name, "Clocked bits");
    assert_eq!(plan.lanes[0].state, InstanceState::Finished);
    assert!(plan.lanes[0].plan.represented() > 0);
    // Overlays: the clock arrows on channel 0 and the data dots on channel 1.
    let channels: Vec<u8> = plan.overlays.iter().map(|o| o.channel).collect();
    assert!(
        channels.contains(&0) && channels.contains(&1),
        "{channels:?}"
    );
    for overlay in &plan.overlays {
        let snapshot = &results.overlays[&overlay.channel];
        assert_eq!(overlay.plan, lanes::plan(snapshot, &view, OVERLAY_ROWS));
        assert_eq!(overlay.instance, id);
    }
    // The clock overlay has edge marks.
    let clock = plan.overlays.iter().find(|o| o.channel == 0).unwrap();
    assert!(
        clock
            .plan
            .rows
            .iter()
            .flatten()
            .any(|c| matches!(c, Cell::Mark(m) if m.edge.is_some()))
    );
}

#[test]
fn the_cell_bounds_hold_at_every_zoom() {
    let dir = TempDir::new("plan-bounds");
    let (mut shell, _) = shell_with_decoder(&dir);
    for end in [30_000_u64, 8_000, 1_000, 120, 30] {
        done(&mut shell, Command::ZoomToRange { start: 0, end });
        let plan = shell.frame_plan(SIZE);
        let w = SIZE.pixels as usize;
        for lane in &plan.lanes {
            assert!(
                lane.plan.cell_count() <= 4 * w,
                "lane {} cells",
                lane.plan.cell_count()
            );
            assert!(lane.plan.rows.len() <= LANE_ROWS);
        }
        for overlay in &plan.overlays {
            assert!(overlay.plan.cell_count() <= 2 * w);
        }
    }
}

#[test]
fn a_hidden_lane_or_channel_removes_its_cells() {
    let dir = TempDir::new("plan-hidden");
    let (mut shell, id) = shell_with_decoder(&dir);
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 0,
            end: 5_000,
        },
    );
    // Hiding channel 1 removes the overlay on it (the clock overlay stays).
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 1,
            visible: false,
        },
    );
    let plan = shell.frame_plan(SIZE);
    assert!(plan.overlays.iter().all(|o| o.channel != 1));
    assert!(plan.overlays.iter().any(|o| o.channel == 0));
    assert_eq!(
        plan.lanes.len(),
        1,
        "the lane does not depend on a channel being shown"
    );
    // Hiding the lane removes the lane and all its overlays.
    done(&mut shell, Command::ShowDecoderLane { id, visible: false });
    let plan = shell.frame_plan(SIZE);
    assert!(plan.lanes.is_empty() && plan.overlays.is_empty());
    done(&mut shell, Command::ShowDecoderLane { id, visible: true });
    assert_eq!(shell.frame_plan(SIZE).lanes.len(), 1);
}

#[test]
fn without_a_capture_or_decoders_the_plan_has_no_lanes() {
    let dir = TempDir::new("plan-none");
    let shell = Shell::new(la_host::ShellConfig::for_tests(dir.path()));
    let plan = shell.frame_plan(SIZE);
    assert!(plan.lanes.is_empty() && plan.overlays.is_empty());
}

fn first_annotation(shell: &Shell) -> AnnotationRef {
    let plan = shell.frame_plan(SIZE);
    let cell = plan.lanes[0]
        .plan
        .rows
        .iter()
        .flatten()
        .next()
        .expect("a cell");
    let (id, start) = match cell {
        Cell::Span(s) => (s.id, s.start),
        Cell::Mark(m) => (m.id, m.start),
    };
    AnnotationRef {
        target: Target::Lane,
        id,
        start,
    }
}

#[test]
fn go_to_the_selected_annotation_shows_it_whole_and_centred() {
    let dir = TempDir::new("plan-goto");
    let (mut shell, id) = shell_with_decoder(&dir);
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 20_000,
            end: 20_400,
        },
    );
    let reference = first_annotation(&shell);
    done(
        &mut shell,
        Command::SelectAnnotation {
            instance: id,
            annotation: reference,
        },
    );
    // Move the view away, then come back to the annotation.
    done(&mut shell, Command::GoToStart);
    done(
        &mut shell,
        Command::GoToAnnotation(AnnotationTarget::Selected),
    );
    let window = shell.state().window;
    let selection = shell.state().decoders.selection.clone().unwrap();
    let (start, end) = (selection.details.start, selection.details.end);
    assert!(
        window.start <= start && end <= window.end(),
        "{window:?} {start}..{end}"
    );
    let centre = (start + end) / 2;
    let window_centre = window.start + window.length / 2;
    assert!(
        centre.abs_diff(window_centre) <= window.length / 10 + 2,
        "centred"
    );
}

#[test]
fn next_and_previous_walk_the_annotations_in_start_order() {
    let dir = TempDir::new("plan-walk");
    let (mut shell, id) = shell_with_decoder(&dir);
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 10_000,
            end: 10_600,
        },
    );
    let reference = first_annotation(&shell);
    done(
        &mut shell,
        Command::SelectAnnotation {
            instance: id,
            annotation: reference,
        },
    );
    let mut starts = vec![
        shell
            .state()
            .decoders
            .selection
            .clone()
            .unwrap()
            .details
            .start,
    ];
    for _ in 0..5 {
        done(&mut shell, Command::GoToAnnotation(AnnotationTarget::Next));
        starts.push(
            shell
                .state()
                .decoders
                .selection
                .clone()
                .unwrap()
                .details
                .start,
        );
    }
    assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
    // The selected one is always inside the window.
    let window = shell.state().window;
    assert!(window.start <= *starts.last().unwrap() && *starts.last().unwrap() < window.end());
    for expected in starts.iter().rev().skip(1) {
        done(
            &mut shell,
            Command::GoToAnnotation(AnnotationTarget::Previous),
        );
        assert_eq!(
            shell
                .state()
                .decoders
                .selection
                .clone()
                .unwrap()
                .details
                .start,
            *expected
        );
    }
    // Going on before the first one is ignored at the start of the results.
    done(&mut shell, Command::GoToStart);
    let first = shell.decoder_results(id).unwrap().lane.window(0, u64::MAX)[0].clone();
    let reference = AnnotationRef {
        target: Target::Lane,
        id: first.id,
        start: first.start(),
    };
    done(
        &mut shell,
        Command::SelectAnnotation {
            instance: id,
            annotation: reference,
        },
    );
    assert!(matches!(
        shell.apply(Command::GoToAnnotation(AnnotationTarget::Previous)),
        Outcome::Ignored(_)
    ));
}

#[test]
fn going_to_an_annotation_needs_a_selection() {
    let dir = TempDir::new("plan-nosel");
    let (mut shell, _) = shell_with_decoder(&dir);
    assert!(matches!(
        shell.apply(Command::GoToAnnotation(AnnotationTarget::Next)),
        Outcome::Ignored(_)
    ));
}
