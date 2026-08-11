//! Hierarchical hashing mode — tree-structured input on hyperbolic space.
//!
//! Parses input as a separator-delimited hierarchy and maps it to the
//! hyperboloid via geodesic walks. Parent labels sit near the origin;
//! deeper labels move progressively outward along geodesics. This preserves
//! the tree structure geometrically.
//!
//! Two label orders are supported (see [`LabelOrder`]):
//! - domain names (`sub.example.com`, root last) — the historical default
//! - taxonomy paths (`science/physics/quantum`, root first) — use
//!   [`HierarchicalHasher::taxonomy`]
//!
//! All walks are bounded by [`WalkLimits`] (depth ≤ 8, radius budget 12 by
//! default) — see the `mixing::geodesic` module docs for the Q22.10 overflow
//! rationale.

use g_math::fixed_point::imperative::FixedVector;

use crate::core::config::CoordinateConfig;
use crate::core::manifold::ManifoldType;
use crate::mixing::geodesic::{GeodesicMixer, GeodesicWalk, LabelOrder, WalkLimits};
use crate::{GeometricHash, ToponError};

/// Hierarchical geometric hasher using hyperbolic geodesic walks.
///
/// Always outputs hyperboloid-model coordinates (dim + 1 components)
/// regardless of the config's manifold type.
pub struct HierarchicalHasher {
    config: CoordinateConfig,
    mixer: GeodesicMixer,
}

impl HierarchicalHasher {
    /// Create with default separator `.` (domain names, root label last).
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`].
    pub fn new(config: CoordinateConfig) -> Result<Self, ToponError> {
        Self::with_separator(config, b'.')
    }

    /// Create with a custom path separator (e.g., `/` for file paths).
    /// Keeps the domain-name label order (root last).
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`].
    pub fn with_separator(config: CoordinateConfig, separator: u8) -> Result<Self, ToponError> {
        Self::with_options(config, separator, LabelOrder::RootLast, WalkLimits::default())
    }

    /// Create a taxonomy-path hasher: separator `/`, root label FIRST
    /// (`science/physics/quantum` walks science → physics → quantum),
    /// default walk limits (depth ≤ 8, radius budget 12).
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`].
    pub fn taxonomy(config: CoordinateConfig) -> Result<Self, ToponError> {
        Self::with_options(config, b'/', LabelOrder::RootFirst, WalkLimits::default())
    }

    /// Fully explicit constructor.
    ///
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `config` fails
    /// [`CoordinateConfig::validate`].
    pub fn with_options(
        config: CoordinateConfig,
        separator: u8,
        order: LabelOrder,
        limits: WalkLimits,
    ) -> Result<Self, ToponError> {
        config.validate()?;
        Ok(HierarchicalHasher {
            mixer: GeodesicMixer::with_order_and_limits(
                config.dimension,
                separator,
                order,
                limits,
            )?,
            config,
        })
    }

    /// The configured walk limits.
    pub fn limits(&self) -> &WalkLimits {
        self.mixer.limits()
    }

    /// Start an incremental walk at the hyperboloid origin.
    ///
    /// Together with [`walk_step`](Self::walk_step) and
    /// [`finish`](Self::finish) this exposes the walk one label at a time, so
    /// a caller positioning a whole taxonomy can clone the walk at interior
    /// nodes and pay one step per tree NODE instead of one full path per leaf.
    pub fn walk_origin(&self) -> GeodesicWalk {
        self.mixer.walk_origin(self.config.dimension)
    }

    /// Advance an incremental walk by one label.
    pub fn walk_step(&self, walk: &GeodesicWalk, label: &[u8]) -> Result<GeodesicWalk, ToponError> {
        self.mixer.walk_step(walk, label, self.config.dimension)
    }

    /// Finalize an incremental walk (hyperboloid re-projection).
    pub fn finish(&self, walk: &GeodesicWalk) -> Result<FixedVector, ToponError> {
        self.mixer.finish(walk, self.config.dimension)
    }
}

impl GeometricHash for HierarchicalHasher {
    fn name(&self) -> &str {
        "Hierarchical (geodesic walk)"
    }

    fn hash(&self, input: &[u8]) -> Result<FixedVector, ToponError> {
        self.mixer.mix_hierarchical(input, self.config.dimension)
    }

    fn output_dimension(&self) -> usize {
        self.config.dimension + 1
    }

    fn manifold_type(&self) -> ManifoldType {
        ManifoldType::Hyperbolic
    }
}
