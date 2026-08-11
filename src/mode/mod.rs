//! User-facing hashing modes.
//!
//! Each mode composes a mixing strategy with core services (manifold projection,
//! normalization) to produce a `GeometricHash` implementation tuned for a specific
//! use case.

pub mod domain;
pub mod speed;
pub mod hierarchical;
