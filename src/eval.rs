//! Evaluation harness for comparing geometric hashing approaches.
//!
//! Computes quantitative metrics over a shared test domain set:
//! collision resistance, distribution quality, inter-dimension
//! correlation, effective dimensionality, avalanche, and determinism.
//!
//! All distances are geodesic distances on the hasher's output manifold
//! (via g_math's `Manifold::distance`), not chordal ambient distances.
//!
//! # Floating point
//!
//! This is the one module in the crate that uses `f64`, and only in the
//! collision-analysis *reporting* path ([`collision_analysis`],
//! [`print_collision_analysis`]): describing a 2^1024 position space and its
//! birthday bound means arithmetic in log₂ space, which fixed-point cannot
//! represent. Those values are derived from the configuration, never from
//! hash output, and never convert back into `FixedPoint`. Every metric
//! computed *from* hashes — distances, variances, correlation, effective
//! dimensionality — is `FixedPoint` throughout, and every hash path in the
//! crate is float-free.

use g_math::fixed_point::imperative::{FixedPoint, FixedVector};

use crate::core::manifold::geodesic_distance;
use crate::{ToponError, GeometricHash, ManifoldType};

// ============================================================================
// Result type
// ============================================================================

/// Evaluation results for a single approach.
pub struct EvalResult {
    pub approach_name: String,
    /// Minimum pairwise geodesic distance (collision resistance).
    pub min_pairwise_distance: FixedPoint,
    /// Mean pairwise geodesic distance.
    pub mean_pairwise_distance: FixedPoint,
    /// Minimum geodesic distance under a single-bit input flip (avalanche).
    /// Should be the same order of magnitude as the mean pairwise distance;
    /// a near-zero value means single-byte changes barely move the output.
    pub min_avalanche_distance: FixedPoint,
    /// Mean geodesic distance under single-bit input flips.
    pub mean_avalanche_distance: FixedPoint,
    /// Per-component variance (spread in each dimension).
    pub component_variances: Vec<FixedPoint>,
    /// Mean of component variances.
    pub mean_variance: FixedPoint,
    /// Variance of component variances (low = uniform spread).
    pub variance_of_variances: FixedPoint,
    /// Maximum absolute Pearson correlation between any sampled dimension pair.
    pub max_abs_correlation: FixedPoint,
    /// Effective dimensionality via participation ratio.
    pub effective_dimensionality: FixedPoint,
    /// Whether determinism check passed.
    pub determinism_verified: bool,
}

// ============================================================================
// Core evaluation
// ============================================================================

/// Evaluate a hasher over a set of test domains.
///
/// # Errors
///
/// Propagates any error the hasher or the distance computation returns — most
/// commonly [`ToponError::DepthExceeded`] when a hierarchical hasher is given a
/// test path deeper than its `max_depth`, or [`ToponError::MathOverflow`] at a
/// narrow profile.
pub fn evaluate_hasher(
    hasher: &dyn GeometricHash,
    test_domains: &[&str],
) -> Result<EvalResult, ToponError> {
    let n = test_domains.len();
    let manifold = hasher.manifold_type();

    // Hash all domains
    let hashes: Vec<FixedVector> = test_domains
        .iter()
        .map(|d| hasher.hash(d.as_bytes()))
        .collect::<Result<_, _>>()?;

    let output_dim = hasher.output_dimension();
    let n_fp = FixedPoint::from_int(n as i32);

    // --- Determinism check ---
    let mut determinism_verified = true;
    for d in test_domains {
        let h1 = hasher.hash(d.as_bytes())?;
        let h2 = hasher.hash(d.as_bytes())?;
        if !vectors_equal(&h1, &h2) {
            determinism_verified = false;
            break;
        }
    }

    // --- Pairwise distances ---
    let (min_dist, mean_dist) = pairwise_distances(&hashes, manifold)?;

    // --- Avalanche ---
    let (min_avalanche, mean_avalanche) = avalanche_distances(hasher, test_domains, manifold)?;

    // --- Per-component statistics ---
    let (means, variances) = component_statistics(&hashes, output_dim, n_fp);

    // --- Variance of variances ---
    let mean_var = mean_of(&variances);
    let var_of_var = variance_of(&variances, mean_var);

    // --- Inter-dimension correlation ---
    let max_corr = max_abs_correlation(&hashes, &means, &variances, output_dim);

    // --- Effective dimensionality (participation ratio) ---
    let eff_dim = participation_ratio(&variances);

    Ok(EvalResult {
        approach_name: hasher.name().to_string(),
        min_pairwise_distance: min_dist,
        mean_pairwise_distance: mean_dist,
        min_avalanche_distance: min_avalanche,
        mean_avalanche_distance: mean_avalanche,
        component_variances: variances,
        mean_variance: mean_var,
        variance_of_variances: var_of_var,
        max_abs_correlation: max_corr,
        effective_dimensionality: eff_dim,
        determinism_verified,
    })
}

