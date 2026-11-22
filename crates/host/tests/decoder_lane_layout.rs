//! The lanes of decoders as lanes of the signal view (spec 016, the change after looking at the running
//! application): a decoder lane has a height of its own, saved with the session, and is moved among
//! the shown lanes like a channel.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{Lcg, TempDir, finished_shell, wait_operation};
use la_decoders::decoders::clocked_bits::ID;
use la_host::decoders::InstanceState;
use la_host::plan::LaneSlot;
use la_host::window::ViewSize;
use la_host::{Command, Outcome, Shell, ShellConfig};

const SIZE: ViewSize = ViewSize {
    pixels: 800,
    scale: 1.0,
};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn rejected(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(!matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while shell
        .state()
        .decoders
        .instances
        .iter()
        .any(|i| i.state == InstanceState::Running)
    {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Four channels and two decoders (on channels 0, 1 and 2, 3): the lanes are C0 C1 C2 C3 D0 D1.
fn two_decoders(dir: &TempDir) -> (Shell, Vec<u32>) {
    let mut shell = finished_shell(dir, 0b1111, 30_000, 11);
    let mut ids = Vec::new();
    for pair in [(0, 1), (2, 3)] {
        done(
            &mut shell,
            Command::AddDecoder {
                decoder: ID.to_owned(),
            },
        );
        let id = shell.state().decoders.instances.last().unwrap().id;
        for (role, channel) in [("clock", pair.0), ("data", pair.1)] {
            done(
                &mut shell,
                Command::SetDecoderChannel {
                    id,
                    role: role.into(),
                    channel: Some(channel),
                },
            );
        }
        ids.push(id);
    }
    settle(&mut shell);
    (shell, ids)
}

/// The lanes as the plan lists them, by name: `C<channel>` or `D<instance>`.
fn lanes(shell: &Shell) -> Vec<String> {
    let plan = shell.frame_plan(SIZE);
    plan.order()
        .iter()
        .map(|slot| match slot {
            LaneSlot::Channel(n) => format!("C{}", plan.channels[*n].channel),
            LaneSlot::Decoder(n) => format!("D{}", plan.lanes[*n].instance),
        })
        .collect()
}

#[test]
fn the_decoder_lanes_follow_the_channels_until_they_are_moved() {
    let dir = TempDir::new("lane-order");
    let (shell, ids) = two_decoders(&dir);
    assert_eq!(
        lanes(&shell),
        ["C0", "C1", "C2", "C3"]
            .into_iter()
            .map(String::from)
            .chain(ids.iter().map(|id| format!("D{id}")))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_decoder_lane_can_be_put_among_the_channels_and_a_channel_below_it() {
    let dir = TempDir::new("lane-move");
    let (mut shell, ids) = two_decoders(&dir);
    let (d0, d1) = (ids[0], ids[1]);
    // The first decoder goes under C1 (position 4 → 2).
    done(&mut shell, Command::MoveLane { from: 4, to: 2 });
    assert_eq!(
        lanes(&shell),
        ["C0", "C1", &format!("D{d0}"), "C2", "C3", &format!("D{d1}")]
    );
    // A channel moves below a decoder lane: C0 goes to the position of D1 (the end).
    done(&mut shell, Command::MoveLane { from: 0, to: 5 });
    assert_eq!(
        lanes(&shell),
        ["C1", &format!("D{d0}"), "C2", "C3", &format!("D{d1}"), "C0"]
    );
    // The second decoder goes to the top.
    done(&mut shell, Command::MoveLane { from: 4, to: 0 });
    assert_eq!(
        lanes(&shell),
        [&format!("D{d1}"), "C1", &format!("D{d0}"), "C2", "C3", "C0"]
    );
    // The order of the channels among themselves follows what was done to the lanes.
    let order: Vec<u8> = shell
        .state()
        .presentation
        .channels
        .iter()
        .map(|c| c.index)
        .take(4)
        .collect();
    assert_eq!(order, [1, 2, 3, 0]);
}

#[test]
fn a_move_that_goes_nowhere_or_outside_the_lanes_is_ignored() {
    let dir = TempDir::new("lane-ignored");
    let (mut shell, _) = two_decoders(&dir);
    let before = lanes(&shell);
    rejected(&mut shell, Command::MoveLane { from: 2, to: 2 });
    rejected(&mut shell, Command::MoveLane { from: 6, to: 0 });
    rejected(&mut shell, Command::MoveLane { from: 0, to: 6 });
    assert_eq!(lanes(&shell), before);
}

#[test]
fn many_moves_give_the_order_of_a_model() {
    let dir = TempDir::new("lane-model");
    let (mut shell, ids) = two_decoders(&dir);
    let mut model: Vec<String> = lanes(&shell);
    let mut random = Lcg(5);
    for round in 0..200_u32 {
        let from = random.below(6) as usize;
        let to = random.below(6) as usize;
        if from == to {
            continue;
        }
        let moved = model.remove(from);
        model.insert(to, moved);
        done(&mut shell, Command::MoveLane { from, to });
        assert_eq!(lanes(&shell), model, "move {round}: {from} to {to}");
    }
    let _ = ids;
}

#[test]
fn a_hidden_channel_keeps_its_place_and_the_shown_ones_move_around_it() {
    let dir = TempDir::new("lane-hidden");
    let (mut shell, ids) = two_decoders(&dir);
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 1,
            visible: false,
        },
    );
    // C0 C2 C3 D D: the first decoder goes under C0.
    assert_eq!(lanes(&shell).len(), 5);
    done(&mut shell, Command::MoveLane { from: 3, to: 1 });
    assert_eq!(
        lanes(&shell),
        [
            "C0",
            &format!("D{}", ids[0]),
            "C2",
            "C3",
            &format!("D{}", ids[1])
        ]
    );
    let order: Vec<u8> = shell
        .state()
        .presentation
        .channels
        .iter()
        .map(|c| c.index)
        .take(4)
        .collect();
    assert_eq!(order, [0, 1, 2, 3], "the hidden channel stays in its place");
}

#[test]
fn the_height_of_a_decoder_lane_is_limited_and_zero_is_automatic() {
    let dir = TempDir::new("lane-height");
    let (mut shell, ids) = two_decoders(&dir);
    let height = |shell: &Shell, id: u32| {
        shell
            .frame_plan(SIZE)
            .lanes
            .iter()
            .find(|l| l.instance == id)
            .map(|l| l.height)
            .unwrap()
    };
    assert_eq!(height(&shell, ids[0]), 0);
    done(
        &mut shell,
        Command::SetDecoderLaneHeight {
            instance: ids[0],
            height: 90,
        },
    );
    assert_eq!(height(&shell, ids[0]), 90);
    assert_eq!(height(&shell, ids[1]), 0);
    for (asked, kept) in [(5, 24), (1_000, 240)] {
        done(
            &mut shell,
            Command::SetDecoderLaneHeight {
                instance: ids[0],
                height: asked,
            },
        );
        assert_eq!(height(&shell, ids[0]), kept);
    }
    done(
        &mut shell,
        Command::SetDecoderLaneHeight {
            instance: ids[0],
            height: 0,
        },
    );
    assert_eq!(height(&shell, ids[0]), 0);
    // An instance that does not exist is refused.
    rejected(
        &mut shell,
        Command::SetDecoderLaneHeight {
            instance: 999,
            height: 50,
        },
    );
}

#[test]
fn removing_a_decoder_or_resetting_the_view_forgets_its_lane() {
    let dir = TempDir::new("lane-forget");
    let (mut shell, ids) = two_decoders(&dir);
    for id in &ids {
        done(
            &mut shell,
            Command::SetDecoderLaneHeight {
                instance: *id,
                height: 60,
            },
        );
    }
    done(&mut shell, Command::MoveLane { from: 4, to: 0 });
    assert!(!shell.state().presentation.decoder_slots.is_empty());
    done(&mut shell, Command::RemoveDecoder(ids[0]));
    let presentation = &shell.state().presentation;
    assert!(!presentation.decoder_heights.contains_key(&ids[0]));
    assert!(!presentation.decoder_slots.contains_key(&ids[0]));
    assert!(presentation.decoder_heights.contains_key(&ids[1]));
    done(&mut shell, Command::ResetPresentation);
    let presentation = &shell.state().presentation;
    assert!(presentation.decoder_heights.is_empty());
    assert!(presentation.decoder_slots.is_empty());
}

#[test]
fn the_height_of_a_decoder_lane_is_saved_with_the_session_and_comes_back() {
    let dir = TempDir::new("lane-session");
    let (mut shell, ids) = two_decoders(&dir);
    done(
        &mut shell,
        Command::SetDecoderLaneHeight {
            instance: ids[1],
            height: 120,
        },
    );
    done(
        &mut shell,
        Command::SetChannelHeight {
            channel: 2,
            height: 50,
        },
    );
    let path = dir.path().join("lanes.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    let mut reopened = Shell::new(ShellConfig::for_tests(dir.path()));
    done(&mut reopened, Command::OpenSession(path));
    wait_operation(&mut reopened);
    settle(&mut reopened);
    let presentation = &reopened.state().presentation;
    assert_eq!(presentation.decoder_heights.get(&ids[1]), Some(&120));
    assert_eq!(presentation.decoder_heights.get(&ids[0]), None);
    assert_eq!(
        presentation
            .channels
            .iter()
            .find(|c| c.index == 2)
            .unwrap()
            .height,
        50
    );
}
