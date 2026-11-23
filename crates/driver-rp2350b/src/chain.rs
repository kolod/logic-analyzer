//! A chain of boards as one device (research R2).
//!
//! The chain is host-side: the driver holds one serial link per board, arms the slaves and then the master,
//! merges the words of the boards into one stream (`words_per_sample` is the number of boards, the channels of the
//! first board first) and drops, from the start of each board's data, the number of samples the user set to line
//! the boards up. The boards do not know they are chained except that each is told its role in the request.

use std::collections::VecDeque;
use std::time::Duration;

use la_core::{Error, Result};
use la_devices::{
    AppliedConfig, Capabilities, Device, DeviceDescriptor, Flow, ModeLimit, READ_TIMEOUT,
    ReadOutcome, ResolvedConfig, SampleRates, TriggerKind, TriggerLimits,
};
use la_proto::{CaptureRequest2, DeviceInfo, Encoding, Memory, Role, TriggerType};

use crate::device::Rp2350bDevice;
use crate::error;

/// The most boards of a chain.
pub const MAX_BOARDS: usize = 5;

/// A chain the user defined: the boards in order (the first is the master) and the shift of each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainDefinition {
    /// The name of the chain (not empty); the device is `rp2350b:chain-<name>`.
    pub name: String,
    /// The identities of the boards, as the text of their `DeviceId`, the master first.
    pub boards: Vec<String>,
    /// For each board, how many samples to drop from the start of its data so that it lines up with the others
    /// (never negative: a board that runs early is lined up by shifting the others).
    pub shifts: Vec<u32>,
}

impl ChainDefinition {
    /// Whether the definition can be used: 2 to 5 boards, a shift for each, a name, no board twice.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (2..=MAX_BOARDS).contains(&self.boards.len())
            && self.shifts.len() == self.boards.len()
            && !self.name.trim().is_empty()
            && self.boards.iter().enumerate().all(|(i, b)| {
                self.boards
                    .iter()
                    .skip(i.saturating_add(1))
                    .all(|other| other != b)
            })
    }

    /// The identity of the device the chain makes.
    #[must_use]
    pub fn device_id(&self) -> la_devices::DeviceId {
        la_devices::DeviceId::new(crate::PROVIDER_NAME, &format!("chain-{}", self.name))
    }
}

/// The capabilities of a chain of boards with these information replies (the master first): the channels of all
/// of them, the rates and limits they all can do, and only the pattern triggers of the master.
#[must_use]
pub fn chain_capabilities(infos: &[&DeviceInfo]) -> Capabilities {
    let boards = u8::try_from(infos.len()).unwrap_or(u8::MAX);
    let master = infos.first().copied();
    let lowest_max = infos.iter().map(|i| i.frequency).min().unwrap_or(0);
    let highest_min = crate::MIN_RATE_HZ;
    let before = infos.iter().flat_map(|i| i.max_before).min().unwrap_or(0);
    let after = infos.iter().flat_map(|i| i.max_after).min().unwrap_or(0);
    let buffer = infos.iter().map(|i| i.buffer).filter(|b| *b > 0).min();
    let pattern = master.map_or(0, |m| m.triggers.pattern);
    let fast = master.map_or(0, |m| m.triggers.fast);
    let mut triggers = Vec::new();
    if pattern > 0 {
        triggers.push(TriggerKind::Pattern);
    }
    if fast > 0 {
        triggers.push(TriggerKind::FastPattern);
    }
    Capabilities {
        channels: boards.saturating_mul(crate::CHANNELS),
        sample_rates: SampleRates::Range {
            min: highest_min,
            max: u64::from(lowest_max).max(highest_min),
        },
        triggers,
        buffer_depth: buffer.map(u64::from),
        trigger_limits: TriggerLimits {
            pattern_width: pattern,
            fast_pattern_width: fast,
            chained: true,
            allowed_in_chain: true,
        },
        pre_post: if after == 0 {
            Vec::new()
        } else {
            vec![ModeLimit {
                up_to_channels: boards.saturating_mul(crate::CHANNELS),
                max_before: u64::from(before),
                max_after: u64::from(after),
            }]
        },
        buffer_depths: Vec::new(),
        bursts: None,
        max_rates: Vec::new(),
        actions: Vec::new(),
    }
}

