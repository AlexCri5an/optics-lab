//! `optics-bvh` — 2-level bounding volume hierarchy for non-sequential scenes.
//!
//! # Architecture note
//! The BVH and the wgpu path tracer are used for **non-sequential** scenes
//! (prisms, glass spheres, arbitrary meshes). They are *not* used for
//! sequential lens analysis: paraxial tracing, spot diagrams, and Seidel
//! aberration calculations live in `optics-core` and are purely analytic.
//! A lens prescription is converted into scene geometry only when it is
//! rendered in the path tracer.
