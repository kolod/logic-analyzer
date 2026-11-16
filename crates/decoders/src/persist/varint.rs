//! The primitives of the section payloads: canonical LEB128 varints, strings and records.
//!
//! A reader rejects a varint that is not canonical (a trailing zero group) or that does not fit 64
//! bits, and checks every length against the bytes that remain before it allocates anything.

/// Why a payload was refused (a short English reason for the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid(pub &'static str);

/// Appends to a payload.
#[derive(Debug, Default)]
pub struct Writer {
    /// The bytes written so far.
    pub bytes: Vec<u8>,
}

impl Writer {
    /// An empty writer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One byte.
    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// A canonical varint.
    pub fn varint(&mut self, mut value: u64) {
        loop {
            let low = u8::try_from(value & 0x7f).unwrap_or(0);
            value >>= 7;
            if value == 0 {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | 0x80);
        }
    }

    /// A signed number as a zigzag varint.
    pub fn zigzag(&mut self, value: i64) {
        let encoded = (value << 1) ^ (value >> 63);
        self.varint(u64::from_ne_bytes(encoded.to_ne_bytes()));
    }

    /// A string: its length in bytes, then the bytes.
    pub fn string(&mut self, text: &str) {
        self.varint(u64::try_from(text.len()).unwrap_or(u64::MAX));
        self.bytes.extend_from_slice(text.as_bytes());
    }

    /// A record: the length of the content, then the content.
    pub fn record(&mut self, content: &[u8]) {
        self.varint(u64::try_from(content.len()).unwrap_or(u64::MAX));
        self.bytes.extend_from_slice(content);
    }
}

/// Reads a payload.
#[derive(Debug, Clone, Copy)]
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// A reader over `bytes`.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// How many bytes remain.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len()
    }

    /// Whether everything was read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// One byte.
    ///
    /// # Errors
    ///
    /// The payload ends.
    pub fn u8(&mut self) -> Result<u8, Invalid> {
        let (first, rest) = self.bytes.split_first().ok_or(Invalid("payload ends"))?;
        self.bytes = rest;
        Ok(*first)
    }

    /// A canonical varint.
    ///
    /// # Errors
    ///
    /// The payload ends, the number does not fit 64 bits, or it is not in its shortest form.
    pub fn varint(&mut self) -> Result<u64, Invalid> {
        let mut value = 0_u64;
        for index in 0..10_u32 {
            let byte = self.u8()?;
            let group = u64::from(byte & 0x7f);
            let shifted = group
                .checked_shl(index.saturating_mul(7))
                .ok_or(Invalid("varint overflows"))?;
            if index == 9 && group > 1 {
                return Err(Invalid("varint overflows"));
            }
            value |= shifted;
            if byte & 0x80 == 0 {
                if index > 0 && byte == 0 {
                    return Err(Invalid("varint is not canonical"));
                }
                return Ok(value);
            }
        }
        Err(Invalid("varint is too long"))
    }

    /// A zigzag varint.
    ///
    /// # Errors
    ///
    /// As [`Reader::varint`].
    pub fn zigzag(&mut self) -> Result<i64, Invalid> {
        let raw = self.varint()?;
        let magnitude = i64::from_ne_bytes((raw >> 1).to_ne_bytes());
        let sign = i64::from_ne_bytes((raw & 1).to_ne_bytes());
        Ok(magnitude ^ sign.wrapping_neg())
    }

    /// A varint that must not be above `max`.
    ///
    /// # Errors
    ///
    /// As [`Reader::varint`], or the number is above `max`.
    pub fn bounded(&mut self, max: u64) -> Result<u64, Invalid> {
        let value = self.varint()?;
        if value > max {
            return Err(Invalid("a count or number is beyond its limit"));
        }
        Ok(value)
    }

    /// `length` bytes.
    ///
    /// # Errors
    ///
    /// Fewer bytes remain.
    pub fn take(&mut self, length: u64) -> Result<&'a [u8], Invalid> {
        let length = usize::try_from(length).map_err(|_| Invalid("length beyond the payload"))?;
        if length > self.bytes.len() {
            return Err(Invalid("length beyond the payload"));
        }
        let (head, rest) = self.bytes.split_at(length);
        self.bytes = rest;
        Ok(head)
    }

    /// A string of at most `max` bytes and, when `min` is set, at least `min`.
    ///
    /// # Errors
    ///
    /// The length is outside its bounds, or the bytes are not UTF-8.
    pub fn string(&mut self, min: u64, max: u64) -> Result<&'a str, Invalid> {
        let length = self.bounded(max)?;
        if length < min {
            return Err(Invalid("a string is too short"));
        }
        let bytes = self.take(length)?;
        std::str::from_utf8(bytes).map_err(|_| Invalid("a string is not UTF-8"))
    }

    /// A record: a reader over its content.
    ///
    /// # Errors
    ///
    /// The length goes beyond the payload.
    pub fn record(&mut self) -> Result<Reader<'a>, Invalid> {
        let length = self.varint()?;
        Ok(Reader::new(self.take(length)?))
    }

    /// The bytes that remain, without reading them.
    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        self.bytes
    }
}
