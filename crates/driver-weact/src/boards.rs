//! The table of boards, their states and the descriptors the provider lists.
//!
//! The table is a plain value behind one lock of the provider; nothing here does I/O. A scan
//! reads the bus first (without the lock) and then calls [`Table::reconcile`] with what it saw.

use std::collections::BTreeMap;
use std::time::Duration;

use la_devices::{Capabilities, DeviceDescriptor, SampleRates};

use crate::protocol::SAMPLE_RATES;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// On the bus and running the firmware: it can be opened.
    Ready,
}

#[derive(Debug, Clone)]
#[expect(
    dead_code,
    reason = "the state and the time are read by T018, the preparation"
)]
pub(crate) struct Board {
    pub(crate) state: State,
    /// When the board was last seen on the bus (the provider's timer).
    pub(crate) last_seen: Duration,
}

/// The boards the provider knows, by position on the bus.
#[derive(Debug, Default)]
pub(crate) struct Table {
    boards: BTreeMap<BoardKey, Board>,
}

impl Table {
    /// Brings the table in line with the bus: a board that runs the firmware is `Ready`; a board
    /// that is no longer there is dropped. Devices that are not boards of this driver are ignored.
    pub(crate) fn reconcile(&mut self, seen: &[UsbInfo], now: Duration) {
        let mut present = Vec::new();
        for info in seen.iter().filter(|info| info.is_weact()) {
            if info.runs_firmware() {
                present.push(info.key.clone());
                self.boards.insert(
                    info.key.clone(),
                    Board {
                        state: State::Ready,
                        last_seen: now,
                    },
                );
            }
        }
        self.boards.retain(|key, _| present.contains(key));
    }

    /// Whether any board is known.
    pub(crate) fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }

    /// The board at `key`, if known.
    #[expect(dead_code, reason = "used by T018, the preparation worker")]
    pub(crate) fn get(&self, key: &BoardKey) -> Option<&Board> {
        self.boards.get(key)
    }

    /// The key whose identity is `id`, if any: the identity is compared as a whole.
    pub(crate) fn key_of(&self, id: &la_devices::DeviceId) -> Option<BoardKey> {
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
