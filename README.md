<p align="center">
  <a href="https://geodineum.com">
    <img src="https://geodineum.com/wp-content/uploads/2026/07/logo_geodineum_launch.png" alt="Geodineum" width="128">
  </a>
</p>

# topon

Every input gets a place, and places have distances.

Built by **Niels Erik Toren** · published as `topon` on [crates.io](https://crates.io/crates/topon)

[![Crates.io](https://img.shields.io/crates/v/topon)](https://crates.io/crates/topon)
[![Documentation](https://docs.rs/topon/badge.svg)](https://docs.rs/topon)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

## What it is

A traditional hash gives you an opaque bit string. Two hashes are either equal
or they are not; there is no notion of *nearby*. `topon` maps input to a point
on a Riemannian manifold, so the output carries geometry: geodesic distance,
containment, hierarchy, interpolation.

The name is from τόπος — *place*. A toponym is a place-name, which is exactly
what the primary mode computes: it turns names into positions.

Everything is fixed-point arithmetic via [`g_math`](https://github.com/nierto/gMath).
No `f32`/`f64` appears in any hash path, so output is bit-identical on every
architecture, every call, forever — the property that makes stored coordinates
meaningful across machines and across time.

It is a library, not a service: pure computation, no I/O, no ambient state. It
is one component of the [Geodineum](https://github.com/nierto) ecosystem.

**This is not a cryptographic hash.** It makes no preimage, second-preimage, or
adversarial-collision claims. It is a coordinate system for data.

## Public build surface

Four items; everything beneath them is internal.

- **`GeometricHash`** — the core trait. `hash(&[u8]) -> Result<FixedVector<FixedPoint>, ToponError>`,
  plus `output_dimension()` and `manifold_type()`. `topon::FixedPoint` is the
  g_math format this build hashes in (`Q64_64` by default; see precision
  profiles below).
- **`DomainHash`** — blanket-implemented for every `GeometricHash`, adding
  `hash_domain(&str)` which normalizes (lowercase, trim, strip trailing dots)
  before hashing.
- **The three hashers** — `DomainHasher`, `SpeedHasher`, `HierarchicalHasher`,
  each constructed from a `CoordinateConfig { dimension, manifold_type }`.
- **`eval`** — the metrics harness: collision, distribution, inter-dimension
  correlation, effective dimensionality, avalanche. All distances geodesic,
  never chordal.

## Modes

### Domain mode — `DomainHasher`

Production-quality domain name hashing. Irrational mixing with a distinct
`√pᵢ` per dimension and periodic `sin()` folding every 4 bytes. Best
distribution metrics of the three: effective dimensionality above 96% of the
theoretical maximum.

### Speed mode — `SpeedHasher`

Throughput-oriented, for arbitrary bytes. Absorbs input into a fixed 8-lane
state via the expanding map `x ← frac(x·√P + injection)` — `O(len + dim)` with
zero transcendentals in the absorb loop and one `sin()` per output dimension.
The map is a beta-transformation with irrational base: ergodic, Lyapunov
exponent `ln(√P) > 0`, so a one-byte change anywhere avalanches rather than
decays.

### Hierarchical mode — `HierarchicalHasher`

Tree-structured input — domain names, taxonomy paths, URIs — walked into
hyperbolic space along geodesics. Parents sit near the origin, children step
outward via `exp_map`. Two label orders: root-last for domain names
(`sub.example.com`), root-first for taxonomy paths via
`HierarchicalHasher::taxonomy()`.

## Quick start

```rust
use topon::{domain_hasher, DomainHash};

let hasher = domain_hasher();                       // S^128, infallible
let coords = hasher.hash_domain("example.geo")?;
```

```rust
use topon::{CoordinateConfig, GeometricHash, ManifoldType, SpeedHasher};

let config = CoordinateConfig::new(16, ManifoldType::Sphere)?;
let hasher = SpeedHasher::new(config)?;
let coords = hasher.hash(b"arbitrary bytes")?;
```

```rust
use topon::{CoordinateConfig, HierarchicalHasher, ManifoldType, geodesic_distance};

let config = CoordinateConfig::new(16, ManifoldType::Hyperbolic)?;
let tax = HierarchicalHasher::taxonomy(config)?;

let parent = tax.hash(b"science")?;
let child  = tax.hash(b"science/physics")?;
let other  = tax.hash(b"art/music")?;

// The child is nearer its parent than an unrelated branch is.
assert!(geodesic_distance(&parent, &child, ManifoldType::Hyperbolic)?
      < geodesic_distance(&parent, &other, ManifoldType::Hyperbolic)?);
```

Positioning a whole taxonomy shares prefixes — one step per tree *node*
instead of one full walk per *leaf*:

```rust
let root = tax.walk_step(&tax.walk_origin(), b"science")?;
for leaf in ["physics", "biology", "chemistry"] {
    let point = tax.finish(&tax.walk_step(&root, leaf.as_bytes())?)?;
}
```

## Notes

Atomic notes: one claim each, stated so it can be checked or refuted. Where a
claim is empirical the evidence is named.

**Geometric hashing and cryptographic hashing are different tools, not
competitors.** SHA-256 answers "are these the same?"; `topon` answers "how far
apart are these?". Nothing here resists an adversary who is choosing inputs.
Use both, for different questions.

**Determinism here is structural, not disciplined.** It does not come from
being careful with floating point — it comes from there being none. `core/`,
`mixing/`, `mode/` and `compat` contain no `f32` or `f64` token at all, so
there is no rounding-mode, FMA-contraction, or x87-extended-precision variance
to control. The only `f64` in the crate is in `eval`'s collision *reporting*,
which describes numbers like 2^1024 in log₂ space and never touches a hash.

**Uniformity is inherited from a theorem, not tuned empirically.** Weyl's
equidistribution theorem gives that `{n·α mod 1}` is equidistributed for
irrational `α`. Using `α = √pᵢ` for distinct primes adds joint equidistribution
across dimensions, because square roots of distinct primes are algebraically
independent. The output fills the product space; it is not fitted to do so.

**`sin()` is a bounding operation, not a decoration.** The accumulator grows
without bound under irrational multiplication; folding through `sin()` maps it
into (−1, 1) while preserving density and non-periodicity. Boundedness is what
makes projection onto a manifold well-defined.

**Expanding maps survive fixed point; contracting ones do not.** Speed mode
uses `x ← frac(x·√P)`, which in Q-format storage is exactly multiplicative
(Knuth) integer hashing. Piecewise-linear chaotic maps like the tent map shift
in a zero bit per iteration and collapse to zero in exact binary arithmetic.
`tests/basic.rs::speed_hasher_avalanche_floor` is the regression test for that
failure, which a previous mixer exhibited.

**Trees fit hyperbolic space because volume grows exponentially with radius.**
A branching hierarchy needs room that grows like its node count. Euclidean
space grows polynomially and forces siblings together; hyperbolic space does
not. This is why hierarchical mode is hyperbolic-only regardless of the
configured manifold.

**Ancestry monotonicity is a test, not an aspiration.** `d(parent, child) <
d(parent, unrelated)` is pinned in `tests/hierarchical.rs::ancestry_monotonicity`
across six taxonomy cases, alongside sibling separation and branch containment.
Treat those tests as the specification of the geometry.

**A path deeper than the cap is an upstream bug, so it fails loudly.** Walks
return `ToponError::DepthExceeded` rather than truncating. Silent truncation
would map two different paths to the same point — a collision manufactured by
the library rather than found in the data.

**The walk radius budget is arithmetic necessity, not a tunable.** The
hyperboloid time component is `cosh(r)`, which grows exponentially in `r`,
while fixed-point range grows linearly in bits. At Q22.10 `cosh(r)` overflows
near r ≈ 15.2. The default budget of 12 bounds the walk at every profile; the
harmonic step schedule uses at most H₈ ≈ 2.72 of it, so the clamp is a safety
invariant rather than something normal operation encounters.

**Projection dominated cost, not mixing — which is why normalization is a
reciprocal-multiply.** Because `ManifoldType::Euclidean` is a pass-through, it
isolates mixing exactly: sphere projection measured 25–75% of total hash time.
A fixed-point division costs roughly 200 multiplies, so `normalized()`'s
per-component division was the bottleneck. Computing `1/‖v‖` once via
`inv_sqrt` and multiplying made speed mode at S^128 1.9× faster.

**Hash output is a public contract, and the contract is enforced.**
`tests/golden.rs` pins fingerprints for all three modes, all three manifolds,
the compat surface and the normalization path. A determinism test that hashes
twice in one process cannot detect an algorithm change; golden vectors can, and
this suite has been verified to fail when the fold period is perturbed.

**Precision is a compile-time property of the output, not of the API.** The
same input under `GMATH_PROFILE=realtime` and `=embedded` produces different
coordinates. Coordinates are comparable only between builds at the same
profile. This is a consequence of storing them at all: a Q16.16 value cannot
hold a Q64.64 position.

**Nothing in the safe API panics.** Every constructor validates its
`CoordinateConfig` and returns `ToponError::InvalidConfig`; the crate carries
`#![forbid(unsafe_code)]`. The single `expect` is in `domain_hasher()`, on a
hardcoded default whose validity is pinned by a test.

## Contract

Hash *values* are part of the public contract, not just the API — anyone who
stores coordinates is broken by a change to mixing constants, walk schedules,
or normalization even when the code still compiles.

- Until 1.0, output may change between releases. **Any release that changes
  output for any input bumps the minor version**; patch releases are
  bit-identical.
- From 1.0, changed output for the same `(input, config, GMATH_PROFILE)` is a
  breaking change requiring a major bump.
- Output always depends on the compile-time profile. Values are comparable only
  across builds using the same one.

`tests/golden.rs` enforces this. If it fails, output moved.

## Capabilities

**Manifolds.**

| Manifold | Model | Volume growth | Output dimension |
|---|---|---|---|
| **Sphere** S^n | Unit sphere in R^{n+1} | Finite (concentrates at equator) | dim + 1 |
| **Euclidean** R^n | Flat space | Polynomial in radius | dim |
| **Hyperbolic** H^n | Hyperboloid `x₀² − Σxᵢ² = 1` | Exponential in radius | dim + 1 |

**Collision space.** Each hash occupies a distinct position; as dimension
grows, collision becomes combinatorially infeasible.

| Profile | Bits/component | Dim 8 | Dim 16 | Dim 128 |
|---|---|---|---|---|
| Compact (Q32.32) | 32 | 2^256 | 2^512 | 2^4096 |
| **Embedded (Q64.64)** | **64** | **2^512** | **2^1024** | **2^8192** |
| Balanced (Q128.128) | 128 | 2^1024 | 2^2048 | 2^16384 |

At the recommended configuration (Q64.64, dim 16) the birthday bound is 2^512.
At dim 8 it is 2^256, matching SHA-256's output space.

**Precision profiles.** Selected at compile time via `GMATH_PROFILE`; `build.rs`
turns it into a `cfg` and into the `topon::FixedPoint` type alias, so changing
it triggers a rebuild. Since g_math 0.7 the format is a type and g_math reads
no environment: topon's `build.rs` keeps the variable and maps it to the type.
On `realtime`, `GMATH_FRAC_BITS` (2..=30, default 16) sets the split, as in
g_math 0.6.

| Value | Format | |
|---|---|---|
| `realtime` | Q16.16 (`Fixed<i32, GMATH_FRAC_BITS>`) | narrowest, fastest |
| `compact` / `fast` | Q32.32 (`Q32_32`) | |
| `embedded` | Q64.64 (`Q64_64`) | **default and recommended** |
| `balanced` | Q128.128 (`Q128_128`) | |
| `scientific` | Q256.256 (`Q256_256`) | widest |

**Evaluation harness.**

```bash
GMATH_PROFILE=embedded cargo run --release --example compare
```

## Performance

`topon` trades throughput for determinism and geometry. Fixed-point with 0 ULP
transcendentals costs roughly two orders of magnitude against a bit-mixing hash
like BLAKE3. Budget accordingly.

Measured on x86-64, `GMATH_PROFILE=embedded` (Q64.64), `--release`. Reproduce:

```bash
GMATH_PROFILE=embedded cargo run --release --example bench
```

**Per-hash latency** (20-byte input, sphere):

| Dimension | Domain mode | Speed mode |
|---|---|---|
| S^8 | 43.5 µs | 29.8 µs |
| S^16 | 64.5 µs | 31.9 µs |
| S^32 | 98.5 µs | 38.3 µs |
| S^128 | 330.8 µs | 83.4 µs |

At S^16 that is ~16.2k domain hashes/sec and ~31.7k speed hashes/sec; at the
S^128 default, ~3.0k/sec and ~12.0k/sec.

**Bulk throughput** (speed mode, S^16): 1.7 MB/s at 64 B, rising to ~5.0 MB/s
once the per-dimension squeeze amortizes (16 KB and above).

**Hierarchical mode** (H^16, three-level path): 154.4 µs for a full path hash
versus 57.5 µs for an incremental leaf step on an already-walked prefix — 2.7×
per token, widening with depth and fan-out.

Domain mode is `O(dimension × input length)`: one Weyl accumulator per output
component. Speed mode is `O(input length + dimension)`, which is why its
advantage grows with dimension (1.5× at S^8, 4.0× at S^128).

## Limits worth knowing

- **Not a cryptographic hash.** No adversarial guarantees of any kind.
- **`domain_hasher()` defaults to S^128 for quality, not speed.** It makes 129
  passes over the input. For volume, use a smaller dimension or `SpeedHasher`.
- **Construct hashers once and reuse them.** The constructor precomputes one
  square root per output dimension; `DomainHasher::new` at S^128 costs ~3 ms.
- **`dimension` is capped at `MAX_DIMENSION` = 1144**, the size of `g_math`'s
  prime table. Zero or larger returns `ToponError::InvalidConfig`.
- **Hierarchy depth is capped at 8 by default.** Deeper paths return
  `DepthExceeded`; raise it via `WalkLimits` only with the `cosh(r)` range
  argument above in mind.
- **Coordinates are not portable across profiles.** Same input, different
  `GMATH_PROFILE`, different output.
- **Switching profiles needs a clean incremental cache**
  (`rm -rf target/*/incremental/`), or stale `cfg` artifacts can crash the build.

## Collaborate

Contributions are welcome. Open issues and pick up work on the ecosystem board
at [geodineum.com](https://geodineum.com); issues tagged `good-first-issue` are
a good place to start.

- Fork, branch, and open a pull request against `main`.
- Any change that moves hash output must update `tests/golden.rs` and
  `CHANGELOG.md` in the same commit, and bump the minor version.
- New geometric properties belong in `tests/hierarchical.rs` as executable
  specification, not in prose.

## Author & support

Built by **Niels Erik Toren**.

If you want to support the work:

| Currency | Address |
|---|---|
| Bitcoin (BTC) | `bc1qwf78fjgapt2gcts4mwf3gnfkclvqgtlg4gpu4d` |
| Ethereum (ETH) | `0xf38b517Dd2005d93E0BDc1e9807665074c5eC731` / `nierto.eth` |
| Monero (XMR) | `8BPaSoq1pEJH4LgbGNQ92kFJA3oi2frE4igHvdP9Lz2giwhFo2VnNvGT8XABYasjtoVY2Qb3LVHv6CP3qwcJ8UnyRtjWRZ5` |

## Disclaimer

This software is provided **"as is"**, without warranty of any kind, express or
implied. Use of this software is entirely at your own risk. In no event shall
the author or contributors be held liable for any damages arising from the use
or inability to use this software.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

"Geodineum" and its associated logo are proprietary marks of Niels Erik Toren;
the license grants no rights to use them (see [NOTICE](NOTICE) and Section 6 of
the license).