/// The requests of a capture of a chain of `boards` boards, one for each, the master first: each board captures
/// the enabled channels that are its own; the master takes the trigger (a pattern on its channels) and the slaves
/// start when the master's trigger line says so (a blast request with the role of a slave).
///
/// # Errors
///
/// `device.unsupported_trigger` for a pattern outside the master's channels, `device.invalid_capture_length` for a
/// length that does not fit the board's 32 bits or is missing.
pub fn build_chain_requests(
    config: &ResolvedConfig,
    boards: usize,
) -> Result<Vec<CaptureRequest2>> {
    let too_long = || {
        la_devices::error::invalid_capture_length(
            "the board needs a capture length that fits in 32 bits".to_owned(),
        )
    };
    let total = config.capture_length().ok_or_else(|| {
        la_devices::error::invalid_capture_length(
            "the boards need a finite capture length".to_owned(),
        )
    })?;
    let before = config.pre_trigger_samples();
    let after = total.saturating_sub(before);
    let (trigger_type, trigger_channel, trigger_length, trigger_value) = match config.trigger() {
        None => (TriggerType::Blast, 0, 0, 0),
        Some(trigger) => match (trigger.kind, config.pattern()) {
            (TriggerKind::Pattern, Some(p)) | (TriggerKind::FastPattern, Some(p)) => {
                if u16::from(p.base_channel).saturating_add(u16::from(p.length)) > 32 {
                    return Err(la_devices::error::unsupported_trigger(
                        "a pattern of a chain must be on the channels of its first board"
                            .to_owned(),
                    ));
                }
                let kind = if trigger.kind == TriggerKind::Pattern {
                    TriggerType::Pattern
                } else {
                    TriggerType::FastPattern
                };
                (kind, p.base_channel, p.length, p.value)
            }
            (kind, _) => {
                return Err(la_devices::error::unsupported_trigger(format!(
                    "a chain has only pattern triggers, not {kind:?}"
                )));
            }
        },
    };
    let mut requests = Vec::with_capacity(boards);
    for board in 0..boards {
        let mut channels = [0_u8; 32];
        let mut count = 0_u8;
        let first = u8::try_from(board.saturating_mul(32)).unwrap_or(u8::MAX);
        for (slot, channel) in channels.iter_mut().zip(
            config
                .enabled_channels()
                .iter()
                .filter(|c| usize::from(*c) / 32 == board),
        ) {
            *slot = channel.saturating_sub(first);
            count = count.saturating_add(1);
        }
        if count == 0 {
            // A board with no channel of its own still takes part, to keep the time; its data is dropped.
            count = 1;
        }
        let highest = channels
            .get(..usize::from(count))
            .and_then(|used| used.iter().copied().max())
            .unwrap_or(0);
        let master = board == 0;
        requests.push(CaptureRequest2 {
            trigger_type: if master {
                trigger_type
            } else {
                TriggerType::Blast
            },
            trigger_channel: if master { trigger_channel } else { 0 },
            trigger_length: if master { trigger_length } else { 0 },
            trigger_value: if master { trigger_value } else { 0 },
            channel_count: count,
            channels,
            mode: highest / 8,
            frequency: u32::try_from(config.sample_rate_hz()).map_err(|_| too_long())?,
            samples_before: if master {
                u32::try_from(before).map_err(|_| too_long())?
            } else {
                0
            },
            samples_after: u32::try_from(after).map_err(|_| too_long())?,
            bursts: 0,
            burst_times: false,
            role: if master { Role::Master } else { Role::Slave },
            encoding: Encoding::Raw,
            memory: Memory::Auto,
        });
    }
    Ok(requests)
}

/// A run of positions of one board: samples it stored, or samples it lost.
enum Run {
    Stored(VecDeque<u32>),
    Lost(u64),
}

/// The data of one board waiting to be merged.
struct Lane {
    runs: VecDeque<Run>,
    /// Positions still to drop from the start (the board's shift).
    skip: u64,
    finished: bool,
}

/// What the merger produced.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Merged {
    /// This many whole samples were appended to the output (one word per board each).
    Samples(usize),
    /// This many positions are lost for the chain: some board did not store them.
    Lost(u64),
}

/// Merges the timelines of the boards into the timeline of the chain. A position is a sample of the chain only if
/// every board stored it; where one board lost positions the chain reports them lost and drops what the others
/// stored there, so positions never disagree.
pub(crate) struct Merger {
    lanes: Vec<Lane>,
}

impl Merger {
    /// A merger for boards with these shifts.
    pub(crate) fn new(shifts: &[u32]) -> Self {
        Self {
            lanes: shifts
                .iter()
                .map(|&shift| Lane {
                    runs: VecDeque::new(),
                    skip: u64::from(shift),
                    finished: false,
                })
                .collect(),
        }
    }

