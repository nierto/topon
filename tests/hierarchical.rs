//! Hierarchical-mode hardening suite (mission O15 phase 1).
//!
//! Covers: determinism, ancestry monotonicity, sibling separation, walk
//! limits (depth cap + radius budget), incremental-walk equivalence, and a
//! 150k-path collision scan. All tests are written for the realtime Q22.10
//! profile (tolerances follow tests/basic.rs).

use topon::FixedPoint;
use g_math::fixed_point::imperative::FixedVector;
use topon::{
    geodesic_distance, CoordinateConfig, ToponError, GeometricHash, HierarchicalHasher, LabelOrder,
    ManifoldType, WalkLimits,
};

mod common;

const DIM: usize = 16;

fn config(dim: usize) -> CoordinateConfig {
    CoordinateConfig { dimension: dim, manifold_type: ManifoldType::Hyperbolic }
}

fn taxonomy_hasher() -> HierarchicalHasher {
    HierarchicalHasher::taxonomy(config(DIM)).unwrap()
}

fn dist(a: &FixedVector<FixedPoint>, b: &FixedVector<FixedPoint>) -> f64 {
    geodesic_distance(a, b, ManifoldType::Hyperbolic).unwrap().to_f64()
}

// ============================================================================
// Determinism
// ============================================================================

#[test]
fn taxonomy_hash_is_deterministic() {
    let h1 = taxonomy_hasher();
    let h2 = taxonomy_hasher();
    for path in ["science", "science/physics", "science/physics/quantum", "art/music/jazz"] {
        let a = h1.hash(path.as_bytes()).unwrap();
        let b = h2.hash(path.as_bytes()).unwrap();
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert_eq!(a[i], b[i], "{}: component {} differs across instances", path, i);
        }
    }
}

#[test]
fn incremental_walk_matches_full_hash() {
    let h = taxonomy_hasher();
    let full = h.hash(b"science/physics/quantum").unwrap();

    let walk = h.walk_origin();
    let walk = h.walk_step(&walk, b"science").unwrap();
    let walk = h.walk_step(&walk, b"physics").unwrap();
    let walk = h.walk_step(&walk, b"quantum").unwrap();
    let inc = h.finish(&walk).unwrap();

    assert_eq!(full.len(), inc.len());
    for i in 0..full.len() {
        assert_eq!(full[i], inc[i], "component {} differs (incremental vs full)", i);
    }
}

#[test]
fn root_first_order_is_used_for_taxonomy() {
    // If the root label were consumed last (domain order), swapping root and
    // leaf would produce the same walk. Assert it does not.
    let h = taxonomy_hasher();
    let a = h.hash(b"science/quantum").unwrap();
    let b = h.hash(b"quantum/science").unwrap();
    assert!(dist(&a, &b) > 1e-3, "root-first and root-last collapsed");
}

// ============================================================================
// Ancestry monotonicity: dist(parent, child) < dist(parent, stranger)
// ============================================================================

#[test]
fn ancestry_monotonicity() {
    let h = taxonomy_hasher();

    // (parent, child, stranger-in-another-top-level-branch)
    let cases = [
        ("science", "science/physics", "art/music"),
        ("science", "science/biology", "law/contracts"),
        ("science/physics", "science/physics/quantum", "art/music/jazz"),
        ("art", "art/music", "science/physics"),
        ("code", "code/rust", "food/baking"),
        ("code/rust", "code/rust/traits", "science/physics/quantum"),
    ];

    for (parent, child, stranger) in cases {
        let p = h.hash(parent.as_bytes()).unwrap();
        let c = h.hash(child.as_bytes()).unwrap();
        let s = h.hash(stranger.as_bytes()).unwrap();
        let d_child = dist(&p, &c);
        let d_stranger = dist(&p, &s);
        assert!(
            d_child < d_stranger,
            "ancestry violated: d({}, {}) = {:.4} >= d({}, {}) = {:.4}",
            parent, child, d_child, parent, stranger, d_stranger
        );
    }
}

