# Changelog

All notable changes to topon are documented here.

Versioning tracks hash output as well as API (see "Hash stability policy" in
the README): any release that changes hash output for any input bumps the
minor version; patch releases are bit-identical.

## 0.2.0 - 2026-08-31

### Changed

- **Hash output moved: g_math 0.4.33 → 0.6 (0.6.1).** The crossing includes
  g_math 0.5.0's rounding change (binary multiply now rounds to nearest instead
  of flooring; divide rounds to nearest instead of truncating), so fingerprints
  that pass through inexact products or quotients differ in the last place
  from 0.1.x. 3 of 15 golden fingerprints changed on the embedded
  profile; the table in `tests/golden.rs` was regenerated with the tool and
  pinned. Per the hash stability policy this is a minor bump. Also picks up
  the 0.5.0 `acos` fix on the realtime profile. 0.6.x itself is additive
  (certified intervals, exact predicates, fused quadratic form) and changes no
  result.

## 0.1.0 — 2026-08-07

Initial release. Licensed Apache-2.0.

Hardening applied before first publish:

- **Hasher constructors are fallible.** `DomainHasher::new`, `SpeedHasher::new`,
  `HierarchicalHasher::{new, with_separator, taxonomy, with_options}`,
  `speed_hasher`, `hierarchical_hasher` and `compat::domain_to_coordinates`
  return `Result<_, ToponError>`. Previously a `dimension` above the prime
  table's 1,145 entries panicked from safe code; it now returns
  `ToponError::InvalidConfig`. `domain_hasher()` stays infallible — the default
  config is valid by construction, pinned by a test.
- Added `CoordinateConfig::{new, validate}` and `MAX_DIMENSION` (1144).
  `dimension = 0` is rejected instead of yielding an empty vector.
- `ToponError` is `#[non_exhaustive]` and derives `Clone`/`PartialEq`/`Eq`;
  `ManifoldType` derives `Eq`/`Hash`; `CoordinateConfig` derives
  `PartialEq`/`Eq`.
- Removed the unused `mixing::lattice` module. It was unreachable (its only
  trait impl was crate-private), untested, and divided by zero for a
  zero-length constant bank.
- Removed `core::equidistribution::create_accumulator_bank` (unused).
- Added golden-vector tests (`tests/golden.rs`) pinning hash output for all
  three modes, the compat shim, and the normalization path, so an accidental
  output change fails CI instead of silently breaking stored coordinates.
- The 150k-path collision scan now keys on a profile-agnostic fingerprint;
  the whole suite builds and passes at all five profiles (previously
  `balanced` and `scientific` failed to compile the tests).
- `eval::evaluate_hasher` returns `Result` instead of panicking on a hasher
  error. This was reachable: `examples/compare.rs` scores a 9-label domain,
  which exceeds the hierarchical default depth cap, and aborted with
  `hash failed`. The example now raises the cap explicitly for the comparison.
- Added `examples/bench.rs`, the source of the README's performance figures.
- CI covers all five precision profiles (was two), and runs both examples.
- Hoisted redundant work out of two hot loops, both **bit-exact** (pinned by
  `tests/golden.rs`): `IrrationalMixer` no longer recomputes `byte_fp * pos_fp`
  once per output dimension (129× redundant at the S^128 default), and
  `ChaoticMixer` precomputes its `1/257` byte-normalization factor at
  construction instead of on every `mix()` call.
- **Manifold projection rewritten against g_math 0.4.33.** Sphere projection
  computes `1/‖v‖` once via `FixedPoint::try_inv_sqrt` and multiplies, instead
  of `normalized()`'s one division per component; the hyperboloid lift uses
  `fused::sqrt_sum_sq` so the squares, the accumulation and the square root
  stay at compute tier with a single rounding. Projection had measured at
  25–75% of total hash time and was the dominant cost in every sphere mode.

  | Config | before | after | |
  |---|---:|---:|---:|
  | domain S^16 | 70.7 µs | 64.5 µs | 1.1× |
  | domain S^128 | 438.6 µs | 330.8 µs | 1.3× |
  | speed S^16 | 39.3 µs | 31.9 µs | 1.2× |
  | speed S^128 | 159.0 µs | 83.4 µs | **1.9×** |

  This **changes hash output on the sphere** (`v * (1/‖v‖)` rounds differently
  from `v / ‖v‖`). Hyperbolic and Euclidean output is bit-identical — the
  fused lift happened to produce the same rounded values. Landed before first
  publish precisely so the faster output is the one frozen by the stability
  policy; after 0.1.0 this would have required a 0.2.0 and a migration for
  anyone with stored coordinates.
- Minimum g_math is now 0.4.33 (for `inv_sqrt` / `fused::inv_sqrt_sum_sq`).

- Three hashing modes behind the `GeometricHash` trait:
  - `DomainHasher` — domain names via irrational mixing (per-dimension √p
    constants, `sin()` wave folding), default S^128.
  - `SpeedHasher` — arbitrary bytes via an expanding multiply-mod-1 sponge;
    zero transcendentals in the absorb loop.
  - `HierarchicalHasher` — tree-structured input (domains, taxonomy paths)
    mapped into hyperbolic space via geodesic walks, with `WalkLimits`
    (depth cap 8, radius budget 12) and loud `DepthExceeded` failure.
- Incremental walk API (`walk_origin` / `walk_step` / `finish`) with cloneable
  `GeodesicWalk` for one-step-per-node taxonomy positioning.
- Evaluation harness (`eval`) — collision, distribution, correlation,
  effective dimensionality, avalanche; all distances geodesic.
- `compat::domain_to_coordinates` — free-function form of domain hashing, for
  callers that want a one-shot call rather than a reusable hasher.
- Compile-time precision profiles via `GMATH_PROFILE` (Q16.16 – Q256.256).
