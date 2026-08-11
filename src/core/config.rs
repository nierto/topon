//! Configuration types for geometric hashing.

use super::manifold::{raw_components, ManifoldType};
use crate::ToponError;

/// Largest `dimension` any hasher accepts.
///
/// Every output dimension consumes one `√p` constant from g_math's prime
/// table, which holds 1,145 primes (indices `0..=1144`, up to 9,973). A
/// sphere of dimension `d` needs `d + 1` constants, so `1144` is the largest
/// dimension that is valid on *every* manifold. See
/// [`crate::core::constants::MAX_PRIME_INDEX`].
pub const MAX_DIMENSION: usize = 1144;

/// Configuration for coordinate generation.
///
/// Fields are public for ergonomic literal construction, but a config is only
/// guaranteed usable once it passes [`CoordinateConfig::validate`] — every
/// hasher constructor validates before allocating its constant bank, so an
/// out-of-range dimension surfaces as [`ToponError::InvalidConfig`] rather than
/// a panic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinateConfig {
    /// Intrinsic dimension of the coordinate space. Must be in
    /// `1..=`[`MAX_DIMENSION`].
    ///
    /// For Sphere and Hyperbolic the output has `dimension + 1` components;
    /// for Euclidean it has `dimension`.
    pub dimension: usize,
    /// Target manifold for projection.
    pub manifold_type: ManifoldType,
}

impl CoordinateConfig {
    /// Create a validated config.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `dimension` is zero or exceeds
    /// [`MAX_DIMENSION`].
    pub fn new(dimension: usize, manifold_type: ManifoldType) -> Result<Self, ToponError> {
        let config = CoordinateConfig { dimension, manifold_type };
        config.validate()?;
        Ok(config)
    }

    /// Check that this config can be realized on the target manifold.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `dimension` is zero (a hash needs at
    /// least one component) or if the manifold would need more `√p` constants
    /// than the prime table provides.
    pub fn validate(&self) -> Result<(), ToponError> {
        if self.dimension == 0 {
            return Err(ToponError::InvalidConfig);
        }
        if self.dimension > MAX_DIMENSION {
            return Err(ToponError::InvalidConfig);
        }
        // Belt and braces: the constant bank is sized by raw_components, so
        // check the number actually requested rather than trusting the bound
        // above to cover every manifold's +1.
        if raw_components(self.dimension, self.manifold_type) > MAX_DIMENSION + 1 {
            return Err(ToponError::InvalidConfig);
        }
        Ok(())
    }
}

impl Default for CoordinateConfig {
    fn default() -> Self {
        CoordinateConfig {
            dimension: 128,
            manifold_type: ManifoldType::Sphere,
        }
    }
}
