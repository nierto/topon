//! Weyl equidistribution engine — the mathematical heart of Topon.
//!
//! The sequence `{n · α mod 1}` for irrational `α` is equidistributed in [0,1)
//! (Weyl's theorem, 1916). When `α = √p` for prime `p`, and we apply `sin()`
//! as the "wave function collapse," the output is dense and non-periodic in
//! [-1, 1]. Across dimensions, algebraic independence of `√p₁, √p₂, ...` for
//! distinct primes guarantees joint equidistribution on the product space.
//!
//! This module formalizes these properties into a reusable accumulator that
//! every mixing strategy builds upon.
//!
//! Note on overflow: accumulator arithmetic deliberately wraps (see
//! [`super::wrapping`]) — the wrap acts as the modular reduction of the
//! state. Transcendental folds use the `try_` variants and propagate overflow.

use g_math::fixed_point::imperative::OverflowDetected;
use crate::FixedPoint;

use super::constants::sqrt_nth_prime;
use super::wrapping::{wrapping_add, wrapping_from_int, wrapping_mul};
use crate::ToponError;

/// A Weyl accumulator for one dimension of the hash output.
///
/// Combines input contributions with a unique irrational constant (√p) and
/// provides wave-function folding via `sin()` to produce equidistributed output.
pub struct WeylAccumulator {
    /// Current accumulated state.
    pub state: FixedPoint,
    /// The irrational constant for this dimension: √(p_i).
    pub irrational: FixedPoint,
}

impl WeylAccumulator {
    /// Create a new accumulator for the given dimension index.
    ///
    /// Computes `√p_i` on the spot. Prefer [`WeylAccumulator::with_constant`]
    /// in hot paths, passing a precomputed constant from a bank.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `dim_index` exceeds the prime table
    /// (see [`super::constants::MAX_PRIME_INDEX`]).
    pub fn new(dim_index: usize) -> Result<Self, ToponError> {
        Ok(Self::with_constant(sqrt_nth_prime(dim_index)?, dim_index))
    }

    /// Create an accumulator from a precomputed irrational constant.
    ///
    /// Initialized to `√p_i * (i + 1)` — a dimension-dependent seed.
    pub fn with_constant(irrational: FixedPoint, dim_index: usize) -> Self {
        let dim_fp = wrapping_from_int((dim_index as i32).wrapping_add(1));
        WeylAccumulator {
            state: wrapping_mul(irrational, dim_fp),
            irrational,
        }
    }

    /// Absorb a contribution into the accumulator.
    ///
    /// `acc = acc * irrational + contribution`
    ///
    /// The irrational multiplier ensures no periodic collisions.
    #[inline]
    pub fn absorb(&mut self, contribution: FixedPoint) {
        self.state = wrapping_add(wrapping_mul(self.state, self.irrational), contribution);
    }

    /// Absorb a contribution, folding via `sin()` every `period` steps.
    ///
    /// Call with `step` as the current iteration index (0-based).
    #[inline]
    pub fn absorb_with_periodic_fold(
        &mut self,
        contribution: FixedPoint,
        step: usize,
        period: usize,
    ) -> Result<(), OverflowDetected> {
        self.absorb(contribution);
        if (step + 1).is_multiple_of(period) {
            self.state = self.state.try_sin()?;
        }
        Ok(())
    }

    /// Fold the accumulated state into (-1, 1) via `sin()`.
    ///
    /// This is the "wave function collapse" — the canonical equidistribution
    /// step that maps the unbounded accumulator to a bounded, dense output.
    #[inline]
    pub fn fold(&self) -> Result<FixedPoint, OverflowDetected> {
        self.state.try_sin()
    }
}

/// Wave-function fold: maps any FixedPoint value to (-1, 1) via `sin()`.
///
/// This is the fundamental bounding operation. `sin()` is computed at 0 ULP
/// by g_math, so folding introduces no precision loss.
#[inline]
pub fn wave_fold(x: FixedPoint) -> Result<FixedPoint, OverflowDetected> {
    x.try_sin()
}

/// Exact fractional part: `frac(x) = x - ⌊x⌋ ∈ [0, 1)`.
///
/// `to_int()` floors toward negative infinity, so this is an exact mod-1
/// reduction in pure fixed-point arithmetic — zero transcendentals. It is
/// the bounding step of the expanding multiply-mod-1 map used by the
/// chaotic mixer.
#[inline]
pub fn frac(x: FixedPoint) -> FixedPoint {
    x - FixedPoint::from_int(x.to_int())
}
