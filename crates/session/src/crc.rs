//! CRC-32C (Castagnoli), the checksum of every section of a session file.
//!
//! Reflected polynomial `0x1EDC6F41` (`0x82F63B78` reversed), initial value and final XOR
//! `0xFFFFFFFF`; the check value of `"123456789"` is `0xE3069283`. A slicing-by-8 fast path
//! processes eight bytes per step; tests cross-check it against a bytewise reference.

const POLY: u32 = 0x82F6_3B78;

/// The eight slicing tables, built at compile time.
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "compile-time table construction with constant, in-range indexes"
)]
const fn make_tables() -> [[u32; 256]; 8] {
    let mut tables = [[0_u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
            bit += 1;
        }
        tables[0][i] = crc;
        i += 1;
    }
    let mut i = 0;
    while i < 256 {
        let mut crc = tables[0][i];
        let mut t = 1;
        while t < 8 {
            crc = tables[0][(crc & 0xFF) as usize] ^ (crc >> 8);
            tables[t][i] = crc;
            t += 1;
        }
        i += 1;
    }
    tables
}

static TABLES: [[u32; 256]; 8] = make_tables();

/// Looks up `index` (masked to a byte, so always below 256) in table `table` (masked to 0..8).
#[allow(
    clippy::indexing_slicing,
    reason = "the table has 8 rows of 256 entries and both indexes are masked into range"
)]
#[inline]
fn lookup(table: usize, index: u32) -> u32 {
    TABLES[table & 7][(index & 0xFF) as usize]
}

/// An incremental CRC-32C computation.
#[derive(Debug, Clone, Copy)]
pub struct Crc32c {
    state: u32,
}

impl Default for Crc32c {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32c {
    /// Starts a new computation.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: u32::MAX }
    }

    /// Adds `data` to the checksum.
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.state;
        let mut chunks = data.chunks_exact(8);
        for chunk in &mut chunks {
            let low = u32::from_le_bytes([
                chunk.first().copied().unwrap_or(0),
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
                chunk.get(3).copied().unwrap_or(0),
            ]) ^ crc;
            let high = u32::from_le_bytes([
                chunk.get(4).copied().unwrap_or(0),
                chunk.get(5).copied().unwrap_or(0),
                chunk.get(6).copied().unwrap_or(0),
                chunk.get(7).copied().unwrap_or(0),
            ]);
            crc = lookup(7, low)
                ^ lookup(6, low >> 8)
                ^ lookup(5, low >> 16)
                ^ lookup(4, low >> 24)
                ^ lookup(3, high)
                ^ lookup(2, high >> 8)
                ^ lookup(1, high >> 16)
                ^ lookup(0, high >> 24);
        }
        for &byte in chunks.remainder() {
            crc = lookup(0, crc ^ u32::from(byte)) ^ (crc >> 8);
        }
        self.state = crc;
    }

    /// The checksum of everything added so far (the computation can continue).
    #[must_use]
    pub const fn finish(&self) -> u32 {
        !self.state
    }
}

/// The checksum of `data`.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = Crc32c::new();
    crc.update(data);
    crc.finish()
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    /// Bytewise reference implementation, independent of the tables.
    fn reference(data: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ POLY
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn the_check_value_of_the_standard_string() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn rfc_3720_test_vectors() {
        assert_eq!(crc32c(&[0_u8; 32]), 0x8A91_36AA);
        assert_eq!(crc32c(&[0xFF_u8; 32]), 0x62A8_AB43);
        let ascending: Vec<u8> = (0..32).collect();
        assert_eq!(crc32c(&ascending), 0x46DD_794E);
        let descending: Vec<u8> = (0..32).rev().collect();
        assert_eq!(crc32c(&descending), 0x113F_DB5C);
    }

    #[test]
    fn empty_input_has_the_checksum_zero() {
        assert_eq!(crc32c(&[]), 0);
        assert_eq!(Crc32c::new().finish(), 0);
    }

    #[test]
    fn the_fast_path_equals_the_reference_for_random_buffers_and_splits() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for _ in 0..10_000 {
            let len = usize::try_from(rng.next() % 200).unwrap();
            let data: Vec<u8> = (0..len).map(|_| rng.next().to_le_bytes()[0]).collect();
            let want = reference(&data);
            assert_eq!(crc32c(&data), want);
            // Incremental updates at random split points give the same result.
            let mut crc = Crc32c::new();
            let mut at = 0;
            while at < data.len() {
                let step = usize::try_from(rng.next() % 17).unwrap() + 1;
                let end = (at + step).min(data.len());
                crc.update(&data[at..end]);
                at = end;
            }
            assert_eq!(crc.finish(), want);
        }
    }

    #[test]
    fn finish_does_not_consume_the_computation() {
        let mut crc = Crc32c::new();
        crc.update(b"1234");
        let _ = crc.finish();
        crc.update(b"56789");
        assert_eq!(crc.finish(), 0xE306_9283);
    }
}
