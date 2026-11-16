//! The bus state machine: start, stop, bits, bytes and transactions (research R4).
//!
//! The machine is fed the edges of SCL and SDA in position order (simultaneous edges: SCL first)
//! and writes annotations to the sink as soon as a construct is complete. It keeps the levels of the
//! two lines itself, so it never asks the reader about a position it has already passed.
//!
//! * **Start**: SDA falls while SCL is high. The first one counts only after the bus has been idle
//!   (both lines high, without a break) for at least one bus period; until then every edge is
//!   ignored without a trace. After a stop, or inside a transaction (a repeated start), no idle is
//!   needed.
//! * **Stop**: SDA rises while SCL is high.
//! * **Bit**: a rising edge of SCL is a *candidate*; it is a bit only if SCL then falls with no SDA
//!   edge in between (an SDA edge while SCL is high is a start or a stop and the candidate was the
//!   set-up pulse of it). The bit is the level SDA had at the rising edge. Nine bits are a byte and
//!   its acknowledge.
//!
//! All arithmetic is checked or saturating and nothing is indexed.

use la_storage::Gap;

use super::text::{self, Display};
use crate::input::{Level, SignalReader};
use crate::model::{Annotation, Kind, Target, Text};
use crate::run::{DecodeError, Sink};

/// The most edges one call looks at (research R2), so that a call takes milliseconds.
pub const WORK_EDGES: usize = 262_144;

/// A rising edge of SCL that may be a bit.
#[derive(Debug, Clone, Copy)]
struct Pulse {
    at: u64,
    bit: bool,
}

/// The transaction in progress.
#[derive(Debug, Clone, Copy)]
struct Transaction {
    start: u64,
    /// The next byte is an address (after a start or a repeated start).
    expect_address: bool,
    /// The address of the first part, once known.
    address: Option<u16>,
    /// The direction of the first part.
    first_read: bool,
    /// The direction of the part in progress.
    read: bool,
}

/// The byte in progress.
#[derive(Debug, Clone, Copy, Default)]
struct Byte {
    value: u16,
    count: u8,
    first: u64,
}

/// The bus state machine.
#[derive(Debug, Clone)]
pub struct Bus {
    scl_channel: u8,
    sda_channel: u8,
    period: u64,
    display: Display,
    /// Everything before this position has been processed.
    cursor: u64,
    started: bool,
    resynced_at: Option<u64>,
    in_gap_until: Option<u64>,
    scl: Option<bool>,
    sda: Option<bool>,
    idle_since: Option<u64>,
    synced: bool,
    transaction: Option<Transaction>,
    byte: Byte,
    pulse: Option<Pulse>,
}

fn put(
    sink: &mut Sink,
    kind: Kind,
    target: Target,
    (text, style): (Text, crate::model::Style),
    error: bool,
) -> Result<(), DecodeError> {
    sink.annotate(Annotation {
        kind,
        target,
        style,
        text,
        error,
    })?;
    Ok(())
}

impl Bus {
    /// A machine for these channels that has seen nothing; `period` is the bus clock period in
    /// samples (at least 1).
    #[must_use]
    pub fn new(scl_channel: u8, sda_channel: u8, period: u64, display: Display) -> Self {
        Self {
            scl_channel,
            sda_channel,
            period: period.max(1),
            display,
            cursor: 0,
            started: false,
            resynced_at: None,
            in_gap_until: None,
            scl: None,
            sda: None,
            idle_since: None,
            synced: false,
            transaction: None,
            byte: Byte::default(),
            pulse: None,
        }
    }

    /// Everything before this position has been processed.
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Processes the next slice of `[cursor, target)`: at most [`WORK_EDGES`] edges. Call again
    /// until [`Bus::cursor`] reaches `target`.
    ///
    /// # Errors
    ///
    /// The limit of the sink was reached; the caller must return at once.
    pub fn advance(
        &mut self,
        reader: &SignalReader,
        target: u64,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        if self.cursor >= target {
            return Ok(());
        }
        let mut end = target;
        if let Some(edge) = reader.edges(self.cursor, target).nth(WORK_EDGES) {
            if edge.position > self.cursor {
                end = edge.position;
            }
        }
        self.slice(reader, self.cursor, end, sink)?;
        self.cursor = end;
        Ok(())
    }

