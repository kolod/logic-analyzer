//! The container framing of a session file: header, section frames and footer.
//!
//! Pure functions on byte slices, no I/O. See `contracts/file-format.md`.

use la_core::Result;

use crate::crc::crc32c;
use crate::error;
use crate::limits::{FOOTER_LEN, FORMAT_MAJOR, HEADER_LEN, MAX_SECTIONS, SECTION_HEAD_LEN};

/// The first eight bytes of every session file (PNG-style: a non-ASCII first byte and line-ending
/// bytes that expose text-mode transfers).
pub const MAGIC: [u8; 8] = [0x89, b'L', b'A', b'S', 0x0D, 0x0A, 0x1A, 0x0A];
/// The first four bytes of the footer.
pub const FOOTER_MAGIC: [u8; 4] = *b"LAEN";
/// The critical bit of a section's flags.
pub const FLAG_CRITICAL: u32 = 1;

/// A section tag: exactly four ASCII letters or digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionTag([u8; 4]);

/// The tag of the capture description section.
pub const TAG_META: SectionTag = SectionTag(*b"META");
/// The tag of the capture section.
pub const TAG_CAPT: SectionTag = SectionTag(*b"CAPT");

impl SectionTag {
    /// Validates and wraps a tag.
    ///
    /// # Errors
    ///
    /// `session.invalid_input` unless the text is exactly four ASCII letters or digits.
    pub fn new(text: &str) -> Result<Self> {
        let bytes: [u8; 4] = text
            .as_bytes()
            .try_into()
            .map_err(|_| error::invalid_input("a section tag must be exactly four characters"))?;
        Self::from_bytes(bytes).map_err(|_| {
            error::invalid_input("a section tag may contain only ASCII letters and digits")
        })
    }

    /// Validates the four bytes of a tag read from a file.
    pub(crate) fn from_bytes(bytes: [u8; 4]) -> Result<Self> {
        if bytes.iter().all(u8::is_ascii_alphanumeric) {
            Ok(Self(bytes))
        } else {
            Err(error::damaged(
                "a section tag contains a character that is not a letter or digit",
            ))
        }
    }

    /// The tag as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }

    /// The four bytes.
    #[must_use]
    pub fn bytes(&self) -> [u8; 4] {
        self.0
    }

    /// `true` for the tags this crate owns (`META`, `CAPT`).
    #[must_use]
    pub fn is_reserved(&self) -> bool {
        *self == TAG_META || *self == TAG_CAPT
    }
}

/// The file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Format major version.
    pub major: u16,
    /// Format minor version.
    pub minor: u16,
    /// Total size of the file in bytes.
    pub file_length: u64,
    /// Number of sections.
    pub section_count: u32,
}

impl Header {
    /// The 32 header bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0_u8; HEADER_LEN];
        let mut at = 0_usize;
        let mut put = |bytes: &[u8]| {
            if let Some(target) = out.get_mut(at..at.saturating_add(bytes.len())) {
                target.copy_from_slice(bytes);
            }
            at = at.saturating_add(bytes.len());
        };
        put(&MAGIC);
        put(&self.major.to_le_bytes());
        put(&self.minor.to_le_bytes());
        put(&0_u32.to_le_bytes());
        put(&self.file_length.to_le_bytes());
        put(&self.section_count.to_le_bytes());
        let crc = out.get(..28).map_or(0, crc32c);
        if let Some(target) = out.get_mut(28..32) {
            target.copy_from_slice(&crc.to_le_bytes());
        }
        out
    }

    /// Decodes and validates the header from the first bytes of a file (`bytes` may be shorter
    /// than a header when the file is).
    ///
    /// # Errors
    ///
    /// `session.not_a_session`, `session.truncated`, `session.unsupported_version`,
    /// `session.damaged` or `session.too_large` as described in the format contract.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(error::not_a_session("the file is empty"));
        }
        let magic_len = bytes.len().min(MAGIC.len());
        if bytes.get(..magic_len) != MAGIC.get(..magic_len) {
            return Err(error::not_a_session(
                "the file does not start with the session file signature",
            ));
        }
        let Some(head) = bytes.get(..HEADER_LEN) else {
            return Err(error::truncated("the file ends inside the header"));
        };
        let major = u16::from_le_bytes([byte(head, 8), byte(head, 9)]);
        if major != FORMAT_MAJOR {
            return Err(error::unsupported_version(major, FORMAT_MAJOR));
        }
        let stored_crc = u32::from_le_bytes([
            byte(head, 28),
            byte(head, 29),
            byte(head, 30),
            byte(head, 31),
        ]);
        if head.get(..28).map_or(0, crc32c) != stored_crc {
            return Err(error::damaged("the header checksum does not match"));
        }
        let flags = u32::from_le_bytes([
            byte(head, 12),
            byte(head, 13),
            byte(head, 14),
            byte(head, 15),
        ]);
        if flags != 0 {
            return Err(error::damaged("a reserved header flag is set"));
        }
        let section_count = u32::from_le_bytes([
            byte(head, 24),
            byte(head, 25),
            byte(head, 26),
            byte(head, 27),
        ]);
        if section_count > MAX_SECTIONS {
            return Err(error::too_large(
                "the number of sections",
                u64::from(section_count),
                u64::from(MAX_SECTIONS),
            ));
        }
        if section_count < 2 {
            return Err(error::damaged("the file declares fewer than two sections"));
        }
        let mut length = [0_u8; 8];
        length.copy_from_slice(head.get(16..24).unwrap_or(&[0; 8]));
        Ok(Self {
            major,
            minor: u16::from_le_bytes([byte(head, 10), byte(head, 11)]),
            file_length: u64::from_le_bytes(length),
            section_count,
        })
    }
}

