//! `optics-render` — WebGPU/wgpu physical path tracing pipeline.
//!
//! # Architecture note
//! This crate handles **non-sequential** GPU rendering: BVH upload,
//! iterative stack-based traversal, thin-lens camera simulation,
//! Snell/Fresnel dielectric interactions, and tone-mapping. Sequential lens
//! analysis (Seidel, spot diagrams, paraxial) lives in `optics-core`.
//!
//! The `wgpu` version is locked across the workspace to match `eframe`
//! (0.32.3 → wgpu 25.0.2).

#![deny(missing_docs)]

pub mod bvh_upload;
pub mod camera;
pub mod context;
pub mod convergence;
pub mod error;
pub mod pipeline;
pub mod shaders;
pub mod target;

pub use bvh_upload::BvhBuffer;
pub use camera::{CameraUniform, OrthographicCamera, ThinLensCamera, concentric_disk_sample};
pub use context::GpuContext;
pub use convergence::{ConvergenceTracker, compute_rmse};
pub use error::RenderError;
pub use pipeline::{PathTracingRenderer, RenderMode, RenderParams};
pub use target::{RenderTarget, ToneMapTarget, resize_render_targets};
