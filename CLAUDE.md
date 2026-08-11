# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Topon is a deterministic geometric hashing library (part of the Geodineum ecosystem): it maps byte input to points on Riemannian manifolds (sphere, Euclidean, hyperbolic) so hashed values have meaningful geometry — geodesic distances, containment, hierarchy. All arithmetic is fixed-point via the `g_math` crate; there is zero floating-point in any hash path, which is what makes output bit-identical across platforms.

## Commands

Precision profile is selected at **compile time** via the `GMATH_PROFILE` env var (build.rs turns it into a `table_format` cfg; changing it triggers a rebuild). Values: `realtime` (Q16.16), `compact`/`fast` (Q32.32), `embedded` (Q64.64, the default and recommended), `balanced` (Q128.128), `scientific` (Q256.256).

```bash
cargo build                                  # defaults to GMATH_PROFILE=embedded
cargo test --release                         # all tests (release: the 150k collision scan is #[ignore]d in debug)
cargo test --test hierarchical               # one test file
cargo test ancestry_monotonicity             # single test by name
GMATH_PROFILE=embedded cargo run --example compare    # evaluation harness (all 3 modes, metrics)
GMATH_PROFILE=embedded cargo run --release --example bench   # throughput figures quoted in the README
GMATH_PROFILE=realtime GMATH_FRAC_BITS=10 cargo run --release --example vocab_bench   # 150k-path taxonomy walk benchmark
```

There is no lint/format config beyond default `cargo clippy` / `cargo fmt`.

## Critical dependency rule

`g_math` must stay a **registry dependency**, never `path = "../gMath"`. A path dep is a distinct crate from the registry one even at the same version, so a downstream consumer that also pulls g_math through another registry crate ends up with two `FixedVector` types that do not unify. For cross-repo work against an unpublished gMath, put a `[patch.crates-io]` in the *consuming binary's* manifest instead. The full rationale is in the comment in `Cargo.toml` — keep it there.

## Architecture

Three layers, composed bottom-up (`src/`):

- **`core/`** — shared mathematical primitives. `equidistribution.rs` is the heart: `WeylAccumulator` implements Weyl equidistribution (`{n·√p mod 1}` for per-dimension distinct primes) with `sin()` "wave folding" to produce equidistributed output in [-1, 1]. Accumulator `+`/`*` deliberately use g_math's *wrapping* ops (the wrap is the modular reduction); transcendental folds use `try_` variants and propagate overflow as `ToponError::MathOverflow`. `manifold.rs` handles projection and geodesic distance; `constants.rs` generates the √p bank; `normalize.rs` normalizes domain strings.
- **`mixing/`** — byte-mixing strategies implementing the crate-private `MixingStrategy` trait (bytes → raw components in ~(-1,1), pre-projection): `irrational` (domain mode), `chaotic` (speed mode's expanding `x ← frac(x·√P + injection)` sponge), and `geodesic` (special case — produces hyperboloid points directly instead of raw components).
- **`mode/`** — the user-facing hashers, each composing a mixer with manifold projection to implement the public `GeometricHash` trait (`lib.rs`): `DomainHasher` (irrational mixing, default S^128), `SpeedHasher` (throughput, arbitrary bytes), `HierarchicalHasher` (tree-structured input → hyperbolic space).

`eval.rs` is the metrics harness (collision, distribution, correlation, effective dimensionality, avalanche — all distances geodesic, never chordal). `compat.rs` is a free-function `domain_to_coordinates()` convenience wrapper.

Every `GeometricHash` gets `hash_domain()` for free via the blanket `DomainHash` impl.

## Invariants that CI enforces

- **No panics from safe API.** Every hasher constructor validates its `CoordinateConfig` (`1..=MAX_DIMENSION`, where `MAX_DIMENSION = 1144` is the prime-table bound) and returns `ToponError::InvalidConfig`. `core::constants` is fallible for the same reason. `domain_hasher()` is the one infallible constructor; its `expect` is pinned by `default_config_is_valid`.
- **Hash output is a public contract.** `tests/golden.rs` pins fingerprints for all three modes at embedded/Q64.64. If output changes, that test fails — do not re-pin it without a deliberate minor-version bump and a CHANGELOG entry (see the README's hash stability policy).
- **All five profiles must build and pass**, hence CI's profile matrix. Never cast `FixedPoint::raw()` to a fixed integer width in tests — it is `I256` at Q256.256. Use `tests/common::fingerprint_u64` instead.

## Hierarchical mode invariants

The geodesic walk (`mixing/geodesic.rs`) is the most subtle part; its constraints exist because `cosh(r)` grows exponentially while fixed-point range grows linearly in bits:

- **`WalkLimits`** bounds every walk: radius budget (default 12) keeps the hyperboloid time component `cosh(r)` in range at every profile (Q22.10 overflows at r ≈ 15.2); depth cap (default 8) makes over-deep paths fail loudly with `ToponError::DepthExceeded` — never silently truncate, deep paths are an upstream taxonomy bug.
- **`LabelOrder`** matters: `RootLast` for domain names (`sub.example.com`, root `com`), `RootFirst` for taxonomy paths (`science/physics/quantum`) via `HierarchicalHasher::taxonomy()`.
- The **incremental walk API** (`walk_origin` / `walk_step` / `finish`, with `GeodesicWalk` cloneable at interior nodes) exists so a whole taxonomy costs one step per tree *node* instead of one full walk per *leaf* — this is how a downstream vocabulary loader positions 150k tokens. Don't break prefix-sharing semantics: `tests/hierarchical.rs::incremental_walk_matches_full_hash` pins incremental output to full-hash output.

Geometric properties (ancestry monotonicity, sibling separation, hyperboloid constraint) are all pinned by `tests/hierarchical.rs` — treat those tests as the spec.
