//! Channel names, colours, order and visibility (spec US2, FR-013, FR-026).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{TempDir, finished_shell};
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::window::ViewSize;
use la_host::{Changes, Command, ConfigEdit, Outcome, Shell, ShellConfig, Unsaved, error};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

fn plan_channels(shell: &Shell) -> Vec<(u8, String, Colour)> {
    shell
        .frame_plan(ViewSize {
            pixels: 400,
            scale: 1.0,
        })
        .channels
        .iter()
        .map(|c| (c.channel, c.name.clone(), c.colour))
        .collect()
}

#[test]
fn the_defaults_list_every_channel_and_a_lane_appears_only_for_a_recorded_one() {
    let dir = TempDir::new("pres-default");
    // Channels 0, 2 and 5 of a 32-channel device are recorded.
    let shell = finished_shell(&dir, 0b100101, 2_000, 1);
    let styles = &shell.state().presentation.channels;
    assert_eq!(styles.len(), 32, "every channel of the capture is listed");
    assert!(styles.iter().all(|s| s.visible), "all are shown by default");
    assert!(styles.iter().all(|s| s.name.is_empty()));
    for style in styles {
        assert_eq!(style.colour, Colour::Palette(style.index % PALETTE_SIZE));
    }
    let order: Vec<u8> = styles.iter().map(|s| s.index).collect();
    assert_eq!(order, (0..32).collect::<Vec<u8>>());
    // Only the recorded channels have data, so only they get a lane.
    let channels = plan_channels(&shell);
    assert_eq!(channels.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 2, 5]);
}

#[test]
fn names_colours_and_order_can_be_set_before_any_capture_and_stay_for_the_next_one() {
    let dir = TempDir::new("pres-before");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    // No capture yet: the list is that of the device, and it can be edited.
    assert!(shell.state().capture.is_none());
    assert_eq!(shell.state().presentation.channels.len(), 32);
    assert_eq!(
        shell.state().unsaved,
        Unsaved::Clean,
        "nothing to lose without a capture"
    );
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 2,
            name: "SCL".into(),
        },
    );
    done(
        &mut shell,
        Command::SetChannelColour {
            channel: 2,
            colour: Colour::Rgb(9, 8, 7),
        },
    );
    done(&mut shell, Command::MoveChannel { from: 2, to: 0 });
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 1,
            visible: false,
        },
    );
    assert_eq!(shell.state().unsaved, Unsaved::Clean);

    // Record 0, 1 and 2 (in any case the flags are independent of showing), then capture.
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(0b111),
        ConfigEdit::Length(2_000),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Seed(Some(3)),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while shell.state().activity != la_host::Activity::Idle {
        shell.poll(std::time::Instant::now());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    // What was set before the capture is what the capture shows: channel 2 first, named and
    // coloured; channel 1 hidden although it was recorded; channel 0 as it was.
    assert_eq!(
        plan_channels(&shell),
        [
            (2, "SCL".to_owned(), Colour::Rgb(9, 8, 7)),
            (0, String::new(), Colour::Palette(0)),
        ]
    );

    // A new capture keeps the user's choices.
    // The unsaved capture asks first; discarding it is the user's explicit choice here.
    assert!(matches!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(_)
    ));
    done(&mut shell, Command::Decide(la_host::Decision::Discard));
    assert!(shell.state().capture.is_none());
    let styles = &shell.state().presentation.channels;
    assert_eq!(styles[0].index, 2);
    assert_eq!(styles[0].name, "SCL");
    assert!(!styles.iter().find(|s| s.index == 1).unwrap().visible);

    // And everything can be put back, with or without a capture.
    done(&mut shell, Command::ResetPresentation);
    let styles = &shell.state().presentation.channels;
    assert_eq!(
        styles.iter().map(|s| s.index).collect::<Vec<_>>(),
        (0..32).collect::<Vec<u8>>()
    );
    assert!(styles.iter().all(|s| s.name.is_empty() && s.visible));
}

