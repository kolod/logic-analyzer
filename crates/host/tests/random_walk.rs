//! A long random walk over every kind of command (spec SC-005, task T085).
//!
//! A real `Shell` with the virtual device and a temporary directory takes a seeded, long
//! sequence of actions: starting and stopping captures, zooming and panning, markers, channel
//! edits, saving, opening good, damaged and missing files, language and settings changes, window
//! sizes, invalid edits, failures of the disk and of the device, and panics. After every action
//! the invariants of the state are checked, and so is the promise of the application: the data of
//! a capture that was shown is still shown, was saved, or was thrown away by an explicit
//! `Decide(Discard)`.
//!
//! Failures print the seed and the index of the action. `LA_WALK_SEED` and `LA_WALK_STEPS` pick
//! another run; the ignored variant makes two million actions (the nightly workflow).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{Lcg, TempDir, finished_shell, wait_operation};
use la_devices::{DeviceId, FailureKind, ScriptedFailure};
use la_host::notices::{MAX_NOTICES, NoticeContent};
use la_host::presentation::Colour;
use la_host::settings::Theme;
use la_host::window::{ViewSize, WindowLimits};
use la_host::{
    Activity, Command, ConfigEdit, Decision, OperationKind, PanStep, Shell, ShellConfig,
    SignalProfile, Unsaved, ZoomAnchor, ZoomDirection, ZoomFactor,
};
use la_i18n::Language;
use la_session::harness::{FaultFs, FaultPlan, Op, RealFileSystem};

/// The files the walk opens.
struct Files {
    good: PathBuf,
    damaged: Vec<PathBuf>,
    missing: PathBuf,
    targets: Vec<PathBuf>,
}

fn make_files(dir: &TempDir) -> Files {
    let source_dir = TempDir::new("walk-source");
    let mut shell = finished_shell(&source_dir, 0b11, 4_000, 7);
    let good = dir.path().join("good.lasession");
    shell.apply(Command::RenameChannel {
        channel: 0,
        name: "walk".into(),
    });
    shell.apply(Command::SaveSessionAs(good.clone()));
    wait_operation(&mut shell);
    let bytes = std::fs::read(&good).unwrap();
    let mut flipped = bytes.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 0xff;
    let variants: [(&str, Vec<u8>); 4] = [
        ("flipped", flipped),
        ("cut", bytes[..bytes.len() - 30].to_vec()),
        ("empty", Vec::new()),
        ("foreign", b"not a session file".to_vec()),
    ];
    let damaged = variants
        .iter()
        .map(|(name, content)| {
            let path = dir.path().join(format!("{name}.lasession"));
            std::fs::write(&path, content).unwrap();
            path
        })
        .collect();
    Files {
        good,
        damaged,
        missing: dir.path().join("missing.lasession"),
        targets: (0..3)
            .map(|i| dir.path().join(format!("walk{i}.lasession")))
            .collect(),
    }
}

/// How many threads this process has (Linux only; `None` elsewhere).
fn thread_count() -> Option<usize> {
    std::fs::read_dir("/proc/self/task")
        .ok()
        .map(Iterator::count)
}

/// How many failures of saving and opening files were reported so far (a failing command that
/// waited for a save is a different failure).
fn notice_total(shell: &Shell) -> u64 {
    shell
        .state()
        .notices
        .iter()
        .filter(
            |n| matches!(&n.content, NoticeContent::Error { code } if code.starts_with("session.")),
        )
        .map(|n| u64::from(n.count))
        .sum()
}

/// The identity of the capture on screen.
fn identity(shell: &Shell) -> Option<std::time::SystemTime> {
    shell
        .state()
        .capture
        .as_ref()
        .map(|c| c.description.started_at())
}

