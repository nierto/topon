//! Speed hashing mode — throughput-optimized for arbitrary byte input.
//!
//! Uses the expanding multiply-mod-1 sponge: input is absorbed into a small
//! fixed-width lane state in O(len) with ZERO transcendentals, then squeezed
//! with ONE `sin()` call per output dimension. The expanding map guarantees
//! a one-byte change anywhere in the input avalanches through the state.

use g_math::fixed_point::imperative::FixedVector;
use crate::FixedPoint;

use crate::core::config::CoordinateConfig;
use crate::core::manifold::{self, ManifoldType};
use crate::mixing::chaotic::ChaoticMixer;
use crate::mixing::MixingStrategy;
use crate::{GeometricHash, ToponError};

/// Speed-optimized geometric hasher.
///
/// Uses the expanding multiply-mod-1 sponge — ergodic with positive Lyapunov
/// exponent, computed with only multiply/add/frac in the hot loop.
pub struct SpeedHasher {
    config: CoordinateConfig,
    mixer: ChaoticMixer,
}

impl SpeedHasher {
    /// Create a speed hasher.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`] — dimension zero, or above
    /// `MAX_DIMENSION`.
    pub fn new(config: CoordinateConfig) -> Result<Self, ToponError> {
        config.validate()?;
        let num = manifold::raw_components(config.dimension, config.manifold_type);
        Ok(SpeedHasher {
            mixer: ChaoticMixer::new(num)?,
            config,
        })
    }
}

impl GeometricHash for SpeedHasher {
    fn name(&self) -> &str {
        "Speed (multiply-mod-1 sponge)"
    }

    fn hash(&self, input: &[u8]) -> Result<FixedVector<FixedPoint>, ToponError> {
        let num = manifold::raw_components(self.config.dimension, self.config.manifold_type);
        let components = self.mixer.mix(input, num)?;
        let raw = FixedVector::from_slice(&components);
        manifold::project_to_manifold(&raw, self.config.manifold_type)
    }

    fn output_dimension(&self) -> usize {
        match self.config.manifold_type {
            ManifoldType::Sphere | ManifoldType::Hyperbolic => self.config.dimension + 1,
            ManifoldType::Euclidean => self.config.dimension,
        }
    }

    fn manifold_type(&self) -> ManifoldType {
        self.config.manifold_type
    }
}
