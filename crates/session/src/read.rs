//! Safe loading of a session file.
//!
//! The file is read once, front to back, through a fixed-size buffer. Every size and count is
//! compared with the bytes that remain in the file or section before anything is read or
//! allocated from it, every section's checksum is verified, and the capture is built by the
//! storage restorer, which enforces the capture's own rules and the memory budget while it grows.

use std::io::{self, Read};
use std::path::Path;

use la_core::Result;

use crate::capture_codec::{self, PayloadSource};
use crate::control::Control;
use crate::crc::Crc32c;
use crate::error;
use crate::format::{Header, OrderChecker, SectionHead, TAG_CAPT, TAG_META, decode_footer};
use crate::fs::{FileSystem, RealFileSystem};
use crate::limits::{
    FOOTER_LEN, HEADER_LEN, IO_BUFFER_BYTES, LoadLimits, MAX_META_BYTES, SECTION_CRC_LEN,
    SECTION_HEAD_LEN,
};
use crate::meta;
use crate::session::{Section, Session};
use crate::varint::VarintDecoder;

/// Loads the session file at `path`.
///
/// # Errors
///
/// `session.not_a_session`, `session.unsupported_version`, `session.truncated`,
/// `session.damaged`, `session.inconsistent`, `session.unknown_critical_section`,
/// `session.too_large`, `session.memory_budget`, `session.io_read` or `session.cancelled`; see
/// the format contract. A failed load returns no partial result.
pub fn load_blocking(path: &Path, limits: &LoadLimits, control: &Control) -> Result<Session> {
    let result = load_with(&RealFileSystem, path, limits, control);
    control.finish(&result);
    result
}

/// Like [`load_blocking`] on an arbitrary file system (used by tests to inject failures).
///
/// # Errors
///
/// As [`load_blocking`].
pub fn load_with(
    fs: &dyn FileSystem,
    path: &Path,
    limits: &LoadLimits,
    control: &Control,
) -> Result<Session> {
    let started = std::time::Instant::now();
    limits.validate()?;
    let (inner, file_length) = fs.open_read(path).map_err(|e| read_failure(&e))?;
    if file_length > limits.max_file_bytes {
        return Err(error::too_large(
            "the file size",
            file_length,
            limits.max_file_bytes,
        ));
    }
    control.set_total(file_length);
    control.check()?;
    let mut reader = FileReader::new(inner, file_length, control);
    let session = read_session(&mut reader, limits)?;
    tracing::info!(
        target: "la_session",
        bytes = file_length,
        elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "session loaded"
    );
    Ok(session)
}

fn read_failure(e: &io::Error) -> la_core::Error {
    let advice = match e.kind() {
        io::ErrorKind::NotFound => "the file does not exist or was moved",
        io::ErrorKind::PermissionDenied => "there is no permission to read the file",
        _ => "check that the file is available and not in use",
    };
    error::io_read(
        format!("loading failed while reading the file: {advice}"),
        io::Error::new(e.kind(), e.to_string()),
    )
}

