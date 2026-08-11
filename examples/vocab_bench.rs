//! Load-time vocab benchmark — how long does positioning a 150k-token
//! BPE vocabulary take at inference dimensionality (H^127, ambient 128)?
//!
//! Compares:
//!   1. naive — one full `hash()` per leaf path (extrapolated from 5k)
//!   2. tree walk — one `walk_step` per taxonomy NODE, walks cloned at
//!      interior nodes (the way a vocabulary loader positions tokens)
//!
//! Run: GMATH_PROFILE=realtime GMATH_FRAC_BITS=10 cargo run --release --example vocab_bench

use std::time::Instant;

use topon::{CoordinateConfig, GeometricHash, HierarchicalHasher, ManifoldType};

const DIM: usize = 127; // H^(head_dim-1) for head_dim = 128
const ROOTS: u32 = 30;
const MIDS: u32 = 50;
const LEAVES: u32 = 100; // 30 × 50 × 100 = 150,000 paths

fn main() {
    let config = CoordinateConfig { dimension: DIM, manifold_type: ManifoldType::Hyperbolic };
    let hasher = HierarchicalHasher::taxonomy(config).unwrap();

    // ---- naive: full hash per path, over a 5k sample ----
    let naive_sample = 5_000u32;
    let t0 = Instant::now();
    let mut count = 0u32;
    'outer: for r in 0..ROOTS {
        for m in 0..MIDS {
            for l in 0..LEAVES {
                let path = format!("root{:02}/mid{:02}/leaf{:03}", r, m, l);
                let point = hasher.hash(path.as_bytes()).expect("hash failed");
                std::hint::black_box(&point);
                count += 1;
                if count >= naive_sample {
                    break 'outer;
                }
            }
        }
    }
    let naive_elapsed = t0.elapsed();
    let naive_per_path = naive_elapsed / naive_sample;
    let naive_150k_est = naive_per_path * 150_000;

    println!("naive (full hash per path), {} sample:", naive_sample);
    println!("  {:?} total, {:?}/path", naive_elapsed, naive_per_path);
    println!("  extrapolated 150k: {:.1?}", naive_150k_est);

    // ---- tree walk: one step per node, clone at interior nodes ----
    let t1 = Instant::now();
    let mut total = 0usize;
    let origin = hasher.walk_origin();
    for r in 0..ROOTS {
        let root_label = format!("root{:02}", r);
        let root_walk = hasher.walk_step(&origin, root_label.as_bytes()).expect("root step");
        for m in 0..MIDS {
            let mid_label = format!("mid{:02}", m);
            let mid_walk = hasher.walk_step(&root_walk, mid_label.as_bytes()).expect("mid step");
            for l in 0..LEAVES {
                let leaf_label = format!("leaf{:03}", l);
                let leaf_walk = hasher.walk_step(&mid_walk, leaf_label.as_bytes()).expect("leaf step");
                let point = hasher.finish(&leaf_walk).expect("finish");
                std::hint::black_box(&point);
                total += 1;
            }
        }
    }
    let tree_elapsed = t1.elapsed();

    println!();
    println!("tree walk (one step per node), {} paths:", total);
    println!("  {:?} total, {:?}/path", tree_elapsed, tree_elapsed / total as u32);
    println!();
    println!(
        "speedup vs naive (extrapolated): {:.1}x",
        naive_150k_est.as_secs_f64() / tree_elapsed.as_secs_f64()
    );
}
