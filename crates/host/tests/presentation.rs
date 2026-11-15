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
fn the_defaults_list_every_channel_and_the_flag_is_on_for_the_recorded_ones() {
    let dir = TempDir::new("pres-default");
    // Channels 0, 2 and 5 of a 32-channel device are recorded.
    let shell = finished_shell(&dir, 0b100101, 2_000, 1);
    let styles = &shell.state().presentation.channels;
    assert_eq!(styles.len(), 32, "every channel of the capture is listed");
    let flagged: Vec<u8> = styles
        .iter()
        .filter(|s| s.visible)
        .map(|s| s.index)
        .collect();
    assert_eq!(
        flagged,
        [0, 2, 5],
        "the one flag is on for the recorded channels"
    );
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
    assert_eq!(shell.state().unsaved, Unsaved::Clean);

    // Record 0, 1 and 2; then the flag of channel 1 is turned off, which also stops recording it.
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(0b111),
        ConfigEdit::Length(2_000),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Seed(Some(3)),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 1,
            visible: false,
        },
    );
    assert_eq!(shell.state().config.enabled_channels, 0b101);
    done(&mut shell, Command::StartCapture);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while shell.state().activity != la_host::Activity::Idle {
        shell.poll(std::time::Instant::now());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    // What was set before the capture is what the capture shows: channel 2 first, named and
    // coloured, then channel 0; channel 1 was not recorded and has no lane.
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
    assert!(styles.iter().all(|s| s.name.is_empty()));
    assert_eq!(
        shell.state().presentation.visible_mask(),
        shell.state().config.enabled_channels,
        "the flags are what is recorded"
    );
}

#[test]
fn one_flag_per_channel_shows_it_and_records_it_in_the_next_capture() {
    let dir = TempDir::new("pres-one-flag");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 4);
    let lanes = |shell: &Shell| -> Vec<u8> { plan_channels(shell).iter().map(|c| c.0).collect() };
    // The capture on screen has data for channels 0 and 1.
    assert_eq!(lanes(&shell), [0, 1]);
    assert_eq!(shell.state().config.enabled_channels, 0b11);

    // Turning a channel off hides its lane and leaves it out of the next capture.
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 0,
            visible: false,
        },
    );
    assert_eq!(lanes(&shell), [1]);
    assert_eq!(shell.state().config.enabled_channels, 0b10);

    // Turning on a channel the capture has no data for records it next time; there is no lane
    // for it until there is data.
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 5,
            visible: true,
        },
    );
    assert_eq!(shell.state().config.enabled_channels, 0b10_0010);
    assert_eq!(lanes(&shell), [1]);

    // Turning channel 0 on again brings its lane back, because the data is still there.
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 0,
            visible: true,
        },
    );
    assert_eq!(lanes(&shell), [0, 1]);
    assert_eq!(shell.state().config.enabled_channels, 0b10_0011);

    // Choosing the channels to record by the configuration moves the flags the same way.
    done(&mut shell, Command::EditConfig(ConfigEdit::Channels(0b1)));
    let flagged: Vec<u8> = shell
        .state()
        .presentation
        .channels
        .iter()
        .filter(|c| c.visible)
        .map(|c| c.index)
        .collect();
    assert_eq!(flagged, [0]);
    assert_eq!(lanes(&shell), [0]);
}

#[test]
fn the_flag_is_locked_while_a_capture_runs() {
    let dir = TempDir::new("pres-locked");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Channels(0b11),
        ConfigEdit::SampleRate(20_000),
        ConfigEdit::Length(10_000_000),
        ConfigEdit::Seed(Some(9)),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    assert_eq!(
        shell.apply(Command::SetChannelVisible {
            channel: 0,
            visible: false
        }),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
    assert_eq!(shell.state().config.enabled_channels, 0b11);
    done(&mut shell, Command::StopCapture);
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

    // Names, colours and order go back to the defaults; the flags are what is recorded (channel 0
    // was turned off above), so they stay as they are.
    done(&mut shell, Command::ResetPresentation);
    let channels = plan_channels(&shell);
    assert_eq!(channels.iter().map(|c| c.0).collect::<Vec<_>>(), [1, 2]);
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