fn read_session<R: Read>(reader: &mut FileReader<'_, R>, limits: &LoadLimits) -> Result<Session> {
    // Header.
    let mut head = [0_u8; HEADER_LEN];
    let got = reader.read_up_to(&mut head)?;
    let header = Header::decode(head.get(..got).unwrap_or(&[]))?;
    if header.file_length > reader.file_length {
        return Err(error::truncated("the file is shorter than its header says"));
    }
    if header.file_length < reader.file_length {
        return Err(error::damaged(
            "the file is longer than its header says (bytes were appended)",
        ));
    }

    let mut order = OrderChecker::new();
    let mut description = None;
    let mut capture = None;
    let mut extras: Vec<Section> = Vec::new();
    let mut extras_bytes = 0_u64;
    for _ in 0..header.section_count {
        reader.start_crc();
        let mut frame = [0_u8; SECTION_HEAD_LEN];
        reader.read_exact_file(&mut frame)?;
        let section = SectionHead::decode(&frame)?;
        order.accept(section.tag)?;
        let available = reader
            .remaining_in_file()
            .saturating_sub(u64::try_from(FOOTER_LEN + SECTION_CRC_LEN).unwrap_or(20));
        if section.payload_len > available {
            return Err(error::truncated(
                "a section is longer than the rest of the file",
            ));
        }
        reader.begin_payload(section.payload_len);
        let content = match read_section(
            reader,
            &section,
            limits,
            &mut extras,
            &mut extras_bytes,
            &mut capture,
        ) {
            Ok(content) => content,
            // A damaged section usually fails its decoding first; report the checksum if that is
            // the real cause.
            Err(e) => return Err(prefer_checksum_error(reader, e)),
        };
        reader.verify_section_crc()?;
        // The description is decoded only after its checksum was verified.
        if let Some(bytes) = content {
            description = Some(meta::decode(&bytes)?);
        }
    }

    // Footer: it must begin exactly where the sections end and end exactly at the end.
    if reader.remaining_in_file() != u64::try_from(FOOTER_LEN).unwrap_or(16) {
        return Err(error::damaged(
            "the sections do not end where the footer begins",
        ));
    }
    let mut footer = [0_u8; FOOTER_LEN];
    reader.read_exact_file(&mut footer)?;
    if decode_footer(&footer)? != header.file_length {
        return Err(error::damaged(
            "the footer and the header disagree about the file length",
        ));
    }
    reader.expect_end()?;

    let (Some(description), Some(capture)) = (description, capture) else {
        return Err(error::damaged("a required section is missing"));
    };
    let snapshot = capture.reader().snapshot();
    if description.channels() != snapshot.channels()
        || description.sample_rate_hz() != snapshot.sample_rate_hz()
    {
        return Err(error::inconsistent(
            "the capture description disagrees with the capture about the channel count or sample rate",
        ));
    }
    Ok(Session::new(description, capture, extras))
}

/// If the section's checksum is wrong, the file is damaged and that is the error to report;
/// otherwise the original error stands.
fn prefer_checksum_error<R: Read>(
    reader: &mut FileReader<'_, R>,
    original: la_core::Error,
) -> la_core::Error {
    if original.code() == error::CANCELLED || original.code() == error::TRUNCATED {
        return original;
    }
    // Draining a large section just to name the cause better would read the whole file; only a
    // small remainder is worth it.
    if reader.payload_left > u64::try_from(IO_BUFFER_BYTES).unwrap_or(0) {
        return original;
    }
    match reader
        .skip_payload()
        .and_then(|()| reader.verify_section_crc())
    {
        Err(e) if e.code() == error::DAMAGED => e,
        _ => original,
    }
}

/// Reads the payload of one section: the capture is decoded in place, the description's bytes
/// are returned (to be decoded after the checksum), unknown sections are kept.
fn read_section<R: Read>(
    reader: &mut FileReader<'_, R>,
    section: &SectionHead,
    limits: &LoadLimits,
    extras: &mut Vec<Section>,
    extras_bytes: &mut u64,
    capture: &mut Option<la_storage::Capture>,
) -> Result<Option<Vec<u8>>> {
    if section.tag == TAG_META {
        if section.payload_len > MAX_META_BYTES {
            return Err(error::too_large(
                "the capture description section",
                section.payload_len,
                MAX_META_BYTES,
            ));
        }
        return Ok(Some(reader.read_payload_vec(section.payload_len)?));
    }
    if section.tag == TAG_CAPT {
        let decoded = capture_codec::decode(&mut *reader, &limits.storage)?;
        reader.expect_payload_consumed()?;
        *capture = Some(decoded);
        return Ok(None);
    }
    if section.critical {
        return Err(error::unknown_critical_section(section.tag.as_str()));
    }
    // An unknown optional section is kept; it counts against the memory budget.
    *extras_bytes = extras_bytes.saturating_add(section.payload_len);
    if *extras_bytes > limits.storage.memory_budget_bytes {
        return Err(error::memory_budget(
            "the optional sections alone exceed the budget",
        ));
    }
    let payload = reader.read_payload_vec(section.payload_len)?;
    extras.push(Section {
        tag: section.tag,
        critical: false,
        payload,
    });
    Ok(None)
}

