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
//! The lane is written as blocks (`specs/016-i2c-lane-blocks/contracts/i2c-lane.md`): the parts of a
//! packet in the upper band, each on the bit cells of the bus period, and the whole packet in the
//! lower band. A block of the upper band is held until the next one is known, so that the earlier is
//! cut where the later begins and none overlaps.
//!
//! All arithmetic is checked or saturating and nothing is indexed.

use la_storage::Gap;

use super::items::{self, Event};
use super::text::{self, Display};
use crate::input::{Level, SignalReader};
use crate::model::{Annotation, Kind, Style, Target, Text};
use crate::run::{DecodeError, Sink};

/// The most edges one call looks at (research R2), so that a call takes milliseconds.
pub const WORK_EDGES: usize = 262_144;

/// A rising edge of SCL that may be a bit.
#[derive(Debug, Clone, Copy)]
struct Pulse {
    at: u64,
    bit: bool,
}

/// The first byte of a 10-bit address, acknowledged; the low byte is still to come.
#[derive(Debug, Clone, Copy)]
struct Pending {
    /// Bits 9 and 8 of the address.
    high: u16,
    /// The rising edge of SCL of the first bit of the header byte.
    first: u64,
    /// The rising edge of SCL of its eighth bit.
    last: u64,
    /// The rising edge of SCL of its acknowledge bit.
    ack: u64,
}

/// The transaction in progress.
#[derive(Debug, Clone, Copy)]
struct Transaction {
    /// Where the block of the whole packet begins (the beginning of its start block).
    from: u64,
    /// A part of it was an error (a NAK that is one, a byte cut short, lost samples).
    errored: bool,
    /// The next byte is an address (after a start or a repeated start).
    expect_address: bool,
    /// The address of the first part, once known.
    address: Option<u16>,
    /// The direction of the first part.
    first_read: bool,
    /// The first part has a 10-bit address.
    first_ten_bit: bool,
    /// The direction of the part in progress.
    read: bool,
    /// The header of a 10-bit address was received and its low byte is expected.
    pending: Option<Pending>,
    /// The 10-bit address the transaction wrote, which a read part after a repeated start repeats.
    ten_bit_address: Option<u16>,
}

/// What a byte is, in the light of what came before it.
enum Kind10 {
    /// The header of a 10-bit write address, acknowledged: the low byte follows.
    Header { high: u16 },
    /// An address (7-bit, or 10-bit when complete).
    Address {
        address: u16,
        read: bool,
        ten_bit: bool,
        start: u64,
        /// The header byte of a 10-bit address, whose low byte this is.
        header: Option<Pending>,
    },
    /// A data byte.
    Data,
}

/// A block of the upper band waiting for the one after it.
#[derive(Debug, Clone)]
struct Held {
    from: u64,
    to: u64,
    text: Text,
    style: Style,
    error: bool,
}

