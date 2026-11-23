//! Chains of two to five boards as one device (feature 017, T061): the channels of the boards in order, the
//! words of the boards merged, the slaves armed before the master, the shift of a board, the triggers a chain
//! allows, a missing or silent board named, boards of different firmware refused, and nothing captured as if the
//! chain were whole. The boards are simulated (`tests/fixtures/`, synthetic until the firmware runs).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;

use la_core::Error;
use la_devices::{
    ChannelSet, DeviceConfig, DeviceDescriptor, DeviceOptions, DeviceProvider, Pattern,
    ReadOutcome, Trigger, TriggerKind, validate,
};
use la_driver_rp2350b::error::{CHAIN_INCOMPLETE, CHAIN_MISMATCH};
use la_driver_rp2350b::link::PortInfo;
use la_driver_rp2350b::testing::{ScriptedPorts, Step, parse_recording};
use la_driver_rp2350b::{ChainDefinition, Rp2350bProvider, USB_PID, USB_VID};
use la_proto::{CaptureRequest2, Event, FrameParser, Role, TriggerType};
use la_storage::Limits;

fn fixture(name: &str) -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    parse_recording(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn port(k: usize) -> PortInfo {
    PortInfo {
        name: format!("COM{}", k + 1),
        vid: USB_VID,
        pid: USB_PID,
        serial: Some(format!("S{}", k + 1)),
    }
}

fn definition(boards: usize, shifts: &[u32]) -> ChainDefinition {
    ChainDefinition {
        name: "lab".to_owned(),
        boards: (1..=boards).map(|k| format!("rp2350b:S{k}")).collect(),
        shifts: shifts.to_vec(),
    }
}

/// `boards` boards on COM1.. whose sessions are an information reply of `infos[k]` and the capture `captures[k]`,
/// and the chain of all of them with these shifts.
fn world(
    infos: &[&str],
    captures: &[&str],
    shifts: &[u32],
) -> (Rp2350bProvider, Arc<ScriptedPorts>) {
    let ports = Arc::new(ScriptedPorts::new());
    for k in 0..infos.len() {
        ports.add_port(port(k), vec![fixture(infos[k]), fixture(captures[k])]);
    }
    let provider = Rp2350bProvider::with_backend(ports.clone());
    provider.set_chains(vec![definition(infos.len(), shifts)]);
    (provider, ports)
}

fn plain_world(boards: usize, shifts: &[u32]) -> (Rp2350bProvider, Arc<ScriptedPorts>) {
    let infos = vec!["info_32ch.rec"; boards];
    let captures: Vec<String> = (0..boards).map(|k| format!("chain_board{k}.rec")).collect();
    let captures: Vec<&str> = captures.iter().map(String::as_str).collect();
    world(&infos, &captures, shifts)
}

fn chain_of(list: &[DeviceDescriptor]) -> &DeviceDescriptor {
    list.iter()
        .find(|d| d.id.as_str() == "rp2350b:chain-lab")
        .expect("the chain is listed")
}

fn config(
    channels: ChannelSet,
    trigger: Option<Trigger>,
    pattern: Option<Pattern>,
) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: channels,
        sample_rate_hz: 1_000_000,
        trigger,
        pattern,
        capture_length: Some(16),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

/// What a run of the chain gave: the pieces seen (a loss or samples, and how many), the words, and how it ended.
type Run = (Vec<(bool, usize)>, Vec<u32>, Result<(), Error>);

/// Opens the chain, starts it and reads it to the end: the pieces seen and the words.
fn run(provider: &Rp2350bProvider, config: &DeviceConfig) -> Run {
    let list = provider.scan().unwrap();
    let chain = chain_of(&list);
    let resolved = validate(&chain.capabilities, config, &Limits::default()).unwrap();
    let stride = usize::from(chain.capabilities.channels / 32);
    let mut device = provider.open(&chain.id).unwrap();
    if let Err(error) = device.start(&resolved) {
        return (Vec::new(), Vec::new(), Err(error));
    }
    let (mut seen, mut words) = (Vec::new(), Vec::new());
    for _ in 0..10_000 {
        let mut piece = Vec::new();
        match device.read(1000, &mut piece) {
            Ok(ReadOutcome::Samples) => {
                seen.push((false, piece.len() / stride));
                words.extend(piece);
            }
            Ok(ReadOutcome::Lost(n)) => seen.push((true, usize::try_from(n).unwrap())),
            Ok(ReadOutcome::Finished) => {
                device.stop().unwrap();
                return (seen, words, Ok(()));
            }
            Ok(ReadOutcome::Idle) => {}
            Err(error) => return (seen, words, Err(error)),
        }
    }
    panic!("the chain neither ended nor failed");
}

fn word(board: u32, i: u32) -> u32 {
    ((board + 1) << 24) + i
}

fn request_of(ports: &ScriptedPorts, name: &str) -> CaptureRequest2 {
    let writes = ports.writes(name);
    let frame = writes.get(1).expect("the capture request was written");
    let mut parser = FrameParser::new();
    let mut body = None;
    parser.feed(frame, |event| {
        if let Event::Frame(b) = event {
            body = Some(b.to_vec());
        }
    });
    CaptureRequest2::from_bytes(&body.unwrap()[1..]).unwrap()
}

#[test]
fn a_chain_has_the_channels_of_its_boards_in_order() {
    for boards in 2..=5_usize {
        let (provider, _) = plain_world(boards, &vec![0; boards]);
        let list = provider.scan().unwrap();
        let chain = chain_of(&list);
        chain.validate().unwrap();
        assert_eq!(usize::from(chain.capabilities.channels), 32 * boards);
        assert!(!chain.simulated);
        assert!(chain.name.contains("lab") && chain.name.contains(&boards.to_string()));
        // The boards are still listed on their own.
        assert_eq!(list.len(), boards + 1);
    }
}

#[test]
fn a_chain_is_listed_only_while_all_its_boards_are_present() {
    let (provider, ports) = plain_world(3, &[0, 0, 0]);
    assert_eq!(provider.scan().unwrap().len(), 4);
    ports.remove_port("COM2");
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 2, "two boards and no chain");
    assert!(list.iter().all(|d| !d.id.as_str().contains("chain")));
}

