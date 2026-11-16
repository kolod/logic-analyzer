//! The `DECO` section: the setup of the decoder instances of a capture
//! (`contracts/deco-section.md`).
//!
//! Reading validates every rule of the contract and never trusts a length before it has been
//! checked against the bytes that remain. A record of a decoder this version does not know (or that a
//! newer version wrote) is kept as the bytes it came in, so that saving writes it back untouched.

use std::collections::BTreeMap;

use super::Rejection;
use super::varint::{Invalid, Reader, Writer};
use crate::descriptor::{Descriptor, OptionKind};
use crate::registry::Registry;
use crate::setup::{MAX_INSTANCES, MAX_NAME_CHARS, MAX_STACK_DEPTH, Setup, Source, Value};
use crate::text::sanitize;

/// The version of the payload this code reads and writes.
pub const VERSION: u8 = 1;
/// The longest saved instance name, in bytes.
pub const MAX_NAME_BYTES: u64 = 256;
/// The longest identifier.
const MAX_ID_BYTES: u64 = 48;
/// The most channels and the most options of an instance.
const MAX_ENTRIES: u64 = 16;

/// What the reader checks a payload against.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    /// The decoders this version knows.
    pub registry: &'a Registry,
    /// The number of channels of the capture.
    pub channels: u8,
}

/// A record that was understood.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// What the user had chosen.
    pub setup: Setup,
    /// The decoder version the options were saved with.
    pub decoder_version: u16,
    /// How many options this version does not know (kept in the raw record).
    pub skipped_options: usize,
}

/// One record of the section.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// The instance id.
    pub id: u32,
    /// The content of the record as it was read (written back as it is when unchanged).
    pub raw: Vec<u8>,
    /// What it means, or `None` for a decoder this version does not know.
    pub parsed: Option<Parsed>,
}

fn identifier(reader: &mut Reader<'_>) -> Result<String, Invalid> {
    let text = reader.string(1, MAX_ID_BYTES)?;
    if !crate::descriptor::is_identifier(text) {
        return Err(Invalid("an identifier is malformed"));
    }
    Ok(text.to_owned())
}

/// One option as written: its id and its value.
fn read_value(reader: &mut Reader<'_>) -> Result<Value, Invalid> {
    match reader.u8()? {
        0 => Ok(Value::Integer(reader.zigzag()?)),
        1 => match reader.u8()? {
            0 => Ok(Value::Boolean(false)),
            1 => Ok(Value::Boolean(true)),
            _ => Err(Invalid("a boolean is neither 0 nor 1")),
        },
        2 => Ok(Value::Choice(reader.string(1, MAX_ID_BYTES)?.to_owned())),
        _ => Err(Invalid("an option has an unknown kind")),
    }
}

fn write_value(writer: &mut Writer, value: &Value) {
    match value {
        Value::Integer(v) => {
            writer.u8(0);
            writer.zigzag(*v);
        }
        Value::Boolean(v) => {
            writer.u8(1);
            writer.u8(u8::from(*v));
        }
        Value::Choice(v) => {
            writer.u8(2);
            writer.string(v);
        }
    }
}

/// Whether `value` is allowed for the option `id` of `descriptor`.
fn fits(descriptor: &Descriptor, id: &str, value: &Value) -> Option<bool> {
    let spec = descriptor.option(id)?;
    Some(match (spec.kind, value) {
        (OptionKind::Choice { choices, .. }, Value::Choice(v)) => {
            choices.iter().any(|(choice, _)| choice == v)
        }
        (OptionKind::Integer { min, max, .. }, Value::Integer(v)) => (min..=max).contains(v),
        (OptionKind::Boolean { .. }, Value::Boolean(_)) => true,
        _ => false,
    })
}

/// The content of the record of one instance.
#[must_use]
pub fn encode_instance(setup: &Setup, decoder_version: u16) -> Vec<u8> {
    let mut w = Writer::new();
    w.varint(u64::from(setup.instance));
    w.string(&setup.decoder);
    w.varint(u64::from(decoder_version));
    w.string(&sanitize(&setup.name));
    w.varint(match setup.source {
        Source::Capture => 0,
        Source::Instance(id) => u64::from(id).saturating_add(1),
    });
    w.u8(u8::from(setup.lane_visible));
    w.varint(u64::try_from(setup.channels.len()).unwrap_or(0));
    for (role, channel) in &setup.channels {
        w.string(role);
        w.varint(u64::from(*channel));
    }
    w.varint(u64::try_from(setup.options.len()).unwrap_or(0));
    for (id, value) in &setup.options {
        w.string(id);
        write_value(&mut w, value);
    }
    let mut generator = Writer::new();
    generator.varint(u64::try_from(setup.generator.len()).unwrap_or(0));
    for (id, value) in &setup.generator {
        generator.string(id);
        write_value(&mut generator, &Value::Integer(*value));
    }
    w.record(&generator.bytes);
    w.bytes
}

/// The payload of the section from the content of its records (in stack order).
#[must_use]
pub fn assemble(records: &[Vec<u8>]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(VERSION);
    w.varint(u64::try_from(records.len()).unwrap_or(0));
    for record in records {
        w.record(record);
    }
    w.bytes
}