#[test]
fn parent_is_closer_to_child_than_to_grandchild_stranger() {
    // A deeper relative in the SAME branch should still beat a same-depth
    // stranger from a different branch.
    let h = taxonomy_hasher();
    let p = h.hash(b"science").unwrap();
    let grandchild = h.hash(b"science/physics/quantum").unwrap();
    let stranger_same_depth = h.hash(b"art/music/jazz").unwrap();
    assert!(
        dist(&p, &grandchild) < dist(&p, &stranger_same_depth),
        "grandchild ({:.4}) should be nearer than cross-branch stranger ({:.4})",
        dist(&p, &grandchild),
        dist(&p, &stranger_same_depth)
    );
}

// ============================================================================
// Sibling separation
// ============================================================================

#[test]
fn siblings_are_separated() {
    let h = taxonomy_hasher();
    let siblings = [
        "science/physics",
        "science/biology",
        "science/chemistry",
        "science/geology",
        "science/astronomy",
    ];
    let points: Vec<FixedVector<FixedPoint>> =
        siblings.iter().map(|p| h.hash(p.as_bytes()).unwrap()).collect();

    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            let d = dist(&points[i], &points[j]);
            assert!(
                d > 0.05,
                "siblings {} and {} nearly collide (d = {:.6})",
                siblings[i], siblings[j], d
            );
        }
    }
}

#[test]
fn siblings_stay_within_branch() {
    // Siblings must separate, but not so far that they leave the parent's
    // neighborhood: each sibling stays closer to the parent than to a
    // different top-level branch root.
    let h = taxonomy_hasher();
    let parent = h.hash(b"science").unwrap();
    let other_root = h.hash(b"art").unwrap();
    for s in ["science/physics", "science/biology", "science/chemistry"] {
        let p = h.hash(s.as_bytes()).unwrap();
        assert!(
            dist(&parent, &p) < dist(&other_root, &p),
            "{} drifted out of its parent branch", s
        );
    }
}

// ============================================================================
// Walk limits: depth cap + radius budget
// ============================================================================

#[test]
fn depth_beyond_max_fails_loudly() {
    let h = taxonomy_hasher();
    // 9 labels > default max_depth = 8
    let too_deep = b"a/b/c/d/e/f/g/h/i";
    match h.hash(too_deep) {
        Err(ToponError::DepthExceeded { max_depth }) => assert_eq!(max_depth, 8),
        other => panic!("expected DepthExceeded, got {:?}", other.map(|_| "Ok(point)")),
    }
}

#[test]
fn depth_eight_is_accepted() {
    let h = taxonomy_hasher();
    let at_limit = b"a/b/c/d/e/f/g/h";
    let point = h.hash(at_limit).unwrap();
    assert_eq!(point.len(), DIM + 1);
}

#[test]
fn radius_budget_clamps_walk() {
    // Custom limits with a small budget: an 8-deep walk (harmonic total
    // ≈ 2.72) must stay inside radius 0.75 of the origin.
    let budget = FixedPoint::from_str("0.75");
    let limits = WalkLimits { max_depth: 8, radius_budget: budget };
    let h = HierarchicalHasher::with_options(config(DIM), b'/', LabelOrder::RootFirst, limits).unwrap();

    let point = h.hash(b"a/b/c/d/e/f/g/h").unwrap();

    let mut origin = FixedVector::new(DIM + 1);
    origin[0] = FixedPoint::one();
    let r = dist(&origin, &point);
    // Cumulative step length bounds the radius (triangle inequality);
    // allow Q22.10 roundoff slack.
    assert!(r <= 0.75 + 0.01, "radius {} exceeds budget 0.75", r);
}

