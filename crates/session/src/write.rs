//! Atomic saving of a session file.
//!
//! The complete file is written to a unique temporary file in the target's directory, flushed and
//! synced, and only then renamed over the target, so the target is at every moment either the
//! complete old content or the complete new content. A failed or cancelled save removes its
//! temporary file and leaves the target untouched.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use la_core::Result;

use crate::capture_codec::{self, ByteSink};
use crate::control::Control;
use crate::crc::Crc32c;
use crate::error;
use crate::format::{Header, SectionHead, TAG_CAPT, TAG_META, encode_footer};
use crate::fs::{FileSystem, PathKind, RealFileSystem, WriteSink};
use crate::limits::{
    FOOTER_LEN, FORMAT_MAJOR, FORMAT_MINOR, HEADER_LEN, IO_BUFFER_BYTES, SECTION_CRC_LEN,
    SECTION_HEAD_LEN,
};
use crate::meta;
use crate::session::SessionData;

/// Temporary files of crashed saves older than this are removed by a later save.
const STALE_AFTER: Duration = Duration::from_secs(60 * 60);
/// The suffix of every temporary file this crate creates.
const TEMP_SUFFIX: &str = ".lasave-tmp";

/// What a successful save did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveReport {
    /// Size of the file in bytes.
    pub bytes_written: u64,
    /// Number of sections in the file.
    pub sections: u32,
    /// `false` if the directory entry could not be forced to disk after the rename (the file is
    /// complete and in place; only its durability across a power failure is not guaranteed).
    pub directory_synced: bool,
}

/// Saves `data` to `path` atomically.
///
/// # Errors
///
/// `session.invalid_input` for an unusable path, section or time; `session.io_write` for a
/// failure of the file system; `session.cancelled` if `control` was cancelled. In every case an
/// existing file at `path` is unchanged and no temporary file is left behind.
pub fn save_blocking(path: &Path, data: &SessionData, control: &Control) -> Result<SaveReport> {
    let result = save_with(&RealFileSystem, path, data, control);
    control.finish(&result);
    result
}

/// Like [`save_blocking`] on an arbitrary file system (used by tests to inject failures).
///
/// # Errors
///
/// As [`save_blocking`].
pub fn save_with(
    fs: &dyn FileSystem,
    path: &Path,
    data: &SessionData,
    control: &Control,
) -> Result<SaveReport> {
    let started = std::time::Instant::now();
    let target = Target::check(fs, path)?;

    // Everything that can fail without touching the disk happens first.
    let meta_payload = meta::encode(data.description())?;
    let snapshot = data.snapshot();
    let layout = capture_codec::plan(&snapshot)?;
    let extras = data.extras();
    let mut sections = vec![
        (
            TAG_META,
            true,
            u64::try_from(meta_payload.len()).unwrap_or(u64::MAX),
        ),
        (TAG_CAPT, true, layout.payload_len),
    ];
    for extra in extras {
        sections.push((
            extra.tag,
            extra.critical,
            u64::try_from(extra.payload.len()).unwrap_or(u64::MAX),
        ));
    }
    let overhead = u64::try_from(SECTION_HEAD_LEN + SECTION_CRC_LEN).unwrap_or(20);
    let mut total = u64::try_from(HEADER_LEN + FOOTER_LEN).unwrap_or(48);
    for (_, _, len) in &sections {
        total = len
            .checked_add(overhead)
            .and_then(|n| total.checked_add(n))
            .ok_or_else(|| error::too_large("the session file", u64::MAX, u64::MAX))?;
    }
    let section_count =
        u32::try_from(sections.len()).map_err(|_| error::invalid_input("too many sections"))?;
    control.set_total(total);
    control.check()?;

    let temp = target.create_temp(fs)?;
    let mut guard = TempGuard {
        fs,
        path: temp.path,
        armed: true,
    };
    let mut writer = Writer {
        sink: temp.sink,
        buffer: Vec::with_capacity(IO_BUFFER_BYTES + 1024),
        crc: Crc32c::new(),
        written: 0,
        control,
    };

    writer.put_raw(
        &Header {
            major: FORMAT_MAJOR,
            minor: FORMAT_MINOR,
            file_length: total,
            section_count,
        }
        .encode(),
    )?;
    writer.begin_section(&SectionHead {
        tag: TAG_META,
        critical: true,
        payload_len: u64::try_from(meta_payload.len()).unwrap_or(u64::MAX),
    })?;
    writer.put(&meta_payload)?;
    writer.end_section()?;
    writer.begin_section(&SectionHead {
        tag: TAG_CAPT,
        critical: true,
        payload_len: layout.payload_len,
    })?;
    capture_codec::encode(&snapshot, &layout, &mut writer)?;
    writer.end_section()?;
    for extra in extras {
        writer.begin_section(&SectionHead {
            tag: extra.tag,
            critical: extra.critical,
            payload_len: u64::try_from(extra.payload.len()).unwrap_or(u64::MAX),
        })?;
        writer.put(&extra.payload)?;
        writer.end_section()?;
    }
    writer.put_raw(&encode_footer(total))?;
    if writer.written != total {
        return Err(error::internal(
            "the written size differs from the planned size",
        ));
    }
    writer.finish()?;

    control.check()?;
    fs.rename(&guard.path, path)
        .map_err(|e| write_failure("replacing the target file", &e))?;
    guard.armed = false;
    let directory_synced = match fs.sync_dir(&target.directory) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(
                target: "la_session",
                error = %e,
                "the directory could not be synchronised after the rename"
            );
            false
        }
    };
    remove_stale_temporaries(fs, &target);
    tracing::info!(
        target: "la_session",
        bytes = total,
        sections = section_count,
        elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "session saved"
    );
    Ok(SaveReport {
        bytes_written: total,
        sections: section_count,
        directory_synced,
    })
}