#[test]
fn a_definition_that_cannot_be_used_makes_no_chain() {
    let ports = Arc::new(ScriptedPorts::new());
    for k in 0..6 {
        ports.add_port(port(k), vec![fixture("info_32ch.rec")]);
    }
    let provider = Rp2350bProvider::with_backend(ports);
    let named = |name: &str, boards: &[&str], shifts: &[u32]| ChainDefinition {
        name: name.to_owned(),
        boards: boards.iter().map(|b| format!("rp2350b:{b}")).collect(),
        shifts: shifts.to_vec(),
    };
    provider.set_chains(vec![
        named("one", &["S1"], &[0]),
        named("twice", &["S1", "S1"], &[0, 0]),
        named("no-shifts", &["S1", "S2"], &[0]),
        named("six", &["S1", "S2", "S3", "S4", "S5", "S6"], &[0; 6]),
        named("", &["S1", "S2"], &[0, 0]),
        named("good", &["S1", "S2"], &[0, 0]),
        named("good", &["S3", "S4"], &[0, 0]),
        named("shares", &["S2", "S3"], &[0, 0]),
    ]);
    let list = provider.scan().unwrap();
    let chains: Vec<&str> = list
        .iter()
        .map(|d| d.id.as_str())
        .filter(|id| id.contains("chain-"))
        .collect();
    assert_eq!(
        chains,
        ["rp2350b:chain-good"],
        "only the first good one, with its boards"
    );
}

#[test]
fn the_words_of_the_boards_are_merged_one_word_per_board() {
    for boards in 2..=5_u32 {
        let (provider, _) = plain_world(boards as usize, &vec![0; boards as usize]);
        let all = ChannelSet::all(u8::try_from(32 * boards).unwrap());
        let (seen, words, result) = run(&provider, &config(all, None, None));
        result.unwrap();
        assert_eq!(seen.iter().map(|(_, n)| n).sum::<usize>(), 16);
        assert_eq!(
            words.len(),
            16 * boards as usize,
            "words_per_sample is the number of boards"
        );
        for i in 0..16_u32 {
            for k in 0..boards {
                assert_eq!(
                    words[(i * boards + k) as usize],
                    word(k, i),
                    "sample {i}, board {k}"
                );
            }
        }
    }
}