/// What must hold after every action.
fn check_invariants(shell: &Shell) -> Result<(), String> {
    let state = shell.state();
    let capture = state.capture.as_ref();
    // The window.
    if let Some(capture) = capture {
        let limits = WindowLimits {
            capture_length: capture.snapshot.length(),
            pixels: state.view_size.pixels.clamp(1, 16_384),
        };
        if !state.window.is_valid(&limits) {
            return Err(format!(
                "window {:?} is invalid for {limits:?}",
                state.window
            ));
        }
    }
    // Markers.
    let markers = &state.markers;
    if markers.list.len() > la_host::markers::MAX_MARKERS {
        return Err("too many markers".into());
    }
    let mut ids = BTreeSet::new();
    for m in &markers.list {
        if !ids.insert(m.id) {
            return Err(format!("marker id {} repeats", m.id));
        }
        let length = capture.map_or(0, |c| c.snapshot.length());
        if m.position >= length.max(1) {
            return Err(format!("marker at {} beyond {length}", m.position));
        }
    }
    for id in markers.selected.into_iter().flatten() {
        if !ids.contains(&id) {
            return Err(format!("selected marker {id} does not exist"));
        }
    }
    if capture.is_none() && !markers.list.is_empty() {
        return Err("markers without a capture".into());
    }
    // The presentation.
    let styles = &state.presentation.channels;
    let mut seen = BTreeSet::new();
    for style in styles {
        if !seen.insert(style.index) {
            return Err(format!("channel {} repeats", style.index));
        }
        if style.name.chars().count() > 64 || style.name.chars().any(char::is_control) {
            return Err("an unsafe channel name".into());
        }
        if let Colour::Palette(i) = style.colour {
            if i >= la_host::presentation::PALETTE_SIZE {
                return Err("a palette colour out of range".into());
            }
        }
    }
    if let Some(capture) = capture {
        if !styles.is_empty() && styles.len() != usize::from(capture.snapshot.channels()) {
            return Err("the presentation does not match the channels".into());
        }
    } else if !styles.is_empty() {
        return Err("a presentation without a capture".into());
    }
    // Activity and unsaved state.
    if state.operation.is_some() && state.activity != Activity::Idle {
        return Err("a save or load while capturing".into());
    }
    if capture.is_none() && state.unsaved != Unsaved::Clean {
        return Err(format!("{:?} without a capture", state.unsaved));
    }
    if (state.activity == Activity::Capturing) != (state.unsaved == Unsaved::Running) {
        return Err(format!(
            "activity {:?} and unsaved {:?} disagree",
            state.activity, state.unsaved
        ));
    }
    if state.pending_decision.is_some() && capture.is_none() {
        return Err("a question about nothing".into());
    }
    // Notices.
    if state.notices.len() > MAX_NOTICES {
        return Err("too many notices".into());
    }
    // The settings stay in their ranges.
    let (low, high) = la_host::settings::UI_SCALE_RANGE;
    if !(low..=high).contains(&state.settings.ui_scale_percent) {
        return Err("the scale is out of range".into());
    }
    if state.settings.recent_files.len() > la_host::settings::MAX_RECENT_FILES {
        return Err("too many recent files".into());
    }
    Ok(())
}