// ============================================================================
// Metric implementations
// ============================================================================

fn vectors_equal(a: &FixedVector, b: &FixedVector) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if a[i] != b[i] {
            return false;
        }
    }
    true
}

fn pairwise_distances(
    hashes: &[FixedVector],
    manifold: ManifoldType,
) -> Result<(FixedPoint, FixedPoint), ToponError> {
    let n = hashes.len();
    let mut min_dist: Option<FixedPoint> = None;
    let mut sum_dist = FixedPoint::ZERO;
    let mut count = 0i32;

    for i in 0..n {
        for j in (i + 1)..n {
            let d = geodesic_distance(&hashes[i], &hashes[j], manifold)?;
            if min_dist.is_none_or(|m| d < m) {
                min_dist = Some(d);
            }
            sum_dist += d;
            count += 1;
        }
    }

    let mean_dist = if count > 0 {
        sum_dist / FixedPoint::from_int(count)
    } else {
        FixedPoint::ZERO
    };

    Ok((min_dist.unwrap_or(FixedPoint::ZERO), mean_dist))
}

/// Avalanche metric: flip a single bit of one input byte (first, middle, and
/// last positions) and measure the geodesic distance between the original and
/// perturbed hashes. A healthy hash moves the output by the same order of
/// magnitude as the mean pairwise distance for ANY single-bit change.
fn avalanche_distances(
    hasher: &dyn GeometricHash,
    test_domains: &[&str],
    manifold: ManifoldType,
) -> Result<(FixedPoint, FixedPoint), ToponError> {
    let mut min_dist: Option<FixedPoint> = None;
    let mut sum_dist = FixedPoint::ZERO;
    let mut count = 0i32;

    for domain in test_domains {
        let bytes = domain.as_bytes();
        if bytes.is_empty() {
            continue;
        }
        let original = hasher.hash(bytes)?;

        let mut positions = vec![0, bytes.len() / 2, bytes.len() - 1];
        positions.dedup();

        for pos in positions {
            let mut mutated = bytes.to_vec();
            mutated[pos] ^= 0x01; // minimal possible input change: one bit
            let perturbed = hasher.hash(&mutated)?;
            let d = geodesic_distance(&original, &perturbed, manifold)?;
            if min_dist.is_none_or(|m| d < m) {
                min_dist = Some(d);
            }
            sum_dist += d;
            count += 1;
        }
    }

    let mean_dist = if count > 0 {
        sum_dist / FixedPoint::from_int(count)
    } else {
        FixedPoint::ZERO
    };

    Ok((min_dist.unwrap_or(FixedPoint::ZERO), mean_dist))
}

fn component_statistics(
    hashes: &[FixedVector],
    output_dim: usize,
    n_fp: FixedPoint,
) -> (Vec<FixedPoint>, Vec<FixedPoint>) {
    let mut means = vec![FixedPoint::ZERO; output_dim];
    let mut variances = vec![FixedPoint::ZERO; output_dim];

    // Compute means
    for d in 0..output_dim {
        let mut sum = FixedPoint::ZERO;
        for h in hashes {
            sum += h[d];
        }
        means[d] = sum / n_fp;
    }

    // Compute variances
    for d in 0..output_dim {
        let mut sum_sq = FixedPoint::ZERO;
        for h in hashes {
            let diff = h[d] - means[d];
            sum_sq += diff * diff;
        }
        variances[d] = sum_sq / n_fp;
    }

    (means, variances)
}

fn mean_of(values: &[FixedPoint]) -> FixedPoint {
    if values.is_empty() {
        return FixedPoint::ZERO;
    }
    let mut sum = FixedPoint::ZERO;
    for v in values {
        sum += *v;
    }
    sum / FixedPoint::from_int(values.len() as i32)
}

fn variance_of(values: &[FixedPoint], mean: FixedPoint) -> FixedPoint {
    if values.is_empty() {
        return FixedPoint::ZERO;
    }
    let mut sum_sq = FixedPoint::ZERO;
    for v in values {
        let diff = *v - mean;
        sum_sq += diff * diff;
    }
    sum_sq / FixedPoint::from_int(values.len() as i32)
}

