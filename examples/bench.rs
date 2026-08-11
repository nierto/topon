//! Throughput benchmark for all three modes — the source of the numbers in
//! the README's Performance section.
//!
//! Run:
//!     GMATH_PROFILE=embedded cargo run --release --example bench
//!
//! Always build with `--release`: the fixed-point inner loops are 10–50×
//! slower unoptimized and debug numbers are meaningless.

use std::hint::black_box;
use std::time::{Duration, Instant};

use topon::{
    CoordinateConfig, DomainHasher, GeometricHash, HierarchicalHasher, ManifoldType, SpeedHasher,
};

/// Run `f` enough times to spend at least `MIN_TIME`, return per-call time.
fn time<F: FnMut()>(mut f: F) -> Duration {
    const MIN_TIME: Duration = Duration::from_millis(250);

    // Warm up caches and branch predictors.
    for _ in 0..16 {
        f();
    }

    let mut iters = 64u32;
    loop {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        let elapsed = start.elapsed();
        if elapsed >= MIN_TIME || iters > 1 << 24 {
            return elapsed / iters;
        }
        iters *= 4;
    }
}

fn per_sec(d: Duration) -> f64 {
    1.0 / d.as_secs_f64()
}

fn profile() -> &'static str {
    if cfg!(table_format = "q16_16") {
        "realtime (Q16.16)"
    } else if cfg!(table_format = "q32_32") {
        "compact (Q32.32)"
    } else if cfg!(table_format = "q64_64") {
        "embedded (Q64.64)"
    } else if cfg!(table_format = "q128_128") {
        "balanced (Q128.128)"
    } else if cfg!(table_format = "q256_256") {
        "scientific (Q256.256)"
    } else {
        "unknown"
    }
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("WARNING: debug build — rerun with --release for meaningful numbers.\n");
    }

    println!("Topon benchmark — profile: {}", profile());
    println!("{}", "=".repeat(64));

    let domain = b"deep.sub.example.com";

    // ---- per-hash latency by mode and dimension ----
    println!("\nSingle hash, input = {:?} ({} bytes)", "deep.sub.example.com", domain.len());
    println!("{:<12} {:>14} {:>14} {:>12}", "dimension", "domain mode", "speed mode", "speed/domain");
    for dim in [8usize, 16, 32, 128] {
        let cfg = CoordinateConfig { dimension: dim, manifold_type: ManifoldType::Sphere };
        let d = DomainHasher::new(cfg.clone()).unwrap();
        let s = SpeedHasher::new(cfg).unwrap();

        let dt = time(|| {
            black_box(d.hash(black_box(domain)).unwrap());
        });
        let st = time(|| {
            black_box(s.hash(black_box(domain)).unwrap());
        });

        println!(
            "S^{:<10} {:>9.1} µs {:>9.1} µs {:>11.1}×",
            dim,
            dt.as_secs_f64() * 1e6,
            st.as_secs_f64() * 1e6,
            dt.as_secs_f64() / st.as_secs_f64(),
        );
    }

    // ---- hashes per second at the two headline configs ----
    println!("\nThroughput (hashes/sec, 20-byte input)");
    for (label, dim) in [("S^16  (recommended)", 16usize), ("S^128 (default)", 128)] {
        let cfg = CoordinateConfig { dimension: dim, manifold_type: ManifoldType::Sphere };
        let d = DomainHasher::new(cfg.clone()).unwrap();
        let s = SpeedHasher::new(cfg).unwrap();
        let dt = time(|| {
            black_box(d.hash(black_box(domain)).unwrap());
        });
        let st = time(|| {
            black_box(s.hash(black_box(domain)).unwrap());
        });
        println!(
            "  {:<20} domain {:>10.0}/s    speed {:>10.0}/s",
            label,
            per_sec(dt),
            per_sec(st)
        );
    }

    // ---- bulk byte throughput (speed mode is the arbitrary-bytes mode) ----
    println!("\nBulk throughput, speed mode on S^16");
    let cfg = CoordinateConfig { dimension: 16, manifold_type: ManifoldType::Sphere };
    let s = SpeedHasher::new(cfg).unwrap();
    for len in [64usize, 1024, 16_384, 262_144] {
        let buf: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let t = time(|| {
            black_box(s.hash(black_box(&buf)).unwrap());
        });
        println!(
            "  {:>8} B  {:>10.1} µs  {:>8.2} MB/s",
            len,
            t.as_secs_f64() * 1e6,
            (len as f64 / 1e6) / t.as_secs_f64()
        );
    }

    // ---- hierarchical: full hash vs incremental walk ----
    // The incremental API is the reason a 150k-token taxonomy is tractable:
    // shared prefixes cost one step per tree NODE, not one walk per LEAF.
    println!("\nHierarchical mode on H^16 (3-level taxonomy path)");
    let hcfg = CoordinateConfig { dimension: 16, manifold_type: ManifoldType::Hyperbolic };
    let h = HierarchicalHasher::taxonomy(hcfg).unwrap();

    let full = time(|| {
        black_box(h.hash(black_box(b"science/physics/quantum")).unwrap());
    });
    println!("  full hash per leaf      {:>10.1} µs  ({:.0}/s)", full.as_secs_f64() * 1e6, per_sec(full));

    // One leaf step + finish, with the parent prefix already walked — what
    // the vocab loader actually pays per token.
    let origin = h.walk_origin();
    let root = h.walk_step(&origin, b"science").unwrap();
    let mid = h.walk_step(&root, b"physics").unwrap();
    let leaf_only = time(|| {
        let w = h.walk_step(black_box(&mid), black_box(b"quantum")).unwrap();
        black_box(h.finish(&w).unwrap());
    });
    println!(
        "  incremental leaf step   {:>10.1} µs  ({:.0}/s)   {:.1}× faster",
        leaf_only.as_secs_f64() * 1e6,
        per_sec(leaf_only),
        full.as_secs_f64() / leaf_only.as_secs_f64()
    );

    println!("\nNote: zero floating point anywhere — every figure above is");
    println!("fixed-point arithmetic with 0 ULP transcendentals.");
}
