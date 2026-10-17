//! Shared test helpers.
//!
//! The key primitive here is a **profile-agnostic fingerprint** of a
//! `FixedVector`. Tests must not cast `FixedPoint::raw()` to a fixed integer
//! width: `raw()` is `i32`-backed at Q16.16 but `I256` at Q256.256, so
//! `raw() as i64` fails to compile at the wider profiles. Formatting the raw
//! value with `Debug` works at every profile and is exact (unlike `to_f64`,
//! which truncates to 53 mantissa bits and would let genuinely distinct
//! outputs compare equal).

#![allow(dead_code)] // each integration test binary uses a different subset

use g_math::fixed_point::imperative::FixedVector;
use topon::FixedPoint;

/// FNV-1a over the exact raw bits of every component.
///
/// Exact: two vectors share a fingerprint only if every component's raw
/// fixed-point value renders identically.
pub fn fingerprint_u64(v: &FixedVector<FixedPoint>) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    let byte = |b: u8, hash: &mut u64| {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(PRIME);
    };

    // Length is folded in so that a vector is never confused with a longer
    // one that happens to share a prefix.
    for b in (v.len() as u64).to_le_bytes() {
        byte(b, &mut hash);
    }
    for i in 0..v.len() {
        for b in format!("{:?}", v[i].raw()).into_bytes() {
            byte(b, &mut hash);
        }
        byte(b'|', &mut hash);
    }
    hash
}

/// Hex rendering of [`fingerprint_u64`], for pinning in golden tests.
pub fn fingerprint(v: &FixedVector<FixedPoint>) -> String {
    format!("{:016x}", fingerprint_u64(v))
}

/// Name of the compiled precision profile, for test messages.
pub fn profile_name() -> &'static str {
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
