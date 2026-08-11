//! Mixing strategies — algorithmic building blocks for geometric hashing.
//!
//! Each strategy transforms raw byte input into a vector of `FixedPoint`
//! components (pre-manifold projection). Strategies are composed with
//! manifold projection and normalization by the mode layer.

use g_math::fixed_point::imperative::FixedPoint;

use crate::ToponError;

pub mod irrational;
pub mod chaotic;
pub mod geodesic;

/// Internal trait for byte-mixing strategies.
///
/// Implementations transform input bytes into `num_components` raw coordinate
/// values in approximately (-1, 1), ready for manifold projection.
/// Arithmetic overflow in the underlying g_math folds propagates as
/// `ToponError::MathOverflow`.
pub(crate) trait MixingStrategy {
    fn mix(&self, input: &[u8], num_components: usize) -> Result<Vec<FixedPoint>, ToponError>;
}
