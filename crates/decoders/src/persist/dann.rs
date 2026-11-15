//! The `DANN` section: the results of the decoder instances (`contracts/deco-section.md`).
//!
//! Annotations are written sorted by start with the start as a delta, message keys through a string
//! table (in order of first use, so that the same state gives the same bytes), and items ordered by
//! end. Reading checks every count against the bytes that remain before allocating, and every
//! annotation and item against the capture exactly as the sink does when a decoder emits it.

use super::varint::{Invalid, Reader, Writer};
use super::{Rejection, intern};
use crate::model::{
    Annotation, Bounds, Item, Kind, MAX_ARG_BYTES, MAX_ARGS, MAX_PAYLOAD_BYTES, MAX_TEXT_BYTES,
    Payload, Shape, Style, Target, Text,
};
use crate::store::DEFAULT_MAX_ANNOTATIONS;
use crate::text::sanitize;

/// The version of the payload this code reads and writes.
pub const VERSION: u8 = 1;
/// The most strings of the table.
const MAX_STRINGS: u64 = 4_096;
/// The longest string of the table (a message key).
const MAX_KEY_BYTES: u64 = 96;
/// The fewest bytes an annotation or an item takes: a count is checked against this.
const MIN_ENTRY_BYTES: u64 = 7;

/// The results of one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The instance (as in `DECO`).
    pub instance: u32,
    /// The setup version the results were made from.
    pub setup_version: u64,
    /// The position up to which the instance had decoded.
    pub covered: u64,
    /// Whether the results are not whole.
    pub incomplete: bool,
    /// The annotations, sorted by start.
    pub annotations: Vec<Annotation>,
    /// The items, ordered by end.
    pub items: Vec<Item>,
}

/// What the reader checks a payload against.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The length of the capture.
    pub capture_length: u64,
    /// The number of channels of the capture.
    pub channels: u8,
    /// The ids of the instances `DECO` defines.
    pub instances: &'a [u32],
}

fn shape_code(shape: Shape) -> u8 {
    match shape {
        Shape::Dot => 0,
        Shape::Arrow => 1,
        Shape::Bar => 2,
        Shape::Flag => 3,
        Shape::Cross => 4,
    }
}

fn shape_of(code: u8) -> Result<Shape, Invalid> {
    Ok(match code {
        0 => Shape::Dot,
        1 => Shape::Arrow,
        2 => Shape::Bar,
        3 => Shape::Flag,
        4 => Shape::Cross,
        _ => return Err(Invalid("a shape is unknown")),
    })
}

/// The payload for these blocks.
#[must_use]
pub fn encode(blocks: &[Block]) -> Vec<u8> {
    let mut table: Vec<&str> = Vec::new();
    for block in blocks {
        for annotation in &block.annotations {
            if let Text::Message { key, .. } = &annotation.text {
                if !table.contains(key) {
                    table.push(key);
                }
            }
        }
    }
    let mut w = Writer::new();
    w.u8(VERSION);
    w.varint(u64::try_from(table.len()).unwrap_or(0));
    for key in &table {
        w.string(key);
    }
    w.varint(u64::try_from(blocks.len()).unwrap_or(0));
    for block in blocks {
        let mut b = Writer::new();
        b.varint(u64::from(block.instance));
        b.varint(block.setup_version);
        b.varint(block.covered);
        b.u8(u8::from(block.incomplete));
        b.varint(u64::try_from(block.annotations.len()).unwrap_or(0));
        b.varint(u64::try_from(block.items.len()).unwrap_or(0));
        let mut previous_start = 0_u64;
        for annotation in &block.annotations {
            let (start, end) = annotation.span();
            let (kind, length) = match annotation.kind {
                Kind::Range { .. } => (0, end.saturating_sub(start)),
                Kind::Point { .. } => (1, 0),
                Kind::Edge { rising: true, .. } => (2, 0),
                Kind::Edge { rising: false, .. } => (3, 0),
            };
            b.u8(kind);
            b.varint(start.saturating_sub(previous_start));
            previous_start = start;
            b.varint(length);
            b.varint(match annotation.target {
                Target::Lane => 0,
                Target::Channel(c) => u64::from(c).saturating_add(1),
            });
            b.u8(annotation.style.colour
                | (shape_code(annotation.style.shape) << 3)
                | (u8::from(annotation.error) << 6));
            match &annotation.text {
                Text::Literal(text) => {
                    b.u8(0);
                    b.string(text);
                }
                Text::Message { key, args } => {
                    b.u8(1);
                    let index = table.iter().position(|k| k == key).unwrap_or(0);
                    b.varint(u64::try_from(index).unwrap_or(0));
                    b.varint(u64::try_from(args.len()).unwrap_or(0));
                    for arg in args {
                        b.string(arg);
                    }
                }
            }
        }
        let mut previous_end = 0_u64;
        for item in &block.items {
            b.string(item.kind);
            b.zigzag(
                i64::try_from(item.start)
                    .unwrap_or(i64::MAX)
                    .saturating_sub(i64::try_from(previous_end).unwrap_or(i64::MAX)),
            );
            previous_end = item.end;
            b.varint(item.end.saturating_sub(item.start));
            b.u8(u8::from(item.error));
            match &item.payload {
                Payload::Integer(v) => {
                    b.u8(0);
                    b.varint(*v);
                }
                Payload::Bytes(bytes) => {
                    b.u8(1);
                    b.varint(u64::try_from(bytes.len()).unwrap_or(0));
                    b.bytes.extend_from_slice(bytes);
                }
                Payload::Text(text) => {
                    b.u8(2);
                    b.string(text);
                }
            }
        }
        w.record(&b.bytes);
    }
    w.bytes
}

