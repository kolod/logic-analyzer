//! Canonical unsigned LEB128 varints and a bounds-checked cursor over a byte slice.
//!
//! Canonical means one value has exactly one encoding: at most 10 bytes, no redundant trailing
//! zero group, and the tenth byte at most 1. A decoder rejects everything else, so a bit flip
//! can never turn one valid encoding into another encoding of the same value.

use la_core::Result;

use crate::error;

/// The longest encoding of a `u64`.
pub const MAX_VARINT_BYTES: usize = 10;

/// Appends the canonical encoding of `value` to `out`.
pub fn encode_into(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7F).unwrap_or(0);
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// The number of bytes the canonical encoding of `value` takes.
#[must_use]
pub fn encoded_len(value: u64) -> usize {
    let bits = u64::BITS.saturating_sub(value.leading_zeros()).max(1);
    usize::try_from(bits.div_ceil(7)).unwrap_or(MAX_VARINT_BYTES)
}

/// A varint decoder fed one byte at a time (used by the slice cursor and the streaming
/// reader, so both enforce exactly the same rules).
#[derive(Debug, Default)]
pub struct VarintDecoder {
    value: u64,
    count: usize,
}

impl VarintDecoder {
    /// Starts a new varint.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds the next byte; returns the value when the varint is complete.
    ///
    /// # Errors
    ///
    /// `session.damaged` for a non-canonical or overlong encoding.
    pub fn feed(&mut self, byte: u8) -> Result<Option<u64>> {
        let shift = u32::try_from(self.count.saturating_mul(7)).unwrap_or(u32::MAX);
        let group = u64::from(byte & 0x7F);
        if self.count == MAX_VARINT_BYTES.saturating_sub(1) && byte > 1 {
            return Err(error::damaged("a varint does not fit 64 bits"));
        }
        self.value |= group.checked_shl(shift).unwrap_or(0);
        self.count = self.count.saturating_add(1);
        if byte & 0x80 == 0 {
            if byte == 0 && self.count > 1 {
                return Err(error::damaged("a varint is not in canonical form"));
            }
            return Ok(Some(self.value));
        }
        if self.count >= MAX_VARINT_BYTES {
            return Err(error::damaged("a varint is longer than 10 bytes"));
        }
        Ok(None)
    }
}