fn random_action(rng: &mut Lcg, files: &Files) -> Command {
    let channel = rng.below(40) as u8; // some beyond the capture: invalid on purpose
    let position = rng.below(6_000);
    match rng.below(64) {
        0..=2 => Command::StartCapture,
        3 => Command::StopCapture,
        4 => Command::NewCapture,
        5 => Command::ZoomStep(if rng.below(2) == 0 {
            ZoomDirection::In
        } else {
            ZoomDirection::Out
        }),
        6 => Command::Zoom {
            factor: ZoomFactor {
                numerator: rng.below(10),
                denominator: rng.below(10),
            },
            anchor: if rng.below(2) == 0 {
                ZoomAnchor::Centre
            } else {
                ZoomAnchor::Column(rng.below(5_000) as u32)
            },
        },
        7 => Command::Pan {
            pixels: [i64::MIN, -3_000, -1, 0, 7, 3_000, i64::MAX][rng.below(7) as usize],
        },
        8 => Command::PanStep(
            [
                PanStep::Left,
                PanStep::Right,
                PanStep::PageLeft,
                PanStep::PageRight,
            ][rng.below(4) as usize],
        ),
        9 => Command::FitAll,
        10 => Command::GoToStart,
        11 => Command::GoToEnd,
        12 => Command::ZoomToRange {
            start: [0, position, u64::MAX][rng.below(3) as usize],
            end: [0, position, u64::MAX][rng.below(3) as usize],
        },
        13 => Command::SetFollowLatest(rng.below(2) == 0),
        14 => Command::RenameChannel {
            channel,
            name: ["", "SDA", "Часы", "\u{202e}x\u{0}", &"w".repeat(500)][rng.below(5) as usize]
                .into(),
        },
        15 => Command::SetChannelColour {
            channel,
            colour: if rng.below(2) == 0 {
                Colour::Palette(rng.below(40) as u8)
            } else {
                Colour::Rgb(1, 2, 3)
            },
        },
        16 => Command::SetChannelVisible {
            channel,
            visible: rng.below(2) == 0,
        },
        17 => Command::MoveChannel {
            from: rng.below(40) as usize,
            to: rng.below(40) as usize,
        },
        18 => Command::ResetPresentation,
        19..=21 => Command::PlaceMarker { position, channel },
        22 => Command::MoveMarker {
            id: rng.below(80) as u32,
            position,
            channel,
        },
        23 => Command::RenameMarker {
            id: rng.below(80) as u32,
            name: "m".into(),
        },
        24 => Command::RemoveMarker(rng.below(80) as u32),
        25 => Command::RemoveAllMarkers,
        26 => Command::SelectMarkers {
            first: Some(rng.below(80) as u32),
            second: [None, Some(rng.below(80) as u32)][rng.below(2) as usize],
        },
        27 => Command::GoToMarker(rng.below(80) as u32),
        28 => Command::SetSnap(rng.below(2) == 0),
        29..=30 => Command::SaveSessionAs(files.targets[rng.below(3) as usize].clone()),
        31 => Command::SaveSession,
        32..=33 => Command::OpenSession(files.good.clone()),
        34 => Command::OpenSession(files.damaged[rng.below(4) as usize].clone()),
        35 => Command::OpenSession(files.missing.clone()),
        36 => Command::CancelOperation,
        37..=39 => Command::Decide(
            [Decision::Save, Decision::Discard, Decision::Cancel][rng.below(3) as usize],
        ),
        40 => Command::SetLanguage(Language::ALL[rng.below(6) as usize]),
        41 => Command::SetTheme(
            [
                Theme::System,
                Theme::Light,
                Theme::Dark,
                Theme::HighContrast,
            ][rng.below(4) as usize],
        ),
        42 => Command::SetUiScale([0, 50, 100, 150, 200, 500, u16::MAX][rng.below(7) as usize]),
        43 => Command::SetMemoryLimit(
            [0, 1, 64 << 20, 256 << 20, 1 << 30, u64::MAX][rng.below(6) as usize],
        ),
        44 => Command::SetViewSize(ViewSize {
            pixels: [0, 1, 7, 300, 800, 2_000, 16_384, 100_000, u32::MAX][rng.below(9) as usize],
            scale: 1.0,
        }),
        45 => Command::EditConfig(match rng.below(6) {
            0 => ConfigEdit::Channels([0, 1, 3, 0xffff, u32::MAX][rng.below(5) as usize]),
            1 => ConfigEdit::SampleRate([0, 1, 20_000, 1_000_000, u64::MAX][rng.below(5) as usize]),
            2 => ConfigEdit::Length([0, 1, 3_000, 50_000, 200_000][rng.below(5) as usize]),
            3 => ConfigEdit::Seed(if rng.below(2) == 0 {
                None
            } else {
                Some(rng.next())
            }),
            4 => ConfigEdit::Profile(
                [
                    SignalProfile::Sparse,
                    SignalProfile::Normal,
                    SignalProfile::Dense,
                ][rng.below(3) as usize],
            ),
            _ => ConfigEdit::Realtime(rng.below(4) == 0),
        }),
        46 => Command::SelectDevice(DeviceId::new("nowhere", &rng.below(5).to_string())),
        47 => Command::RefreshDevices,
        48 => Command::DismissAllNotices,
        49 => Command::DismissNotice(la_host::notices::NoticeId(rng.below(60))),
        50 => Command::TestError(
            [
                "storage.memory_limit",
                "device.disconnected",
                "host.no_such_code",
            ][rng.below(3) as usize],
        ),
        51 if rng.below(8) == 0 => Command::TestPanic,
        52 => Command::RequestClose,
        53 => Command::Quit,
        _ => Command::StartCapture,
    }
}