#[test]
fn the_slaves_are_armed_before_the_master_and_each_board_gets_its_own_request() {
    let (provider, ports) = plain_world(3, &[0, 0, 0]);
    let pattern = Pattern {
        base_channel: 4,
        length: 3,
        value: 0b101,
    };
    let trigger = Trigger {
        channel: 4,
        kind: TriggerKind::Pattern,
    };
    // Channels: all of board 0, channels 40 and 63 of board 1, none of board 2.
    let mut enabled = ChannelSet::all(32);
    enabled.insert(40);
    enabled.insert(63);
    let (_, _, result) = run(&provider, &config(enabled, Some(trigger), Some(pattern)));
    result.unwrap();
    let order = ports.write_order();
    // The three probes first, then the requests: the slaves, the master last.
    assert_eq!(&order[3..6], ["COM2", "COM3", "COM1"]);
    let (master, second, third) = (
        request_of(&ports, "COM1"),
        request_of(&ports, "COM2"),
        request_of(&ports, "COM3"),
    );
    assert_eq!(master.role, Role::Master);
    assert_eq!(master.trigger_type, TriggerType::Pattern);
    assert_eq!(
        (
            master.trigger_channel,
            master.trigger_length,
            master.trigger_value
        ),
        (4, 3, 0b101)
    );
    assert_eq!(master.channel_count, 32);
    for slave in [&second, &third] {
        assert_eq!(slave.role, Role::Slave);
        assert_eq!(
            slave.trigger_type,
            TriggerType::Blast,
            "a slave starts on the master's line"
        );
    }
    assert_eq!(
        second.used_channels(),
        [8, 31],
        "channels 40 and 63 are 8 and 31 of board 1"
    );
    assert_eq!(
        third.used_channels(),
        [0],
        "a board with no channel of its own still takes part"
    );
    assert_eq!(second.frequency, 1_000_000);
}

#[test]
fn a_boards_shift_drops_that_many_samples_from_the_start_of_its_data() {
    let (provider, _) = plain_world(2, &[0, 2]);
    let (_, words, result) = run(&provider, &config(ChannelSet::all(64), None, None));
    result.unwrap();
    assert_eq!(
        words.len(),
        14 * 2,
        "the chain is as long as the shortest board after its shift"
    );
    for i in 0..14_u32 {
        assert_eq!(words[(i * 2) as usize], word(0, i));
        assert_eq!(words[(i * 2 + 1) as usize], word(1, i + 2));
    }
}

#[test]
fn the_channels_that_are_not_enabled_read_as_zero() {
    // Board 0 records its channels 24 to 31 (one byte per sample); board 1 has no channel of its own and takes part
    // with channel 0 only, which must not show.
    let (provider, _) = world(
        &["info_32ch.rec", "info_32ch.rec"],
        &["capture_24_31.rec", "capture_24_31.rec"],
        &[0, 0],
    );
    let mut enabled = ChannelSet::empty();
    for channel in 24..32 {
        enabled.insert(channel);
    }
    let mut config = config(enabled, None, None);
    config.capture_length = Some(8);
    let (_, words, result) = run(&provider, &config);
    result.unwrap();
    for i in 0..8_u32 {
        assert_eq!(
            words[(i * 2) as usize],
            1 << (24 + i),
            "sample {i}, board 0"
        );
        assert_eq!(
            words[(i * 2 + 1) as usize],
            0,
            "sample {i}, board 1 has nothing enabled"
        );
    }
}

#[test]
fn a_chain_has_only_the_pattern_triggers_of_its_master() {
    let (provider, _) = plain_world(2, &[0, 0]);
    let list = provider.scan().unwrap();
    let caps = &chain_of(&list).capabilities;
    assert!(caps.triggers.contains(&TriggerKind::Pattern));
    assert!(!caps.triggers.contains(&TriggerKind::RisingEdge));
    assert!(!caps.triggers.contains(&TriggerKind::FallingEdge));
    // The core refuses an edge trigger before any board is asked.
    let edge = config(
        ChannelSet::all(64),
        Some(Trigger {
            channel: 0,
            kind: TriggerKind::RisingEdge,
        }),
        None,
    );
    assert_eq!(
        validate(caps, &edge, &Limits::default())
            .unwrap_err()
            .code(),
        la_devices::error::UNSUPPORTED_TRIGGER
    );
    // A pattern on the master's channels is accepted; one on another board's channels is refused at the start.
    let on_master = config(
        ChannelSet::all(64),
        Some(Trigger {
            channel: 2,
            kind: TriggerKind::Pattern,
        }),
        Some(Pattern {
            base_channel: 2,
            length: 4,
            value: 0b1010,
        }),
    );
    assert!(validate(caps, &on_master, &Limits::default()).is_ok());
    let (provider, ports) = plain_world(2, &[0, 0]);
    let beyond = config(
        ChannelSet::all(64),
        Some(Trigger {
            channel: 30,
            kind: TriggerKind::Pattern,
        }),
        Some(Pattern {
            base_channel: 30,
            length: 4,
            value: 0,
        }),
    );
    let (_, _, result) = run(&provider, &beyond);
    assert_eq!(
        result.unwrap_err().code(),
        la_devices::error::UNSUPPORTED_TRIGGER
    );
    assert_eq!(
        ports.writes("COM1").len(),
        1,
        "no board was asked to capture"
    );
}