    /// The levels of both lines at `at` (unknown inside a gap).
    fn resync(&mut self, reader: &SignalReader, at: u64) {
        let level = |role: &str| match reader.level(role, at) {
            Level::High => Some(true),
            Level::Low => Some(false),
            Level::Missing => None,
        };
        self.scl = level("scl");
        self.sda = level("sda");
        self.idle_since = (self.scl == Some(true) && self.sda == Some(true)).then_some(at);
        self.resynced_at = Some(at);
        self.started = true;
    }

    fn slice(
        &mut self,
        reader: &SignalReader,
        from: u64,
        to: u64,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        let mut position = from;
        if let Some(until) = self.in_gap_until {
            if to <= until {
                return Ok(());
            }
            self.in_gap_until = None;
            self.resync(reader, until);
            position = until;
        } else if !self.started {
            self.resync(reader, from);
        }
        let gaps: Vec<Gap> = reader.gaps_in(from, to);
        for gap in &gaps {
            let gap_end = gap.end().unwrap_or(u64::MAX);
            if gap_end <= position {
                continue;
            }
            if gap.start >= to {
                break;
            }
            if gap.start > position {
                self.edges(reader, position, gap.start, sink)?;
            }
            self.gap_begins();
            if gap_end >= to {
                self.in_gap_until = Some(gap_end);
                return Ok(());
            }
            self.resync(reader, gap_end);
            position = gap_end;
        }
        if position < to {
            self.edges(reader, position, to, sink)?;
        }
        Ok(())
    }