fn walk(seed: u64, steps: u64) {
    let threads_before = thread_count();
    {
        let dir = TempDir::new("walk");
        let files = make_files(&dir);
        let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0b11),
            ConfigEdit::SampleRate(1_000_000),
            ConfigEdit::Length(3_000),
            ConfigEdit::Seed(Some(seed)),
        ] {
            shell.apply(Command::EditConfig(edit));
        }
        shell.apply(Command::SetViewSize(ViewSize {
            pixels: 800,
            scale: 1.0,
        }));
        let mut rng = Lcg(seed);
        let mut history: Vec<String> = Vec::new();
        let mut discard_open = false;
        let mut at_risk_steps = 0_u64;
        for step in 0..steps {
            let fail = |what: String, history: &[String]| -> ! {
                panic!(
                    "seed {seed}, action {step}: {what}\nlast actions: {:#?}",
                    &history[history.len().saturating_sub(10)..]
                )
            };
            let before = identity(&shell);
            let at_risk = before.is_some() && shell.state().unsaved != Unsaved::Clean;
            let saving_before = shell
                .state()
                .operation
                .is_some_and(|o| o.kind == OperationKind::Save);
            let notices_before = notice_total(&shell);
            if !shell.operation_running() {
                discard_open = false;
            }
            let pending_before = shell.state().pending_decision.clone();
            let before_state = format!(
                "before: unsaved {:?}, activity {:?}, operation {:?}, pending {:?}, path {:?}, discard_open {discard_open}",
                shell.state().unsaved,
                shell.state().activity,
                shell.state().operation.map(|o| o.kind),
                shell.state().pending_decision,
                shell.state().session_path
            );

            // Sometimes the next save meets a failing disk, sometimes the device fails.
            match rng.below(200) {
                0 => shell.set_session_fs(Arc::new(FaultFs::failing(FaultPlan {
                    op: [Op::Create, Op::Write, Op::SyncAll, Op::Rename][rng.below(4) as usize],
                    nth: 0,
                    kind: std::io::ErrorKind::Other,
                    partial_write: false,
                }))),
                1 => shell.set_session_fs(Arc::new(RealFileSystem)),
                2 => shell.script_failures(vec![ScriptedFailure {
                    position: rng.below(2_000),
                    kind: [
                        FailureKind::Loss(10),
                        FailureKind::Overrun(5),
                        FailureKind::TransportError,
                        FailureKind::Disconnect,
                    ][rng.below(4) as usize],
                }]),
                3 => shell.script_failures(Vec::new()),
                _ => {}
            }

            let command = random_action(&mut rng, &files);
            let discarding = matches!(command, Command::Decide(Decision::Discard));
            history.push(format!("{step}: {command:?}"));
            let outcome = shell.apply(command);
            if let Some(last) = history.last_mut() {
                last.push_str(&format!(" -> {outcome:?}"));
            }
            let discarded_now = discarding && matches!(outcome, la_host::Outcome::Done(_));
            if discarded_now && matches!(pending_before, Some(la_host::Guarded::OpenSession(_))) {
                discard_open = true;
            }
            if rng.below(3) == 0 {
                shell.poll(Instant::now());
            }
            if rng.below(50) == 0 {
                std::thread::sleep(Duration::from_millis(2));
            }
            if rng.below(97) == 0 {
                // Drawing must never fail, whatever the state.
                let plan = shell.frame_plan(ViewSize {
                    pixels: [1, 300, 2_000][rng.below(3) as usize],
                    scale: 1.0,
                });
                if plan.channels.len() > 32 {
                    fail("a plan with more than 32 channels".into(), &history);
                }
            }

            if let Err(what) = check_invariants(&shell) {
                fail(what, &history);
            }
            if at_risk {
                at_risk_steps += 1;
                let replaced = identity(&shell) != before;
                let saved_now = saving_before
                    && !shell.operation_running()
                    && notice_total(&shell) <= notices_before;
                if replaced && !(discarded_now || discard_open || saved_now) {
                    fail(
                        format!(
                            "unsaved data was replaced without a decision or a save; {before_state}; after: unsaved {:?}, operation {:?}",
                            shell.state().unsaved,
                            shell.state().operation.map(|o| o.kind)
                        ),
                        &history,
                    );
                }
            }
        }
        assert!(
            at_risk_steps > steps / 20,
            "the walk rarely held unsaved data ({at_risk_steps} of {steps})"
        );

        // Wind down: nothing is left running and the shell can be dropped.
        shell.set_session_fs(Arc::new(RealFileSystem));
        shell.apply(Command::Decide(Decision::Cancel));
        shell.apply(Command::StopCapture);
        let deadline = Instant::now() + Duration::from_secs(60);
        while shell.state().activity != Activity::Idle || shell.operation_running() {
            shell.poll(Instant::now());
            assert!(Instant::now() < deadline, "seed {seed}: did not settle");
            std::thread::sleep(Duration::from_millis(3));
        }
    }
    // No thread is left behind (counted where the system lets us).
    if let Some(before) = threads_before {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let now = thread_count().unwrap_or(before);
            if now <= before {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "seed {seed}: {now} threads at the end, {before} at the start"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn a_long_random_walk_keeps_every_invariant_and_loses_no_data() {
    walk(
        setting("LA_WALK_SEED", 2026),
        setting("LA_WALK_STEPS", 100_000),
    );
}

#[test]
#[ignore = "two million actions; the nightly workflow runs it"]
fn a_very_long_random_walk() {
    walk(
        setting("LA_WALK_SEED", 4711),
        setting("LA_WALK_STEPS", 2_000_000),
    );
}
