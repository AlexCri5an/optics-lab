//! WGSL shader source code modules.

/// Physical path tracer compute shader with iterative BVH traversal and dielectric refraction.
pub const PATH_TRACER_WGSL: &str = include_str!("path_tracer.wgsl");

/// ACES filmic tonemapping and piecewise IEC 61966-2-1 sRGB gamma transfer shader.
pub const TONEMAPPING_WGSL: &str = include_str!("tonemapping.wgsl");

/// Traversal cost heatmap color ramp shader.
pub const BVH_HEATMAP_WGSL: &str = include_str!("bvh_heatmap.wgsl");