    /// Samples a board stored.
    pub(crate) fn stored(&mut self, board: usize, words: &[u32]) {
        let Some(lane) = self.lanes.get_mut(board) else {
            return;
        };
        let drop = usize::try_from(lane.skip)
            .unwrap_or(usize::MAX)
            .min(words.len());
        lane.skip = lane.skip.saturating_sub(u64::try_from(drop).unwrap_or(0));
        let rest = words.get(drop..).unwrap_or(&[]);
        if rest.is_empty() {
            return;
        }
        if let Some(Run::Stored(last)) = lane.runs.back_mut() {
            last.extend(rest.iter().copied());
        } else {
            lane.runs
                .push_back(Run::Stored(rest.iter().copied().collect()));
        }
    }

    /// Positions a board lost.
    pub(crate) fn lost(&mut self, board: usize, count: u64) {
        let Some(lane) = self.lanes.get_mut(board) else {
            return;
        };
        let drop = lane.skip.min(count);
        lane.skip = lane.skip.saturating_sub(drop);
        let rest = count.saturating_sub(drop);
        if rest == 0 {
            return;
        }
        if let Some(Run::Lost(last)) = lane.runs.back_mut() {
            *last = last.saturating_add(rest);
        } else {
            lane.runs.push_back(Run::Lost(rest));
        }
    }

    /// A board sent everything it will send.
    pub(crate) fn finish(&mut self, board: usize) {
        if let Some(lane) = self.lanes.get_mut(board) {
            lane.finished = true;
        }
    }

    /// The next piece of the chain, at most `max` samples, appended to `out`; `None` when some board has nothing
    /// yet (or the chain is over).
    pub(crate) fn pop(&mut self, max: usize, out: &mut Vec<u32>) -> Option<Merged> {
        if self.lanes.iter().any(|l| l.runs.is_empty()) {
            return None;
        }
        let all_stored = self
            .lanes
            .iter()
            .all(|l| matches!(l.runs.front(), Some(Run::Stored(_))));
        let head_len = |lane: &Lane| -> u64 {
            match lane.runs.front() {
                Some(Run::Stored(words)) => u64::try_from(words.len()).unwrap_or(u64::MAX),
                Some(Run::Lost(n)) => *n,
                None => 0,
            }
        };
        let shortest = self.lanes.iter().map(head_len).min().unwrap_or(0);
        if all_stored {
            let n = usize::try_from(shortest)
                .unwrap_or(usize::MAX)
                .min(max.max(1));
            for _ in 0..n {
                for lane in &mut self.lanes {
                    if let Some(Run::Stored(words)) = lane.runs.front_mut() {
                        out.push(words.pop_front().unwrap_or(0));
                    }
                }
            }
            for lane in &mut self.lanes {
                if matches!(lane.runs.front(), Some(Run::Stored(w)) if w.is_empty()) {
                    lane.runs.pop_front();
                }
            }
            Some(Merged::Samples(n))
        } else {
            for lane in &mut self.lanes {
                match lane.runs.front_mut() {
                    Some(Run::Stored(words)) => {
                        words.drain(
                            ..usize::try_from(shortest)
                                .unwrap_or(usize::MAX)
                                .min(words.len()),
                        );
                        if words.is_empty() {
                            lane.runs.pop_front();
                        }
                    }
                    Some(Run::Lost(n)) => {
                        *n = n.saturating_sub(shortest);
                        if *n == 0 {
                            lane.runs.pop_front();
                        }
                    }
                    None => {}
                }
            }
            Some(Merged::Lost(shortest))
        }
    }

    /// Whether the chain has ended: a board has sent everything and nothing of it is left to merge.
    pub(crate) fn is_over(&self) -> bool {
        self.lanes.iter().any(|l| l.finished && l.runs.is_empty())
    }
}

/// A chain of boards that captures as one device.
pub struct ChainDevice {
    descriptor: DeviceDescriptor,
    name: String,
    boards: Vec<(String, Rp2350bDevice)>,
    shifts: Vec<u32>,
    /// The enabled channels of each board, as the bits of its word.
    masks: Vec<u32>,
    merger: Option<Merger>,
    finished: Vec<bool>,
    started: Vec<bool>,
}

impl ChainDevice {
    pub(crate) fn new(
        descriptor: DeviceDescriptor,
        name: String,
        boards: Vec<(String, Rp2350bDevice)>,
        shifts: Vec<u32>,
    ) -> Self {
        let count = boards.len();
        Self {
            descriptor,
            name,
            boards,
            shifts,
            masks: vec![0; count],
            merger: None,
            finished: vec![false; count],
            started: vec![false; count],
        }
    }