fn max_abs_correlation(
    hashes: &[FixedVector],
    means: &[FixedPoint],
    variances: &[FixedPoint],
    output_dim: usize,
) -> FixedPoint {
    let n_fp = FixedPoint::from_int(hashes.len() as i32);
    let mut max_corr = FixedPoint::ZERO;

    // Sample dimension pairs — cap at 200 pairs for large dimensions
    let max_pairs = 200usize;
    let mut pairs_checked = 0usize;

    for d1 in 0..output_dim {
        if pairs_checked >= max_pairs {
            break;
        }
        let std_d1 = variances[d1].sqrt();
        if std_d1.is_zero() {
            continue;
        }

        for d2 in (d1 + 1)..output_dim {
            if pairs_checked >= max_pairs {
                break;
            }
            let std_d2 = variances[d2].sqrt();
            if std_d2.is_zero() {
                continue;
            }

            // Covariance
            let mut cov = FixedPoint::ZERO;
            for h in hashes {
                cov += (h[d1] - means[d1]) * (h[d2] - means[d2]);
            }
            cov /= n_fp;

            // Pearson correlation
            let corr = cov / (std_d1 * std_d2);
            let abs_corr = corr.abs();
            if abs_corr > max_corr {
                max_corr = abs_corr;
            }

            pairs_checked += 1;
        }
    }

    max_corr
}

fn participation_ratio(variances: &[FixedPoint]) -> FixedPoint {
    let mut sum_lambda = FixedPoint::ZERO;
    let mut sum_lambda_sq = FixedPoint::ZERO;

    for &v in variances {
        sum_lambda += v;
        sum_lambda_sq += v * v;
    }

    if sum_lambda_sq.is_zero() {
        return FixedPoint::ZERO;
    }

    (sum_lambda * sum_lambda) / sum_lambda_sq
}

// ============================================================================
// Collision probability analysis
// ============================================================================

/// Fractional bits for the current FixedPoint profile.
/// Determines the resolution per component.
fn frac_bits() -> u32 {
    if cfg!(table_format = "q16_16") { 16 }
    else if cfg!(table_format = "q32_32") { 32 }
    else if cfg!(table_format = "q64_64") { 64 }
    else if cfg!(table_format = "q128_128") { 128 }
    else if cfg!(table_format = "q256_256") { 256 }
    else { 64 } // fallback
}

/// Profile name string.
fn profile_name() -> &'static str {
    if cfg!(table_format = "q16_16") { "Q16.16 (realtime)" }
    else if cfg!(table_format = "q32_32") { "Q32.32 (compact)" }
    else if cfg!(table_format = "q64_64") { "Q64.64 (embedded)" }
    else if cfg!(table_format = "q128_128") { "Q128.128 (balanced)" }
    else if cfg!(table_format = "q256_256") { "Q256.256 (scientific)" }
    else { "unknown" }
}

/// Collision analysis for a geometric hash space.
///
/// All computations in log₂ space to handle astronomically large numbers.
pub struct CollisionAnalysis {
    /// FixedPoint profile fractional bits.
    pub frac_bits: u32,
    /// Intrinsic dimension n (degrees of freedom in the output).
    pub intrinsic_dim: u32,
    /// log₂(distinguishable positions) on the manifold.
    pub log2_positions: f64,
    /// log₂(birthday bound) — population at 50% collision probability.
    pub log2_birthday_bound: f64,
    /// Maximum input length in bytes.
    pub max_input_bytes: u32,
    /// log₂(possible inputs) — upper bound on distinct inputs.
    pub log2_possible_inputs: f64,
    /// Collision probability for specific population sizes.
    pub population_probs: Vec<(u64, f64)>,
}

