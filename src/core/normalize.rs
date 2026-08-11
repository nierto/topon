//! Domain name normalization.

use crate::ToponError;

/// Normalize a domain name: lowercase, trim whitespace, strip trailing dots.
pub fn normalize_domain(domain: &str) -> Result<String, ToponError> {
    let normalized = domain
        .trim()
        .to_lowercase()
        .trim_end_matches('.')
        .to_string();
    if normalized.is_empty() {
        return Err(ToponError::InvalidDomain);
    }
    Ok(normalized)
}