/// A bounds-checked reader over a byte slice. Every overrun is `session.damaged`.
#[derive(Debug)]
pub struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Starts at the beginning of `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Bytes not yet read.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Fails unless every byte was read.
    ///
    /// # Errors
    ///
    /// `session.damaged` if bytes are left over.
    pub fn finish(self) -> Result<()> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(error::damaged("unexpected bytes after the last field"))
        }
    }

    /// Reads `n` bytes.
    ///
    /// # Errors
    ///
    /// `session.damaged` if fewer than `n` bytes remain.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| error::damaged("a length overflows"))?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| error::damaged("a field runs past the end of its section"))?;
        self.pos = end;
        Ok(slice)
    }

    /// Reads one byte.
    ///
    /// # Errors
    ///
    /// `session.damaged` at the end of the data.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?.first().copied().unwrap_or(0))
    }

    /// Reads a little-endian `u32`.
    ///
    /// # Errors
    ///
    /// `session.damaged` at the end of the data.
    pub fn u32(&mut self) -> Result<u32> {
        let bytes: [u8; 4] = self
            .bytes(4)?
            .try_into()
            .map_err(|_| error::damaged("a field is too short"))?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Reads a little-endian `u64`.
    ///
    /// # Errors
    ///
    /// `session.damaged` at the end of the data.
    pub fn u64(&mut self) -> Result<u64> {
        let bytes: [u8; 8] = self
            .bytes(8)?
            .try_into()
            .map_err(|_| error::damaged("a field is too short"))?;
        Ok(u64::from_le_bytes(bytes))
    }

    /// Reads a little-endian `i64`.
    ///
    /// # Errors
    ///
    /// `session.damaged` at the end of the data.
    pub fn i64(&mut self) -> Result<i64> {
        let bytes: [u8; 8] = self
            .bytes(8)?
            .try_into()
            .map_err(|_| error::damaged("a field is too short"))?;
        Ok(i64::from_le_bytes(bytes))
    }

    /// Reads a canonical varint.
    ///
    /// # Errors
    ///
    /// `session.damaged` for a non-canonical, overlong or cut-off varint.
    pub fn varint(&mut self) -> Result<u64> {
        let mut decoder = VarintDecoder::new();
        loop {
            let byte = self.u8()?;
            if let Some(value) = decoder.feed(byte)? {
                return Ok(value);
            }
        }
    }

    /// Reads a string: a varint byte length (at most `max`) and that many bytes of UTF-8.
    ///
    /// # Errors
    ///
    /// `session.damaged` for a length above `max`, a cut-off string or invalid UTF-8.
    pub fn string(&mut self, max: usize) -> Result<String> {
        let len = usize::try_from(self.varint()?)
            .ok()
            .filter(|len| *len <= max)
            .ok_or_else(|| error::damaged("a text field is longer than allowed"))?;
        let bytes = self.bytes(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| error::damaged("a text field is not valid UTF-8"))
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn encode(value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        encode_into(value, &mut out);
        out
    }

    fn decode(bytes: &[u8]) -> Result<u64> {
        Cursor::new(bytes).varint()
    }

    #[test]
    fn values_round_trip_and_the_length_is_exact() {
        let mut values = vec![
            0,
            1,
            127,
            128,
            16_383,
            16_384,
            u64::from(u32::MAX),
            u64::MAX,
            u64::MAX - 1,
            1 << 63,
        ];
        let mut rng = Rng(7);
        for _ in 0..100_000 {
            let shift = rng.next() % 64;
            values.push(rng.next() >> shift);
        }
        for value in values {
            let bytes = encode(value);
            assert_eq!(encoded_len(value), bytes.len(), "{value}");
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(cursor.varint().unwrap(), value);
            cursor.finish().unwrap();
        }
    }

    #[test]
    fn known_encodings() {
        assert_eq!(encode(0), [0x00]);
        assert_eq!(encode(127), [0x7F]);
        assert_eq!(encode(128), [0x80, 0x01]);
        assert_eq!(encode(300), [0xAC, 0x02]);
        assert_eq!(encode(u64::MAX).len(), 10);
        assert_eq!(*encode(u64::MAX).last().unwrap(), 1);
    }

    #[test]
    fn non_canonical_overlong_and_cut_off_encodings_are_rejected() {
        let damaged = |bytes: &[u8]| decode(bytes).unwrap_err().code() == error::DAMAGED;
        assert!(damaged(&[0x80, 0x00]), "trailing zero group");
        assert!(
            damaged(&[0x81, 0x80, 0x00]),
            "trailing zero group after data"
        );
        assert!(damaged(&[0xFF; 11]), "longer than 10 bytes");
        assert!(damaged(&[0xFF; 10]), "10th byte has the continuation bit");
        let mut ten_bytes_too_big = vec![0xFF; 9];
        ten_bytes_too_big.push(2);
        assert!(damaged(&ten_bytes_too_big), "10th byte above 1");
        assert!(damaged(&[0x80]), "cut off");
        assert!(damaged(&[]), "empty");
        // The maximum is fine.
        let mut max = vec![0xFF; 9];
        max.push(1);
        assert_eq!(decode(&max).unwrap(), u64::MAX);
    }

    #[test]
    fn the_cursor_never_reads_past_the_end_and_reads_fixed_width_values() {
        let mut data = vec![7_u8];
        data.extend_from_slice(&0xAABB_CCDD_u32.to_le_bytes());
        data.extend_from_slice(&u64::MAX.to_le_bytes());
        data.extend_from_slice(&(-5_i64).to_le_bytes());
        let mut c = Cursor::new(&data);
        assert_eq!(c.u8().unwrap(), 7);
        assert_eq!(c.u32().unwrap(), 0xAABB_CCDD);
        assert_eq!(c.u64().unwrap(), u64::MAX);
        assert_eq!(c.i64().unwrap(), -5);
        assert_eq!(c.remaining(), 0);
        assert_eq!(c.u8().unwrap_err().code(), error::DAMAGED);
        assert_eq!(c.bytes(1).unwrap_err().code(), error::DAMAGED);
        assert_eq!(
            Cursor::new(&[1, 2]).bytes(usize::MAX).unwrap_err().code(),
            error::DAMAGED
        );
        assert_eq!(
            Cursor::new(&[1, 2, 3]).u32().unwrap_err().code(),
            error::DAMAGED
        );
        assert_eq!(
            Cursor::new(&[1]).finish().unwrap_err().code(),
            error::DAMAGED
        );
    }

    #[test]
    fn strings_are_length_checked_and_must_be_utf8() {
        let text = |s: &str| {
            let mut out = Vec::new();
            encode_into(s.len() as u64, &mut out);
            out.extend_from_slice(s.as_bytes());
            out
        };
        assert_eq!(
            Cursor::new(&text("héllo ж")).string(100).unwrap(),
            "héllo ж"
        );
        assert_eq!(Cursor::new(&text("")).string(100).unwrap(), "");
        assert_eq!(
            Cursor::new(&text("toolong")).string(3).unwrap_err().code(),
            error::DAMAGED
        );
        // Length larger than the data, and invalid UTF-8.
        assert_eq!(
            Cursor::new(&[5, b'a']).string(100).unwrap_err().code(),
            error::DAMAGED
        );
        assert_eq!(
            Cursor::new(&[2, 0xFF, 0xFE])
                .string(100)
                .unwrap_err()
                .code(),
            error::DAMAGED
        );
        // A claimed length of u64::MAX is rejected without allocating.
        let mut huge = Vec::new();
        encode_into(u64::MAX, &mut huge);
        assert_eq!(
            Cursor::new(&huge).string(4096).unwrap_err().code(),
            error::DAMAGED
        );
    }
}