/// Compute collision analysis for a given configuration.
///
/// `intrinsic_dim` is the number of free degrees of freedom:
/// - Sphere S^n: n (the sphere constraint removes 1 from n+1 ambient dims)
/// - Euclidean R^n: n
/// - Hyperbolic H^n: n (hyperboloid constraint removes 1)
///
/// `max_input_bytes`: maximum domain length (253 for DNS).
pub fn collision_analysis(
    intrinsic_dim: u32,
    _manifold: crate::ManifoldType,
    max_input_bytes: u32,
) -> CollisionAnalysis {
    let k = frac_bits();

    // Distinguishable positions:
    // Each of the intrinsic_dim free components has 2^k distinct values
    // in the range covered by sin/tanh folding (roughly [-1, 1]).
    // Total positions ≈ (2^k)^n = 2^{k*n}
    //
    // For Sphere: the unit-norm constraint removes 1 DOF, so n = dim
    // For Euclidean: components are in (-1,1) after folding, n = dim
    // For Hyperbolic: spatial components free, x₀ determined, n = dim
    let log2_positions = (k as f64) * (intrinsic_dim as f64);

    // Birthday bound: collision at 50% when m ≈ √(π/2 · N)
    // log₂(m) ≈ log₂(positions)/2 + log₂(√(π/2))
    // log₂(√(π/2)) ≈ 0.3257
    let log2_birthday = log2_positions / 2.0 + 0.3257;

    // Possible inputs: max_input_bytes bytes, each 0-255
    // But after normalization (lowercase, a-z 0-9 hyphen dot ≈ 38 chars):
    // Effective alphabet ≈ 38, so log₂(inputs) ≈ max_len * log₂(38) ≈ max_len * 5.25
    let log2_inputs_practical = (max_input_bytes as f64) * (38.0_f64).log2();

    // Collision probability for various population sizes:
    // P(collision) ≈ 1 - e^{-m²/(2N)}
    // In log space: exponent = -m² / (2 · 2^{log2_positions})
    //             = -2^{2·log₂(m) - 1 - log2_positions}
    let populations: Vec<u64> = vec![
        1_000,              // small network
        1_000_000,          // million domains
        1_000_000_000,      // billion domains
        350_000_000_000,    // ~all possible .com domains
    ];

    let population_probs: Vec<(u64, f64)> = populations
        .iter()
        .map(|&m| {
            let log2_m = (m as f64).log2();
            // exponent = m² / (2N) = 2^{2·log₂(m) - 1 - log₂(N)}
            let log2_exponent = 2.0 * log2_m - 1.0 - log2_positions;
            let prob = if log2_exponent > 0.0 {
                // exponent > 1 → collision nearly certain
                1.0
            } else if log2_exponent < -53.0 {
                // Below f64 precision → effectively 0
                // But we can give the log₂ of the probability
                // P ≈ m²/(2N) when small, so log₂(P) ≈ log2_exponent
                0.0
            } else {
                let exponent = 2.0_f64.powf(log2_exponent);
                1.0 - (-exponent).exp()
            };
            (m, prob)
        })
        .collect();

    CollisionAnalysis {
        frac_bits: k,
        intrinsic_dim,
        log2_positions,
        log2_birthday_bound: log2_birthday,
        max_input_bytes,
        log2_possible_inputs: log2_inputs_practical,
        population_probs,
    }
}

/// Print collision analysis report.
pub fn print_collision_analysis(
    dim: u32,
    manifold: crate::ManifoldType,
    max_input_bytes: u32,
) {
    let ca = collision_analysis(dim, manifold, max_input_bytes);
    let manifold_name = match manifold {
        crate::ManifoldType::Sphere => format!("S^{}", dim),
        crate::ManifoldType::Euclidean => format!("R^{}", dim),
        crate::ManifoldType::Hyperbolic => format!("H^{}", dim),
    };

    println!("Collision Probability Analysis");
    println!("==============================");
    println!();
    println!("  Profile:           {}", profile_name());
    println!("  Fractional bits:   {} bits per component", ca.frac_bits);
    println!("  Manifold:          {}", manifold_name);
    println!("  Intrinsic dim:     {}", ca.intrinsic_dim);
    println!("  Max input:         {} bytes (DNS limit: 253)", ca.max_input_bytes);
    println!();
    println!("  Distinguishable positions:  2^{:.0}", ca.log2_positions);
    println!("  Birthday bound (50%):       2^{:.1}  ({:.1e} domains)",
        ca.log2_birthday_bound,
        2.0_f64.powf(ca.log2_birthday_bound.min(1023.0)), // cap for f64
    );
    println!("  Possible inputs (practical): 2^{:.0}  (alphabet ~38)", ca.log2_possible_inputs);
    println!();
    println!("  Population        | P(collision)       | Safety margin (bits)");
    println!("  ------------------+--------------------+---------------------");

    for &(m, prob) in &ca.population_probs {
        let log2_m = (m as f64).log2();
        // Safety margin = how many bits of birthday bound remain unused
        let margin = ca.log2_birthday_bound - log2_m;
        let prob_str = if prob == 0.0 {
            let log2_prob = 2.0 * log2_m - 1.0 - ca.log2_positions;
            format!("< 2^{:.0} (≈0)", log2_prob)
        } else if prob >= 0.999 {
            "≈ 1.0 (certain)".to_string()
        } else {
            format!("{:.2e}", prob)
        };
        println!("  {:>16}  |  {:<18} |  {:.0}",
            format_population(m), prob_str, margin);
    }
    println!();

    // Recommendations
    if ca.log2_positions > ca.log2_possible_inputs {
        println!("  --> Output space (2^{:.0}) EXCEEDS input space (2^{:.0}).",
            ca.log2_positions, ca.log2_possible_inputs);
        println!("      Collisions are limited by hash quality, not space size.");
    } else {
        println!("  --> Input space (2^{:.0}) exceeds output space (2^{:.0}).",
            ca.log2_possible_inputs, ca.log2_positions);
        println!("      Pigeonhole: collisions guaranteed for >2^{:.0} inputs.",
            ca.log2_positions);
    }
    println!();

    // Dimension scaling table
    println!("  Dimension scaling (birthday bound for 50% collision):");
    println!("  -------------------------------------------------------");
    for &d in &[8, 16, 32, 64, 128, 256, 512] {
        let log2_pos = (ca.frac_bits as f64) * (d as f64);
        let log2_bb = log2_pos / 2.0 + 0.3257;
        let feasible = format!("2^{:.0}", log2_pos);
        let birthday = format!("2^{:.0}", log2_bb);
        println!("    dim {:>4}:  positions = {:<12}  birthday bound = {}", d, feasible, birthday);
    }
    println!();
}

