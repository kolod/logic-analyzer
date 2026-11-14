//! The values that are saved and loaded.

use la_core::Result;
use la_devices::CaptureDescription;
use la_storage::{Capture, CaptureReader, Snapshot};

use crate::error;
use crate::format::SectionTag;

/// An optional section carried through a session. `META` and `CAPT` are reserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The section's tag.
    pub tag: SectionTag,
    /// `true` if a reader that does not understand the tag must refuse the file.
    pub critical: bool,
    /// The section's content, opaque to this crate.
    pub payload: Vec<u8>,
}

fn insert_sorted(extras: &mut Vec<Section>, section: Section) -> Result<()> {
    if section.tag.is_reserved() {
        return Err(error::invalid_input(format!(
            "the section tag `{}` is reserved",
            section.tag.as_str()
        )));
    }
    match extras.binary_search_by(|s| s.tag.cmp(&section.tag)) {
        Ok(_) => Err(error::invalid_input(format!(
            "a section with the tag `{}` was already added",
            section.tag.as_str()
        ))),
        Err(at) => {
            extras.insert(at, section);
            Ok(())
        }
    }
}

/// What to save: a capture description, a snapshot of the capture and optional sections.
///
/// Cheap to build: it holds a snapshot, not a copy of the data, and a snapshot of a capture that
/// is still being recorded is a consistent prefix of it (such a session is saved as interrupted).
#[derive(Debug, Clone)]
pub struct SessionData {
    description: CaptureDescription,
    snapshot: Snapshot,
    extras: Vec<Section>,
}

impl SessionData {
    /// A session of `snapshot` described by `description`.
    #[must_use]
    pub fn new(description: CaptureDescription, snapshot: Snapshot) -> Self {
        Self {
            description,
            snapshot,
            extras: Vec::new(),
        }
    }

    /// Adds an optional section.
    ///
    /// # Errors
    ///
    /// `session.invalid_input` for a reserved tag (`META`, `CAPT`) or a tag already added.
    pub fn with_section(mut self, section: Section) -> Result<Self> {
        insert_sorted(&mut self.extras, section)?;
        Ok(self)
    }

    /// The session of an already loaded one, to save it again.
    #[must_use]
    pub fn from_session(session: &Session) -> Self {
        Self {
            description: session.description.clone(),
            snapshot: session.snapshot(),
            extras: session.extras.clone(),
        }
    }

    /// The capture description.
    #[must_use]
    pub fn description(&self) -> &CaptureDescription {
        &self.description
    }

    /// The snapshot that will be saved.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }

    /// The optional sections, in ascending tag order.
    #[must_use]
    pub fn extras(&self) -> &[Section] {
        &self.extras
    }
}

/// A loaded session. Its capture is closed.
///
/// The capture description is read-only: there is no way to clear its simulated mark, and a loaded
/// capture that is incomplete stays incomplete.
#[derive(Debug)]
pub struct Session {
    description: CaptureDescription,
    capture: Capture,
    extras: Vec<Section>,
}

impl Session {
    pub(crate) fn new(
        description: CaptureDescription,
        capture: Capture,
        extras: Vec<Section>,
    ) -> Self {
        Self {
            description,
            capture,
            extras,
        }
    }

    /// Where the data came from.
    #[must_use]
    pub fn description(&self) -> &CaptureDescription {
        &self.description
    }

    /// A reader for taking snapshots of the capture.
    #[must_use]
    pub fn reader(&self) -> CaptureReader {
        self.capture.reader()
    }

    /// A snapshot of the capture (it never changes).
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.capture.reader().snapshot()
    }

    /// The optional sections found in the file, in ascending tag order.
    #[must_use]
    pub fn extras(&self) -> &[Section] {
        &self.extras
    }

    /// Memory the capture uses, in bytes.
    #[must_use]
    pub fn memory_used(&self) -> u64 {
        self.capture.memory_used()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{TAG_CAPT, TAG_META};

    fn section(tag: &str) -> Section {
        Section {
            tag: SectionTag::new(tag).unwrap(),
            critical: false,
            payload: vec![1, 2, 3],
        }
    }

    #[test]
    fn extras_stay_sorted_and_unique_and_reserved_tags_are_refused() {
        let mut extras = Vec::new();
        for tag in ["ZZZZ", "AAAA", "MMMM"] {
            insert_sorted(&mut extras, section(tag)).unwrap();
        }
        let tags: Vec<&str> = extras.iter().map(|s| s.tag.as_str()).collect();
        assert_eq!(tags, ["AAAA", "MMMM", "ZZZZ"]);
        let duplicate = insert_sorted(&mut extras, section("MMMM")).unwrap_err();
        assert_eq!(duplicate.code(), error::INVALID_INPUT);
        for reserved in [TAG_META, TAG_CAPT] {
            let mut s = section("AAAB");
            s.tag = reserved;
            assert_eq!(
                insert_sorted(&mut extras, s).unwrap_err().code(),
                error::INVALID_INPUT
            );
        }
        assert_eq!(extras.len(), 3);
    }
}