/// The byte at `index` of a slice known to be long enough (zero otherwise; callers check the
/// length first).
fn byte(slice: &[u8], index: usize) -> u8 {
    slice.get(index).copied().unwrap_or(0)
}

/// The frame head of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHead {
    /// The tag.
    pub tag: SectionTag,
    /// `true` if a reader that does not understand the tag must refuse the file.
    pub critical: bool,
    /// The payload size in bytes.
    pub payload_len: u64,
}

impl SectionHead {
    /// The 16 head bytes.
    #[must_use]
    pub fn encode(&self) -> [u8; SECTION_HEAD_LEN] {
        let mut out = [0_u8; SECTION_HEAD_LEN];
        let flags = if self.critical { FLAG_CRITICAL } else { 0 };
        let mut at = 0_usize;
        let mut put = |bytes: &[u8]| {
            if let Some(target) = out.get_mut(at..at.saturating_add(bytes.len())) {
                target.copy_from_slice(bytes);
            }
            at = at.saturating_add(bytes.len());
        };
        put(&self.tag.bytes());
        put(&flags.to_le_bytes());
        put(&self.payload_len.to_le_bytes());
        out
    }

    /// Decodes and validates a head.
    ///
    /// # Errors
    ///
    /// `session.damaged` for an invalid tag or a reserved flag bit.
    pub fn decode(bytes: &[u8; SECTION_HEAD_LEN]) -> Result<Self> {
        let tag = SectionTag::from_bytes([
            byte(bytes, 0),
            byte(bytes, 1),
            byte(bytes, 2),
            byte(bytes, 3),
        ])?;
        let flags = u32::from_le_bytes([
            byte(bytes, 4),
            byte(bytes, 5),
            byte(bytes, 6),
            byte(bytes, 7),
        ]);
        if flags & !FLAG_CRITICAL != 0 {
            return Err(error::damaged("a reserved section flag is set"));
        }
        let mut length = [0_u8; 8];
        length.copy_from_slice(bytes.get(8..16).unwrap_or(&[0; 8]));
        Ok(Self {
            tag,
            critical: flags & FLAG_CRITICAL != 0,
            payload_len: u64::from_le_bytes(length),
        })
    }
}

/// Encodes the footer for a file of `file_length` bytes.
#[must_use]
pub fn encode_footer(file_length: u64) -> [u8; FOOTER_LEN] {
    let mut out = [0_u8; FOOTER_LEN];
    if let Some(target) = out.get_mut(..4) {
        target.copy_from_slice(&FOOTER_MAGIC);
    }
    if let Some(target) = out.get_mut(4..12) {
        target.copy_from_slice(&file_length.to_le_bytes());
    }
    let crc = out.get(..12).map_or(0, crc32c);
    if let Some(target) = out.get_mut(12..16) {
        target.copy_from_slice(&crc.to_le_bytes());
    }
    out
}

/// Decodes the footer and returns the file length it declares.
///
/// # Errors
///
/// `session.truncated` for too few bytes, `session.damaged` for a wrong signature or checksum.
pub fn decode_footer(bytes: &[u8]) -> Result<u64> {
    let Some(footer) = bytes.get(..FOOTER_LEN) else {
        return Err(error::truncated("the file ends inside the footer"));
    };
    if footer.get(..4) != Some(&FOOTER_MAGIC[..]) {
        return Err(error::damaged("the footer signature is wrong"));
    }
    let stored = u32::from_le_bytes([
        byte(footer, 12),
        byte(footer, 13),
        byte(footer, 14),
        byte(footer, 15),
    ]);
    if footer.get(..12).map_or(0, crc32c) != stored {
        return Err(error::damaged("the footer checksum does not match"));
    }
    let mut length = [0_u8; 8];
    length.copy_from_slice(footer.get(4..12).unwrap_or(&[0; 8]));
    Ok(u64::from_le_bytes(length))
}