/// The byte in progress.
#[derive(Debug, Clone, Copy, Default)]
struct Byte {
    value: u16,
    count: u8,
    first: u64,
    /// The rising edge of SCL of the last bit read.
    last: u64,
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
    /// The number of positions of the capture (blocks are cut at its end).
    length: u64,
    /// The block of the upper band that waits for the next one.
    held: Option<Held>,
    /// Where the last block of the upper band ended.
    last_end: u64,
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
            length: u64::MAX,
            held: None,
            last_end: 0,
        }
    }

    /// Half a bus period, rounded down: how far a bit's cell reaches before its rising edge.
    fn before(&self) -> u64 {
        self.period / 2
    }

    /// The rest of the bus period: how far it reaches after the rising edge.
    fn after(&self) -> u64 {
        self.period.saturating_sub(self.before())
    }

    /// The block of a bit read at the rising edge `at`: `(from, to)`.
    fn cell(&self, at: u64) -> (u64, u64) {
        (
            at.saturating_sub(self.before()),
            at.saturating_add(self.after()),
        )
    }

    /// Writes a block to the lane, cut at the end of the capture; nothing for an empty one.
    fn emit_block(
        &self,
        sink: &mut Sink,
        from: u64,
        to: u64,
        ts: (Text, Style),
        error: bool,
    ) -> Result<(), DecodeError> {
        let to = to.min(self.length);
        if from >= to {
            return Ok(());
        }
        put(
            sink,
            Kind::Range {
                start: from,
                end: to,
            },
            Target::Lane,
            ts,
            error,
        )
    }

    /// A block of the upper band: the one before it is cut where this one begins and written.
    fn push_part(
        &mut self,
        sink: &mut Sink,
        from: u64,
        to: u64,
        (text, style): (Text, Style),
        error: bool,
    ) -> Result<(), DecodeError> {
        if let Some(held) = self.held.take() {
            self.emit_block(
                sink,
                held.from,
                held.to.min(from),
                (held.text, held.style),
                held.error,
            )?;
        }
        self.last_end = to;
        self.held = Some(Held {
            from,
            to,
            text,
            style,
            error,
        });
        Ok(())
    }

    /// Writes the block that is waiting (nothing follows it).
    fn release(&mut self, sink: &mut Sink) -> Result<(), DecodeError> {
        if let Some(held) = self.held.take() {
            self.emit_block(
                sink,
                held.from,
                held.to,
                (held.text, held.style),
                held.error,
            )?;
        }
        Ok(())
    }

    /// The header byte of a 10-bit address whose low byte never came: its blocks, as far as they go.
    fn flush_header(&mut self, sink: &mut Sink) -> Result<(), DecodeError> {
        let Some(transaction) = &mut self.transaction else {
            return Ok(());
        };
        let Some(header) = transaction.pending.take() else {
            return Ok(());
        };
        let shown = self.display.address(header.high << 8, false, true);
        let (from, to) = (
            header.first.saturating_sub(self.before()),
            header.last.saturating_add(self.after()),
        );
        self.push_part(sink, from, to, text::address(false, shown), false)?;
        let (from, to) = self.cell(header.ack);
        self.push_part(sink, from, to, text::acknowledge(true), false)
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
        self.length = reader.length();
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
            self.gap_begins(gap.start, gap_end.min(reader.length()), sink)?;
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

    /// Samples were lost: what is in progress ends (as an error where something was in progress)
    /// and the bus must be found idle again.
    fn gap_begins(&mut self, at: u64, end: u64, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.transaction.is_some() {
            self.flush_header(sink)?;
            self.push_part(sink, at, end, text::gap(), true)?;
            self.release(sink)?;
            if let Some(transaction) = self.transaction.take() {
                if transaction.address.is_some() {
                    // Lost samples are an error: the packet in progress is red.
                    self.emit_block(sink, transaction.from, at, text::incomplete(true), true)?;
                }
            }
        }
        self.byte = Byte::default();
        self.pulse = None;
        self.synced = false;
        self.scl = None;
        self.sda = None;
        self.idle_since = None;
        Ok(())
    }

    /// The pass ends at `at`: what is in progress is shown as it is (a byte that was begun is cut
    /// short, the transaction is unfinished: red if it already had an error, else grey).
    ///
    /// # Errors
    ///
    /// The limit of the sink was reached.
    pub fn flush(&mut self, _at: u64, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.transaction.is_none() {
            return self.release(sink);
        }
        self.flush_header(sink)?;
        let read = self.transaction.is_some_and(|t| t.read);
        let cut = self.cut_byte(read, sink)?;
        self.release(sink)?;
        self.pulse = None;
        if let Some(transaction) = self.transaction.take() {
            if transaction.address.is_some() {
                let to = self.last_end.max(transaction.from.saturating_add(1));
                self.emit_block(
                    sink,
                    transaction.from,
                    to,
                    text::incomplete(transaction.errored || cut),
                    transaction.errored || cut,
                )?;
            }
        }
        Ok(())
    }

    /// A byte that was begun and not finished is a block over the bits that were read (an error);
    /// returns whether there was one.
    fn cut_byte(&mut self, read: bool, sink: &mut Sink) -> Result<bool, DecodeError> {
        let byte = std::mem::take(&mut self.byte);
        if byte.count == 0 {
            return Ok(false);
        }
        let from = byte.first.saturating_sub(self.before());
        let to = byte.last.saturating_add(self.after());
        self.push_part(sink, from, to, text::byte_cut(byte.count, read), true)?;
        Ok(true)
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
        self.flush_header(sink)?;
        // Between bytes a start inside a transaction is a repeated start; in the middle of a byte
        // it cuts the byte short and begins a new transaction.
        let between_bytes = self.byte.count == 0;
        if !between_bytes {
            let read = self.transaction.is_some_and(|t| t.read);
            let cut = self.cut_byte(read, sink)?;
            if let Some(old) = self.transaction.take() {
                if old.address.is_some() {
                    let to = self.last_end.max(old.from.saturating_add(1));
                    let bad = old.errored || cut;
                    self.emit_block(sink, old.from, to, text::incomplete(bad), bad)?;
                }
            }
        }
        self.byte = Byte::default();
        let from = at.saturating_sub(self.before());
        let to = at.saturating_add(self.after());
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
                from,
                errored: false,
                expect_address: true,
                address: None,
                first_read: false,
                first_ten_bit: false,
                read: false,
                pending: None,
                ten_bit_address: None,
            });
            text::start()
        };
        self.push_part(sink, from, to, (text, style), false)?;
        let kind = if repeated {
            items::REPEATED_START
        } else {
            items::START
        };
        sink.emit(Event::condition(kind, items::UNKNOWN).item(at, at.saturating_add(1)))?;
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
        if self.transaction.is_none() {
            // A stop with nothing to stop (before the first start, or after a stop).
            return Ok(());
        }
        self.flush_header(sink)?;
        let Some(transaction) = self.transaction.take() else {
            return Ok(());
        };
        let cut = self.byte.count > 0;
        self.cut_byte(transaction.read, sink)?;
        let direction = match (transaction.address, transaction.read) {
            (None, _) => items::UNKNOWN,
            (Some(_), true) => items::READ,
            (Some(_), false) => items::WRITE,
        };
        sink.emit(Event::condition(items::STOP, direction).item(at, at.saturating_add(1)))?;
        // The stop block: the period that ends at the rising edge of SDA, but not before the end of
        // the block before it.
        let mut from = self.last_end.max(at.saturating_sub(self.period));
        if from >= at {
            from = at.saturating_sub(self.period);
        }
        // The stop block waits for what follows: a start less than a period later cuts it short.
        self.push_part(sink, from, at, text::stop(), false)?;
        put(
            sink,
            Kind::Point { at },
            Target::Channel(self.sda_channel),
            text::event_dot(text::stop_text()),
            false,
        )?;
        let bad = transaction.errored || cut;
        if let Some(address) = transaction.address {
            self.emit_block(
                sink,
                transaction.from,
                at,
                text::transaction(
                    transaction.first_read,
                    self.display.address(
                        address,
                        transaction.first_read,
                        transaction.first_ten_bit,
                    ),
                    bad,
                ),
                bad,
            )?;
        } else if cut {
            // A byte that was an address was cut short by the stop.
            self.emit_block(sink, transaction.from, at, text::incomplete(true), true)?;
        } else {
            // A start and a stop with no clock pulse between them.
            self.emit_block(sink, transaction.from, at, text::empty(), true)?;
        }
        Ok(())
    }

    /// A clock pulse ended without an SDA edge in it: it was a bit.
    fn commit(&mut self, pulse: Pulse, sink: &mut Sink) -> Result<(), DecodeError> {
        if self.byte.count < 8 {
            if self.byte.count == 0 {
                self.byte.first = pulse.at;
            }
            self.byte.last = pulse.at;
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
        let Some(mut transaction) = self.transaction.take() else {
            return put(
                sink,
                Kind::Edge {
                    at: pulse.at,
                    rising: true,
                },
                Target::Channel(self.scl_channel),
                text::ack_arrow(ack),
                false,
            );
        };
        let item_end = pulse.at.saturating_add(1);
        let acknowledgement = if ack { items::ACK } else { items::NAK };
        let what = if let Some(pending) = transaction.pending.take() {
            // The low byte of a 10-bit address.
            Kind10::Address {
                address: pending.high << 8 | byte.value,
                read: false,
                ten_bit: true,
                start: pending.first,
                header: Some(pending),
            }
        } else if transaction.expect_address {
            let header = byte.value >> 3 == 0b1_1110;
            let read = byte.value & 1 != 0;
            let high = (byte.value >> 1) & 3;
            if header && !read && ack {
                Kind10::Header { high }
            } else if header && !read {
                // The header was not acknowledged: the address ends here.
                Kind10::Address {
                    address: high << 8,
                    read: false,
                    ten_bit: true,
                    start: byte.first,
                    header: None,
                }
            } else if let (true, true, Some(known)) = (header, read, transaction.ten_bit_address) {
                if known >> 8 == high {
                    // The read part of a 10-bit address: the address is the one already known.
                    Kind10::Address {
                        address: known,
                        read: true,
                        ten_bit: true,
                        start: byte.first,
                        header: None,
                    }
                } else {
                    Kind10::Address {
                        address: byte.value >> 1,
                        read,
                        ten_bit: false,
                        start: byte.first,
                        header: None,
                    }
                }
            } else {
                Kind10::Address {
                    address: byte.value >> 1,
                    read,
                    ten_bit: false,
                    start: byte.first,
                    header: None,
                }
            }
        } else {
            Kind10::Data
        };
        let byte_from = byte.first.saturating_sub(self.before());
        let byte_to = byte.last.saturating_add(self.after());
        let (ack_from, ack_to) = self.cell(pulse.at);
        // The error text of an unacknowledged bit goes on the arrow of that bit.
        let (error, arrow) = match what {
            Kind10::Header { high } => {
                transaction.pending = Some(Pending {
                    high,
                    first: byte.first,
                    last: byte.last,
                    ack: pulse.at,
                });
                transaction.expect_address = false;
                (false, text::ack_arrow(true))
            }
            Kind10::Address {
                address,
                read,
                ten_bit,
                start,
                header,
            } => {
                transaction.expect_address = false;
                transaction.read = read;
                if transaction.address.is_none() {
                    transaction.address = Some(address);
                    transaction.first_read = read;
                    transaction.first_ten_bit = ten_bit;
                }
                if ten_bit && !read {
                    transaction.ten_bit_address = Some(address);
                }
                let shown = self.display.address(address, read, ten_bit);
                if let Some(header) = header {
                    // The header byte of a 10-bit address and its acknowledge, now that the address is known.
                    let from = header.first.saturating_sub(self.before());
                    let to = header.last.saturating_add(self.after());
                    self.push_part(sink, from, to, text::address(read, shown.clone()), false)?;
                    let (from, to) = self.cell(header.ack);
                    self.push_part(sink, from, to, text::acknowledge(true), false)?;
                }
                self.push_part(
                    sink,
                    byte_from,
                    byte_to,
                    text::address(read, shown.clone()),
                    false,
                )?;
                sink.emit(
                    Event {
                        kind: items::ADDRESS,
                        direction: if read { items::READ } else { items::WRITE },
                        ack: acknowledgement,
                        ten_bit,
                        value: address,
                        error: !ack,
                    }
                    .item(start, item_end),
                )?;
                self.push_part(sink, ack_from, ack_to, text::acknowledge(ack), !ack)?;
                (
                    !ack,
                    if ack {
                        text::ack_arrow(true)
                    } else {
                        text::address_nak(shown)
                    },
                )
            }
            Kind10::Data => {
                // A NAK is an error after a byte written to a target; the controller's own NAK of
                // a byte it reads is how a read ends.
                let error = !ack && !transaction.read;
                let shown = self.display.byte(byte.value);
                self.push_part(
                    sink,
                    byte_from,
                    byte_to,
                    text::data(transaction.read, shown.clone()),
                    false,
                )?;
                sink.emit(
                    Event {
                        kind: items::DATA,
                        direction: if transaction.read {
                            items::READ
                        } else {
                            items::WRITE
                        },
                        ack: acknowledgement,
                        ten_bit: false,
                        value: byte.value,
                        error,
                    }
                    .item(byte.first, item_end),
                )?;
                self.push_part(sink, ack_from, ack_to, text::acknowledge(ack), error)?;
                (
                    error,
                    if error {
                        text::data_nak(shown)
                    } else {
                        text::ack_arrow(ack)
                    },
                )
            }
        };
        transaction.errored |= error;
        self.transaction = Some(transaction);
        put(
            sink,
            Kind::Edge {
                at: pulse.at,
                rising: true,
            },
            Target::Channel(self.scl_channel),
            arrow,
            error,
        )
    }
}
