//! # Topon — Geodineum Geometric Hash Library
//!
//! Deterministic geometric hashing that maps arbitrary input to points on
//! Riemannian manifolds. Unlike traditional hashes, Topon output has meaningful
//! geometric structure: distances, geodesics, and containment.
//!
//! ## Modes
//!
//! - [`mode::domain`] — Domain name hashing (irrational mixing, proven quality)
//! - [`mode::speed`] — Throughput-optimized (expanding multiply-mod-1 sponge)
//! - [`mode::hierarchical`] — Tree-structured input on hyperbolic space
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use topon::{domain_hasher, DomainHash};
//!
//! let hasher = domain_hasher();
//! let coords = hasher.hash_domain("example.geo").unwrap();
//! ```
//!
//! ## Hash stability
//!
//! Hash *values* are part of the public contract, not just the API. Until
//! 1.0 output may change between releases: any release that changes output
//! for any input bumps the minor version; patch releases are bit-identical.
//! Output depends on the compile-time `GMATH_PROFILE` — values are only
//! comparable between builds using the same profile.

// Determinism is the crate's core guarantee; no part of it needs `unsafe`.
#![forbid(unsafe_code)]

use g_math::fixed_point::imperative::FixedVector;

pub mod core;
pub mod mixing;
pub mod mode;
pub mod eval;
pub mod compat;

// Re-export key types for convenience
pub use crate::core::config::{CoordinateConfig, MAX_DIMENSION};
pub use crate::core::manifold::{geodesic_distance, ManifoldType};
pub use crate::mixing::geodesic::{GeodesicWalk, LabelOrder, WalkLimits};
pub use mode::domain::DomainHasher;
pub use mode::speed::SpeedHasher;
pub use mode::hierarchical::HierarchicalHasher;

// ============================================================================
// Error type
// ============================================================================

/// Errors returned by Topon operations.
///
/// Non-exhaustive: new variants may be added in future releases without a
/// major version bump, so downstream `match` arms need a `_` fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToponError {
    /// Input is empty or invalid.
    InvalidDomain,
    /// Underlying g_math overflow during computation.
    MathOverflow,
    /// Dimension or manifold configuration is invalid — the dimension is zero
    /// or exceeds [`CoordinateConfig`]'s `MAX_DIMENSION`.
    InvalidConfig,
    /// A zero vector cannot be projected onto the target manifold.
    DegenerateVector,
    /// A hierarchical path has more labels than the configured `max_depth`.
    /// Deep paths are an upstream (taxonomy) bug — fail loudly, never truncate.
    DepthExceeded { max_depth: usize },
}

impl std::fmt::Display for ToponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToponError::InvalidDomain => write!(f, "invalid input"),
            ToponError::MathOverflow => write!(f, "arithmetic overflow"),
            ToponError::InvalidConfig => write!(f, "invalid configuration"),
            ToponError::DegenerateVector => write!(f, "degenerate zero vector"),
            ToponError::DepthExceeded { max_depth } => {
                write!(f, "hierarchy deeper than max_depth = {}", max_depth)
            }
        }
    }
}

impl std::error::Error for ToponError {}

impl From<g_math::fixed_point::imperative::OverflowDetected> for ToponError {
    fn from(_: g_math::fixed_point::imperative::OverflowDetected) -> Self {
        ToponError::MathOverflow
    }
}

// ============================================================================
// Core trait
// ============================================================================

/// Core geometric hashing trait.
///
/// Maps raw byte input to a `FixedVector` on a Riemannian manifold.
/// All implementations guarantee determinism, boundedness, and zero float.
pub trait GeometricHash {
    /// Human-readable name for this hasher.
    fn name(&self) -> &str;

    /// Hash raw bytes to a coordinate vector on the configured manifold.
    fn hash(&self, input: &[u8]) -> Result<FixedVector, ToponError>;

    /// Number of components in the output vector.
    fn output_dimension(&self) -> usize;

    /// The manifold type of the output.
    fn manifold_type(&self) -> ManifoldType;
}

/// Domain-name convenience trait — automatically derived for all `GeometricHash` types.
///
/// Adds `hash_domain()` which normalizes the domain string (lowercase, trim,
/// strip trailing dots) before hashing.
pub trait DomainHash: GeometricHash {
    fn hash_domain(&self, domain: &str) -> Result<FixedVector, ToponError> {
        let normalized = crate::core::normalize::normalize_domain(domain)?;
        self.hash(normalized.as_bytes())
    }
}

/// Blanket implementation: every `GeometricHash` is also a `DomainHash`.
impl<T: GeometricHash> DomainHash for T {}

// ============================================================================
// Factory functions
// ============================================================================

/// Create the default domain hasher (irrational mixing, S^128).
///
/// Infallible: the default [`CoordinateConfig`] is always valid (pinned by
/// `default_config_is_valid`).
///
/// Note that S^128 is the *quality* default, not the fast one — it makes 129
/// passes over the input. For throughput-sensitive callers, construct a
/// [`DomainHasher`] at a smaller dimension, or use [`speed_hasher`].
pub fn domain_hasher() -> DomainHasher {
    DomainHasher::new(CoordinateConfig::default())
        .expect("the default CoordinateConfig is always valid")
}

/// Create a speed-optimized hasher for arbitrary byte input.
///
/// # Errors
///
/// [`ToponError::InvalidConfig`] if `config` fails [`CoordinateConfig::validate`].
pub fn speed_hasher(config: CoordinateConfig) -> Result<SpeedHasher, ToponError> {
    SpeedHasher::new(config)
}

/// Create a hierarchical hasher for tree-structured input.
///
/// # Errors
///
/// [`ToponError::InvalidConfig`] if `config` fails [`CoordinateConfig::validate`].
pub fn hierarchical_hasher(config: CoordinateConfig) -> Result<HierarchicalHasher, ToponError> {
    HierarchicalHasher::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the invariant that `domain_hasher()`'s `expect` relies on.
    #[test]
    fn default_config_is_valid() {
        CoordinateConfig::default()
            .validate()
            .expect("default config must stay valid");
        let _ = domain_hasher();
    }
}