    fn edges(
        &mut self,
        reader: &SignalReader,
        from: u64,
        to: u64,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        for edge in reader.edges(from, to) {
            // An edge at the first position after a gap compares with an unknown level.
            if self.resynced_at == Some(edge.position) {
                continue;
            }
            match edge.role {
                "scl" => self.scl_edge(edge.position, edge.rising, sink)?,
                "sda" => self.sda_edge(edge.position, edge.rising, sink)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Samples were lost: what is in progress ends and the bus must be found idle again.
    fn gap_begins(&mut self) {
        self.transaction = None;
        self.byte = Byte::default();
        self.pulse = None;
        self.synced = false;
        self.scl = None;
        self.sda = None;
        self.idle_since = None;
    }

    fn update_idle(&mut self, at: u64) {
        if self.scl == Some(true) && self.sda == Some(true) {
            if self.idle_since.is_none() {
                self.idle_since = Some(at);
            }
        } else {
            self.idle_since = None;
        }
    }

    fn scl_edge(&mut self, at: u64, rising: bool, sink: &mut Sink) -> Result<(), DecodeError> {
        self.scl = Some(rising);
        self.update_idle(at);
        if rising {
            self.pulse = match (self.transaction.is_some(), self.sda) {
                (true, Some(bit)) => Some(Pulse { at, bit }),
                _ => None,
            };
        } else if let Some(pulse) = self.pulse.take() {
            self.commit(pulse, sink)?;
        }
        Ok(())
    }

    fn sda_edge(&mut self, at: u64, rising: bool, sink: &mut Sink) -> Result<(), DecodeError> {
        let idle_before = self.idle_since;
        self.sda = Some(rising);
        if self.scl == Some(true) {
            // An SDA edge while SCL is high is a start or a stop; the rising edge of SCL before it
            // was the set-up pulse, not a bit.
            self.pulse = None;
            if rising {
                self.stop(at, sink)?;
            } else {
                let idle_long_enough =
                    idle_before.is_some_and(|since| at.saturating_sub(since) >= self.period);
                self.start(at, idle_long_enough, sink)?;
            }
        }
        self.update_idle(at);
        Ok(())
    }

    fn start(
        &mut self,
        at: u64,
        idle_long_enough: bool,
        sink: &mut Sink,
    ) -> Result<(), DecodeError> {
        if !self.synced && !idle_long_enough {
            return Ok(());
        }
        self.synced = true;
        // Between bytes a start inside a transaction is a repeated start; in the middle of a byte
        // it begins a new transaction.
        let between_bytes = self.byte.count == 0;
        self.byte = Byte::default();
        let repeated = match &mut self.transaction {
            Some(transaction) if between_bytes => {
                transaction.expect_address = true;
                true
            }
            _ => false,
        };
        let (text, style) = if repeated {
            text::repeated_start()
        } else {
            self.transaction = Some(Transaction {
                start: at,
                expect_address: true,
                address: None,
                first_read: false,
                read: false,
            });
            text::start()
        };
        put(sink, Kind::Point { at }, Target::Lane, (text, style), false)?;
        let dot_text = if repeated {
            text::repeated_start_text()
        } else {
            text::start_text()
        };
        put(
            sink,
            Kind::Point { at },
            Target::Channel(self.sda_channel),
            text::event_dot(dot_text),
            false,
        )
    }

    fn stop(&mut self, at: u64, sink: &mut Sink) -> Result<(), DecodeError> {
        let Some(transaction) = self.transaction.take() else {
            // A stop with nothing to stop (before the first start, or after a stop).
            return Ok(());
        };
        self.byte = Byte::default();
        put(sink, Kind::Point { at }, Target::Lane, text::stop(), false)?;
        put(
            sink,
            Kind::Point { at },
            Target::Channel(self.sda_channel),
            text::event_dot(text::stop_text()),
            false,
        )?;
        if let Some(address) = transaction.address {
            put(
                sink,
                Kind::Range {
                    start: transaction.start,
                    end: at.saturating_add(1),
                },
                Target::Lane,
                text::transaction(transaction.first_read, self.display.value(address)),
                false,
            )?;
        }
        Ok(())
    }

    /// A clock pulse ended without an SDA edge in it: it was a bit.
    fn commit(&mut self, pulse: Pulse, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.byte.count < 8 {
            if self.byte.count == 0 {
                self.byte.first = pulse.at;
            }
            self.byte.value = (self.byte.value << 1) | u16::from(pulse.bit);
            self.byte.count = self.byte.count.saturating_add(1);
            return put(
                sink,
                Kind::Edge {
                    at: pulse.at,
                    rising: true,
                },
                Target::Channel(self.scl_channel),
                text::bit_arrow(pulse.bit),
                false,
            );
        }
        // The ninth bit: the acknowledge (low is ACK).
        let ack = !pulse.bit;
        let byte = std::mem::take(&mut self.byte);
        put(
            sink,
            Kind::Edge {
                at: pulse.at,
                rising: true,
            },
            Target::Channel(self.scl_channel),
            text::ack_arrow(ack),
            false,
        )?;
        let Some(transaction) = &mut self.transaction else {
            return Ok(());
        };
        let range = Kind::Range {
            start: byte.first,
            end: pulse.at.saturating_add(1),
        };
        let is_address = transaction.expect_address;
        if is_address {
            let read = byte.value & 1 != 0;
            let address = byte.value >> 1;
            transaction.expect_address = false;
            transaction.read = read;
            if transaction.address.is_none() {
                transaction.address = Some(address);
                transaction.first_read = read;
            }
            put(
                sink,
                range,
                Target::Lane,
                text::address(read, self.display.value(address)),
                false,
            )?;
        } else {
            put(
                sink,
                range,
                Target::Lane,
                text::data(self.display.value(byte.value)),
                false,
            )?;
        }
        // A NAK is an error after an address and after a byte written to a target; the controller's
        // own NAK of a byte it reads is how a read ends.
        let error = !ack && (is_address || !transaction.read);
        put(
            sink,
            Kind::Point { at: pulse.at },
            Target::Lane,
            text::acknowledge(ack),
            error,
        )
    }
}