/// Reads the file front to back through a fixed buffer, bounded by the file's length, while
/// checksumming the current section.
pub(crate) struct FileReader<'a, R: Read> {
    inner: R,
    buffer: Vec<u8>,
    pos: usize,
    len: usize,
    /// Start of the part of the buffer that the running checksum has not seen yet.
    crc_from: usize,
    crc: Crc32c,
    /// File bytes consumed before the current buffer.
    before: u64,
    file_length: u64,
    /// Payload bytes of the current section not yet read.
    payload_left: u64,
    control: &'a Control,
}

impl<'a, R: Read> FileReader<'a, R> {
    pub(crate) fn new(inner: R, file_length: u64, control: &'a Control) -> Self {
        Self {
            inner,
            buffer: vec![0; IO_BUFFER_BYTES],
            pos: 0,
            len: 0,
            crc_from: 0,
            crc: Crc32c::new(),
            before: 0,
            file_length,
            payload_left: 0,
            control,
        }
    }

    /// Bytes consumed so far.
    fn position(&self) -> u64 {
        self.before
            .saturating_add(u64::try_from(self.pos).unwrap_or(u64::MAX))
    }

    fn remaining_in_file(&self) -> u64 {
        self.file_length.saturating_sub(self.position())
    }

    /// Folds the consumed part of the buffer into the running checksum.
    fn fold_crc(&mut self) {
        if let Some(seen) = self.buffer.get(self.crc_from..self.pos) {
            self.crc.update(seen);
        }
        self.crc_from = self.pos;
    }

    /// Starts the checksum of a new section at the current position.
    fn start_crc(&mut self) {
        self.crc = Crc32c::new();
        self.crc_from = self.pos;
    }

