//! Geodesic mixing — hyperbolic space geodesic walks.
//!
//! Exploits the exponential volume growth of hyperbolic space by parsing
//! input as a hierarchy of labels and walking along geodesics via `exp_map`.
//! Parent labels sit closer to the origin; deeper labels move outward.
//!
//! This mixer is special: it produces hyperboloid-model output directly
//! rather than raw components for generic manifold projection.
//!
//! ## Walk limits (radius budget + depth cap)
//!
//! Every walk is bounded by [`WalkLimits`]:
//!
//! - **Radius budget** (default 12.0): the cumulative geodesic step length is
//!   clamped so the walked point never leaves a ball of radius `radius_budget`
//!   around the origin. The hyperboloid time component is `x₀ = cosh(r)`, which
//!   grows exponentially in `r` while fixed-point range grows only linearly in
//!   bits — so the range is log-bounded in EVERY profile and the budget is an
//!   intrinsic requirement, not a workaround. At realtime Q22.10 (±2,097,151
//!   integer range) `cosh(r)` overflows at r ≈ 15.2; budget 12 (cosh ≈ 81,377)
//!   leaves headroom for downstream tangent/exp_map arithmetic.
//! - **Depth cap** (default 8): paths deeper than `max_depth` non-empty labels
//!   are an upstream (taxonomy builder) bug and fail loudly with
//!   [`ToponError::DepthExceeded`] rather than silently truncating.
//!
//! With the default harmonic step schedule (step at depth d = 1/(d+1), which
//! shrinks with depth) a depth-8 walk uses at most H₈ ≈ 2.72 of the budget,
//! so the clamp never binds in normal operation — it is a safety invariant.

use g_math::fixed_point::imperative::{FixedPoint, FixedVector};
use g_math::fixed_point::imperative::manifold::{HyperbolicSpace, Manifold};

use crate::core::constants::constant_bank;
use crate::core::manifold::hyperboloid_lift;
use crate::ToponError;

/// Which end of the separated input is the hierarchy root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelOrder {
    /// Root is the LAST label (domain names: `sub.example.com` → root `com`).
    /// This is the historical default.
    RootLast,
    /// Root is the FIRST label (taxonomy paths: `science/physics/quantum`).
    RootFirst,
}

/// Hard bounds on a geodesic walk. See module docs for rationale.
#[derive(Clone, Copy, Debug)]
pub struct WalkLimits {
    /// Maximum number of non-empty labels in one path.
    pub max_depth: usize,
    /// Maximum cumulative geodesic step length from the origin.
    pub radius_budget: FixedPoint,
}

impl Default for WalkLimits {
    fn default() -> Self {
        WalkLimits {
            max_depth: 8,
            radius_budget: FixedPoint::from_int(12),
        }
    }
}

/// Incremental state of a geodesic walk down a hierarchy.
///
/// Obtained from [`GeodesicMixer::walk_origin`], advanced one label at a time
/// with [`GeodesicMixer::walk_step`], finalized with [`GeodesicMixer::finish`].
/// Cloning a walk at an interior node lets callers amortize shared prefixes:
/// positioning an entire taxonomy is one `walk_step` per tree NODE instead of
/// one full walk per LEAF.
#[derive(Clone)]
pub struct GeodesicWalk {
    point: FixedVector,
    depth: usize,
    used_radius: FixedPoint,
}

impl GeodesicWalk {
    /// Current on-hyperboloid point (not yet re-lifted; see `finish`).
    pub fn point(&self) -> &FixedVector {
        &self.point
    }

    /// Number of labels consumed so far.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Cumulative geodesic step length consumed from the radius budget.
    pub fn used_radius(&self) -> FixedPoint {
        self.used_radius
    }
}

/// Geodesic mixer for tree-structured input.
pub struct GeodesicMixer {
    constants: Vec<FixedPoint>,
    separator: u8,
    order: LabelOrder,
    limits: WalkLimits,
}

impl GeodesicMixer {
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `dimension` is zero or exceeds the
    /// prime table.
    pub fn new(dimension: usize, separator: u8) -> Result<Self, ToponError> {
        Self::with_order_and_limits(dimension, separator, LabelOrder::RootLast, WalkLimits::default())
    }

    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `dimension` is zero or exceeds the
    /// prime table.
    pub fn with_order_and_limits(
        dimension: usize,
        separator: u8,
        order: LabelOrder,
        limits: WalkLimits,
    ) -> Result<Self, ToponError> {
        Ok(GeodesicMixer {
            constants: constant_bank(dimension)?,
            separator,
            order,
            limits,
        })
    }

    /// The configured walk limits.
    pub fn limits(&self) -> &WalkLimits {
        &self.limits
    }