/// Turns an I/O error into an actionable `session.io_write` error.
fn write_failure(step: &str, e: &io::Error) -> la_core::Error {
    let advice = match e.kind() {
        io::ErrorKind::StorageFull | io::ErrorKind::WriteZero => {
            "the disk is full: free some space or choose another location"
        }
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            "there is no permission to write there: choose another location or check the folder's permissions"
        }
        io::ErrorKind::NotFound => "the folder does not exist or was disconnected",
        _ => "check that the location is available and not in use",
    };
    error::io_write(
        format!("saving failed while {step}: {advice}"),
        io::Error::new(e.kind(), e.to_string()),
    )
}

/// The checked destination of a save.
struct Target {
    directory: PathBuf,
    name: OsString,
}

/// A temporary file that was just created.
struct Temp {
    path: PathBuf,
    sink: Box<dyn WriteSink>,
}

impl Target {
    /// Checks the path before anything is written.
    fn check(fs: &dyn FileSystem, path: &Path) -> Result<Self> {
        let Some(name) = path.file_name() else {
            return Err(error::invalid_input("the path has no file name"));
        };
        let directory = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        match fs.kind(&directory) {
            Ok(Some(PathKind::Directory)) => {}
            Ok(_) => {
                return Err(error::invalid_input(
                    "the folder to save in does not exist; choose another location",
                ));
            }
            Err(e) => return Err(write_failure("looking at the folder", &e)),
        }
        match fs.kind(path) {
            Ok(Some(PathKind::Directory)) => Err(error::invalid_input(
                "the path is a folder, not a file; choose a file name",
            )),
            Ok(_) => Ok(Self {
                directory,
                name: name.to_owned(),
            }),
            Err(e) => Err(write_failure("looking at the target", &e)),
        }
    }

    /// `.<name>.<16 hex digits>.lasave-tmp`.
    fn temp_name(&self, unique: u64) -> OsString {
        let mut name = OsString::from(".");
        name.push(&self.name);
        name.push(format!(".{unique:016x}{TEMP_SUFFIX}"));
        name
    }

