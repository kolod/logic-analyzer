//! A size-bounded log writer made of a fixed number of segment files.
//!
//! Entries are appended to the active segment. When the next entry would not fit, the
//! writer moves to the next segment (wrapping around), truncates it and continues, so
//! the oldest segment is always the one that is discarded. The sum of all segment
//! sizes therefore never exceeds the configured total. Each entry is written with a
//! single `write` call and is never split across segments, so an abrupt termination
//! can damage at most the last entry.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::{FormatTime, SystemTime};

/// Version of the log entry format (see `contracts/log-entry-format.md`).
pub const FORMAT_VERSION: u32 = 1;

/// Space reserved at the start of every segment for the `log started` header.
const HEADER_RESERVE: u64 = 256;

/// Current time as RFC 3339 UTC text, formatted exactly like `tracing` events.
pub(crate) fn timestamp_now() -> String {
    let mut text = String::new();
    let mut writer = Writer::new(&mut text);
    // Writing into a `String` cannot fail; an empty timestamp would be harmless anyway.
    let _ = SystemTime.format_time(&mut writer);
    text
}

/// A log writer with a hard bound on the total size of its files.
#[derive(Debug)]
pub struct BoundedLogWriter {
    dir: PathBuf,
    prefix: String,
    segments: usize,
    segment_limit: u64,
    active: usize,
    file: File,
    written: u64,
}

impl BoundedLogWriter {
    /// Opens (creating the directory if needed) a writer for `prefix` in `dir`.
    ///
    /// Segment files are named `<prefix>.<segment>.log`. The total size of all of
    /// them never exceeds `max_total_bytes`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or the first segment cannot be created.
    pub fn open(
        dir: &Path,
        prefix: &str,
        segments: usize,
        max_total_bytes: u64,
    ) -> io::Result<Self> {
        let segments = segments.max(2);
        let count = u64::try_from(segments).unwrap_or(2);
        let segment_limit = max_total_bytes.checked_div(count).unwrap_or(0);
        std::fs::create_dir_all(dir)?;
        let file = open_segment(dir, prefix, 0)?;
        let mut writer = Self {
            dir: dir.to_path_buf(),
            prefix: prefix.to_owned(),
            segments,
            segment_limit,
            active: 0,
            file,
            written: 0,
        };
        writer.write_header()?;
        Ok(writer)
    }

    /// Appends one complete entry (normally a single JSON line ending in `\n`).
    ///
    /// An entry larger than a segment can hold is replaced by a short notice so the
    /// size bound is never violated.
    ///
    /// # Errors
    ///
    /// Returns an error if the file system rejects the write.
    pub fn write_record(&mut self, record: &[u8]) -> io::Result<()> {
        let max_record = self.segment_limit.saturating_sub(HEADER_RESERVE);
        let len = u64::try_from(record.len()).unwrap_or(u64::MAX);
        let notice;
        let (record, len) = if len > max_record {
            notice = oversize_notice(len);
            let notice_len = u64::try_from(notice.len()).unwrap_or(u64::MAX);
            (notice.as_bytes(), notice_len)
        } else {
            (record, len)
        };
        if self.written.saturating_add(len) > self.segment_limit {
            self.advance()?;
        }
        self.file.write_all(record)?;
        self.written = self.written.saturating_add(len);
        Ok(())
    }

    fn advance(&mut self) -> io::Result<()> {
        let next = self
            .active
            .checked_add(1)
            .and_then(|n| n.checked_rem(self.segments))
            .unwrap_or(0);
        self.file = open_segment(&self.dir, &self.prefix, next)?;
        self.active = next;
        self.written = 0;
        self.write_header()
    }

    fn write_header(&mut self) -> io::Result<()> {
        let header = format!(
            "{{\"timestamp\":\"{}\",\"level\":\"INFO\",\"target\":\"la_diagnostics::logging\",\"fields\":{{\"message\":\"log started\",\"format_version\":{FORMAT_VERSION}}}}}\n",
            timestamp_now()
        );
        self.file.write_all(header.as_bytes())?;
        self.written = self
            .written
            .saturating_add(u64::try_from(header.len()).unwrap_or(HEADER_RESERVE));
        Ok(())
    }
}

/// Opens (creating or truncating) one segment file.
fn open_segment(dir: &Path, prefix: &str, index: usize) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dir.join(format!("{prefix}.{index}.log")))
}

fn oversize_notice(dropped_bytes: u64) -> String {
    format!(
        "{{\"timestamp\":\"{}\",\"level\":\"WARN\",\"target\":\"la_diagnostics::logging\",\"fields\":{{\"message\":\"log entry too large, dropped\",\"dropped_bytes\":{dropped_bytes}}}}}\n",
        timestamp_now()
    )
}
