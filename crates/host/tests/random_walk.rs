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
use la_decoders::setup::{Source, Value};
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
use la_session::{Control, LoadLimits, Section, SectionTag, SessionData};

/// The files the walk opens.
struct Files {
    good: PathBuf,
    damaged: Vec<PathBuf>,
    missing: PathBuf,
    targets: Vec<PathBuf>,
    /// Files that open with decoder sections: good, with results, damaged in each way, unknown.
    decoders: Vec<PathBuf>,
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
    // Files with decoder sections, made from a session that has decoders and results.
    let mut with_decoders = finished_shell(&source_dir, 0b11, 4_000, 8);
    with_decoders.apply(Command::AddDecoder {
        decoder: la_decoders::decoders::clocked_bits::ID.into(),
    });
    let id = with_decoders.state().decoders.instances[0].id;
    for (role, channel) in [("clock", 0), ("data", 1)] {
        with_decoders.apply(Command::SetDecoderChannel {
            id,
            role: role.into(),
            channel: Some(channel),
        });
    }
    with_decoders.apply(Command::SetSaveDecoderResults(true));
    let deadline = Instant::now() + Duration::from_secs(30);
    while with_decoders.state().decoders.instances[0].state
        == la_host::decoders::InstanceState::Running
        && Instant::now() < deadline
    {
        with_decoders.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    let rich = dir.path().join("decoders.lasession");
    with_decoders.apply(Command::SaveSessionAs(rich.clone()));
    wait_operation(&mut with_decoders);
    let loaded = la_session::load_blocking(&rich, &LoadLimits::default(), &Control::new()).unwrap();
    let payload = |tag: &str| {
        loaded
            .extras()
            .iter()
            .find(|s| s.tag.as_str() == tag)
            .map(|s| s.payload.clone())
            .unwrap_or_default()
    };
    let (deco, dann) = (payload("DECO"), payload("DANN"));
    let rewritten = |name: &str, deco: Vec<u8>, dann: Vec<u8>| {
        let mut data = SessionData::new(loaded.description().clone(), loaded.snapshot());
        for (tag, payload) in [("DECO", deco), ("DANN", dann)] {
            if !payload.is_empty() {
                data = data
                    .with_section(Section {
                        tag: SectionTag::new(tag).unwrap(),
                        critical: false,
                        payload,
                    })
                    .unwrap();
            }
        }
        let path = dir.path().join(name);
        la_session::save_blocking(&path, &data, &Control::new()).unwrap();
        path
    };
    let mut flipped_deco = deco.clone();
    let middle = flipped_deco.len() / 2;
    flipped_deco[middle] ^= 0xff;
    let mut flipped_dann = dann.clone();
    let middle = flipped_dann.len() / 2;
    flipped_dann[middle] ^= 0xff;
    let decoders = vec![
        rich,
        rewritten("bad-deco.lasession", vec![1, 200, 7], dann.clone()),
        rewritten("bad-dann.lasession", deco.clone(), vec![1, 0, 9, 9]),
        rewritten("flipped-deco.lasession", flipped_deco, dann.clone()),
        rewritten("flipped-dann.lasession", deco.clone(), flipped_dann),
        rewritten("newer-deco.lasession", vec![2, 1, 2, 3], Vec::new()),
        rewritten("setup-only.lasession", deco, Vec::new()),
    ];
    Files {
        good,
        decoders,
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
    // Markers: every invariant of the model.
    let markers = &state.markers;
    markers.check(capture.map_or(0, |c| c.snapshot.length()))?;
    if capture.is_none() && !markers.list.is_empty() {
        return Err("markers without a capture".into());
    }
    // Placing and the view an automatic zoom replaced.
    if state.placement.is_some() && capture.is_none() {
        return Err("a placement without a capture".into());
    }
    if state.snap_zoom.is_some() && state.placement.is_none() && state.drag.is_none() {
        return Err("a saved view with nothing being placed or dragged".into());
    }
    if state.drag.is_some_and(|(id, _)| markers.get(id).is_none()) {
        return Err("a drag of a marker that does not exist".into());
    }
    if state.activity == la_host::Activity::Cropping && state.operation.is_none() {
        return Err("a cut without its operation".into());
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
    // The presentation describes the channels of the capture on screen, or else those of the
    // selected device; what the user set stays across captures.
    let expected = capture.map_or_else(
        || {
            state
                .selected_device
                .as_ref()
                .and_then(|id| state.devices.iter().find(|d| d.id == *id))
                .map(|d| d.capabilities.channels)
        },
        |c| Some(c.snapshot.channels()),
    );
    if let Some(expected) = expected {
        if styles.len() != usize::from(expected) {
            return Err(format!(
                "the presentation has {} channels, expected {expected}",
                styles.len()
            ));
        }
    }
    // The one flag of a channel is both "shown" and "recorded by the next capture". (For the
    // channels of the selected device: the flags of channels beyond it only describe a capture
    // opened from a file made with a bigger device.)
    let device = state
        .selected_device
        .as_ref()
        .and_then(|id| state.devices.iter().find(|d| d.id == *id))
        .map_or(8, |d| d.capabilities.channels);
    let limit = if device >= 32 {
        u32::MAX
    } else {
        (1_u32 << device) - 1
    };
    if state.presentation.visible_mask() & limit != state.config.enabled_channels & limit {
        return Err(format!(
            "the flags {:#x} differ from the recording mask {:#x}",
            state.presentation.visible_mask() & limit,
            state.config.enabled_channels & limit
        ));
    }
    // Activity and unsaved state.
    // A save or load runs with the activity idle; a cut runs with the activity `Cropping`, and it may
    // well still be running when the action is over (it runs in the background).
    if let Some(operation) = state.operation {
        let cut =
            operation.kind == la_host::OperationKind::Crop && state.activity == Activity::Cropping;
        if state.activity != Activity::Idle && !cut {
            return Err("a save or load while capturing".into());
        }
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
    if let Some(style) = state
        .presentation
        .channels
        .iter()
        .find(|c| c.height != 0 && !(24..=240).contains(&c.height))
    {
        return Err(format!("the height {} is out of range", style.height));
    }
    if state.settings.recent_files.len() > la_host::settings::MAX_RECENT_FILES {
        return Err("too many recent files".into());
    }
    check_decoders(shell)
}

/// What must hold for the decoders after every action.
fn check_decoders(shell: &Shell) -> Result<(), String> {
    use la_host::decoders::InstanceState;
    let state = shell.state();
    let decoders = &state.decoders;
    if decoders.instances.len() > la_decoders::setup::MAX_INSTANCES {
        return Err(format!("{} instances", decoders.instances.len()));
    }
    if state.capture.is_none() && !decoders.instances.is_empty() {
        return Err("decoders without a capture".into());
    }
    let channels = state.capture.as_ref().map_or(0, |c| c.snapshot.channels());
    let mut ids = BTreeSet::new();
    for instance in &decoders.instances {
        if !ids.insert(instance.id) {
            return Err(format!("instance id {} repeats", instance.id));
        }
        if instance.setup.name.chars().count() > 64
            || instance.setup.name.chars().any(char::is_control)
        {
            return Err("an unsafe instance name".into());
        }
        if instance
            .setup
            .channels
            .values()
            .any(|c| *c >= channels.max(1))
            && channels > 0
        {
            return Err(format!(
                "instance {} uses a channel the capture lacks",
                instance.id
            ));
        }
        if instance.state == InstanceState::Running && !instance.ready {
            return Err(format!(
                "instance {} runs without a complete setup",
                instance.id
            ));
        }
        if instance.fault.is_some() && instance.state != InstanceState::Failed {
            return Err(format!(
                "instance {} has a fault but is {:?}",
                instance.id, instance.state
            ));
        }
        if instance.loaded && instance.state != InstanceState::Finished {
            return Err(format!(
                "instance {} is loaded but {:?}",
                instance.id, instance.state
            ));
        }
        if instance.outdated && shell.decoder_results(instance.id).is_none() {
            return Err(format!(
                "instance {} is outdated with nothing to show",
                instance.id
            ));
        }
        if let Source::Instance(below) = instance.setup.source {
            if !decoders.instances.iter().any(|i| i.id == below) {
                return Err(format!(
                    "instance {} reads from a missing instance",
                    instance.id
                ));
            }
        }
    }
    // Stacks are acyclic and at most four deep.
    for instance in &decoders.instances {
        let mut depth = 1;
        let mut current = instance.setup.source;
        while let Source::Instance(below) = current {
            depth += 1;
            if depth > la_decoders::setup::MAX_STACK_DEPTH {
                return Err(format!(
                    "instance {} is in a stack that is too deep or a cycle",
                    instance.id
                ));
            }
            current = decoders
                .instances
                .iter()
                .find(|i| i.id == below)
                .map_or(Source::Capture, |i| i.setup.source);
        }
    }
    if let Some(selection) = &decoders.selection {
        if !ids.contains(&selection.instance) {
            return Err("the selection names an instance that does not exist".into());
        }
    }
    Ok(())
}

/// One of the ids of the instances on screen, or one that does not exist.
fn some_instance(rng: &mut Lcg, shell: &Shell) -> u32 {
    let ids: Vec<u32> = shell
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.id)
        .collect();
    if ids.is_empty() || rng.below(10) == 0 {
        return rng.below(30) as u32;
    }
    ids[rng.below(ids.len() as u64) as usize]
}

/// An action on the decoders.
fn decoder_action(rng: &mut Lcg, shell: &Shell, files: &Files) -> Command {
    const DECODERS: [&str; 12] = [
        "sample.clocked_bits",
        "test.lower_bytes",
        "test.upper_words",
        "test.healthy",
        "test.slow",
        "test.returns_error",
        "test.panics",
        "test.never_returns",
        "test.grows_memory",
        "test.too_many_annotations",
        "test.invalid_output",
        "test.nothing",
    ];
    let id = some_instance(rng, shell);
    match rng.below(24) {
        0..=3 => Command::AddDecoder {
            decoder: DECODERS[rng.below(DECODERS.len() as u64) as usize].into(),
        },
        4..=7 => Command::SetDecoderChannel {
            id,
            role: ["clock", "data", "mystery"][rng.below(3) as usize].into(),
            channel: [None, Some(0), Some(1), Some(rng.below(40) as u8)][rng.below(4) as usize],
        },
        8 => Command::SetDecoderOption {
            id,
            option: ["bits", "idle", "bit_order", "nothing"][rng.below(4) as usize].into(),
            value: match rng.below(3) {
                0 => Value::Integer(rng.below(40) as i64 - 5),
                1 => Value::Choice(["msb", "lsb", "rising", "?"][rng.below(4) as usize].into()),
                _ => Value::Boolean(rng.below(2) == 0),
            },
        },
        9 => Command::RenameDecoder {
            id,
            name: ["", "Bus", "\u{202e}x\u{0}", &"n".repeat(100)][rng.below(4) as usize].into(),
        },
        10 => Command::ShowDecoderLane {
            id,
            visible: rng.below(2) == 0,
        },
        11 => Command::RemoveDecoder(id),
        12 => Command::StackDecoder {
            id,
            source: if rng.below(3) == 0 {
                Source::Capture
            } else {
                Source::Instance(some_instance(rng, shell))
            },
        },
        13 | 14 => Command::RunDecoder(id),
        15 => Command::CancelDecoder(id),
        16 => Command::SetSaveDecoderResults(rng.below(2) == 0),
        17 => Command::SetDecoderMemoryLimit(
            [0, 16 << 20, 32 << 20, 64 << 20, 256 << 20, u64::MAX][rng.below(6) as usize],
        ),
        18 => Command::SelectAnnotation {
            instance: id,
            annotation: la_host::decoders::AnnotationRef {
                target: la_decoders::model::Target::Lane,
                id: rng.below(50),
                start: rng.below(4_000),
            },
        },
        19 => Command::GoToAnnotation(
            [
                la_host::AnnotationTarget::Selected,
                la_host::AnnotationTarget::Next,
                la_host::AnnotationTarget::Previous,
            ][rng.below(3) as usize],
        ),
        20 => Command::ClearAnnotationSelection,
        21..=22 => Command::OpenSession(
            files.decoders[rng.below(files.decoders.len() as u64) as usize].clone(),
        ),
        _ => Command::SetLanguage(Language::ALL[rng.below(6) as usize]),
    }
}

/// A pointer anywhere over (and beyond) the signal area, over a lane or not.
fn random_pointer(rng: &mut Lcg, channel: u8) -> la_host::markers::Pointer {
    la_host::markers::Pointer {
        x: [-50.0, f32::NAN, 0.0, 400.0, 799.0, 5_000.0][rng.below(6) as usize]
            + rng.below(100) as f32,
        lane: [None, Some(channel)][rng.below(2) as usize],
    }
}

/// Any command about markers: placing, dragging, the actions of both kinds, the cuts.
fn marker_action(rng: &mut Lcg, channel: u8) -> Command {
    use la_host::markers::{End, Kind};
    let id = rng.below(90) as u32;
    match rng.below(26) {
        0..=2 => Command::BeginPlacement(if rng.below(2) == 0 {
            Kind::Single
        } else {
            Kind::Pair
        }),
        3..=6 => Command::FixPlacement {
            pointer: random_pointer(rng, channel),
        },
        7 => Command::CancelPlacement,
        8 | 9 => Command::MoveMarkerEnd {
            id,
            end: [End::Point, End::Start, End::End][rng.below(3) as usize],
            pointer: random_pointer(rng, channel),
        },
        10 => Command::EndMarkerDrag,
        11 => Command::PlacementZoomTo {
            start: rng.below(8_000),
            length: [0, 1, 40, 800, 9_000, u64::MAX][rng.below(6) as usize],
        },
        12 => Command::SetMarkerColour {
            id,
            colour: rng.below(10) as u8,
        },
        13 => Command::SetMarkerComment {
            id,
            comment: ["m", "", "\u{202e}x\u{0}", &"c".repeat(200)][rng.below(4) as usize].into(),
        },
        14 => Command::SelectMarker([None, Some(id)][rng.below(2) as usize]),
        15 => Command::SetTimeOrigin(id),
        16 => Command::ResetTimeOrigin,
        17 => Command::CutLeft(id),
        18 => Command::CutRight(id),
        19 => Command::TrimToPair(id),
        20 => Command::ScrollToMarker(id),
        21 => Command::FitToPair(id),
        22 => Command::SplitPair(id),
        23 => Command::MakePair {
            first: id,
            second: rng.below(90) as u32,
        },
        24 => Command::RemoveMarker(id),
        _ => [
            Command::RemoveAllMarkers,
            Command::SetSnap(rng.below(2) == 0),
        ][rng.below(2) as usize]
            .clone(),
    }
}

fn random_action(rng: &mut Lcg, files: &Files, shell: &Shell) -> Command {
    let channel = rng.below(40) as u8; // some beyond the capture: invalid on purpose
    let position = rng.below(6_000);
    match rng.below(100) {
        64..=99 => decoder_action(rng, shell, files),
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
        19..=28 => marker_action(rng, channel),
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
        // These write the settings file, so they are drawn rarely: a slow disk would otherwise
        // dominate the time of the walk.
        54 if rng.below(8) == 0 => Command::SetSidePanel(
            [
                la_host::SidePanel::Capture,
                la_host::SidePanel::Decoders,
                la_host::SidePanel::Markers,
                la_host::SidePanel::Hidden,
            ][rng.below(4) as usize],
        ),
        55..=57 => Command::ScrollToEdge {
            channel,
            direction: if rng.below(2) == 0 {
                la_host::EdgeDirection::Next
            } else {
                la_host::EdgeDirection::Previous
            },
        },
        58 if rng.below(8) == 0 => Command::RemoveRecentFile(match rng.below(3) {
            0 => files.good.clone(),
            1 => files.targets[rng.below(3) as usize].clone(),
            _ => files.missing.clone(),
        }),
        59 if rng.below(8) == 0 => Command::ClearRecentFiles,
        60..=61 => Command::SetChannelHeight {
            channel,
            height: [0, 1, 24, 100, 240, 241, u16::MAX][rng.below(7) as usize],
        },
        _ => Command::StartCapture,
    }
}

fn walk(seed: u64, steps: u64) {
    let threads_before = thread_count();
    {
        let dir = TempDir::new("walk");
        let files = make_files(&dir);
        let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
        for (descriptor, create) in la_decoders::testing::fixtures() {
            shell.register_decoder(descriptor, create).unwrap();
        }
        shell.set_decoder_chunk_limits(la_decoders::run::ChunkLimits {
            max_edges: 200,
            max_span: 1_000,
        });
        shell.set_decoder_deadlines(la_decoders::run::Deadlines {
            chunk: Duration::from_millis(300),
            cancel: Duration::from_millis(300),
        });
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
        let mut limit_max = la_decoders::store::DEFAULT_MEMORY_LIMIT;
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

            let command = random_action(&mut rng, &files, &shell);
            if let Command::SetDecoderMemoryLimit(bytes) = &command {
                if la_host::settings::decoder_memory_limit_is_valid(*bytes) {
                    limit_max = limit_max.max(*bytes);
                }
            }
            let discarding = matches!(command, Command::Decide(Decision::Discard));
            // A save that starts in this step may also end in it (a fast disk and a poll), and the
            // command that waited for the save then goes on: that is a save, not a loss.
            let starts_save = matches!(
                command,
                Command::SaveSession | Command::SaveSessionAs(_) | Command::Decide(Decision::Save)
            );
            let scrolled = matches!(command, Command::ScrollToEdge { .. });
            history.push(format!("{step}: {command:?}"));
            let outcome = shell.apply(command);
            if scrolled && matches!(outcome, la_host::Outcome::Done(_)) {
                // A scroll to an edge turns following the newest data off and keeps the zoom valid.
                if shell.state().follow_latest {
                    fail("following is still on after a scroll".into(), &history);
                }
            }
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
            if shell.decoder_memory_used() > limit_max {
                fail(
                    format!(
                        "the results of the decoders use {} bytes, the largest limit was {limit_max}",
                        shell.decoder_memory_used()
                    ),
                    &history,
                );
            }
            if rng.below(1_500) == 0 {
                // Let the decoders that never return end, so that threads do not pile up.
                la_decoders::testing::release_stuck_decoders();
                std::thread::sleep(Duration::from_millis(20));
                la_decoders::testing::hold_stuck_decoders();
            }
            if at_risk {
                at_risk_steps += 1;
                let replaced = identity(&shell) != before;
                let saved_now = (saving_before
                    || (starts_save && matches!(outcome, la_host::Outcome::Done(_))))
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
        la_decoders::testing::release_stuck_decoders();
        let deadline = Instant::now() + Duration::from_secs(60);
        while shell.state().activity != Activity::Idle
            || shell.operation_running()
            || shell.state().decoders.instances.iter().any(|i| {
                i.state == la_host::decoders::InstanceState::Running
                    || (i.state == la_host::decoders::InstanceState::Waiting && i.ready)
            })
        {
            shell.poll(Instant::now());
            assert!(
                Instant::now() < deadline,
                "seed {seed}: did not settle; {}",
                unsettled(&shell)
            );
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

/// What still keeps the wind-down of the walk waiting, for the message of a timeout (a flake seen
/// on Debian whose cause is not known: the message must say which condition held).
fn unsettled(shell: &Shell) -> String {
    let state = shell.state();
    let instances: Vec<String> = state
        .decoders
        .instances
        .iter()
        .map(|i| {
            format!(
                "#{} {:?} ready={} covered={}/{} loaded={} fault={:?}",
                i.id, i.state, i.ready, i.covered, i.length, i.loaded, i.fault
            )
        })
        .collect();
    format!(
        "activity {:?}, operation {:?}, running {}, pending decision {:?}, instances {instances:#?}",
        state.activity,
        state
            .operation
            .map(|o| (o.kind, o.done_bytes, o.total_bytes)),
        shell.operation_running(),
        state.pending_decision
    )
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