#[test]
fn a_board_that_went_away_is_named_and_nothing_is_captured() {
    let (provider, ports) = plain_world(3, &[0, 0, 0]);
    let list = provider.scan().unwrap();
    let chain = chain_of(&list).clone();
    ports.remove_port("COM3");
    let error = provider.open(&chain.id).err().unwrap();
    assert_eq!(error.code(), CHAIN_INCOMPLETE);
    assert!(error.detail().contains("rp2350b:S3"), "{}", error.detail());
    assert!(error.detail().contains("lab"), "{}", error.detail());
}

#[test]
fn a_board_that_does_not_answer_the_request_makes_the_chain_incomplete_and_the_armed_boards_stop() {
    // The master (armed last) is silent: the slaves were armed and must be stopped.
    let (provider, ports) = world(
        &["info_32ch.rec", "info_32ch.rec", "info_32ch.rec"],
        &["no_answer.rec", "chain_board1.rec", "chain_board2.rec"],
        &[0, 0, 0],
    );
    let (seen, words, result) = run(&provider, &config(ChannelSet::all(96), None, None));
    let error = result.unwrap_err();
    assert_eq!(error.code(), CHAIN_INCOMPLETE);
    assert!(error.detail().contains("rp2350b:S1"), "{}", error.detail());
    assert!(
        seen.is_empty() && words.is_empty(),
        "nothing is delivered as if the chain were whole"
    );
    for slave in ["COM2", "COM3"] {
        assert!(
            ports.writes(slave).iter().any(|w| w == &[0x00]),
            "{slave} was told to stop"
        );
    }
}

#[test]
fn boards_of_different_firmware_are_refused_with_what_each_runs() {
    let (provider, _) = world(
        &["info_32ch.rec", "info_32ch_fw11.rec"],
        &["chain_board0.rec", "chain_board1.rec"],
        &[0, 0],
    );
    let list = provider.scan().unwrap();
    let chain = chain_of(&list);
    let error = provider.open(&chain.id).err().unwrap();
    assert_eq!(error.code(), CHAIN_MISMATCH);
    assert!(
        error.detail().contains("1.0.0") && error.detail().contains("1.1.0"),
        "{}",
        error.detail()
    );
}

#[test]
fn a_loss_on_one_board_is_a_loss_of_the_chain_and_the_positions_stay_together() {
    let (provider, _) = world(
        &["info_32ch.rec", "info_32ch.rec"],
        &["chain_loss_board0.rec", "chain_loss_board1.rec"],
        &[0, 0],
    );
    let mut config = config(ChannelSet::all(64), None, None);
    config.capture_length = Some(19);
    let (seen, words, result) = run(&provider, &config);
    result.unwrap();
    let stored: usize = seen.iter().filter(|(lost, _)| !lost).map(|(_, n)| n).sum();
    let lost: usize = seen.iter().filter(|(lost, _)| *lost).map(|(_, n)| n).sum();
    assert_eq!(stored + lost, 19, "the positions the capture covers");
    assert_eq!(lost, 3);
    assert_eq!(words.len(), 2 * stored);
    // Board 0 stored every position; the chain keeps those that board 1 stored too: 0..4 and 7..19 of board 0,
    // and 0..4 and 4..16 of board 1.
    let expected: Vec<(u32, u32)> = (0..4)
        .map(|i| (word(0, i), word(1, i)))
        .chain((4..16).map(|j| (word(0, j + 3), word(1, j))))
        .collect();
    let got: Vec<(u32, u32)> = words.chunks(2).map(|c| (c[0], c[1])).collect();
    assert_eq!(got, expected);
    assert_eq!(seen.iter().filter(|(lost, _)| *lost).count(), 1);
}
