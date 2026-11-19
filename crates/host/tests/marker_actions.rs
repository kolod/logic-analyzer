//! The actions of single and pair markers that change the capture or the view: cut left, cut
//! right, trim to a pair (`specs/015-markers-rework`, user stories 4 and 5). Expected values are
//! written by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell, place_pair, place_single};
use la_decoders::decoders::clocked_bits::ID;
use la_host::markers::{Pointer, Shape};
use la_host::notices::{NoticeContent, Severity};
use la_host::{
    Activity, Command, ConfigEdit, CropPlan, CropSide, Decision, Guarded, Outcome, Shell, Unsaved,
    error,
};

const LENGTH: u64 = 10_000;

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

fn wait_idle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while shell.state().activity != Activity::Idle || shell.operation_running() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the cut did not end");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The ids of: B at 1,000, A at 3,000, C at 6,000 (single markers), P 4,000..5,000 and H
/// 2,500..3,500 (pairs); the time counts from C; the view shows 2,000..4,000.
struct Scene {
    shell: Shell,
    _dir: TempDir,
    b: u32,
    a: u32,
    c: u32,
    p: u32,
    h: u32,
}

fn scene(name: &str) -> Scene {
    let dir = TempDir::new(name);
    let mut shell = finished_shell(&dir, 0b11, LENGTH, 1);
    let b = place_single(&mut shell, 1_000);
    let a = place_single(&mut shell, 3_000);
    let c = place_single(&mut shell, 6_000);
    let p = place_pair(&mut shell, 4_000, 5_000);
    let h = place_pair(&mut shell, 2_500, 3_500);
    done(&mut shell, Command::SetTimeOrigin(c));
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 2_000,
            end: 4_000,
        },
    );
    Scene {
        shell,
        _dir: dir,
        b,
        a,
        c,
        p,
        h,
    }
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

fn shapes(shell: &Shell) -> Vec<Shape> {
    shell.state().markers.list.iter().map(|m| m.shape).collect()
}

/// Asks for a cut and confirms it; the shell is left with the cut under way.
fn cut(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::NeedsDecision(_)), "{outcome:?}");
    done(shell, Command::Decide(Decision::Discard));
}