#[test]
fn recording_a_channel_and_showing_it_are_separate_choices() {
    let dir = TempDir::new("pres-independent");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 4);
    // The capture on screen recorded channels 0 and 1. Choosing what the next capture records
    // does not change what is shown now, and hiding does not change what will be recorded.
    done(&mut shell, Command::EditConfig(ConfigEdit::Channels(0b100)));
    assert_eq!(
        plan_channels(&shell)
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 0,
            visible: false,
        },
    );
    assert_eq!(shell.state().config.enabled_channels, 0b100);
    assert_eq!(
        plan_channels(&shell)
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
fn rename_colour_visibility_and_order_change_the_plan() {
    let dir = TempDir::new("pres-edit");
    let mut shell = finished_shell(&dir, 0b111, 2_000, 2);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    let before = shell.state().versions;

    let changes = done(
        &mut shell,
        Command::RenameChannel {
            channel: 1,
            name: "  SDA  ".into(),
        },
    );
    assert_eq!(changes, Changes::PRESENTATION);
    done(
        &mut shell,
        Command::SetChannelColour {
            channel: 1,
            colour: Colour::Rgb(1, 2, 3),
        },
    );
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 0,
            visible: false,
        },
    );
    // Move channel 2 (position 2 in the order) to the front.
    done(&mut shell, Command::MoveChannel { from: 2, to: 0 });

    let channels = plan_channels(&shell);
    assert_eq!(
        channels,
        [
            (2, String::new(), Colour::Palette(2)),
            (1, "SDA".to_owned(), Colour::Rgb(1, 2, 3)),
        ]
    );
    let after = shell.state().versions;
    assert_eq!(after.presentation, before.presentation + 4);
    assert_eq!(after.capture, before.capture);
    assert_eq!(after.window, before.window);

    done(&mut shell, Command::ResetPresentation);
    let channels = plan_channels(&shell);
    assert_eq!(channels.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(channels.iter().all(|c| c.1.is_empty()));
}

#[test]
fn invalid_edits_are_ignored() {
    let dir = TempDir::new("pres-invalid");
    let mut shell = finished_shell(&dir, 0b11, 1_000, 3);
    let version = shell.state().version;
    for command in [
        Command::RenameChannel {
            channel: 40,
            name: "x".into(),
        },
        Command::SetChannelColour {
            channel: 40,
            colour: Colour::Palette(1),
        },
        Command::SetChannelVisible {
            channel: 40,
            visible: true,
        },
        Command::MoveChannel { from: 99, to: 0 },
        Command::MoveChannel { from: 0, to: 99 },
    ] {
        assert_eq!(
            shell.apply(command.clone()),
            Outcome::Ignored(error::INVALID_COMMAND),
            "{command:?}"
        );
    }
    assert_eq!(shell.state().version, version);
}

#[test]
fn hostile_names_are_sanitized_and_limited() {
    let dir = TempDir::new("pres-hostile");
    let mut shell = finished_shell(&dir, 0b1, 1_000, 4);
    let hostile = [
        "\u{202e}evil\u{202c}",
        "a\u{0}b\nc",
        "😀 bus",
        "שלום",
        &"x".repeat(10_000),
        "   ",
    ];
    for name in hostile {
        done(
            &mut shell,
            Command::RenameChannel {
                channel: 0,
                name: name.into(),
            },
        );
        let stored = shell.state().presentation.channels[0].name.clone();
        assert!(stored.chars().count() <= 64);
        assert!(!stored.chars().any(char::is_control));
        assert!(!stored.contains('\u{202e}') && !stored.contains('\u{202c}'));
    }
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 0,
            name: "\u{202e}evil\u{202c}".into(),
        },
    );
    assert_eq!(shell.state().presentation.channels[0].name, "evil");
}

#[test]
fn editing_the_presentation_of_a_clean_capture_makes_it_dirty() {
    let dir = TempDir::new("pres-dirty");
    let mut shell = finished_shell(&dir, 0b1, 1_000, 5);
    // A capture that was discarded and replaced is clean until something changes.
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    done(&mut shell, Command::ResetPresentation);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
}