fn format_population(m: u64) -> String {
    if m >= 1_000_000_000_000 {
        format!("{:.0}T", m as f64 / 1e12)
    } else if m >= 1_000_000_000 {
        format!("{:.0}B", m as f64 / 1e9)
    } else if m >= 1_000_000 {
        format!("{:.0}M", m as f64 / 1e6)
    } else if m >= 1_000 {
        format!("{:.0}K", m as f64 / 1e3)
    } else {
        format!("{}", m)
    }
}

// ============================================================================
// Display
// ============================================================================

/// Print a comparison table of evaluation results.
pub fn print_comparison(results: &[EvalResult]) {
    println!("╔══════════════════════════════════╦══════════════╦══════════════╦══════════════╦══════════════╦══════════════╦══════════════╦══════════════╦═══════╗");
    println!("║ Approach                         ║ Min Dist     ║ Mean Dist    ║ Aval Min     ║ Aval Mean    ║ Mean Var     ║ Max |Corr|   ║ Eff Dim      ║ Det?  ║");
    println!("╠══════════════════════════════════╬══════════════╬══════════════╬══════════════╬══════════════╬══════════════╬══════════════╬══════════════╬═══════╣");

    for r in results {
        println!(
            "║ {:<32} ║ {:>12} ║ {:>12} ║ {:>12} ║ {:>12} ║ {:>12} ║ {:>12} ║ {:>12} ║ {:<5} ║",
            r.approach_name,
            format_fp(r.min_pairwise_distance),
            format_fp(r.mean_pairwise_distance),
            format_fp(r.min_avalanche_distance),
            format_fp(r.mean_avalanche_distance),
            format_fp(r.mean_variance),
            format_fp(r.max_abs_correlation),
            format_fp(r.effective_dimensionality),
            if r.determinism_verified { "yes" } else { "NO" },
        );
    }

    println!("╚══════════════════════════════════╩══════════════╩══════════════╩══════════════╩══════════════╩══════════════╩══════════════╩══════════════╩═══════╝");
    println!();
    println!("Legend (all distances geodesic on the output manifold):");
    println!("  Min Dist  — minimum pairwise distance (higher = better collision resistance)");
    println!("  Mean Dist — mean pairwise distance");
    println!("  Aval Min  — minimum distance under a single-bit input flip");
    println!("              (should be the same order as Mean Dist; ~0 = broken diffusion)");
    println!("  Aval Mean — mean distance under single-bit input flips");
    println!("  Mean Var  — mean per-component variance (higher = better spread)");
    println!("  Max |Corr|— worst inter-dimension Pearson correlation (lower = more independent)");
    println!("  Eff Dim   — participation ratio (closer to output_dim = better)");
    println!("  Det?      — determinism check passed");
}

fn format_fp(v: FixedPoint) -> String {
    format!("{:.6}", v.to_f64())
}
