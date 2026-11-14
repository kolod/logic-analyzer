//! In-memory files, crafted files and allocation counting for the hostile-input tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation,
    unsafe_code
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::{self, Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use la_session::harness::{DirEntryInfo, FileSystem, PathKind, WriteSink};
use la_storage::Limits;

use super::{End, TempDir, data, record};

/// A file system that serves one file from memory (and nothing else).
#[derive(Clone)]
pub struct MemFs {
    pub bytes: Arc<Vec<u8>>,
    /// The length reported when the file is opened (differs from `bytes.len()` to simulate a
    /// file that changes while it is read).
    pub reported_length: u64,
    /// Total bytes handed out by `read`.
    pub bytes_read: Arc<AtomicU64>,
}

impl MemFs {
    pub fn new(bytes: Vec<u8>) -> Self {
        let reported_length = bytes.len() as u64;
        Self {
            bytes: Arc::new(bytes),
            reported_length,
            bytes_read: Arc::new(AtomicU64::new(0)),
        }
    }
}

struct CountingReader {
    inner: Cursor<Vec<u8>>,
    counter: Arc<AtomicU64>,
}

impl Read for CountingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.counter.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl FileSystem for MemFs {
    fn create_exclusive(&self, _: &Path) -> io::Result<Box<dyn WriteSink>> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    fn open_read(&self, _: &Path) -> io::Result<(Box<dyn Read + Send>, u64)> {
        Ok((
            Box::new(CountingReader {
                inner: Cursor::new((*self.bytes).clone()),
                counter: Arc::clone(&self.bytes_read),
            }),
            self.reported_length,
        ))
    }

    fn rename(&self, _: &Path, _: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    fn remove_file(&self, _: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    fn sync_dir(&self, _: &Path) -> io::Result<()> {
        Ok(())
    }

    fn list_dir(&self, _: &Path) -> io::Result<Vec<DirEntryInfo>> {
        Ok(Vec::new())
    }

    fn kind(&self, _: &Path) -> io::Result<Option<PathKind>> {
        Ok(Some(PathKind::File))
    }
}

/// Loads session bytes from memory.
pub fn load_bytes(
    bytes: &[u8],
    limits: &la_session::LoadLimits,
) -> la_core::Result<la_session::Session> {
    la_session::harness::load_with(
        &MemFs::new(bytes.to_vec()),
        Path::new("memory"),
        limits,
        &la_session::Control::new(),
    )
}

/// The error code of loading `bytes` (panics if it loads).
pub fn load_error_code(bytes: &[u8]) -> &'static str {
    load_bytes(bytes, &la_session::LoadLimits::default())
        .map(|_| ())
        .expect_err("the file must be rejected")
        .code()
}

/// Bitwise CRC-32C, written independently of the crate.
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0x82F6_3B78
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// One section of a crafted file.
#[derive(Clone, Debug)]
pub struct RawSection {
    pub tag: [u8; 4],
    pub critical: bool,
    pub payload: Vec<u8>,
}

/// Splits a well-formed file into its sections without validating anything.
pub fn split_sections(bytes: &[u8]) -> Vec<RawSection> {
    let count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let mut at = 32;
    let mut out = Vec::new();
    for _ in 0..count {
        let tag: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
        let flags = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap());
        let len = u64::from_le_bytes(bytes[at + 8..at + 16].try_into().unwrap()) as usize;
        let payload = bytes[at + 16..at + 16 + len].to_vec();
        out.push(RawSection {
            tag,
            critical: flags & 1 == 1,
            payload,
        });
        at += 16 + len + 4;
    }
    out
}

/// Builds a file from sections with every checksum correct.
pub fn build_file(sections: &[RawSection]) -> Vec<u8> {
    let mut body = Vec::new();
    for s in sections {
        let mut frame = Vec::new();
        frame.extend_from_slice(&s.tag);
        frame.extend_from_slice(&u32::from(s.critical).to_le_bytes());
        frame.extend_from_slice(&(s.payload.len() as u64).to_le_bytes());
        frame.extend_from_slice(&s.payload);
        let crc = crc32c(&frame);
        body.extend_from_slice(&frame);
        body.extend_from_slice(&crc.to_le_bytes());
    }
    let total = (32 + body.len() + 16) as u64;
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'L', b'A', b'S', 0x0D, 0x0A, 0x1A, 0x0A]);
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&(sections.len() as u32).to_le_bytes());
    let crc = crc32c(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&body);
    let mut footer = Vec::new();
    footer.extend_from_slice(b"LAEN");
    footer.extend_from_slice(&total.to_le_bytes());
    let crc = crc32c(&footer);
    out.extend_from_slice(&footer);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

