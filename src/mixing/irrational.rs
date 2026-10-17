//! Irrational mixing — the proven-quality strategy.
//!
//! Each dimension gets a unique `√p` constant. Input bytes are mixed via
//! dual-constant addressing with periodic `sin()` folding every 4 bytes.
//! Best empirical metrics of all strategies (eff dim 16.3/17, min dist 0.74
//! on S^16).

use crate::FixedPoint;

use crate::core::constants::constant_bank;
use crate::core::equidistribution::WeylAccumulator;
use crate::core::wrapping::{wrapping_from_int, wrapping_mul};
use crate::ToponError;
use super::MixingStrategy;

const FOLD_PERIOD: usize = 4;

/// Irrational mixing with per-dimension `√p` constants.
pub struct IrrationalMixer {
    constants: Vec<FixedPoint>,
}

impl IrrationalMixer {
    /// # Errors
    ///
    /// [`ToponError::InvalidConfig`] if `num_constants` exceeds the prime table.
    pub fn new(num_constants: usize) -> Result<Self, ToponError> {
        Ok(IrrationalMixer {
            constants: constant_bank(num_constants.max(1))?,
        })
    }

    /// Mix one output dimension from the shared per-position products.
    ///
    /// `positional` holds `byte_fp * pos_fp` for each input position — see
    /// [`MixingStrategy::mix`] for why it is precomputed.
    fn mix_dimension(
        &self,
        positional: &[FixedPoint],
        dim_index: usize,
    ) -> Result<FixedPoint, ToponError> {
        let n = self.constants.len();
        // Reuse the precomputed √p from the bank — no per-hash sqrt().
        let mut acc = WeylAccumulator::with_constant(self.constants[dim_index % n], dim_index);

        for (pos, &base) in positional.iter().enumerate() {
            // Dual-constant addressing: coprime strides avoid alignment
            let secondary = self.constants[(pos * 7 + dim_index * 3 + 1) % n];
            let contribution = wrapping_mul(base, secondary);

            acc.absorb_with_periodic_fold(contribution, pos, FOLD_PERIOD)?;
        }

        Ok(acc.fold()?)
    }
}

impl MixingStrategy for IrrationalMixer {
    fn mix(&self, input: &[u8], num_components: usize) -> Result<Vec<FixedPoint>, ToponError> {
        // `byte_fp * pos_fp` depends only on the input, not on the output
        // dimension, so it is hoisted out of the per-dimension loop: at the
        // default S^128 it was being recomputed 129 times per input byte.
        //
        // Bit-exact with the previous per-dimension form: `*` is
        // left-associative, so `byte_fp * pos_fp * secondary` already grouped
        // as `(byte_fp * pos_fp) * secondary`. Same operands, same order, same
        // rounding — pinned by tests/golden.rs.
        let positional: Vec<FixedPoint> = input
            .iter()
            .enumerate()
            .map(|(pos, &byte)| {
                wrapping_mul(wrapping_from_int(byte as i32 + 1), wrapping_from_int(pos as i32 + 1))
            })
            .collect();

        (0..num_components)
            .map(|d| self.mix_dimension(&positional, d))
            .collect()
    }
}