/// Checks the canonical section order while a file is read: `META`, `CAPT`, then optional
/// sections with strictly ascending tags (which also rules out duplicates).
#[derive(Debug, Default)]
pub struct OrderChecker {
    index: u32,
    last_optional: Option<SectionTag>,
}

impl OrderChecker {
    /// Starts at the first section.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts the next section's tag or explains why it is out of order.
    ///
    /// # Errors
    ///
    /// `session.damaged` if the tag is not the one expected at this position.
    pub fn accept(&mut self, tag: SectionTag) -> Result<()> {
        match self.index {
            0 if tag != TAG_META => {
                return Err(error::damaged(
                    "the first section must be the capture description",
                ));
            }
            1 if tag != TAG_CAPT => {
                return Err(error::damaged("the second section must be the capture"));
            }
            0 | 1 => {}
            _ => {
                if tag.is_reserved() {
                    return Err(error::damaged("a reserved section appears twice"));
                }
                if self.last_optional.is_some_and(|last| tag <= last) {
                    return Err(error::damaged(
                        "optional sections must have strictly ascending tags",
                    ));
                }
                self.last_optional = Some(tag);
            }
        }
        self.index = self.index.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn header(file_length: u64, count: u32) -> Header {
        Header {
            major: 1,
            minor: 0,
            file_length,
            section_count: count,
        }
    }

    /// Re-seals a header after a test changed some of its bytes.
    fn reseal(mut bytes: [u8; HEADER_LEN]) -> [u8; HEADER_LEN] {
        let crc = crc32c(&bytes[..28]);
        bytes[28..].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    #[test]
    fn the_header_layout_is_exactly_the_documented_one() {
        let bytes = header(0x0102_0304_0506_0708, 3).encode();
        // Written by hand from the contract, independent of the encoder.
        let mut expected = vec![0x89, 0x4C, 0x41, 0x53, 0x0D, 0x0A, 0x1A, 0x0A];
        expected.extend_from_slice(&[1, 0]); // major
        expected.extend_from_slice(&[0, 0]); // minor
        expected.extend_from_slice(&[0, 0, 0, 0]); // reserved flags
        expected.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]); // file length
        expected.extend_from_slice(&[3, 0, 0, 0]); // section count
        assert_eq!(&bytes[..28], &expected[..]);
        assert_eq!(&bytes[28..], &crc32c(&expected).to_le_bytes());
        assert_eq!(
            Header::decode(&bytes).unwrap(),
            header(0x0102_0304_0506_0708, 3)
        );
    }

    #[test]
    fn a_header_is_rejected_with_the_documented_codes() {
        let good = header(100, 2).encode();
        let code = |bytes: &[u8]| Header::decode(bytes).unwrap_err().code();
        // Not a session file: empty, wrong magic, a longer file of another format.
        assert_eq!(code(&[]), error::NOT_A_SESSION);
        assert_eq!(
            code(b"PK\x03\x04 and more bytes follow here, a zip file"),
            error::NOT_A_SESSION
        );
        assert_eq!(code(&[0x89, b'P', b'N', b'G']), error::NOT_A_SESSION);
        // The signature is right but the file ends early.
        assert_eq!(code(&good[..3]), error::TRUNCATED);
        assert_eq!(code(&good[..8]), error::TRUNCATED);
        assert_eq!(code(&good[..31]), error::TRUNCATED);
        // A newer major version names both versions; a newer minor is accepted.
        let mut v2 = good;
        v2[8] = 2;
        let e = Header::decode(&v2).unwrap_err();
        assert_eq!(e.code(), error::UNSUPPORTED_VERSION);
        assert!(e.detail().contains("2.x") && e.detail().contains("1.x"));
        let mut minor = good;
        minor[10] = 7;
        assert_eq!(Header::decode(&reseal(minor)).unwrap().minor, 7);
        // Checksum, reserved flags, section counts.
        let mut flipped = good;
        flipped[17] ^= 1;
        assert_eq!(code(&flipped), error::DAMAGED);
        let mut flags = good;
        flags[12] = 1;
        assert_eq!(code(&reseal(flags)), error::DAMAGED);
        for (count, want) in [
            (0_u32, error::DAMAGED),
            (1, error::DAMAGED),
            (65, error::TOO_LARGE),
        ] {
            let mut bytes = good;
            bytes[24..28].copy_from_slice(&count.to_le_bytes());
            assert_eq!(code(&reseal(bytes)), want, "count {count}");
        }
        assert_eq!(
            Header::decode(&reseal({
                let mut b = good;
                b[24..28].copy_from_slice(&64_u32.to_le_bytes());
                b
            }))
            .unwrap()
            .section_count,
            64
        );
    }

