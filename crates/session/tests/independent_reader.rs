//! An independently written reader and writer of the session file format (spec SC-012).
//!
//! This file implements `contracts/file-format.md` from scratch (std only: its own CRC-32C,
//! varints, framing and decoders) and shares no code with the crate's loader; it must not import
//! crate internals. It proves the contract is sufficient to read and write the format, and that
//! the crate agrees with it on every committed fixture and on generated files.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use common::{End, Rng, TempDir, data, record};
use la_devices::TriggerKind;
use la_session::{Control, LoadLimits, load_blocking, save_blocking};
use la_storage::Limits;

// ---- the independent implementation ------------------------------------------------------

fn crc(data: &[u8]) -> u32 {
    let mut table = [0_u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0x82F6_3B78
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    let mut c = 0xFFFF_FFFF_u32;
    for &b in data {
        c = table[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("overflow")?;
        let s = self.b.get(self.at..end).ok_or("short")?;
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn varint(&mut self) -> Result<u64, String> {
        let mut value = 0_u64;
        for i in 0..10 {
            let byte = self.u8()?;
            if i == 9 && byte > 1 {
                return Err("varint too big".into());
            }
            value |= u64::from(byte & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                if byte == 0 && i > 0 {
                    return Err("non-canonical varint".into());
                }
                return Ok(value);
            }
        }
        Err("varint too long".into())
    }
    fn string(&mut self) -> Result<String, String> {
        let len = self.varint()?;
        if len > 4096 {
            return Err("text too long".into());
        }
        String::from_utf8(self.take(len as usize)?.to_vec()).map_err(|_| "utf8".to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Model {
    simulated: bool,
    seed: Option<u64>,
    device_id: String,
    device_name: String,
    desc_channels: u8,
    enabled_mask: u32,
    requested_rate: u64,
    applied_rate: u64,
    samples_before_trigger: u64,
    trigger: Option<(u8, u8)>,
    start_secs: i64,
    start_nanos: u32,
    channels: u8,
    rate: u64,
    length: u64,
    stopped: bool,
    interrupted: bool,
    gaps: Vec<(u64, u64)>,
    initial: Vec<bool>,
    transitions: Vec<Vec<u64>>,
    extras: Vec<([u8; 4], bool, Vec<u8>)>,
}

fn parse(file: &[u8]) -> Result<Model, String> {
    if file.len() < 32 + 16 {
        return Err("too short".into());
    }
    if file[..8] != [0x89, b'L', b'A', b'S', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err("magic".into());
    }
    let mut r = Reader { b: file, at: 8 };
    let (major, _minor, flags) = (r.take(2)?, r.take(2)?, r.u32()?);
    if u16::from_le_bytes(major.try_into().unwrap()) != 1 || flags != 0 {
        return Err("version or flags".into());
    }
    let length = r.u64()?;
    let count = r.u32()?;
    if r.u32()? != crc(&file[..28]) {
        return Err("header crc".into());
    }
    if length != file.len() as u64 || !(2..=64).contains(&count) {
        return Err("length or count".into());
    }
    let mut sections = Vec::new();
    for _ in 0..count {
        let start = r.at;
        let tag: [u8; 4] = r.take(4)?.try_into().unwrap();
        let flags = r.u32()?;
        let len = r.u64()?;
        if flags & !1 != 0 || !tag.iter().all(u8::is_ascii_alphanumeric) {
            return Err("section head".into());
        }
        let payload = r.take(usize::try_from(len).map_err(|_| "len")?)?.to_vec();
        let end = r.at;
        if r.u32()? != crc(&file[start..end]) {
            return Err("section crc".into());
        }
        sections.push((tag, flags & 1 == 1, payload));
    }
    // Footer.
    if r.at + 16 != file.len() {
        return Err("footer position".into());
    }
    let footer = r.take(16)?;
    if &footer[..4] != b"LAEN"
        || u64::from_le_bytes(footer[4..12].try_into().unwrap()) != length
        || u32::from_le_bytes(footer[12..].try_into().unwrap()) != crc(&footer[..12])
    {
        return Err("footer".into());
    }
    if sections[0].0 != *b"META" || sections[1].0 != *b"CAPT" {
        return Err("order".into());
    }
    for pair in sections[2..].windows(2) {
        if pair[0].0 >= pair[1].0 {
            return Err("optional order".into());
        }
    }

    // META.
    let mut m = Reader {
        b: &sections[0].2,
        at: 0,
    };
    let flags = m.u8()?;
    if flags & !7 != 0 {
        return Err("meta flags".into());
    }
    let seed = if flags & 2 != 0 { Some(m.u64()?) } else { None };
    let device_id = m.string()?;
    let device_name = m.string()?;
    let desc_channels = m.u8()?;
    let enabled_mask = m.u32()?;
    let requested_rate = m.u64()?;
    let applied_rate = m.u64()?;
    let samples_before_trigger = m.u64()?;
    let trigger = if flags & 4 != 0 {
        Some((m.u8()?, m.u8()?))
    } else {
        None
    };
    let start_secs = m.i64()?;
    let start_nanos = m.u32()?;
    if m.at != m.b.len() {
        return Err("meta trailing".into());
    }

    // CAPT.
    let mut c = Reader {
        b: &sections[1].2,
        at: 0,
    };
    let channels = c.u8()?;
    let cflags = c.u8()?;
    if cflags & !3 != 0 {
        return Err("capt flags".into());
    }
    let rate = c.u64()?;
    let capture_length = c.u64()?;
    let gap_count = c.varint()?;
    let mut gaps = Vec::new();
    let mut end = 0_u64;
    for i in 0..gap_count {
        let delta = c.varint()?;
        let len = c.varint()?;
        if len == 0 || (i > 0 && delta == 0) {
            return Err("gap".into());
        }
        let start = end.checked_add(delta).ok_or("gap overflow")?;
        end = start.checked_add(len).ok_or("gap overflow")?;
        gaps.push((start, len));
    }
    let mut table = Vec::new();
    for _ in 0..channels {
        let initial = c.u8()?;
        if initial > 1 {
            return Err("initial".into());
        }
        table.push((initial == 1, c.varint()?, c.varint()?));
    }
    let mut initial = Vec::new();
    let mut transitions = Vec::new();
    for (high, count, len) in table {
        let before = c.at;
        let mut previous = 0_u64;
        let mut list = Vec::new();
        for _ in 0..count {
            previous = previous.checked_add(c.varint()?).ok_or("overflow")?;
            list.push(previous);
        }
        if (c.at - before) as u64 != len {
            return Err("stream length".into());
        }
        initial.push(high);
        transitions.push(list);
    }
    if c.at != c.b.len() {
        return Err("capt trailing".into());
    }
    Ok(Model {
        simulated: flags & 1 != 0,
        seed,
        device_id,
        device_name,
        desc_channels,
        enabled_mask,
        requested_rate,
        applied_rate,
        samples_before_trigger,
        trigger,
        start_secs,
        start_nanos,
        channels,
        rate,
        length: capture_length,
        stopped: cflags & 1 != 0,
        interrupted: cflags & 2 != 0,
        gaps,
        initial,
        transitions,
        extras: sections[2..].to_vec(),
    })
}

fn put_varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn put_string(s: &str, out: &mut Vec<u8>) {
    put_varint(s.len() as u64, out);
    out.extend_from_slice(s.as_bytes());
}

fn write(m: &Model) -> Vec<u8> {
    let mut meta = vec![
        u8::from(m.simulated)
            | (u8::from(m.seed.is_some()) << 1)
            | (u8::from(m.trigger.is_some()) << 2),
    ];
    if let Some(seed) = m.seed {
        meta.extend_from_slice(&seed.to_le_bytes());
    }
    put_string(&m.device_id, &mut meta);
    put_string(&m.device_name, &mut meta);
    meta.push(m.desc_channels);
    meta.extend_from_slice(&m.enabled_mask.to_le_bytes());
    meta.extend_from_slice(&m.requested_rate.to_le_bytes());
    meta.extend_from_slice(&m.applied_rate.to_le_bytes());
    meta.extend_from_slice(&m.samples_before_trigger.to_le_bytes());
    if let Some((channel, kind)) = m.trigger {
        meta.extend_from_slice(&[channel, kind]);
    }
    meta.extend_from_slice(&m.start_secs.to_le_bytes());
    meta.extend_from_slice(&m.start_nanos.to_le_bytes());

    let mut capt = vec![
        m.channels,
        u8::from(m.stopped) | (u8::from(m.interrupted) << 1),
    ];
    capt.extend_from_slice(&m.rate.to_le_bytes());
    capt.extend_from_slice(&m.length.to_le_bytes());
    put_varint(m.gaps.len() as u64, &mut capt);
    let mut end = 0_u64;
    for (start, len) in &m.gaps {
        put_varint(start - end, &mut capt);
        put_varint(*len, &mut capt);
        end = start + len;
    }
    let mut streams = Vec::new();
    for list in &m.transitions {
        let mut s = Vec::new();
        let mut previous = 0_u64;
        for p in list {
            put_varint(p - previous, &mut s);
            previous = *p;
        }
        streams.push(s);
    }
    for ((high, list), s) in m.initial.iter().zip(&m.transitions).zip(&streams) {
        capt.push(u8::from(*high));
        put_varint(list.len() as u64, &mut capt);
        put_varint(s.len() as u64, &mut capt);
    }
    for s in &streams {
        capt.extend_from_slice(s);
    }

    let mut all = vec![(*b"META", true, meta), (*b"CAPT", true, capt)];
    all.extend(m.extras.iter().cloned());
    let mut body = Vec::new();
    for (tag, critical, payload) in &all {
        let mut frame = Vec::new();
        frame.extend_from_slice(tag);
        frame.extend_from_slice(&u32::from(*critical).to_le_bytes());
        frame.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        frame.extend_from_slice(payload);
        body.extend_from_slice(&frame);
        body.extend_from_slice(&crc(&frame).to_le_bytes());
    }
    let total = (32 + body.len() + 16) as u64;
    let mut out = vec![
        0x89, b'L', b'A', b'S', 0x0D, 0x0A, 0x1A, 0x0A, 1, 0, 0, 0, 0, 0, 0, 0,
    ];
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&(all.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc(&out).to_le_bytes());
    out.extend_from_slice(&body);
    let mut footer = b"LAEN".to_vec();
    footer.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&footer);
    out.extend_from_slice(&crc(&footer).to_le_bytes());
    out
}

// ---- comparison with the crate -------------------------------------------------------------

fn kind_code(kind: TriggerKind) -> u8 {
    match kind {
        TriggerKind::Level(false) => 0,
        TriggerKind::Level(true) => 1,
        TriggerKind::RisingEdge => 2,
        TriggerKind::FallingEdge => 3,
        TriggerKind::AnyEdge => 4,
    }
}

fn assert_agrees(model: &Model, session: &la_session::Session, what: &str) {
    let s = session.snapshot();
    let d = session.description();
    assert_eq!(model.channels, s.channels(), "{what}");
    assert_eq!(model.rate, s.sample_rate_hz(), "{what}");
    assert_eq!(model.length, s.length(), "{what}");
    let gaps: Vec<(u64, u64)> = s.gaps().iter().map(|g| (g.start, g.length)).collect();
    assert_eq!(model.gaps, gaps, "{what}");
    assert_eq!(model.interrupted, s.status().interrupted, "{what}");
    assert_eq!(model.stopped, s.status().stopped_by_limit, "{what}");
    for channel in 0..s.channels() {
        let list: Vec<u64> = if s.transition_count(channel).unwrap() == 0 {
            vec![]
        } else {
            s.transitions(channel, 0, s.length())
                .unwrap()
                .map(|e| e.position)
                .collect()
        };
        assert_eq!(
            model.transitions[usize::from(channel)],
            list,
            "{what}: channel {channel}"
        );
        if !list.is_empty() {
            assert_eq!(
                model.initial[usize::from(channel)],
                s.initial_high(channel).unwrap(),
                "{what}"
            );
        }
    }
    assert_eq!(model.simulated, d.simulated(), "{what}");
    assert_eq!(model.seed, d.seed(), "{what}");
    assert_eq!(model.device_id, d.device_id().as_str(), "{what}");
    assert_eq!(model.device_name, d.device_name(), "{what}");
    assert_eq!(model.desc_channels, d.channels(), "{what}");
    assert_eq!(model.enabled_mask, d.enabled_mask(), "{what}");
    assert_eq!(model.requested_rate, d.requested_sample_rate_hz(), "{what}");
    assert_eq!(model.applied_rate, d.sample_rate_hz(), "{what}");
    assert_eq!(
        model.samples_before_trigger,
        d.samples_before_trigger(),
        "{what}"
    );
    assert_eq!(
        model.trigger,
        d.trigger().map(|t| (t.channel, kind_code(t.kind))),
        "{what}"
    );
    let (secs, nanos) = match d.started_at().duration_since(UNIX_EPOCH) {
        Ok(after) => (after.as_secs() as i64, after.subsec_nanos()),
        Err(before) => {
            let before = before.duration();
            if before.subsec_nanos() == 0 {
                (-(before.as_secs() as i64), 0)
            } else {
                (
                    -(before.as_secs() as i64) - 1,
                    1_000_000_000 - before.subsec_nanos(),
                )
            }
        }
    };
    assert_eq!(
        (model.start_secs, model.start_nanos),
        (secs, nanos),
        "{what}"
    );
    let extras: Vec<([u8; 4], bool, Vec<u8>)> = session
        .extras()
        .iter()
        .map(|e| {
            (
                e.tag.as_str().as_bytes().try_into().unwrap(),
                e.critical,
                e.payload.clone(),
            )
        })
        .collect();
    assert_eq!(model.extras, extras, "{what}");
}

fn fixture_files() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    for dir in ["reference", "v1_0"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "lasession") {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(files.len() >= 6, "{files:?}");
    files
}

#[test]
fn the_independent_reader_agrees_with_the_loader_on_every_fixture() {
    for path in fixture_files() {
        let bytes = std::fs::read(&path).unwrap();
        let what = path.file_name().unwrap().to_string_lossy().into_owned();
        let model = parse(&bytes)
            .unwrap_or_else(|e| panic!("{what}: the independent reader refused it: {e}"));
        let session = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
        assert_agrees(&model, &session, &what);
        // Writing the model again with the independent writer gives the same bytes.
        assert_eq!(
            write(&model),
            bytes,
            "{what}: the independent writer differs"
        );
    }
}

#[test]
fn the_independent_reader_agrees_with_the_loader_on_generated_files() {
    let mut rng = Rng::new(808);
    let dir = TempDir::new("indep");
    for case in 0..200_u64 {
        let channels = u8::try_from(1 + rng.below(32)).unwrap();
        let mut parts = vec![(None, usize::try_from(rng.below(4_000)).unwrap())];
        if rng.below(2) == 0 {
            parts.push((
                Some(1 + rng.below(300)),
                usize::try_from(rng.below(1_000)).unwrap(),
            ));
        }
        let end =
            [End::Closed, End::Interrupted, End::Open][usize::try_from(rng.below(3)).unwrap()];
        let capture = record(channels, 30, &parts, end, case, Limits::default());
        let path = dir.join("g");
        save_blocking(
            &path,
            &data(&capture, case % 2 == 0, Some(case)),
            &Control::new(),
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let model = parse(&bytes).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let session = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
        assert_agrees(&model, &session, &format!("case {case}"));
    }
}

#[test]
fn a_file_written_by_the_independent_writer_loads_in_the_crate() {
    let model = Model {
        simulated: true,
        seed: Some(42),
        device_id: "virtual:0".into(),
        device_name: "Independent".into(),
        desc_channels: 3,
        enabled_mask: 0b101,
        requested_rate: 1_000_000,
        applied_rate: 2_000_000,
        samples_before_trigger: 11,
        trigger: Some((2, 4)),
        start_secs: -5,
        start_nanos: 999_999_900,
        channels: 3,
        rate: 2_000_000,
        length: 300,
        stopped: false,
        interrupted: true,
        gaps: vec![(0, 10), (100, 20)],
        initial: vec![true, false, false],
        transitions: vec![vec![11, 50, 120, 299], vec![], vec![99, 130]],
        extras: vec![(*b"NOTE", false, b"hi".to_vec()), (*b"ZETA", false, vec![])],
    };
    let bytes = write(&model);
    assert_eq!(
        parse(&bytes).unwrap(),
        model,
        "the independent reader reads its own writer"
    );
    let dir = TempDir::new("indep-write");
    let path = dir.join("w");
    std::fs::write(&path, &bytes).unwrap();
    let session = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    assert_agrees(&model, &session, "independent writer");
}
