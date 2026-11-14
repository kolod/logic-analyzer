//! Checked position arithmetic and pixel-boundary math.
//!
//! The workspace denies unchecked arithmetic, so all position math goes through the
//! helpers here.

use la_core::Result;

/// Adds two positions, failing with `storage.position_overflow` instead of wrapping.
pub(crate) fn checked_position_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(crate::error::position_overflow)
}

/// Boundary `i` (0..=`w`) of `w` pixel columns over the window `[start, start + len)`:
/// `start + floor(i * len / w)`, computed in `u128` so it cannot overflow.
///
/// Returns `None` if `w` is zero or `i` is above `w`.
pub(crate) fn pixel_boundary(start: u64, len: u64, i: u32, w: u32) -> Option<u64> {
    if w == 0 || i > w {
        return None;
    }
    let scaled = u128::from(len).checked_mul(u128::from(i))?;
    let offset = scaled.checked_div(u128::from(w))?;
    u64::try_from(offset).ok()?.checked_add(start)
}

/// Worst-case number of temporary bytes needed to convert a chunk of `len` samples for
/// `channels` channels (4 bytes per sample and channel). `None` on overflow.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "used by the memory budget check (phase 6)")
)]
pub(crate) fn chunk_temp_bytes(len: usize, channels: u8) -> Option<u64> {
    u64::try_from(len)
        .ok()?
        .checked_mul(u64::from(channels))?
        .checked_mul(4)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn position_add_detects_overflow() {
        assert_eq!(checked_position_add(1, 2).unwrap(), 3);
        assert_eq!(checked_position_add(u64::MAX - 1, 1).unwrap(), u64::MAX);
        let err = checked_position_add(u64::MAX, 1).unwrap_err();
        assert_eq!(err.code(), crate::error::POSITION_OVERFLOW);
        assert!(checked_position_add(u64::MAX, u64::MAX).is_err());
    }

    #[test]
    fn boundaries_span_the_window_exactly() {
        for (start, len, w) in [
            (0_u64, 1000_u64, 7_u32),
            (5, 1, 2000),
            (10, 99, 1),
            (0, u64::MAX, 65_536),
        ] {
            assert_eq!(pixel_boundary(start, len, 0, w), Some(start));
            assert_eq!(pixel_boundary(start, len, w, w), start.checked_add(len));
        }
    }

    #[test]
    fn boundaries_are_monotonic_and_floor_divided() {
        let (start, len, w) = (1_000_u64, 12_345_u64, 977_u32);
        let mut previous = start;
        for i in 0..=w {
            let b = pixel_boundary(start, len, i, w).unwrap();
            assert!(b >= previous, "boundary {i} went backwards");
            assert_eq!(
                u128::from(b - start),
                u128::from(len) * u128::from(i) / u128::from(w)
            );
            previous = b;
        }
    }

    #[test]
    fn boundaries_do_not_overflow_for_huge_windows() {
        // len * i would overflow u64 by a wide margin.
        let b = pixel_boundary(0, u64::MAX, 65_535, 65_536).unwrap();
        assert!(b < u64::MAX);
        assert_eq!(
            pixel_boundary(u64::MAX - 10, 10, 65_536, 65_536),
            Some(u64::MAX)
        );
    }

    #[test]
    fn narrow_windows_have_zero_width_columns() {
        // 3 positions over 10 pixels: some consecutive boundaries are equal.
        let b: Vec<u64> = (0..=10)
            .map(|i| pixel_boundary(100, 3, i, 10).unwrap())
            .collect();
        assert!(b.windows(2).any(|p| p[0] == p[1]));
        assert_eq!(b[0], 100);
        assert_eq!(b[10], 103);
    }

    #[test]
    fn invalid_arguments_return_none() {
        assert_eq!(pixel_boundary(0, 10, 0, 0), None);
        assert_eq!(pixel_boundary(0, 10, 11, 10), None);
    }

    #[test]
    fn chunk_temp_bytes_is_exact_and_overflow_safe() {
        assert_eq!(chunk_temp_bytes(65_536, 32), Some(65_536 * 32 * 4));
        assert_eq!(chunk_temp_bytes(0, 8), Some(0));
        assert_eq!(chunk_temp_bytes(usize::MAX, 32), None);
    }
}