fn info_notices(shell: &Shell) -> Vec<(&'static str, Option<String>)> {
    shell
        .state()
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Info { key, text } if n.severity == Severity::Info => {
                Some((*key, text.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn cutting_left_keeps_the_marker_and_what_is_right_of_it() {
    let mut s = scene("crop-left");
    let before = s.shell.state().capture.as_ref().unwrap().snapshot.clone();
    // The question names what is kept; nothing has changed yet.
    let plan = CropPlan {
        side: CropSide::Left,
        start: 3_000,
        end: LENGTH,
        total: LENGTH,
    };
    assert_eq!(
        s.shell.apply(Command::CutLeft(s.a)),
        Outcome::NeedsDecision(Guarded::Crop(plan))
    );
    assert_eq!(plan.lost(), 3_000);
    assert_eq!(plan.kept(), 7_000);
    assert_eq!(
        s.shell.state().capture.as_ref().unwrap().snapshot.length(),
        LENGTH
    );
    done(&mut s.shell, Command::Decide(Decision::Cancel));
    assert_eq!(s.shell.state().pending_decision, None);
    assert_eq!(s.shell.state().markers.list.len(), 5);
    // Confirmed: the cut runs in the background.
    cut(&mut s.shell, Command::CutLeft(s.a));
    assert_eq!(s.shell.state().activity, Activity::Cropping);
    assert!(s.shell.state().operation.is_some());
    wait_idle(&mut s.shell);
    let state = s.shell.state();
    let after = &state.capture.as_ref().unwrap().snapshot;
    assert_eq!(after.length(), 7_000);
    for channel in 0..2 {
        for position in (3_000..LENGTH).step_by(37) {
            assert_eq!(
                after.level_at(channel, position - 3_000).unwrap(),
                before.level_at(channel, position).unwrap(),
                "channel {channel} position {position}"
            );
        }
    }
    // B (left of the cut) and H (one end left of it) went; the others moved by 3,000.
    assert_eq!(names(&s.shell), ["T2", "T3", "P1"]);
    assert_eq!(
        shapes(&s.shell),
        [
            Shape::Point(0),
            Shape::Point(3_000),
            Shape::Interval {
                start: 1_000,
                end: 2_000
            }
        ]
    );
    assert!(state.markers.get(s.b).is_none() && state.markers.get(s.h).is_none());
    assert!(state.markers.get(s.p).is_some() && state.markers.get(s.c).is_some());
    assert_eq!(state.markers.origin, 3_000, "the origin keeps its point");
    // The view shows the same span as far as it exists: 2,000..4,000 became 0..2,000 (less than
    // the 1,000 that was cut from its start), and shows 2,000 samples.
    assert_eq!((state.window.start, state.window.length), (0, 2_000));
    assert_eq!(state.unsaved, Unsaved::Dirty);
    assert_eq!(state.placement, None);
    assert_eq!(
        info_notices(&s.shell),
        [("marker-cut-pair-removed", Some("1".to_owned()))]
    );
}

#[test]
fn cutting_right_keeps_the_marker_as_the_last_sample() {
    let mut s = scene("crop-right");
    cut(&mut s.shell, Command::CutRight(s.a));
    wait_idle(&mut s.shell);
    let state = s.shell.state();
    assert_eq!(state.capture.as_ref().unwrap().snapshot.length(), 3_001);
    assert_eq!(names(&s.shell), ["T1", "T2"]);
    assert_eq!(shapes(&s.shell), [Shape::Point(1_000), Shape::Point(3_000)]);
    assert_eq!(state.markers.origin, 0, "the origin was cut away");
    // The view of 2,000 samples starting at 2,000 is held inside the shorter capture.
    assert_eq!((state.window.start, state.window.length), (1_001, 2_000));
    // H lost an end; P lay wholly outside, so only one pair is reported.
    assert_eq!(
        info_notices(&s.shell),
        [("marker-cut-pair-removed", Some("1".to_owned()))]
    );
}

#[test]
fn trimming_to_a_pair_keeps_its_data_and_the_pair_sits_at_the_two_ends() {
    let mut s = scene("crop-trim");
    let before = s.shell.state().capture.as_ref().unwrap().snapshot.clone();
    cut(&mut s.shell, Command::TrimToPair(s.p));
    wait_idle(&mut s.shell);
    let state = s.shell.state();
    let after = &state.capture.as_ref().unwrap().snapshot;
    assert_eq!(after.length(), 1_001);
    for channel in 0..2 {
        for position in 4_000..=5_000 {
            assert_eq!(
                after.level_at(channel, position - 4_000).unwrap(),
                before.level_at(channel, position).unwrap()
            );
        }
    }
    assert_eq!(names(&s.shell), ["P1"]);
    assert_eq!(
        shapes(&s.shell),
        [Shape::Interval {
            start: 0,
            end: 1_000
        }]
    );
    assert!(info_notices(&s.shell).is_empty(), "no pair lost an end");
    assert!(state.auto_fit || state.window.start + state.window.length <= 1_001);
}

#[test]
fn a_cut_that_is_cancelled_leaves_everything_as_it_was() {
    let mut s = scene("crop-cancel");
    let before = s.shell.state().clone();
    s.shell.hold_crop(true);
    cut(&mut s.shell, Command::CutLeft(s.a));
    assert_eq!(s.shell.state().activity, Activity::Cropping);
    // While it runs nothing else may change the capture or the markers.
    for command in [
        Command::BeginPlacement(la_host::markers::Kind::Single),
        Command::RemoveMarker(s.b),
        Command::CutRight(s.a),
        Command::SplitPair(s.p),
        Command::SetTimeOrigin(s.a),
    ] {
        assert!(
            matches!(s.shell.apply(command.clone()), Outcome::Ignored(_)),
            "{command:?}"
        );
    }
    done(&mut s.shell, Command::CancelOperation);
    s.shell.hold_crop(false);
    wait_idle(&mut s.shell);
    let after = s.shell.state();
    assert_eq!(
        after.capture.as_ref().unwrap().snapshot.length(),
        before.capture.as_ref().unwrap().snapshot.length()
    );
    assert_eq!(after.markers, before.markers);
    assert_eq!(after.window, before.window);
    assert_eq!(after.unsaved, before.unsaved);
    assert_eq!(after.operation, None);
    assert!(
        after.notices.iter().next().is_none(),
        "a cancelled cut is the user's own doing: no message"
    );
}

#[test]
fn a_cut_that_does_not_fit_the_memory_budget_fails_and_changes_nothing() {
    let dir = TempDir::new("crop-memory");
    // Every channel changes often: the capture takes far more than the smallest budget.
    let mut shell = finished_shell(&dir, u32::MAX, 1_500_000, 4);
    let used = shell
        .state()
        .capture
        .as_ref()
        .unwrap()
        .snapshot
        .memory_used();
    assert!(
        used > 3 * 1024 * 1024,
        "the capture is too small for the test: {used}"
    );
    let id = place_single(&mut shell, 10);
    done(&mut shell, Command::SetMemoryLimit(1024 * 1024));
    let before_length = shell.state().capture.as_ref().unwrap().snapshot.length();
    cut(&mut shell, Command::CutLeft(id));
    wait_idle(&mut shell);
    let state = shell.state();
    assert_eq!(
        state.capture.as_ref().unwrap().snapshot.length(),
        before_length
    );
    assert_eq!(state.markers.list.len(), 1);
    let errors: Vec<&str> = state
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Error { code } => Some(code.as_str()),
            NoticeContent::Info { .. } => None,
        })
        .collect();
    assert_eq!(errors, ["storage.memory_limit"]);
}

#[test]
fn a_cut_needs_something_to_delete_and_a_capture_that_is_not_busy() {
    let mut s = scene("crop-refuse");
    // A pair is not a point; an unknown marker is nothing.
    ignored(&mut s.shell, Command::CutLeft(s.p));
    ignored(&mut s.shell, Command::CutRight(s.h));
    ignored(&mut s.shell, Command::CutLeft(999));
    ignored(&mut s.shell, Command::TrimToPair(s.a));
    // At an end of the capture there is nothing to delete on that side.
    done(&mut s.shell, Command::FitAll);
    let first = place_single(&mut s.shell, 0);
    let last = place_single(&mut s.shell, LENGTH - 1);
    ignored(&mut s.shell, Command::CutLeft(first));
    ignored(&mut s.shell, Command::CutRight(last));
    // A pair that covers the whole capture would keep everything.
    let whole = place_pair(&mut s.shell, 0, LENGTH - 1);
    ignored(&mut s.shell, Command::TrimToPair(whole));
    // A marker at the other end can still cut.
    assert!(matches!(
        s.shell.apply(Command::CutRight(first)),
        Outcome::NeedsDecision(_)
    ));
    done(&mut s.shell, Command::Decide(Decision::Cancel));
}

#[test]
fn the_question_is_always_asked_even_when_an_answer_is_remembered() {
    let mut s = scene("crop-asked");
    done(
        &mut s.shell,
        Command::SetUnsavedChoice(Some(Decision::Discard)),
    );
    assert!(matches!(
        s.shell.apply(Command::CutLeft(s.a)),
        Outcome::NeedsDecision(Guarded::Crop(_))
    ));
    assert_eq!(s.shell.state().activity, Activity::Idle, "nothing started");
    // Save is not an answer to this question.
    ignored(&mut s.shell, Command::Decide(Decision::Save));
    assert!(matches!(
        s.shell.state().pending_decision,
        Some(Guarded::Crop(_))
    ));
    done(&mut s.shell, Command::Decide(Decision::Cancel));
}

#[test]
fn a_cut_is_not_possible_while_a_capture_runs() {
    let dir = TempDir::new("crop-running");
    let mut shell = Shell::new(la_host::ShellConfig::for_tests(dir.path()));
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
    done(
        &mut shell,
        Command::BeginPlacement(la_host::markers::Kind::Single),
    );
    done(
        &mut shell,
        Command::FixPlacement {
            pointer: Pointer {
                x: 400.0,
                lane: None,
            },
        },
    );
    let id = shell.state().markers.list[0].id;
    ignored(&mut shell, Command::CutLeft(id));
    ignored(&mut shell, Command::CutRight(id));
    done(&mut shell, Command::StopCapture);
    wait_idle(&mut shell);
}

#[test]
fn the_decoders_decode_the_shorter_capture_after_a_cut() {
    let dir = TempDir::new("crop-decoders");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 2);
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
    let busy = |s: &Shell| {
        s.state().decoders.instances.iter().any(|i| {
            matches!(
                i.state,
                la_host::decoders::InstanceState::Running
                    | la_host::decoders::InstanceState::Waiting
            ) && i.ready
        })
    };
    let settle = |shell: &mut Shell| {
        let deadline = Instant::now() + Duration::from_secs(60);
        while busy(shell) {
            shell.poll(Instant::now());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    settle(&mut shell);
    assert_eq!(shell.state().decoders.instances[0].length, 20_000);
    let marker = place_single(&mut shell, 8_000);
    cut(&mut shell, Command::CutLeft(marker));
    wait_idle(&mut shell);
    settle(&mut shell);
    let view = &shell.state().decoders.instances[0];
    assert_eq!(view.length, 12_000, "the pass works on the shorter capture");
    assert_eq!(view.covered, 12_000);
    assert!(!view.outdated);
    assert_eq!(
        shell.state().decoders.instances.len(),
        1,
        "the decoder stays"
    );
}

// ---- fitting the view to a pair ---------------------------------------------------------------

#[test]
fn the_view_is_fitted_to_a_pair_with_room_for_its_marks() {
    let dir = TempDir::new("fit");
    let mut shell = finished_shell(&dir, 0b11, LENGTH, 1);
    let pair = place_pair(&mut shell, 1_000, 2_000);
    done(&mut shell, Command::SetFollowLatest(true));
    done(&mut shell, Command::FitToPair(pair));
    // 1,000 samples between the marks and 16 pixels beside each at 800 pixels: the window is
    // ceil(1000 * 800 / 768) = 1,042 samples with 21 of them on each side.
    let state = shell.state();
    assert_eq!((state.window.start, state.window.length), (979, 1_042));
    assert!(!state.auto_fit);
    assert!(!state.follow_latest);
    // At the start of the capture the left margin is what the capture has.
    let near_start = {
        done(&mut shell, Command::FitAll);
        place_pair(&mut shell, 5, 1_005)
    };
    done(&mut shell, Command::FitToPair(near_start));
    assert_eq!(
        (shell.state().window.start, shell.state().window.length),
        (0, 1_042)
    );
    // A pair of zero length is shown at the closest zoom the view allows.
    done(&mut shell, Command::FitAll);
    let point = place_pair(&mut shell, 4_000, 4_000);
    done(&mut shell, Command::FitToPair(point));
    assert_eq!(shell.state().window.length, 800_u64.div_ceil(64));
    // A single marker and an unknown one have no pair to fit.
    let single = place_single(&mut shell, 100);
    ignored(&mut shell, Command::FitToPair(single));
    ignored(&mut shell, Command::FitToPair(999));
}
