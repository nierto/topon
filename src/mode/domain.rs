//! Domain hashing mode — proven quality for domain name resolution.
//!
//! Uses the irrational mixing strategy (best distribution metrics) with
//! full `sin()` folding. This is the production mode for domain-name workloads.

use g_math::fixed_point::imperative::FixedVector;
use crate::FixedPoint;

use crate::core::config::CoordinateConfig;
use crate::core::manifold::{self, ManifoldType};
use crate::mixing::irrational::IrrationalMixer;
use crate::mixing::MixingStrategy;
use crate::{GeometricHash, ToponError};

/// Domain-resolution geometric hasher.
///
/// Uses irrational mixing with periodic `sin()` folding — the highest-quality
/// mixing strategy, proven to achieve effective dimensionality > 96% of
/// theoretical maximum.
pub struct DomainHasher {
    config: CoordinateConfig,
    mixer: IrrationalMixer,
}

impl DomainHasher {
    /// Create a domain hasher.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`] — dimension zero, or above
    /// `MAX_DIMENSION`.
    pub fn new(config: CoordinateConfig) -> Result<Self, ToponError> {
        config.validate()?;
        let num = manifold::raw_components(config.dimension, config.manifold_type);
        Ok(DomainHasher {
            mixer: IrrationalMixer::new(num)?,
            config,
        })
    }
}

impl GeometricHash for DomainHasher {
    fn name(&self) -> &str {
        "Domain (irrational √p)"
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