/// Reads the payload.
///
/// # Errors
///
/// [`Rejection::Newer`] when the version byte is above this one's; [`Rejection::Invalid`] naming the
/// first rule of the contract that is broken.
pub fn decode(bytes: &[u8], context: &Context<'_>) -> Result<Vec<Record>, Rejection> {
    let mut reader = Reader::new(bytes);
    let version = reader.u8().map_err(Rejection::Invalid)?;
    if version > VERSION {
        return Err(Rejection::Newer);
    }
    if version != VERSION {
        return Err(Rejection::Invalid(Invalid("the version is not 1")));
    }
    read_records(&mut reader, context).map_err(Rejection::Invalid)
}

fn read_records(reader: &mut Reader<'_>, context: &Context<'_>) -> Result<Vec<Record>, Invalid> {
    let count = reader.bounded(MAX_INSTANCES as u64)?;
    let mut records: Vec<Record> = Vec::new();
    // For every record read so far, the instance it reads from, to check chains.
    let mut sources: BTreeMap<u32, Source> = BTreeMap::new();
    for _ in 0..count {
        let raw_reader = reader.record()?;
        let raw = raw_reader.rest().to_vec();
        let mut r = raw_reader;
        let id = u32::try_from(r.varint()?).map_err(|_| Invalid("an id is beyond 32 bits"))?;
        if sources.contains_key(&id) {
            return Err(Invalid("an id repeats"));
        }
        let decoder = identifier(&mut r)?;
        let decoder_version = u16::try_from(r.varint()?)
            .map_err(|_| Invalid("a decoder version is beyond 16 bits"))?;
        let name = r.string(0, MAX_NAME_BYTES)?;
        if sanitize(name) != name {
            return Err(Invalid("a name is not in its sanitized form"));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(Invalid("a name is too long"));
        }
        let source = match r.varint()? {
            0 => Source::Capture,
            n => {
                let below = u32::try_from(n.saturating_sub(1))
                    .map_err(|_| Invalid("a source is beyond 32 bits"))?;
                if !sources.contains_key(&below) {
                    return Err(Invalid("a source is not an earlier record"));
                }
                Source::Instance(below)
            }
        };
        let mut depth = 1_usize;
        let mut current = source;
        while let Source::Instance(below) = current {
            depth = depth.saturating_add(1);
            current = sources.get(&below).copied().unwrap_or(Source::Capture);
        }
        if depth > MAX_STACK_DEPTH {
            return Err(Invalid("a chain is deeper than allowed"));
        }
        let flags = r.u8()?;
        if flags & !1 != 0 {
            return Err(Invalid("a reserved bit is set"));
        }
        let channel_count = r.bounded(MAX_ENTRIES)?;
        let mut channels: BTreeMap<String, u8> = BTreeMap::new();
        let mut previous: Option<String> = None;
        for _ in 0..channel_count {
            let role = identifier(&mut r)?;
            if previous.as_ref().is_some_and(|p| *p >= role) {
                return Err(Invalid("roles are not in ascending order"));
            }
            let channel = u8::try_from(r.bounded(31)?).map_err(|_| Invalid("a channel"))?;
            if channel >= context.channels {
                return Err(Invalid("a channel is beyond the capture"));
            }
            previous = Some(role.clone());
            channels.insert(role, channel);
        }
        let option_count = r.bounded(MAX_ENTRIES)?;
        let mut options: Vec<(String, Value)> = Vec::new();
        let mut previous: Option<String> = None;
        for _ in 0..option_count {
            let option = identifier(&mut r)?;
            if previous.as_ref().is_some_and(|p| *p >= option) {
                return Err(Invalid("options are not in ascending order"));
            }
            let value = read_value(&mut r)?;
            previous = Some(option.clone());
            options.push((option, value));
        }
        let mut generator_reader = r.record()?;
        let generator_count = generator_reader.bounded(MAX_ENTRIES)?;
        let mut generator: BTreeMap<String, i64> = BTreeMap::new();
        let mut previous: Option<String> = None;
        for _ in 0..generator_count {
            let option = identifier(&mut generator_reader)?;
            if previous.as_ref().is_some_and(|p| *p >= option) {
                return Err(Invalid("generator options are not in ascending order"));
            }
            match read_value(&mut generator_reader)? {
                Value::Integer(v) => {
                    generator.insert(option.clone(), v);
                }
                _ => return Err(Invalid("a generator option is not a number")),
            }
            previous = Some(option);
        }
        if !generator_reader.is_empty() || !r.is_empty() {
            return Err(Invalid("bytes remain in a record"));
        }
        sources.insert(id, source);
        let known = context
            .registry
            .get(&decoder)
            .filter(|d| decoder_version <= d.version);
        let parsed = match known {
            None => None,
            Some(descriptor) => {
                let mut setup = Setup::new(id, &decoder);
                setup.name = name.to_owned();
                setup.source = source;
                setup.lane_visible = flags & 1 != 0;
                setup.generator = generator;
                for role in channels.keys() {
                    if descriptor.signal(role).is_none() {
                        return Err(Invalid("a signal role is unknown to its decoder"));
                    }
                }
                setup.channels = channels;
                let mut skipped = 0;
                for (option, value) in options {
                    match fits(descriptor, &option, &value) {
                        None => skipped = usize::saturating_add(skipped, 1),
                        Some(true) => {
                            setup.options.insert(option, value);
                        }
                        Some(false) => {
                            return Err(Invalid("an option value is outside its range or type"));
                        }
                    }
                }
                Some(Parsed {
                    setup,
                    decoder_version,
                    skipped_options: skipped,
                })
            }
        };
        records.push(Record { id, raw, parsed });
    }
    if !reader.is_empty() {
        return Err(Invalid("bytes remain after the last record"));
    }
    Ok(records)
}
