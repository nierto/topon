//! Chaotic mixing — expanding multiply-mod-1 sponge (speed mode).
//!
//! Absorbs input into a small fixed-width state (LANES fixed-point lanes,
//! independent of output dimension) via the expanding map
//!
//! ```text
//! lane ← frac(lane · √P + injection)
//! ```
//!
//! `frac(x · √P)` is a beta-transformation with irrational base — ergodic
//! with positive Lyapunov exponent ln(√P), so a perturbation *grows* every
//! step instead of decaying. On Q-format storage the same operation is
//! exactly Knuth's multiplicative hashing, which is why it stays
//! well-behaved in exact binary fixed point (piecewise-linear maps like the
//! tent map collapse there: each doubling shifts in a zero bit).
//!
//! Cost: O(len + dim) — LANES multiply-adds per input byte, zero
//! transcendentals in the absorb loop, and ONE `sin()` per output dimension
//! in the squeeze step.

use g_math::fixed_point::imperative::FixedPoint;

use crate::core::constants::{constant_bank, sqrt_nth_prime};
use crate::core::equidistribution::{frac, wave_fold};
use crate::ToponError;
use super::MixingStrategy;

/// Number of state lanes. Total state = LANES × frac_bits of true entropy.
const LANES: usize = 8;

/// Cross-lane diffusion rounds after absorption, so the final input bytes
/// avalanche through the full state before squeezing.
const FINALIZE_ROUNDS: usize = 4;

/// Prime-table offset for the per-lane multipliers: √p₉₆..√p₁₀₃ ≈ 22.4–23.9,
/// giving ~4.5 bits of expansion per absorb step.
const MULTIPLIER_BASE: usize = 96;

/// Speed-optimized mixing via an expanding multiply-mod-1 sponge.
pub struct ChaoticMixer {
    /// √p bank used for byte injection and the squeeze step.
    constants: Vec<FixedPoint>,
    /// Per-lane expanding multipliers (large √p, pairwise distinct).
    multipliers: [FixedPoint; LANES],
    /// 1/257, the byte-normalization factor. Precomputed because division is
    /// ~200× the cost of a multiply in fixed point, and this value is
    /// constant across every hash.
    inv_257: FixedPoint,
}

impl ChaoticMixer {
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `num_constants` exceeds the prime table.
    pub fn new(num_constants: usize) -> Result<Self, ToponError> {
        let mut multipliers = [FixedPoint::ZERO; LANES];
        for (i, m) in multipliers.iter_mut().enumerate() {
            *m = sqrt_nth_prime(MULTIPLIER_BASE + i)?;
        }
        Ok(ChaoticMixer {
            constants: constant_bank(num_constants.max(LANES))?,
            multipliers,
            inv_257: FixedPoint::one() / FixedPoint::from_int(257),
        })
    }

    /// Deterministic nonzero lane seeds: frac(√P_i · (i + 1)).
    fn seed_lanes(&self) -> [FixedPoint; LANES] {
        let mut lanes = [FixedPoint::ZERO; LANES];
        for (i, lane) in lanes.iter_mut().enumerate() {
            *lane = frac(self.multipliers[i] * FixedPoint::from_int(i as i32 + 1));
        }
        lanes
    }
}

impl MixingStrategy for ChaoticMixer {
    fn mix(&self, input: &[u8], num_components: usize) -> Result<Vec<FixedPoint>, ToponError> {
        let n = self.constants.len();
        let inv_257 = self.inv_257;
        let mut lanes = self.seed_lanes();

        // Phase 1: Absorb — zero transcendentals.
        // Every byte perturbs every lane; the expanding multiplier then
        // amplifies that perturbation by ~√P per subsequent step, so early
        // bytes influence the state MORE than late ones, never less.
        for (pos, &byte) in input.iter().enumerate() {
            let byte_norm = FixedPoint::from_int(byte as i32 + 1) * inv_257;
            for (i, lane) in lanes.iter_mut().enumerate() {
                let injection = byte_norm * self.constants[(pos + i) % n];
                *lane = frac(*lane * self.multipliers[i] + injection);
            }
        }

        // Phase 2: Finalize — cross-lane diffusion so the last bytes of the
        // input avalanche through all lanes. Still zero transcendentals.
        for _ in 0..FINALIZE_ROUNDS {
            for i in 0..LANES {
                let neighbor = lanes[(i + 1) % LANES];
                lanes[i] = frac(lanes[i] * self.multipliers[i] + neighbor);
            }
        }

        // Phase 3: Squeeze — ONE sin() per output dimension. Each dimension
        // pairs the lanes with a shifted window of the √p bank; since the
        // bank holds at least num_components constants, the windows are
        // distinct for every dimension regardless of bank size.
        let mut components = Vec::with_capacity(num_components);
        for d in 0..num_components {
            let mut acc = FixedPoint::ZERO;
            for (i, lane) in lanes.iter().enumerate() {
                acc += *lane * self.constants[(d + i) % n];
            }
            components.push(wave_fold(acc * self.constants[d % n])?);
        }

        Ok(components)
    }
}
