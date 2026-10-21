//! `optics-bvh` — 2-level bounding volume hierarchy for non-sequential scenes.
//!
//! # Architecture note
//! The BVH and the wgpu path tracer are used for **non-sequential** scenes
//! (prisms, glass spheres, arbitrary meshes). They are *not* used for
//! sequential lens analysis: paraxial tracing, spot diagrams, and Seidel
//! aberration calculations live in `optics-core` and are purely analytic.
//! A lens prescription is converted into scene geometry only when it is
//! rendered in the path tracer.
//!
//! # Memory layout
//! Bottom-Level Acceleration Structures (BLAS) accelerate individual triangle meshes.
//! Top-Level Acceleration Structures (TLAS) accelerate instances of meshes with affine
//! world transforms.
//!
//! The GPU buffer format ([`GpuBvhNode`]) is packed into 16-byte aligned 32-byte records
//! strictly following WebGPU storage buffer alignment rules.

#![deny(missing_docs)]

pub mod aabb;
pub mod builder;
pub mod cost;
pub mod gpu;
pub mod scene;
pub mod tessellate;
pub mod triangle;

pub use aabb::{Aabb, ray_aabb_slab_test};
pub use builder::{
    BvhNode, MAX_LEAF_PRIMITIVES, SAH_BINS, SAH_INTERSECTION_COST, SAH_TRAVERSAL_COST, SplitDecision, build_bvh,
    find_best_split,
};
pub use cost::trace_bvh_cost;
pub use gpu::{GpuBvhNode, GpuTriangle, pack_bvh_to_gpu_buffer, pack_triangles_to_gpu_buffer};
pub use scene::{BlasInstance, BvhScene};
pub use tessellate::{prescription_to_trimesh, prescription_to_trimesh_with_materials};
pub use triangle::Triangle;
