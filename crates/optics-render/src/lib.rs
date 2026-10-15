//! `optics-render` — wgpu-based path tracing pipeline.
//!
//! # Architecture note
//! This crate handles **non-sequential** GPU rendering: BVH construction,
//! upload, compute-shader dispatch, and tone-mapping. Sequential lens
//! analysis (Seidel, spot diagrams, paraxial) lives in `optics-core`.
//!
//! The `wgpu` version must stay in sync with `eframe` (currently 0.32.3 →
//! wgpu 25.0.2). See workspace `Cargo.toml` for the single-version policy.