    /// Creates a temporary file with a name nobody else uses.
    fn create_temp(&self, fs: &dyn FileSystem) -> Result<Temp> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| {
            u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0)
        });
        for _ in 0..16 {
            let unique = nanos
                .rotate_left(17)
                .wrapping_add(u64::from(std::process::id()).rotate_left(40))
                .wrapping_add(
                    COUNTER
                        .fetch_add(1, Ordering::Relaxed)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15),
                );
            let path = self.directory.join(self.temp_name(unique));
            match fs.create_exclusive(&path) {
                Ok(sink) => return Ok(Temp { path, sink }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(write_failure("creating the temporary file", &e)),
            }
        }
        Err(error::io_write(
            "saving failed while creating the temporary file: no unused file name was found",
            io::Error::from(io::ErrorKind::AlreadyExists),
        ))
    }

    /// Whether `name` is a temporary file of an earlier save of this target.
    fn owns_temp(&self, name: &std::ffi::OsStr) -> bool {
        let Some(name) = name.to_str() else {
            return false;
        };
        let Some(own) = self.name.to_str() else {
            return false;
        };
        let Some(rest) = name
            .strip_prefix('.')
            .and_then(|n| n.strip_prefix(own))
            .and_then(|n| n.strip_prefix('.'))
            .and_then(|n| n.strip_suffix(TEMP_SUFFIX))
        else {
            return false;
        };
        rest.len() == 16
            && rest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }
}

/// Removes the temporary file when the save does not complete.
struct TempGuard<'a> {
    fs: &'a dyn FileSystem,
    path: PathBuf,
    armed: bool,
}

impl Drop for TempGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.fs.remove_file(&self.path);
        }
    }
}

/// Best effort: removes stale temporary files of earlier crashed saves of the same target.
fn remove_stale_temporaries(fs: &dyn FileSystem, target: &Target) {
    let Ok(entries) = fs.list_dir(&target.directory) else {
        return;
    };
    for entry in entries {
        let stale = entry.is_file
            && target.owns_temp(&entry.name)
            && entry.age.is_some_and(|age| age >= STALE_AFTER);
        if stale {
            let _ = fs.remove_file(&target.directory.join(&entry.name));
        }
    }
}

/// Buffers, checksums and counts everything written to the temporary file.
struct Writer<'a> {
    sink: Box<dyn WriteSink>,
    buffer: Vec<u8>,
    crc: Crc32c,
    written: u64,
    control: &'a Control,
}

impl Writer<'_> {
    /// Writes bytes that are not part of any section checksum (header, footer, checksums).
    fn put_raw(&mut self, data: &[u8]) -> Result<()> {
        self.buffer.extend_from_slice(data);
        self.written = self
            .written
            .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        if self.buffer.len() >= IO_BUFFER_BYTES {
            self.drain()?;
        }
        Ok(())
    }

    /// Hands the buffered bytes to the file; also the point where cancellation is noticed.
    fn drain(&mut self) -> Result<()> {
        self.control.check()?;
        self.sink
            .write_all(&self.buffer)
            .map_err(|e| write_failure("writing the temporary file", &e))?;
        self.control.set_done(self.written);
        self.buffer.clear();
        Ok(())
    }

    fn begin_section(&mut self, head: &SectionHead) -> Result<()> {
        self.crc = Crc32c::new();
        self.put(&head.encode())
    }

    fn end_section(&mut self) -> Result<()> {
        let crc = self.crc.finish();
        self.put_raw(&crc.to_le_bytes())
    }

    /// Flushes everything and forces it to stable storage.
    fn finish(&mut self) -> Result<()> {
        self.drain()?;
        self.sink
            .flush()
            .map_err(|e| write_failure("flushing the temporary file", &e))?;
        self.sink
            .sync_all()
            .map_err(|e| write_failure("forcing the temporary file to disk", &e))
    }
}

impl ByteSink for Writer<'_> {
    fn put(&mut self, data: &[u8]) -> Result<()> {
        self.crc.update(data);
        self.put_raw(data)
    }
}
