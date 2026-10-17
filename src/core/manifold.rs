//! Manifold types, projection, and geodesic distance.
//!
//! Projection and distance delegate to g_math's Riemannian manifold
//! implementations (`Sphere`, `EuclideanSpace`, `HyperbolicSpace`) rather
//! than reimplementing the geometry here.

use g_math::fixed_point::imperative::fused;
use g_math::fixed_point::imperative::manifold::{
    EuclideanSpace, HyperbolicSpace, Manifold, Sphere,
};
use g_math::fixed_point::imperative::FixedVector;
use crate::FixedPoint;

use crate::ToponError;

/// Which manifold to project coordinates onto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ManifoldType {
    /// Unit sphere S^n (ambient dimension = n+1).
    Sphere,
    /// Flat Euclidean R^n.
    Euclidean,
    /// Hyperboloid H^n.
    Hyperbolic,
}

/// Lift spatial components onto the hyperboloid: prepend x₀ = √(1 + Σxᵢ²).
///
/// The single implementation of the hyperboloid constraint, shared by
/// manifold projection and the geodesic mixer's re-projection step.
///
/// Computed as `√(1² + Σxᵢ²)` via [`fused::sqrt_sum_sq`], so the squares, the
/// accumulation and the square root all stay at compute tier with a single
/// rounding at the end — instead of rounding once per `x*x`, once per `+=`,
/// and again at the `sqrt`.
pub fn hyperboloid_lift(spatial: &[FixedPoint]) -> Result<FixedVector<FixedPoint>, ToponError> {
    // Seed slot 0 with 1 so the fused norm computes √(1 + Σxᵢ²) directly,
    // then overwrite it with the result — one allocation, no second pass.
    let mut components = Vec::with_capacity(spatial.len() + 1);
    components.push(FixedPoint::one());
    components.extend_from_slice(spatial);
    components[0] = fused::sqrt_sum_sq(&components);
    Ok(FixedVector::from_slice(&components))
}

/// Project a raw coordinate vector onto the target manifold.
///
/// - **Sphere**: normalize to unit length (errors on the zero vector).
/// - **Euclidean**: pass through unchanged.
/// - **Hyperbolic**: hyperboloid model — prepend x₀ = √(1 + Σxᵢ²).
pub fn project_to_manifold(
    raw: &FixedVector<FixedPoint>,
    manifold_type: ManifoldType,
) -> Result<FixedVector<FixedPoint>, ToponError> {
    match manifold_type {
        ManifoldType::Sphere => {
            let norm_sq = raw.length_squared();
            if norm_sq.is_zero() {
                return Err(ToponError::DegenerateVector);
            }
            // Reciprocal-multiply rather than `normalized()`, which divides
            // once per component. A fixed-point division costs ~200 multiplies,
            // so at S^128 this replaces ~129 divisions with one `inv_sqrt` and
            // 129 multiplies — the dominant cost in sphere-mode hashing.
            //
            // `try_inv_sqrt` (not `fused::inv_sqrt_sum_sq`) because the fused
            // form panics on a zero norm or a reciprocal that overflows
            // storage; this crate returns errors instead of panicking.
            let inv_norm = norm_sq.try_inv_sqrt()?;
            Ok(raw.map(|v| v * inv_norm))
        }
        ManifoldType::Euclidean => Ok(raw.clone()),
        ManifoldType::Hyperbolic => {
            let spatial: Vec<FixedPoint> = raw.iter().copied().collect();
            hyperboloid_lift(&spatial)
        }
    }
}

/// Geodesic distance between two points on the given manifold.
///
/// Delegates to g_math's `Manifold::distance`: great-circle distance on the
/// sphere, straight-line distance in Euclidean space, and hyperbolic distance
/// on the hyperboloid. Both points must lie on the manifold (as all Topon
/// hash outputs do).
pub fn geodesic_distance(
    a: &FixedVector<FixedPoint>,
    b: &FixedVector<FixedPoint>,
    manifold_type: ManifoldType,
) -> Result<FixedPoint, ToponError> {
    let d = match manifold_type {
        ManifoldType::Sphere => Sphere { dim: a.len().saturating_sub(1) }.distance(a, b)?,
        ManifoldType::Euclidean => EuclideanSpace { dim: a.len() }.distance(a, b)?,
        ManifoldType::Hyperbolic => {
            HyperbolicSpace { dim: a.len().saturating_sub(1) }.distance(a, b)?
        }
    };
    Ok(d)
}

/// Number of raw components to generate before manifold projection.
pub fn raw_components(dimension: usize, manifold_type: ManifoldType) -> usize {
    match manifold_type {
        ManifoldType::Sphere => dimension + 1,
        ManifoldType::Euclidean | ManifoldType::Hyperbolic => dimension,
    }
}
