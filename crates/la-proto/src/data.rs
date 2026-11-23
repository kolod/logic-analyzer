//! The head of the data a board returns after a capture: the encoding, the number of samples stored and the
//! loss marks (`contracts/protocol.md`, "The data after a capture").

use crate::request::Encoding;

/// The length of the head before the loss marks.
pub const HEADER_LEN: usize = 7;
/// The length of one loss mark.
pub const MARK_LEN: usize = 8;
/// The most loss marks of a capture.
pub const MAX_LOSS_MARKS: usize = 1024;

/// A run of samples the board missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossMark {
    /// The index in the stored samples before which the run belongs; the number stored means after the last.
    pub position: u32,
    /// How many samples the run covers, at least 1.
    pub lost: u32,
}

impl LossMark {
    /// The 8 bytes of a mark.
    #[must_use]
    pub fn to_bytes(self) -> [u8; MARK_LEN] {
        let [p0, p1, p2, p3] = self.position.to_le_bytes();
        let [l0, l1, l2, l3] = self.lost.to_le_bytes();
        [p0, p1, p2, p3, l0, l1, l2, l3]
    }

    /// Reads a mark from 8 bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; MARK_LEN]) -> Self {
        let [p0, p1, p2, p3, l0, l1, l2, l3] = bytes;
        Self {
            position: u32::from_le_bytes([p0, p1, p2, p3]),
            lost: u32::from_le_bytes([l0, l1, l2, l3]),
        }
    }
}

/// What is wrong with a head or with the marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataError {
    /// The encoding byte is not 0 or 1.
    BadEncoding,
    /// More than [`MAX_LOSS_MARKS`] marks are announced.
    TooManyMarks,
    /// A mark covers no samples.
    EmptyMark,
    /// The marks are not in strictly ascending order of position (two runs at one place are one mark).
    MarksNotAscending,
    /// A mark is beyond the samples stored.
    MarkBeyondSamples,
    /// The samples stored plus the samples lost are more than the request asked for.
    MoreThanRequested,
}

impl core::fmt::Display for DataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::BadEncoding => "the encoding byte is not 0 or 1",
            Self::TooManyMarks => "more loss marks than a capture can have",
            Self::EmptyMark => "a loss mark covers no samples",
            Self::MarksNotAscending => "the loss marks are not in ascending order",
            Self::MarkBeyondSamples => "a loss mark is beyond the samples stored",
            Self::MoreThanRequested => "the samples stored and lost are more than were requested",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DataError {}

/// The head of the data of a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataHeader {
    /// The encoding of the samples that follow.
    pub encoding: Encoding,
    /// The number of samples stored.
    pub samples: u32,
    /// The number of loss marks that follow the head.
    pub marks: u16,
}

impl DataHeader {
    /// The 7 bytes of the head.
    #[must_use]
    pub fn to_bytes(self) -> [u8; HEADER_LEN] {
        let [s0, s1, s2, s3] = self.samples.to_le_bytes();
        let [m0, m1] = self.marks.to_le_bytes();
        [
            match self.encoding {
                Encoding::Raw => 0,
                Encoding::Compact => 1,
            },
            s0,
            s1,
            s2,
            s3,
            m0,
            m1,
        ]
    }

    /// Reads the head.
    ///
    /// # Errors
    ///
    /// [`DataError::BadEncoding`] or [`DataError::TooManyMarks`].
    pub fn from_bytes(bytes: [u8; HEADER_LEN]) -> Result<Self, DataError> {
        let [encoding, s0, s1, s2, s3, m0, m1] = bytes;
        let marks = u16::from_le_bytes([m0, m1]);
        if usize::from(marks) > MAX_LOSS_MARKS {
            return Err(DataError::TooManyMarks);
        }
        Ok(Self {
            encoding: match encoding {
                0 => Encoding::Raw,
                1 => Encoding::Compact,
                _ => return Err(DataError::BadEncoding),
            },
            samples: u32::from_le_bytes([s0, s1, s2, s3]),
            marks,
        })
    }
}

/// Checks the marks of a capture against the number of samples stored and the number the request asked for
/// (before plus after the trigger). Returns the samples lost in all.
///
/// # Errors
///
/// The first rule the marks break.
pub fn check_marks(marks: &[LossMark], stored: u32, requested: u64) -> Result<u64, DataError> {
    let mut previous: Option<u32> = None;
    let mut lost = 0_u64;
    for mark in marks {
        if mark.lost == 0 {
            return Err(DataError::EmptyMark);
        }
        if previous.is_some_and(|p| mark.position <= p) {
            return Err(DataError::MarksNotAscending);
        }
        if mark.position > stored {
            return Err(DataError::MarkBeyondSamples);
        }
        previous = Some(mark.position);
        lost = lost.saturating_add(u64::from(mark.lost));
    }
    if u64::from(stored).saturating_add(lost) > requested {
        return Err(DataError::MoreThanRequested);
    }
    Ok(lost)
}
