//! Core invariants: determinism, manifold constraints, avalanche, and
//! edge-case inputs across all three hashing modes.

use topon::{
    geodesic_distance, CoordinateConfig, DomainHasher, GeometricHash, HierarchicalHasher,
    ManifoldType, SpeedHasher,
};

const DIM: usize = 8;

fn sphere_config() -> CoordinateConfig {
    CoordinateConfig { dimension: DIM, manifold_type: ManifoldType::Sphere }
}

fn hyperbolic_config() -> CoordinateConfig {
    CoordinateConfig { dimension: DIM, manifold_type: ManifoldType::Hyperbolic }
}

fn all_hashers() -> Vec<Box<dyn GeometricHash>> {
    vec![
        Box::new(DomainHasher::new(sphere_config()).unwrap()),
        Box::new(SpeedHasher::new(sphere_config()).unwrap()),
        Box::new(HierarchicalHasher::new(hyperbolic_config()).unwrap()),
    ]
}

#[test]
fn determinism_across_calls() {
    for hasher in all_hashers() {
        let a = hasher.hash(b"example.com").unwrap();
        let b = hasher.hash(b"example.com").unwrap();
        assert_eq!(a.len(), b.len(), "{}", hasher.name());
        for i in 0..a.len() {
            assert_eq!(a[i], b[i], "{}: component {} differs", hasher.name(), i);
        }
    }
}

#[test]
fn sphere_outputs_have_unit_norm() {
    for hasher in [DomainHasher::new(sphere_config()).unwrap(), DomainHasher::new(sphere_config()).unwrap()]
        .into_iter()
        .map(|h| Box::new(h) as Box<dyn GeometricHash>)
        .chain(std::iter::once(Box::new(SpeedHasher::new(sphere_config()).unwrap()) as Box<dyn GeometricHash>))
    {
        let h = hasher.hash(b"example.com").unwrap();
        let norm = h.length().to_f64();
        // Tolerance is profile-dependent: manifold constraints hold to
        // ~FRAC_BITS resolution. Realtime Q22.10 → ~5e-3; wider profiles
        // are far tighter. (Was 1e-9, written under a wide profile.)
        assert!((norm - 1.0).abs() < 5e-3, "{}: norm = {}", hasher.name(), norm);
    }
}

#[test]
fn hyperboloid_outputs_satisfy_constraint() {
    let hasher = HierarchicalHasher::new(hyperbolic_config()).unwrap();
    let h = hasher.hash(b"deep.sub.example.com").unwrap();
    // x0^2 - sum(xi^2) = 1
    let mut spatial_sq = 0.0;
    for i in 1..h.len() {
        spatial_sq += h[i].to_f64() * h[i].to_f64();
    }
    let constraint = h[0].to_f64() * h[0].to_f64() - spatial_sq;
    // Profile-dependent tolerance (see sphere_outputs_have_unit_norm).
    assert!((constraint - 1.0).abs() < 5e-3, "constraint = {}", constraint);
}

/// A single-bit flip anywhere in a long input must move the output a
/// macroscopic geodesic distance. This is the regression test for the old
/// tent-map mixer, which contracted away all but the last few bytes
/// (distance ~1e-18 for a first-byte change).
#[test]
fn speed_hasher_avalanche_floor() {
    let hasher = SpeedHasher::new(sphere_config()).unwrap();
    let base = vec![b'a'; 200];

    for pos in [0usize, 100, 199] {
        let mut mutated = base.clone();
        mutated[pos] ^= 0x01;
        let h1 = hasher.hash(&base).unwrap();
        let h2 = hasher.hash(&mutated).unwrap();
        let d = geodesic_distance(&h1, &h2, ManifoldType::Sphere)
            .unwrap()
            .to_f64();
        assert!(
            d > 0.3,
            "single-bit flip at byte {} only moved output by {}",
            pos,
            d
        );
    }
}

#[test]
fn domain_hasher_avalanche_floor() {
    let hasher = DomainHasher::new(sphere_config()).unwrap();
    let h1 = hasher.hash(b"example.com").unwrap();
    let h2 = hasher.hash(b"fxample.com").unwrap();
    let d = geodesic_distance(&h1, &h2, ManifoldType::Sphere)
        .unwrap()
        .to_f64();
    assert!(d > 0.3, "one-byte change only moved output by {}", d);
}

/// Empty input must not panic (the old SpeedHasher divided by zero when
/// normalizing an all-zero vector). All modes return a valid point.
#[test]
fn empty_input_does_not_panic() {
    for hasher in all_hashers() {
        let result = hasher.hash(b"");
        let point = result.unwrap_or_else(|e| panic!("{}: {}", hasher.name(), e));
        assert_eq!(point.len(), hasher.output_dimension(), "{}", hasher.name());
    }
}

#[test]
fn distinct_inputs_are_distant() {
    for hasher in all_hashers() {
        let inputs: [&[u8]; 4] = [b"a.com", b"b.com", b"example.com", b"example.org"];
        let hashes: Vec<_> = inputs.iter().map(|i| hasher.hash(i).unwrap()).collect();
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                let d = geodesic_distance(&hashes[i], &hashes[j], hasher.manifold_type())
                    .unwrap()
                    .to_f64();
                assert!(
                    d > 1e-6,
                    "{}: {:?} and {:?} nearly collide (d = {})",
                    hasher.name(),
                    inputs[i],
                    inputs[j],
                    d
                );
            }
        }
    }
}