/// Recomputes the header, section and footer checksums of a (possibly mutated) file, as far as
/// its framing can still be followed. Used to get mutated files past the checksum checks.
pub fn repair_checksums(bytes: &mut [u8]) {
    if bytes.len() >= 32 {
        let crc = crc32c(&bytes[..28]);
        bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    }
    let count = if bytes.len() >= 28 {
        u32::from_le_bytes(bytes[24..28].try_into().unwrap())
    } else {
        0
    };
    let mut at = 32_usize;
    for _ in 0..count.min(64) {
        if at + 16 > bytes.len() {
            return;
        }
        let len = u64::from_le_bytes(bytes[at + 8..at + 16].try_into().unwrap());
        let Some(end) = usize::try_from(len)
            .ok()
            .and_then(|l| (at + 16).checked_add(l))
        else {
            return;
        };
        if end + 4 > bytes.len() {
            return;
        }
        let crc = crc32c(&bytes[at..end]);
        bytes[end..end + 4].copy_from_slice(&crc.to_le_bytes());
        at = end + 4;
    }
    if bytes.len() >= at + 16 {
        let crc = crc32c(&bytes[at..at + 12]);
        bytes[at + 12..at + 16].copy_from_slice(&crc.to_le_bytes());
    }
}

/// Appends a canonical varint.
pub fn varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// A hand-built `CAPT` payload.
#[derive(Clone)]
pub struct CaptPayload {
    pub channels: u8,
    pub flags: u8,
    pub rate: u64,
    pub length: u64,
    pub gaps: Vec<(u64, u64)>,
    pub table: Vec<(u8, u64, u64)>,
    pub streams: Vec<Vec<u64>>,
}

impl CaptPayload {
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = vec![self.channels, self.flags];
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&self.length.to_le_bytes());
        varint(self.gaps.len() as u64, &mut out);
        for (delta, len) in &self.gaps {
            varint(*delta, &mut out);
            varint(*len, &mut out);
        }
        for (initial, count, len) in &self.table {
            out.push(*initial);
            varint(*count, &mut out);
            varint(*len, &mut out);
        }
        for stream in &self.streams {
            for v in stream {
                varint(*v, &mut out);
            }
        }
        out
    }
}

/// A real 2-channel session (12.5 MHz, 100 samples) saved to bytes.
pub fn real_file() -> Vec<u8> {
    let capture = record(2, 50, &[(None, 100)], End::Closed, 1, Limits::default());
    let dir = TempDir::new("meta-src");
    let path = dir.join("s");
    la_session::save_blocking(
        &path,
        &data(&capture, false, None),
        &la_session::Control::new(),
    )
    .unwrap();
    std::fs::read(&path).unwrap()
}

/// A file with the `META` of a real 2-channel session at 12.5 MHz and the given `CAPT` payload.
pub fn file_with_capture(capt: &[u8]) -> Vec<u8> {
    let mut sections = split_sections(&real_file());
    sections[1].payload = capt.to_vec();
    build_file(&sections)
}

/// A valid `CAPT` for [`file_with_capture`]: 2 channels, 12.5 MHz, 50 positions, one gap of 5 at
/// position 20, transitions at 10 and 30 on channel 0 and 25 and 40 on channel 1.
pub fn small_capt() -> CaptPayload {
    CaptPayload {
        channels: 2,
        flags: 0,
        rate: 12_500_000,
        length: 50,
        gaps: vec![(20, 5)],
        table: vec![(0, 2, 2), (1, 2, 2)],
        streams: vec![vec![10, 20], vec![25, 15]],
    }
}

thread_local! {
    static CURRENT: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
}

/// Per-thread allocation counting: install with
/// `#[global_allocator] static ALLOC: common::Counting = common::Counting;`.
pub struct Counting;

fn note(delta: i64) {
    let _ = CURRENT.try_with(|c| {
        let now = c.get() + delta;
        c.set(now);
        let _ = PEAK.try_with(|p| {
            if now > p.get() {
                p.set(now);
            }
        });
    });
}

// SAFETY: every method forwards to `System` unchanged; the counters only observe the sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size() as i64);
        // SAFETY: the caller upholds the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note(-(layout.size() as i64));
        // SAFETY: the caller upholds the contract of `GlobalAlloc::dealloc`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size as i64 - layout.size() as i64);
        // SAFETY: the caller upholds the contract of `GlobalAlloc::realloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Runs `work` and returns its result with the peak number of bytes this thread had allocated
/// above the level at the start (needs the `Counting` allocator to be installed).
pub fn peak_allocation<T>(work: impl FnOnce() -> T) -> (T, i64) {
    let start = CURRENT.with(Cell::get);
    PEAK.with(|p| p.set(start));
    let result = work();
    (result, PEAK.with(Cell::get) - start)
}
