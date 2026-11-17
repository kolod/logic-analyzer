//! The table of boards, their states and the descriptors the provider lists.
//!
//! The table is a plain value behind one lock; nothing here does I/O. A scan reads the bus first
//! (without the lock) and then calls [`Table::reconcile`] with what it saw; a preparation worker
//! reports its result with [`Table::finish`].
//!
//! A board is in one of three states:
//!
//! - `Ready`: on the bus and running the firmware; it can be opened.
//! - `Preparing`: it has no firmware and a worker is loading it. It stays listed (with the same
//!   identity) while it is off the bus during the renumeration; the worker gives up after 5 s.
//! - `Failed`: the preparation failed. The board stays listed while it is on the bus, and for
//!   [`FAILED_LISTING`] after the moment it failed if it left the bus. `open` hands the error over
//!   once; after that the next scan prepares the board again.

use std::collections::BTreeMap;
use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;
use la_devices::{Capabilities, DeviceDescriptor, DeviceId, SampleRates};

use crate::protocol::{FAILED_LISTING, SAMPLE_RATES};
use crate::usb::{BoardKey, UsbInfo};

/// The name of the board in the device list (a proper name, not translated).
pub(crate) const NAME: &str = "WeAct LogicAnalyzer V1";

/// What the board can do: 8 channels, the exact rates of the protocol, no trigger, no memory.
pub(crate) fn capabilities() -> Capabilities {
    Capabilities {
        channels: 8,
        sample_rates: SampleRates::List(SAMPLE_RATES.to_vec()),
        triggers: Vec::new(),
        buffer_depth: None,
    }
}

/// Where a board is in its life.
#[derive(Debug)]
pub(crate) enum State {
    /// On the bus and running the firmware: it can be opened.
    Ready,
    /// A worker is loading the firmware.
    Preparing,
    /// The preparation failed; `error` is waiting to be handed over by `open`.
    Failed { error: Option<Error> },
}

#[derive(Debug)]
pub(crate) struct Board {
    pub(crate) state: State,
    /// When the state last changed (the provider's timer).
    pub(crate) since: Duration,
}

/// The boards the provider knows, by position on the bus.
#[derive(Debug, Default)]
pub(crate) struct Table {
    boards: BTreeMap<BoardKey, Board>,
}

impl Table {
    /// Brings the table in line with the bus and returns the boards that need a preparation
    /// worker now (the caller starts one for each).
    ///
    /// Devices that are not boards of this driver are ignored. A board that is gone is dropped,
    /// except one that is being prepared (it is off the bus on purpose) and one that failed
    /// recently.
    pub(crate) fn reconcile(&mut self, seen: &[UsbInfo], now: Duration) -> Vec<BoardKey> {
        let mut to_prepare = Vec::new();
        let mut present = Vec::new();
        for info in seen.iter().filter(|info| info.is_weact()) {
            present.push(info.key.clone());
            let runs = info.runs_firmware();
            let next = match (self.boards.get(&info.key).map(|board| &board.state), runs) {
                // The worker finishes its own board.
                (Some(State::Preparing), _) => None,
                (Some(State::Failed { error: Some(_) }), _) | (Some(State::Ready), true) => None,
                (_, true) => Some(State::Ready),
                // Blank and nothing is being done: prepare it (again).
                (_, false) => {
                    to_prepare.push(info.key.clone());
                    Some(State::Preparing)
                }
            };
            if let Some(state) = next {
                tracing::info!(
                    target: "la_driver_weact",
                    board = %info.key,
                    state = state_name(&state),
                    "the state of the board changed"
                );
                self.boards
                    .insert(info.key.clone(), Board { state, since: now });
            }
        }
        self.boards.retain(|key, board| {
            present.contains(key)
                || match board.state {
                    State::Ready => false,
                    State::Preparing => true,
                    State::Failed { .. } => now.saturating_sub(board.since) < FAILED_LISTING,
                }
        });
        to_prepare
    }

    /// The result of a preparation worker. Ignored if the board is not being prepared (it was
    /// dropped meanwhile).
    pub(crate) fn finish(&mut self, key: &BoardKey, result: Result<(), Error>, now: Duration) {
        let Some(board) = self.boards.get_mut(key) else {
            return;
        };
        if !matches!(board.state, State::Preparing) {
            return;
        }
        board.state = match result {
            Ok(()) => State::Ready,
            Err(error) => {
                tracing::warn!(
                    target: "la_driver_weact",
                    board = %key,
                    code = error.code(),
                    detail = error.detail(),
                    "the preparation of the board failed"
                );
                State::Failed { error: Some(error) }
            }
        };
        board.since = now;
        tracing::info!(
            target: "la_driver_weact",
            board = %key,
            state = state_name(&board.state),
            "the state of the board changed"
        );
    }

    /// Whether any board is known.
    pub(crate) fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }

    /// What `open` of `id` may do: the key and descriptor of a ready board, or the typed result
    /// of every other state (a failure is handed over once).
    pub(crate) fn take_open(
        &mut self,
        id: &DeviceId,
    ) -> Result<(BoardKey, DeviceDescriptor), Error> {
        let Some(key) = self.key_of(id) else {
            return Err(device_error::not_found(id.as_str()));
        };
        let descriptor = self
            .descriptors()
            .into_iter()
            .find(|descriptor| descriptor.id == *id)
            .ok_or_else(|| device_error::not_found(id.as_str()))?;
        match self.boards.get_mut(&key).map(|board| &mut board.state) {
            Some(State::Ready) => Ok((key, descriptor)),
            Some(State::Failed { error }) => {
                Err(error.take().unwrap_or_else(crate::error::preparing))
            }
            Some(State::Preparing) => Err(crate::error::preparing()),
            None => Err(device_error::not_found(id.as_str())),
        }
    }

    /// The key whose identity is `id`, if any: the identity is compared as a whole.
    fn key_of(&self, id: &DeviceId) -> Option<BoardKey> {
        self.boards
            .keys()
            .find(|key| key.device_id() == *id)
            .cloned()
    }

    /// The descriptors of the known boards, in the order of their keys. When there are several
    /// boards the names carry a number (1, 2, ...) in that order; a single board has none.
    pub(crate) fn descriptors(&self) -> Vec<DeviceDescriptor> {
        let several = self.boards.len() > 1;
        self.boards
            .keys()
            .enumerate()
            .map(|(index, key)| DeviceDescriptor {
                id: key.device_id(),
                name: if several {
                    format!("{NAME} ({})", index.saturating_add(1))
                } else {
                    NAME.to_owned()
                },
                simulated: false,
                capabilities: capabilities(),
            })
            .collect()
    }
}

fn state_name(state: &State) -> &'static str {
    match state {
        State::Ready => "ready",
        State::Preparing => "preparing",
        State::Failed { .. } => "failed",
    }
}