/// Reads the payload.
///
/// # Errors
///
/// [`Rejection::Newer`] when the version byte is above this one's; [`Rejection::Invalid`] naming the
/// first rule of the contract that is broken.
pub fn decode(bytes: &[u8], context: &Context<'_>) -> Result<Vec<Block>, Rejection> {
    let mut reader = Reader::new(bytes);
    let version = reader.u8().map_err(Rejection::Invalid)?;
    if version > VERSION {
        return Err(Rejection::Newer);
    }
    if version != VERSION {
        return Err(Rejection::Invalid(Invalid("the version is not 1")));
    }
    read_blocks(&mut reader, context).map_err(Rejection::Invalid)
}

fn read_blocks(reader: &mut Reader<'_>, context: &Context<'_>) -> Result<Vec<Block>, Invalid> {
    let count = reader.bounded(MAX_STRINGS)?;
    // Every string takes at least two bytes: a count of strings is checked before allocating.
    if count.saturating_mul(2) > u64::try_from(reader.remaining()).unwrap_or(0) {
        return Err(Invalid("the string table is longer than the payload"));
    }
    let mut table: Vec<&'static str> = Vec::new();
    for _ in 0..count {
        let key = reader.string(1, MAX_KEY_BYTES)?;
        if table.contains(&key) {
            return Err(Invalid("a string repeats in the table"));
        }
        table.push(intern(key).ok_or(Invalid("too many distinct message keys"))?);
    }
    let block_count = reader.bounded(8)?;
    let mut blocks: Vec<Block> = Vec::new();
    let mut used: Vec<&'static str> = Vec::new();
    for _ in 0..block_count {
        let mut r = reader.record()?;
        let instance =
            u32::try_from(r.varint()?).map_err(|_| Invalid("an instance id is beyond 32 bits"))?;
        if !context.instances.contains(&instance) {
            return Err(Invalid("a block names an instance DECO does not define"));
        }
        if blocks.iter().any(|b| b.instance == instance) {
            return Err(Invalid("a block repeats an instance"));
        }
        let setup_version = r.varint()?;
        let covered = r.varint()?;
        let flags = r.u8()?;
        if flags & !1 != 0 {
            return Err(Invalid("a reserved bit is set"));
        }
        let annotation_count = r.bounded(DEFAULT_MAX_ANNOTATIONS as u64)?;
        let item_count = r.bounded(DEFAULT_MAX_ANNOTATIONS as u64)?;
        let remaining = u64::try_from(r.remaining()).unwrap_or(0);
        if annotation_count
            .saturating_add(item_count)
            .saturating_mul(MIN_ENTRY_BYTES)
            > remaining
        {
            return Err(Invalid("the counts are larger than the block"));
        }
        let bounds = Bounds {
            length: context.capture_length,
            channels: context.channels,
        };
        let mut annotations = Vec::with_capacity(usize::try_from(annotation_count).unwrap_or(0));
        let mut start = 0_u64;
        for _ in 0..annotation_count {
            let kind = r.u8()?;
            start = start
                .checked_add(r.varint()?)
                .ok_or(Invalid("a start overflows"))?;
            let length = r.varint()?;
            let target = r.bounded(32)?;
            let style = r.u8()?;
            if style & 0x80 != 0 {
                return Err(Invalid("a reserved bit is set"));
            }
            let text_kind = r.u8()?;
            let text = match text_kind {
                0 => {
                    let text = r.string(0, MAX_TEXT_BYTES as u64)?;
                    if sanitize(text) != text {
                        return Err(Invalid("a text is not in its sanitized form"));
                    }
                    Text::Literal(text.to_owned())
                }
                1 => {
                    let index = r.bounded(MAX_STRINGS)?;
                    let key = *table
                        .get(usize::try_from(index).unwrap_or(usize::MAX))
                        .ok_or(Invalid("a string index is out of range"))?;
                    if !used.contains(&key) {
                        used.push(key);
                    }
                    let argc = r.bounded(MAX_ARGS as u64)?;
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        let arg = r.string(0, MAX_ARG_BYTES as u64)?;
                        if sanitize(arg) != arg {
                            return Err(Invalid("an argument is not in its sanitized form"));
                        }
                        args.push(arg.to_owned());
                    }
                    Text::Message { key, args }
                }
                _ => return Err(Invalid("a text kind is unknown")),
            };
            let kind = match kind {
                0 => Kind::Range {
                    start,
                    end: start
                        .checked_add(length)
                        .ok_or(Invalid("an end overflows"))?,
                },
                1..=3 => {
                    if length != 0 {
                        return Err(Invalid("a point or an edge has a length"));
                    }
                    if kind == 1 {
                        Kind::Point { at: start }
                    } else {
                        Kind::Edge {
                            at: start,
                            rising: kind == 2,
                        }
                    }
                }
                _ => return Err(Invalid("an annotation kind is unknown")),
            };
            let annotation = Annotation {
                kind,
                target: match target {
                    0 => Target::Lane,
                    n => Target::Channel(
                        u8::try_from(n.saturating_sub(1)).map_err(|_| Invalid("a channel"))?,
                    ),
                },
                style: Style {
                    colour: style & 7,
                    shape: shape_of((style >> 3) & 7)?,
                },
                text,
                error: style & 0x40 != 0,
            };
            annotation
                .validate(&bounds)
                .map_err(|_| Invalid("an annotation does not fit the capture"))?;
            annotations.push(annotation);
        }
        let mut items = Vec::with_capacity(usize::try_from(item_count).unwrap_or(0));
        let mut previous_end = 0_u64;
        for _ in 0..item_count {
            let kind = r.string(1, 48)?;
            if !crate::descriptor::is_identifier(kind) {
                return Err(Invalid("an item kind is malformed"));
            }
            let kind = intern(kind).ok_or(Invalid("too many distinct item kinds"))?;
            let delta = r.zigzag()?;
            let item_start = i64::try_from(previous_end)
                .ok()
                .and_then(|p| p.checked_add(delta))
                .and_then(|s| u64::try_from(s).ok())
                .ok_or(Invalid("an item start is out of range"))?;
            let length = r.varint()?;
            let end = item_start
                .checked_add(length)
                .ok_or(Invalid("an item end overflows"))?;
            let item_flags = r.u8()?;
            if item_flags & !1 != 0 {
                return Err(Invalid("a reserved bit is set"));
            }
            let payload = match r.u8()? {
                0 => Payload::Integer(r.varint()?),
                1 => {
                    let n = r.bounded(MAX_PAYLOAD_BYTES as u64)?;
                    Payload::Bytes(r.take(n)?.to_vec())
                }
                2 => Payload::Text(r.string(0, MAX_PAYLOAD_BYTES as u64)?.to_owned()),
                _ => return Err(Invalid("a payload kind is unknown")),
            };
            let item = Item {
                kind,
                start: item_start,
                end,
                payload,
                error: item_flags & 1 != 0,
            };
            item.validate(&bounds, previous_end)
                .map_err(|_| Invalid("an item does not fit the capture"))?;
            previous_end = end;
            items.push(item);
        }
        if !r.is_empty() {
            return Err(Invalid("bytes remain in a block"));
        }
        blocks.push(Block {
            instance,
            setup_version,
            covered,
            incomplete: flags & 1 != 0,
            annotations,
            items,
        });
    }
    if !reader.is_empty() {
        return Err(Invalid("bytes remain after the last block"));
    }
    if used != table {
        return Err(Invalid("the string table is not in order of first use"));
    }
    Ok(blocks)
}