    /// An error of one board while the chain starts: a board that does not answer or went away makes the chain
    /// incomplete, naming it; a refusal keeps its own code and names the board.
    fn board_error(&self, board: usize, error: &Error) -> Error {
        let name = self.boards.get(board).map_or("?", |(n, _)| n.as_str());
        let incomplete = [
            error::TIMEOUT,
            la_devices::error::DISCONNECTED,
            error::PORT_BUSY,
            error::NOT_FOUND,
            error::ACCESS_DENIED,
        ];
        if incomplete.contains(&error.code()) {
            error::chain_incomplete(
                &self.name,
                name,
                &format!("did not take part: {}", error.detail()),
            )
        } else {
            la_core::Error::new(
                la_core::ErrorKind::Device,
                error.code(),
                format!("board {name}: {}", error.detail()),
            )
        }
    }

    fn stop_started(&mut self) {
        for (index, (_, device)) in self.boards.iter_mut().enumerate().rev() {
            if self.started.get(index).copied().unwrap_or(false) {
                let _ = device.stop();
            }
        }
        self.started.iter_mut().for_each(|s| *s = false);
    }
}

impl Device for ChainDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn flow(&self) -> Flow {
        Flow::Realtime
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        let requests = build_chain_requests(config, self.boards.len())?;
        for (board, mask) in self.masks.iter_mut().enumerate() {
            *mask = config
                .enabled_channels()
                .iter()
                .filter(|c| usize::from(*c) / 32 == board)
                .fold(0_u32, |m, c| {
                    m | 1_u32.checked_shl(u32::from(c % 32)).unwrap_or(0)
                });
        }
        self.merger = Some(Merger::new(&self.shifts));
        self.finished.iter_mut().for_each(|f| *f = false);
        // The slaves are armed first, the master last: the master's trigger starts the others.
        let order: Vec<usize> = (1..self.boards.len()).chain(std::iter::once(0)).collect();
        for board in order {
            let Some(request) = requests.get(board) else {
                continue;
            };
            let outcome = match self.boards.get_mut(board) {
                Some((_, device)) => device.start_request(request, config.sample_rate_hz()),
                None => continue,
            };
            match outcome {
                Ok(_) => {
                    if let Some(started) = self.started.get_mut(board) {
                        *started = true;
                    }
                }
                Err(error) => {
                    // Nothing is captured as if the chain were whole: the boards already armed are stopped.
                    let error = self.board_error(board, &error);
                    self.stop_started();
                    return Err(error);
                }
            }
        }
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let share = READ_TIMEOUT
            .checked_div(u32::try_from(self.boards.len().max(1)).unwrap_or(1))
            .unwrap_or(READ_TIMEOUT)
            .max(Duration::from_millis(5));
        let Some(merger) = self.merger.as_mut() else {
            return Err(la_devices::error::invalid_state(
                "the chain was read before a capture was started".to_owned(),
            ));
        };
        for (board, (_, device)) in self.boards.iter_mut().enumerate() {
            if self.finished.get(board).copied().unwrap_or(true) {
                continue;
            }
            let mut piece = Vec::new();
            match device.read_within(max_samples, &mut piece, share)? {
                ReadOutcome::Samples => {
                    let mask = self.masks.get(board).copied().unwrap_or(0);
                    for word in &mut piece {
                        *word &= mask;
                    }
                    merger.stored(board, &piece);
                }
                ReadOutcome::Lost(count) => merger.lost(board, count),
                ReadOutcome::Idle => {}
                ReadOutcome::Finished => {
                    if let Some(done) = self.finished.get_mut(board) {
                        *done = true;
                    }
                    merger.finish(board);
                }
            }
        }
        Ok(match merger.pop(max_samples, out) {
            Some(Merged::Samples(_)) => ReadOutcome::Samples,
            Some(Merged::Lost(count)) => ReadOutcome::Lost(count),
            None if merger.is_over() => ReadOutcome::Finished,
            None => ReadOutcome::Idle,
        })
    }

    fn stop(&mut self) -> Result<()> {
        let mut first = None;
        for (_, device) in self.boards.iter_mut().rev() {
            if let Err(error) = device.stop() {
                first.get_or_insert(error);
            }
        }
        self.started.iter_mut().for_each(|s| *s = false);
        first.map_or(Ok(()), Err)
    }
}
