//! Run all three hashing modes against a shared test set and compare metrics.
//!
//! Usage:
//!     export GMATH_PROFILE=embedded    # Q64.64 — recommended
//!     cargo run --example compare

use topon::{eval, CoordinateConfig, GeometricHash, LabelOrder, ManifoldType, WalkLimits};
use topon::mode::domain::DomainHasher;
use topon::mode::speed::SpeedHasher;
use topon::mode::hierarchical::HierarchicalHasher;

fn main() {
    println!("Topon — Geodineum Geometric Hash Library");
    println!("========================================");
    println!();

    let dim = 16;

    let test_domains: Vec<&str> = vec![
        "example.com", "example.org", "example.net",
        "google.com", "amazon.com",
        "test.com", "test1.com", "test2.com",
        "a.com", "b.com", "c.com", "d.com",
        "sub.example.com", "deep.sub.example.com",
        "x.x.x.x.x.x.x.x.com",
        "this-is-a-long-domain-name.test.example.com",
    ];

    // ---- Sphere S^16 ----
    println!("Manifold: Sphere S^{} (ambient R^{})", dim, dim + 1);
    println!();

    let sphere_config = CoordinateConfig {
        dimension: dim,
        manifold_type: ManifoldType::Sphere,
    };

    let sphere_hashers: Vec<Box<dyn GeometricHash>> = vec![
        Box::new(DomainHasher::new(sphere_config.clone()).unwrap()),
        Box::new(SpeedHasher::new(sphere_config.clone()).unwrap()),
    ];

    let mut sphere_results = Vec::new();
    for hasher in &sphere_hashers {
        println!("  Evaluating: {} ...", hasher.name());
        sphere_results.push(eval::evaluate_hasher(hasher.as_ref(), &test_domains).unwrap());
    }
    println!();
    eval::print_comparison(&sphere_results);

    // ---- Hyperbolic H^16 ----
    println!();
    println!("Manifold: Hyperbolic H^{} (hyperboloid in R^{})", dim, dim + 1);
    println!();

    let hyper_config = CoordinateConfig {
        dimension: dim,
        manifold_type: ManifoldType::Hyperbolic,
    };

    // The corpus deliberately includes a 9-label domain
    // ("x.x.x.x.x.x.x.x.com") to stress deep hierarchies, which exceeds the
    // default depth cap of 8. Raise the cap for this comparison so every mode
    // is scored on the identical corpus — in production, DepthExceeded is the
    // correct response to an over-deep path (see mixing::geodesic docs).
    let hyper_limits = WalkLimits { max_depth: 12, ..WalkLimits::default() };

    let hyper_hashers: Vec<Box<dyn GeometricHash>> = vec![
        Box::new(DomainHasher::new(hyper_config.clone()).unwrap()),
        Box::new(SpeedHasher::new(hyper_config.clone()).unwrap()),
        Box::new(
            HierarchicalHasher::with_options(
                hyper_config.clone(),
                b'.',
                LabelOrder::RootLast,
                hyper_limits,
            )
            .unwrap(),
        ),
    ];

    let mut hyper_results = Vec::new();
    for hasher in &hyper_hashers {
        println!("  Evaluating: {} ...", hasher.name());
        hyper_results.push(eval::evaluate_hasher(hasher.as_ref(), &test_domains).unwrap());
    }
    println!();
    eval::print_comparison(&hyper_results);

    // ---- Euclidean R^16 ----
    println!();
    println!("Manifold: Euclidean R^{}", dim);
    println!();

    let euclid_config = CoordinateConfig {
        dimension: dim,
        manifold_type: ManifoldType::Euclidean,
    };

    let euclid_hashers: Vec<Box<dyn GeometricHash>> = vec![
        Box::new(DomainHasher::new(euclid_config.clone()).unwrap()),
        Box::new(SpeedHasher::new(euclid_config.clone()).unwrap()),
    ];

    let mut euclid_results = Vec::new();
    for hasher in &euclid_hashers {
        println!("  Evaluating: {} ...", hasher.name());
        euclid_results.push(eval::evaluate_hasher(hasher.as_ref(), &test_domains).unwrap());
    }
    println!();
    eval::print_comparison(&euclid_results);

    // ---- Collision Analysis ----
    println!();
    println!("================================================================");
    println!();
    eval::print_collision_analysis(dim as u32, ManifoldType::Sphere, 253);
}
