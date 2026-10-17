//! Modular fixed-point arithmetic: the wrap is the reduction.
//!
//! The mixers' accumulators grow without bound and rely on overflow wrapping
//! modulo the storage word, as g_math up to 0.6.3 did for `+`, `*` and
//! `from_int`. From 0.6.4 those operators panic on overflow and 0.7 has no
//! wrapping twin, so the wrap is spelled out here.
//!
//! Each function returns exactly what g_math 0.6.1 returned at every profile
//! (in range, the same value as the checked operator):
//!
//! - `wrapping_add(a, b)`: `a + b` mod 2^N.
//! - `wrapping_mul(a, b)`: the product rounded to nearest, ties toward +∞
//!   (`⌊ab / 2^F⌋ + [bit F-1 of ab]`), mod 2^N.
//! - `wrapping_from_int(v)`: `v · 2^F` mod 2^N.
//!
//! N is the storage width and F the fraction bits of [`FixedPoint`].

use g_math::fixed_point::{FixedNum, Storage, WideInt};

use crate::FixedPoint;

type Raw = <FixedPoint as FixedNum>::Storage;
type Wide = <Raw as Storage>::Compute;

const F: u32 = <FixedPoint as FixedNum>::FRAC_BITS;

/// The low `Raw::BITS` bits of `w`, as a two's-complement storage word.
#[inline]
fn truncate(w: Wide) -> Raw {
    let bytes = (Raw::BITS / 8) as usize;
    let words = (Raw::BITS as usize).div_ceil(64);
    let mut be = Vec::with_capacity(words * 8);
    for i in (0..words as u32).rev() {
        be.extend_from_slice(&w.word(i).to_be_bytes());
    }
    Raw::from_be_slice(&be[be.len() - bytes..])
}

/// `a + b` modulo the storage word.
#[inline]
pub fn wrapping_add(a: FixedPoint, b: FixedPoint) -> FixedPoint {
    // Two N-bit values cannot overflow the 2N-bit compute word.
    FixedPoint::from_raw(truncate(a.raw().widen() + b.raw().widen()))
}

/// `a * b` rounded to nearest (ties toward +∞), modulo the storage word.
#[inline]
pub fn wrapping_mul(a: FixedPoint, b: FixedPoint) -> FixedPoint {
    // The exact product fits the 2N-bit compute word; shifting it right
    // by F keeps it there, so adding the rounding bit cannot overflow.
    let product = a.raw().mul_wide(b.raw());
    let mut scaled = product.shr_bits(F);
    if product.bit(F - 1) {
        scaled = scaled + Wide::one();
    }
    FixedPoint::from_raw(truncate(scaled))
}

/// `v` as a fixed-point value, modulo the storage word.
#[inline]
pub fn wrapping_from_int(v: i32) -> FixedPoint {
    // |v| < 2^31 and F < N, so v · 2^F fits the 2N-bit compute word.
    FixedPoint::from_raw(truncate(Wide::from_i64(v as i64).shl_bits(F)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_range_matches_checked_ops() {
        let vals = ["0", "1", "-1", "0.5", "-0.75", "3.25", "-7.125", "1.4142135", "-0.0001"];
        for a in vals {
            for b in vals {
                let (x, y) = (FixedPoint::from_str(a), FixedPoint::from_str(b));
                assert_eq!(wrapping_add(x, y), x + y, "{a} + {b}");
                assert_eq!(wrapping_mul(x, y), x * y, "{a} * {b}");
            }
        }
        for v in [-100, -1, 0, 1, 7, 100] {
            assert_eq!(wrapping_from_int(v), FixedPoint::from_int(v));
        }
    }

    #[test]
    fn overflow_wraps_instead_of_panicking() {
        // MAX + one ulp wraps to MIN, exactly as two's-complement addition.
        let max = FixedPoint::from_raw(truncate(Wide::one().shl_bits(Raw::BITS - 1) - Wide::one()));
        let ulp = FixedPoint::from_raw(truncate(Wide::one()));
        let min = FixedPoint::from_raw(truncate(Wide::one().shl_bits(Raw::BITS - 1)));
        assert_eq!(wrapping_add(max, ulp), min);
        // An overflowing product does not panic.
        let _ = wrapping_mul(max, max);
    }
}
