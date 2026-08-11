//! Core mathematical foundations for geometric hashing.
//!
//! This module provides the shared primitives used by all mixing strategies
//! and modes: irrational constant generation, the Weyl equidistribution engine,
//! manifold projection, domain normalization, and configuration types.

pub mod constants;
pub mod equidistribution;
pub mod manifold;
pub mod normalize;
pub mod config;
