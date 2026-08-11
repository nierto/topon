//! Free-function convenience API.
//!
//! Provides `domain_to_coordinates()` for callers that want to hash a single
//! domain in one call rather than construct and hold a [`DomainHasher`].
//!
//! Prefer building a hasher directly when hashing more than once: the
//! constructor precomputes a bank of `√p` constants (one square root per
//! output dimension), so this function pays that cost on every call.

use g_math::fixed_point::imperative::FixedVector;

use crate::core::config::CoordinateConfig;
use crate::mode::domain::DomainHasher;
use crate::{DomainHash, ToponError};

/// Generate a deterministic coordinate vector from a domain name.
///
/// Equivalent to `DomainHasher::new(config)?.hash_domain(domain)`.
///
/// # Errors
///
/// [`ToponError::InvalidConfig`] if `config` is out of range,
/// [`ToponError::InvalidDomain`] if `domain` normalizes to the empty string.
pub fn domain_to_coordinates(
    domain: &str,
    config: &CoordinateConfig,
) -> Result<FixedVector, ToponError> {
    let hasher = DomainHasher::new(config.clone())?;
    hasher.hash_domain(domain)
}