    /// Reads the next block of the file into the (fully consumed) buffer.
    fn fill(&mut self) -> Result<()> {
        self.control.check()?;
        self.fold_crc();
        self.before = self
            .before
            .saturating_add(u64::try_from(self.len).unwrap_or(0));
        self.pos = 0;
        self.len = 0;
        self.crc_from = 0;
        let want = usize::try_from(
            self.remaining_in_file()
                .min(u64::try_from(self.buffer.len()).unwrap_or(u64::MAX)),
        )
        .unwrap_or(0);
        if want == 0 {
            return Err(error::truncated("the file ends before its content does"));
        }
        let target = self
            .buffer
            .get_mut(..want)
            .ok_or_else(|| error::internal("the read buffer"))?;
        loop {
            match self.inner.read(target) {
                Ok(0) => return Err(error::truncated("the file ends before its content does")),
                Ok(n) => {
                    self.len = n;
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(read_failure(&e)),
            }
        }
        self.control.set_done(
            self.before
                .saturating_add(u64::try_from(self.len).unwrap_or(0)),
        );
        Ok(())
    }

    fn byte(&mut self) -> Result<u8> {
        if self.pos >= self.len {
            self.fill()?;
        }
        let byte = self
            .buffer
            .get(self.pos)
            .copied()
            .ok_or_else(|| error::internal("the read buffer"))?;
        self.pos = self.pos.saturating_add(1);
        Ok(byte)
    }

    /// Reads exactly `out.len()` bytes of the file (not limited by the section).
    fn read_exact_file(&mut self, out: &mut [u8]) -> Result<()> {
        if u64::try_from(out.len()).unwrap_or(u64::MAX) > self.remaining_in_file() {
            return Err(error::truncated("the file ends before its content does"));
        }
        let mut done = 0;
        while done < out.len() {
            if self.pos >= self.len {
                self.fill()?;
            }
            let available = self.len.saturating_sub(self.pos);
            let n = available.min(out.len().saturating_sub(done));
            let from = self
                .buffer
                .get(self.pos..self.pos.saturating_add(n))
                .ok_or_else(|| error::internal("the read buffer"))?;
            let to = out
                .get_mut(done..done.saturating_add(n))
                .ok_or_else(|| error::internal("the output buffer"))?;
            to.copy_from_slice(from);
            self.pos = self.pos.saturating_add(n);
            done = done.saturating_add(n);
        }
        Ok(())
    }

    /// Reads up to `out.len()` bytes at the very start of the file (a short file is not an
    /// error here; the header decoder classifies it).
    fn read_up_to(&mut self, out: &mut [u8]) -> Result<usize> {
        let n = out
            .len()
            .min(usize::try_from(self.remaining_in_file()).unwrap_or(usize::MAX));
        let (target, _) = out.split_at_mut(n);
        self.read_exact_file(target)?;
        Ok(n)
    }

    /// Declares how many payload bytes the current section has.
    fn begin_payload(&mut self, payload_len: u64) {
        self.payload_left = payload_len;
    }

    /// Reads `n` payload bytes into a vector, growing it only with bytes that really exist.
    fn read_payload_vec(&mut self, n: u64) -> Result<Vec<u8>> {
        if n > self.payload_left {
            return Err(error::damaged("a field runs past the end of its section"));
        }
        let mut out = Vec::new();
        let mut left = n;
        let mut chunk =
            vec![0_u8; IO_BUFFER_BYTES.min(usize::try_from(n).unwrap_or(IO_BUFFER_BYTES))];
        while left > 0 {
            let take = usize::try_from(left).unwrap_or(usize::MAX).min(chunk.len());
            let (part, _) = chunk.split_at_mut(take);
            self.read_exact_file(part)?;
            out.extend_from_slice(part);
            left = left.saturating_sub(u64::try_from(take).unwrap_or(0));
        }
        self.payload_left = self.payload_left.saturating_sub(n);
        Ok(out)
    }

    /// Skips the rest of the current payload (still feeding the checksum).
    fn skip_payload(&mut self) -> Result<()> {
        let mut chunk = vec![0_u8; IO_BUFFER_BYTES];
        while self.payload_left > 0 {
            let take = usize::try_from(self.payload_left)
                .unwrap_or(usize::MAX)
                .min(chunk.len());
            let (part, _) = chunk.split_at_mut(take);
            self.read_exact_file(part)?;
            self.payload_left = self
                .payload_left
                .saturating_sub(u64::try_from(take).unwrap_or(0));
        }
        Ok(())
    }

    fn expect_payload_consumed(&self) -> Result<()> {
        if self.payload_left == 0 {
            Ok(())
        } else {
            Err(error::damaged("a section has bytes after its last field"))
        }
    }

    /// Reads the stored checksum after the payload and compares it with the computed one.
    fn verify_section_crc(&mut self) -> Result<()> {
        if self.payload_left != 0 {
            return Err(error::damaged("a section has bytes after its last field"));
        }
        self.fold_crc();
        let computed = self.crc.finish();
        let mut stored = [0_u8; SECTION_CRC_LEN];
        self.read_exact_file(&mut stored)?;
        // The stored checksum itself is not part of the next section's checksum.
        self.start_crc();
        if u32::from_le_bytes(stored) != computed {
            return Err(error::damaged("a section checksum does not match"));
        }
        Ok(())
    }

    /// Fails unless the whole file was consumed.
    fn expect_end(&mut self) -> Result<()> {
        if self.pos < self.len {
            return Err(error::damaged("there are bytes after the footer"));
        }
        let mut probe = [0_u8; 1];
        loop {
            match self.inner.read(&mut probe) {
                Ok(0) => return Ok(()),
                Ok(_) => return Err(error::damaged("there are bytes after the footer")),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(read_failure(&e)),
            }
        }
    }
}

impl<R: Read> PayloadSource for FileReader<'_, R> {
    fn remaining(&self) -> u64 {
        self.payload_left
    }

    fn u8(&mut self) -> Result<u8> {
        if self.payload_left == 0 {
            return Err(error::damaged("a field runs past the end of its section"));
        }
        let byte = self.byte()?;
        self.payload_left = self.payload_left.saturating_sub(1);
        Ok(byte)
    }

    fn u64(&mut self) -> Result<u64> {
        let mut bytes = [0_u8; 8];
        for slot in &mut bytes {
            *slot = PayloadSource::u8(self)?;
        }
        Ok(u64::from_le_bytes(bytes))
    }

    fn varint(&mut self) -> Result<u64> {
        let mut decoder = VarintDecoder::new();
        loop {
            let byte = PayloadSource::u8(self)?;
            if let Some(value) = decoder.feed(byte)? {
                return Ok(value);
            }
        }
    }
}