    #[test]
    fn a_section_head_round_trips_with_the_documented_layout() {
        let head = SectionHead {
            tag: SectionTag::new("TeSt").unwrap(),
            critical: true,
            payload_len: 0x0A0B,
        };
        let bytes = head.encode();
        assert_eq!(
            bytes,
            [
                b'T', b'e', b'S', b't', 1, 0, 0, 0, 0x0B, 0x0A, 0, 0, 0, 0, 0, 0
            ]
        );
        assert_eq!(SectionHead::decode(&bytes).unwrap(), head);
        let optional = SectionHead {
            critical: false,
            ..head
        };
        assert_eq!(optional.encode()[4], 0);
    }

    #[test]
    fn invalid_tags_and_reserved_flags_are_rejected() {
        let mut bytes = SectionHead {
            tag: TAG_CAPT,
            critical: false,
            payload_len: 1,
        }
        .encode();
        for bad in [b' ', b'-', 0, 0xC3, b'\n'] {
            let mut b = bytes;
            b[2] = bad;
            assert_eq!(SectionHead::decode(&b).unwrap_err().code(), error::DAMAGED);
        }
        bytes[5] = 1; // a reserved bit
        assert_eq!(
            SectionHead::decode(&bytes).unwrap_err().code(),
            error::DAMAGED
        );
        bytes[5] = 0;
        bytes[4] = 2;
        assert_eq!(
            SectionHead::decode(&bytes).unwrap_err().code(),
            error::DAMAGED
        );
        // Tag construction from text.
        for bad in ["", "ABC", "ABCDE", "AB D", "ÄBCD", "AB-D"] {
            assert_eq!(
                SectionTag::new(bad).unwrap_err().code(),
                error::INVALID_INPUT,
                "{bad:?}"
            );
        }
        assert_eq!(SectionTag::new("A1b2").unwrap().as_str(), "A1b2");
        assert!(TAG_META.is_reserved() && TAG_CAPT.is_reserved());
        assert!(!SectionTag::new("DECO").unwrap().is_reserved());
    }

    #[test]
    fn the_footer_round_trips_and_is_checked() {
        let bytes = encode_footer(0x1122);
        assert_eq!(&bytes[..4], b"LAEN");
        assert_eq!(&bytes[4..12], &0x1122_u64.to_le_bytes());
        assert_eq!(decode_footer(&bytes).unwrap(), 0x1122);
        assert_eq!(
            decode_footer(&bytes[..15]).unwrap_err().code(),
            error::TRUNCATED
        );
        let mut magic = bytes;
        magic[0] = b'X';
        assert_eq!(decode_footer(&magic).unwrap_err().code(), error::DAMAGED);
        for i in 4..16 {
            let mut flipped = bytes;
            flipped[i] ^= 0x10;
            assert_eq!(
                decode_footer(&flipped).unwrap_err().code(),
                error::DAMAGED,
                "byte {i}"
            );
        }
    }

    #[test]
    fn the_section_order_is_meta_capt_then_ascending_optional_tags() {
        let tag = |s: &str| SectionTag::new(s).unwrap();
        let check = |tags: &[&str]| {
            let mut order = OrderChecker::new();
            tags.iter().try_for_each(|t| order.accept(tag(t)))
        };
        assert!(check(&["META", "CAPT"]).is_ok());
        assert!(check(&["META", "CAPT", "AAAA", "DECO", "ZZZZ"]).is_ok());
        // Optional tags may sort before the reserved ones.
        assert!(check(&["META", "CAPT", "ABCD"]).is_ok());
        for bad in [
            vec!["CAPT", "META"],
            vec!["DECO", "META", "CAPT"],
            vec!["META", "DECO"],
            vec!["META", "CAPT", "DECO", "DECO"],
            vec!["META", "CAPT", "ZZZZ", "AAAA"],
            vec!["META", "CAPT", "META"],
            vec!["META", "CAPT", "CAPT"],
            vec!["META", "META"],
        ] {
            assert_eq!(check(&bad).unwrap_err().code(), error::DAMAGED, "{bad:?}");
        }
    }
}
