//! Known-answer (golden vector) tests — the enforcement mechanism for the
//! hash stability policy in the README.
//!
//! `determinism_across_calls` in `basic.rs` only proves that two calls in one
//! process agree; it cannot notice that a refactor changed the algorithm.
//! These tests pin the actual output so any change to mixing constants, the
//! fold schedule, the walk step schedule, or normalization fails loudly and
//! forces a deliberate version bump.
//!
//! ## Profile gating
//!
//! Hash output is a function of `(input, config, GMATH_PROFILE)`. These
//! vectors are pinned at **embedded / Q64.64**, the documented default and
//! recommended profile, and the assertions are `cfg`-gated to it. At other
//! profiles the tests still compile and still verify determinism, but skip
//! the pinned comparison.
//!
//! ## Regenerating (only for a deliberate, version-bumped output change)
//!
//! ```text
//! GMATH_PROFILE=embedded cargo test --test golden -- --ignored --nocapture regenerate
//! ```
//!
//! That prints the current fingerprints; paste them into `CASES` below and
//! bump the minor version per the stability policy.

use topon::{
    compat, CoordinateConfig, DomainHash, DomainHasher, GeometricHash, HierarchicalHasher, LabelOrder,
    ManifoldType, SpeedHasher, WalkLimits,
};

mod common;
use common::{fingerprint, profile_name};

/// Is this build the profile the vectors were pinned at?
const PINNED_PROFILE: bool = cfg!(table_format = "q64_64");

// ============================================================================
// The pinned vectors
// ============================================================================
//
// (case name, fingerprint at embedded/Q64.64)
const CASES: &[(&str, &str)] = &[
    ("domain/s8/example.com", "d2777e5e411fd470"),
    ("domain/s8/example.org", "444ae2b3379fcb94"),
    ("domain/s8/empty", "05726d41aa77b778"),
    ("domain/s16/example.com", "01db5747a6cdd793"),
    ("domain/e8/example.com", "97c3bb38d066fc2b"),
    ("domain/h8/example.com", "67e12477bd2de279"),
    ("domain/s128/example.com", "e3bf17ca0e11a96c"),
    ("speed/s8/example.com", "1dcc9d46541a0cb9"),
    ("speed/s8/binary", "79a708e6dcc5b3b9"),
    ("speed/s16/long-4kb", "627cc66d9228f09b"),
    ("hier/h16/deep.sub.example.com", "0bed75ef5ff0651e"),
    ("hier/h16/taxonomy", "955d076f117ed700"),
    ("hier/h16/taxonomy-depth8", "6ad7d61a90ac556b"),
    ("compat/s16/example.com", "01db5747a6cdd793"),
    ("domain_hash/s8/UPPER.COM", "d8abab2819fe5c7e"),
];

// ============================================================================
// Case construction — one place, used by both the assertion and regeneration
// ============================================================================

fn cfg_for(dim: usize, m: ManifoldType) -> CoordinateConfig {
    CoordinateConfig { dimension: dim, manifold_type: m }
}