#[test]
fn default_walk_stays_far_inside_budget() {
    // With harmonic depth decay and depth ≤ 8, the cumulative walk length is
    // H₈ ≈ 2.718 — far inside the budget of 12 (cosh(12) ≈ 81k, safely
    // representable at Q22.10; cosh overflows at r ≈ 15.2).
    let h = taxonomy_hasher();
    let walk = {
        let mut w = h.walk_origin();
        for label in [&b"a"[..], b"b", b"c", b"d", b"e", b"f", b"g", b"h"] {
            w = h.walk_step(&w, label).unwrap();
        }
        w
    };
    let used = walk.used_radius().to_f64();
    assert!(used < 2.8, "harmonic walk used {} > H8", used);

    // x0 = cosh(r) must be far below Q22.10's integer ceiling.
    let point = h.finish(&walk).unwrap();
    assert!(point[0].to_f64() < 10.0, "x0 = {} unexpectedly large", point[0].to_f64());
}

#[test]
fn depth_decay_steps_shrink() {
    // step_i = 1/(i+1): each deeper step consumes strictly less budget.
    let h = taxonomy_hasher();
    let mut w = h.walk_origin();
    let mut last_step = f64::INFINITY;
    for label in [&b"alpha"[..], b"beta", b"gamma", b"delta"] {
        let prev_used = w.used_radius().to_f64();
        w = h.walk_step(&w, label).unwrap();
        let step = w.used_radius().to_f64() - prev_used;
        assert!(step > 0.0, "step did not move");
        assert!(step < last_step, "step {} did not shrink (prev {})", step, last_step);
        last_step = step;
    }
}

#[test]
fn all_outputs_satisfy_hyperboloid_constraint() {
    let h = taxonomy_hasher();
    for path in ["a", "a/b", "science/physics/quantum/entanglement", "a/b/c/d/e/f/g/h"] {
        let p = h.hash(path.as_bytes()).unwrap();
        let mut spatial_sq = 0.0;
        for i in 1..p.len() {
            spatial_sq += p[i].to_f64() * p[i].to_f64();
        }
        let constraint = p[0].to_f64() * p[0].to_f64() - spatial_sq;
        // x0 is quantized to the 2⁻¹⁰ grid, so the constraint residual grows
        // like 2·x0·2⁻¹⁰ (≈0.015 at the deep-walk x0 ≈ 7.5) — tolerance must
        // scale with x0, not stay flat.
        let tol = 5e-3 + 3.0 * p[0].to_f64() / 1024.0;
        assert!(
            (constraint - 1.0).abs() < tol,
            "{}: x0² − Σxᵢ² = {} (should be 1 ± {})",
            path, constraint, tol
        );
    }
}

// ============================================================================
// Collision scan — 150k synthetic taxonomy paths
// ============================================================================

/// Exact-collision scan over 150,000 distinct taxonomy paths.
///
/// Uses the incremental walk API the way the vocab loader will: one
/// walk_step per tree node (30 roots × 50 mids × 100 leaves), cloning walks
/// at interior nodes. Collision = bit-identical output coordinates.
#[test]
#[cfg_attr(debug_assertions, ignore = "minutes unoptimized — run with `cargo test --release`")]
fn collision_scan_150k_paths() {
    let h = taxonomy_hasher();

    let mut seen = std::collections::HashSet::with_capacity(150_000);
    let mut total = 0usize;

    let origin = h.walk_origin();
    for r in 0..30u32 {
        let root_label = format!("root{:02}", r);
        let root_walk = h.walk_step(&origin, root_label.as_bytes()).unwrap();
        for m in 0..50u32 {
            let mid_label = format!("mid{:02}", m);
            let mid_walk = h.walk_step(&root_walk, mid_label.as_bytes()).unwrap();
            for l in 0..100u32 {
                let leaf_label = format!("leaf{:03}", l);
                let leaf_walk = h.walk_step(&mid_walk, leaf_label.as_bytes()).unwrap();
                let point = h.finish(&leaf_walk).unwrap();

                // Key on the exact raw bits of every component. Uses the
                // profile-agnostic fingerprint: `raw() as i64` does not
                // compile at Q128.128/Q256.256, where raw() is an I256.
                let key = common::fingerprint_u64(&point);
                assert!(
                    seen.insert(key),
                    "exact collision at {}/{}/{}", root_label, mid_label, leaf_label
                );
                total += 1;
            }
        }
    }
    assert_eq!(total, 150_000);
}
