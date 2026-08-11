//! Irrational constant generation from prime square roots.
//!
//! Each `√p` for prime `p` is irrational and algebraically independent from
//! `√q` for any distinct prime `q`. This guarantees that different dimensions
//! of the hash output have no algebraic relationship — the foundation of the
//! fractal independence property.

use g_math::fixed_point::imperative::FixedPoint;
use g_math::fixed_point::tables::prime_table::nth_prime;

use crate::ToponError;

/// Largest index accepted by [`sqrt_nth_prime`].
///
/// g_math's prime table holds 1,145 primes (`nth_prime(1144) == Some(9973)`,
/// `nth_prime(1145) == None`). Pinned by a unit test below so a table change
/// upstream fails loudly here rather than as an out-of-range error at runtime.
pub const MAX_PRIME_INDEX: usize = 1144;

/// Generate √p for the nth prime p (0-indexed).
///
/// # Errors
///
/// [`ToponError::InvalidConfig`] if `n` exceeds [`MAX_PRIME_INDEX`].
pub fn sqrt_nth_prime(n: usize) -> Result<FixedPoint, ToponError> {
    let p = nth_prime(n).ok_or(ToponError::InvalidConfig)?;
    Ok(FixedPoint::from_int(p as i32).sqrt())
}

/// Precompute a bank of `n` irrational constants: `[√p₀, √p₁, ..., √pₙ₋₁]`.
///
/// # Errors
///
/// [`ToponError::InvalidConfig`] if `n` exceeds `MAX_PRIME_INDEX + 1`, or if
/// `n` is zero — every consumer indexes the bank modulo its length, so an
/// empty bank would divide by zero.
pub fn constant_bank(n: usize) -> Result<Vec<FixedPoint>, ToponError> {
    if n == 0 || n > MAX_PRIME_INDEX + 1 {
        return Err(ToponError::InvalidConfig);
    }
    (0..n).map(sqrt_nth_prime).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the prime-table bound that `MAX_DIMENSION` is derived from.
    #[test]
    fn max_prime_index_matches_table() {
        assert!(nth_prime(MAX_PRIME_INDEX).is_some(), "table shrank");
        assert!(nth_prime(MAX_PRIME_INDEX + 1).is_none(), "table grew");
    }

    #[test]
    fn bank_rejects_empty_and_oversized() {
        assert!(constant_bank(0).is_err());
        assert!(constant_bank(MAX_PRIME_INDEX + 2).is_err());
        assert!(constant_bank(MAX_PRIME_INDEX + 1).is_ok());
        assert!(constant_bank(1).is_ok());
    }
}