/// Compute every case's fingerprint, in the same order as `CASES`.
fn compute_all() -> Vec<(&'static str, String)> {
    use ManifoldType::{Euclidean, Hyperbolic, Sphere};

    let mut out: Vec<(&'static str, String)> = Vec::new();

    let mut push = |name: &'static str, fp: String| out.push((name, fp));

    // ---- domain mode ----
    let d_s8 = DomainHasher::new(cfg_for(8, Sphere)).unwrap();
    push("domain/s8/example.com", fingerprint(&d_s8.hash(b"example.com").unwrap()));
    push("domain/s8/example.org", fingerprint(&d_s8.hash(b"example.org").unwrap()));
    push("domain/s8/empty", fingerprint(&d_s8.hash(b"").unwrap()));

    let d_s16 = DomainHasher::new(cfg_for(16, Sphere)).unwrap();
    push("domain/s16/example.com", fingerprint(&d_s16.hash(b"example.com").unwrap()));

    let d_e8 = DomainHasher::new(cfg_for(8, Euclidean)).unwrap();
    push("domain/e8/example.com", fingerprint(&d_e8.hash(b"example.com").unwrap()));

    let d_h8 = DomainHasher::new(cfg_for(8, Hyperbolic)).unwrap();
    push("domain/h8/example.com", fingerprint(&d_h8.hash(b"example.com").unwrap()));

    // The default config, which `domain_hasher()` returns.
    let d_default = DomainHasher::new(CoordinateConfig::default()).unwrap();
    push("domain/s128/example.com", fingerprint(&d_default.hash(b"example.com").unwrap()));

    // ---- speed mode ----
    let s_s8 = SpeedHasher::new(cfg_for(8, Sphere)).unwrap();
    push("speed/s8/example.com", fingerprint(&s_s8.hash(b"example.com").unwrap()));
    // Non-UTF8 bytes, including 0x00 and 0xFF, to pin byte handling exactly.
    let binary: Vec<u8> = (0..=255u8).collect();
    push("speed/s8/binary", fingerprint(&s_s8.hash(&binary).unwrap()));

    let s_s16 = SpeedHasher::new(cfg_for(16, Sphere)).unwrap();
    let long: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
    push("speed/s16/long-4kb", fingerprint(&s_s16.hash(&long).unwrap()));

    // ---- hierarchical mode ----
    let h_dom = HierarchicalHasher::new(cfg_for(16, Hyperbolic)).unwrap();
    push(
        "hier/h16/deep.sub.example.com",
        fingerprint(&h_dom.hash(b"deep.sub.example.com").unwrap()),
    );

    let h_tax = HierarchicalHasher::taxonomy(cfg_for(16, Hyperbolic)).unwrap();
    push("hier/h16/taxonomy", fingerprint(&h_tax.hash(b"science/physics/quantum").unwrap()));
    // Exactly at the depth cap — pins the step schedule at every depth.
    push("hier/h16/taxonomy-depth8", fingerprint(&h_tax.hash(b"a/b/c/d/e/f/g/h").unwrap()));

    // ---- compat shim (free-function surface) ----
    push(
        "compat/s16/example.com",
        fingerprint(&compat::domain_to_coordinates("example.com", &cfg_for(16, Sphere)).unwrap()),
    );

    // ---- normalization path (hash_domain, not hash) ----
    push(
        "domain_hash/s8/UPPER.COM",
        fingerprint(&d_s8.hash_domain("  UPPER.COM.  ").unwrap()),
    );

    out
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn golden_vectors_are_stable() {
    let computed = compute_all();

    assert_eq!(
        computed.len(),
        CASES.len(),
        "case list and CASES table are out of sync"
    );

    if !PINNED_PROFILE {
        eprintln!(
            "golden vectors skipped: pinned at embedded (Q64.64), running at {}",
            profile_name()
        );
        return;
    }

    let mut drift: Vec<String> = Vec::new();
    for ((name, actual), (pinned_name, expected)) in computed.iter().zip(CASES) {
        assert_eq!(name, pinned_name, "case order changed");
        if actual != expected {
            drift.push(format!("  {name}\n    expected {expected}\n    actual   {actual}"));
        }
    }

    assert!(
        drift.is_empty(),
        "hash output changed for {} case(s) — this is a BREAKING output change.\n\
         If deliberate: bump the minor version per the README stability policy, \
         regenerate with `cargo test --test golden -- --ignored --nocapture regenerate`, and \
         note it in CHANGELOG.md.\n{}",
        drift.len(),
        drift.join("\n")
    );
}

/// Independent of the pinned table: the same config must produce the same
/// output from two separately constructed hashers, at every profile.
#[test]
fn output_is_stable_across_instances() {
    let a = compute_all();
    let b = compute_all();
    assert_eq!(a, b, "hashing is not reproducible at {}", profile_name());
}

/// Prints the current fingerprints for pasting into `CASES`.
/// Ignored by default — this is a tool, not an assertion.
#[test]
#[ignore = "regeneration tool: run with --nocapture to print the CASES table"]
fn regenerate() {
    println!("// fingerprints at {}", profile_name());
    println!("const CASES: &[(&str, &str)] = &[");
    for (name, fp) in compute_all() {
        println!("    ({:?}, {:?}),", name, fp);
    }
    println!("];");
}

// ============================================================================
// Config validation — the panic that used to be reachable from safe code
// ============================================================================

#[test]
fn oversized_dimension_is_rejected_not_panicked() {
    let cfg = cfg_for(topon::MAX_DIMENSION + 1, ManifoldType::Sphere);
    assert_eq!(cfg.validate(), Err(topon::ToponError::InvalidConfig));
    assert!(DomainHasher::new(cfg.clone()).is_err());
    assert!(SpeedHasher::new(cfg.clone()).is_err());
    assert!(HierarchicalHasher::new(cfg.clone()).is_err());
    assert!(topon::speed_hasher(cfg.clone()).is_err());
    assert!(topon::hierarchical_hasher(cfg.clone()).is_err());
    assert!(compat::domain_to_coordinates("example.com", &cfg).is_err());
    assert!(
        HierarchicalHasher::with_options(cfg, b'/', LabelOrder::RootFirst, WalkLimits::default())
            .is_err()
    );
}

#[test]
fn zero_dimension_is_rejected() {
    for m in [ManifoldType::Sphere, ManifoldType::Euclidean, ManifoldType::Hyperbolic] {
        let cfg = cfg_for(0, m);
        assert_eq!(cfg.validate(), Err(topon::ToponError::InvalidConfig), "{m:?}");
        assert!(DomainHasher::new(cfg.clone()).is_err(), "{m:?}");
        assert!(SpeedHasher::new(cfg.clone()).is_err(), "{m:?}");
        assert!(HierarchicalHasher::new(cfg).is_err(), "{m:?}");
    }
}

#[test]
fn max_dimension_is_accepted() {
    // The largest dimension the prime table supports, on the manifold that
    // needs the most constants (sphere: dimension + 1).
    let cfg = cfg_for(topon::MAX_DIMENSION, ManifoldType::Sphere);
    assert_eq!(cfg.validate(), Ok(()));
    let h = DomainHasher::new(cfg).expect("MAX_DIMENSION must be constructible");
    assert_eq!(h.hash(b"example.com").unwrap().len(), topon::MAX_DIMENSION + 1);
}

#[test]
fn validated_constructor_matches_validate() {
    assert!(CoordinateConfig::new(16, ManifoldType::Sphere).is_ok());
    assert!(CoordinateConfig::new(0, ManifoldType::Sphere).is_err());
    assert!(CoordinateConfig::new(topon::MAX_DIMENSION + 1, ManifoldType::Sphere).is_err());
}