    /// Hash a single label's bytes into a spatial direction vector.
    fn label_to_direction(
        &self,
        label: &[u8],
        depth: usize,
        dim: usize,
    ) -> Result<Vec<FixedPoint>, ToponError> {
        let n = self.constants.len();
        let mut direction = vec![FixedPoint::ZERO; dim];

        for (pos, &byte) in label.iter().enumerate() {
            let byte_fp = FixedPoint::from_int(byte as i32 + 1);
            let pos_fp = FixedPoint::from_int(pos as i32 + 1);

            for (d, component) in direction.iter_mut().enumerate() {
                let c = self.constants[(pos + d + depth * 37) % n];
                *component += byte_fp * c * pos_fp;
            }
        }

        for component in direction.iter_mut() {
            *component = component.try_sin()?;
        }

        Ok(direction)
    }

    /// Build a tangent vector at `point` on the hyperboloid from spatial components.
    fn build_tangent(
        &self,
        point: &FixedVector,
        spatial: &[FixedPoint],
        step_size: FixedPoint,
        dim: usize,
        manifold: &HyperbolicSpace,
    ) -> FixedVector {
        // Minkowski orthogonality: v₀ = Σ(vᵢ · pᵢ) / p₀
        let mut spatial_dot = FixedPoint::ZERO;
        for d in 0..dim {
            spatial_dot += spatial[d] * point[d + 1];
        }
        let v0 = spatial_dot / point[0];

        let mut tangent_components = Vec::with_capacity(dim + 1);
        tangent_components.push(v0);
        tangent_components.extend_from_slice(spatial);
        let tangent = FixedVector::from_slice(&tangent_components);

        let norm = manifold.norm(point, &tangent);
        if norm.is_zero() {
            return tangent;
        }

        &tangent * (step_size / norm)
    }

    /// Start a walk at the hyperboloid origin (1, 0, ..., 0).
    pub fn walk_origin(&self, dimension: usize) -> GeodesicWalk {
        let mut origin = vec![FixedPoint::ZERO; dimension + 1];
        origin[0] = FixedPoint::one();
        GeodesicWalk {
            point: FixedVector::from_slice(&origin),
            depth: 0,
            used_radius: FixedPoint::ZERO,
        }
    }

    /// Advance a walk by one label (one hierarchy level).
    ///
    /// The step length follows the harmonic depth-decay schedule
    /// `1 / (depth + 1)` — deeper labels take progressively smaller steps —
    /// and is clamped so `used_radius` never exceeds the radius budget.
    /// Exceeding `max_depth` fails loudly.
    pub fn walk_step(
        &self,
        walk: &GeodesicWalk,
        label: &[u8],
        dimension: usize,
    ) -> Result<GeodesicWalk, ToponError> {
        if walk.depth >= self.limits.max_depth {
            return Err(ToponError::DepthExceeded {
                max_depth: self.limits.max_depth,
            });
        }

        let manifold = HyperbolicSpace { dim: dimension };
        let spatial = self.label_to_direction(label, walk.depth, dimension)?;

        // Harmonic depth decay: step shrinks with depth.
        let mut step_size = FixedPoint::one()
            / FixedPoint::from_int(walk.depth as i32 + 1);

        // Radius budget: clamp the step to whatever budget remains. The walked
        // point's distance from origin is bounded by the cumulative step
        // length (triangle inequality), so this bounds cosh(r) too.
        let remaining = self.limits.radius_budget - walk.used_radius;
        if remaining <= FixedPoint::ZERO {
            // Budget exhausted: the point stops moving. Unreachable with the
            // default limits (H₈ ≈ 2.72 < 12) — safety clamp only.
            return Ok(GeodesicWalk {
                point: walk.point.clone(),
                depth: walk.depth + 1,
                used_radius: walk.used_radius,
            });
        }
        if step_size > remaining {
            step_size = remaining;
        }

        let tangent = self.build_tangent(&walk.point, &spatial, step_size, dimension, &manifold);
        let point = manifold.exp_map(&walk.point, &tangent)?;

        Ok(GeodesicWalk {
            point,
            depth: walk.depth + 1,
            used_radius: walk.used_radius + step_size,
        })
    }

    /// Finalize a walk: re-project to enforce the hyperboloid constraint.
    pub fn finish(&self, walk: &GeodesicWalk, dimension: usize) -> Result<FixedVector, ToponError> {
        let spatial: Vec<FixedPoint> = (1..=dimension).map(|d| walk.point[d]).collect();
        hyperboloid_lift(&spatial)
    }

    /// Mix input as a separator-delimited hierarchy, producing hyperboloid coordinates.
    pub fn mix_hierarchical(
        &self,
        input: &[u8],
        dimension: usize,
    ) -> Result<FixedVector, ToponError> {
        let mut labels: Vec<&[u8]> = input.split(|&b| b == self.separator).collect();
        if self.order == LabelOrder::RootLast {
            labels.reverse();
        }

        let mut walk = self.walk_origin(dimension);
        for label in labels {
            if label.is_empty() {
                continue;
            }
            walk = self.walk_step(&walk, label, dimension)?;
        }

        self.finish(&walk, dimension)
    }
}
