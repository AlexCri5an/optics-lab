//! GPU buffer packing and memory layout for WGSL storage buffers.
//!
//! Provides 16-byte aligned data structures matching WebGPU storage buffer alignment rules.

use bytemuck::{Pod, Zeroable};

use crate::builder::BvhNode;
use crate::triangle::Triangle;

/// 16-byte aligned flat BVH node for GPU storage buffers.
///
/// Matches WGSL memory layout:
/// ```wgsl
/// struct BvhNode {
///     min: vec3<f32>,
///     left_or_first: u32,
///     max: vec3<f32>,
///     count: u32,
/// };
/// ```
/// Total size: exactly 32 bytes, alignment: 16 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuBvhNode {
    /// Bounding box minimum coordinate (x, y, z).
    pub min: [f32; 3],
    /// If leaf: index of first primitive. If internal: index of left child node.
    pub left_or_first: u32,
    /// Bounding box maximum coordinate (x, y, z).
    pub max: [f32; 3],
    /// If leaf: primitive count. If internal: 0.
    pub count: u32,
}

impl GpuBvhNode {
    /// Converts a CPU [`BvhNode`] into a GPU-compatible [`GpuBvhNode`].
    #[must_use]
    pub fn from_cpu(node: &BvhNode) -> Self {
        Self {
            min: [
                node.aabb.min.x as f32,
                node.aabb.min.y as f32,
                node.aabb.min.z as f32,
            ],
            left_or_first: node.left_or_first,
            max: [
                node.aabb.max.x as f32,
                node.aabb.max.y as f32,
                node.aabb.max.z as f32,
            ],
            count: node.count,
        }
    }
}

/// 16-byte aligned triangle representation for GPU storage buffers.
///
/// Contains 4 vectors of 4 single-precision floats:
/// - `v0`: vertex 0 position $(x, y, z, 1.0)$
/// - `v1`: vertex 1 position $(x, y, z, 1.0)$
/// - `v2`: vertex 2 position $(x, y, z, 1.0)$
/// - `normal`: surface normal $(n_x, n_y, n_z)$ with `material_id` packed in $w$.
///
/// Total size: exactly 64 bytes, alignment: 16 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuTriangle {
    /// Vertex 0 coordinate.
    pub v0: [f32; 4],
    /// Vertex 1 coordinate.
    pub v1: [f32; 4],
    /// Vertex 2 coordinate.
    pub v2: [f32; 4],
    /// Geometric normal and material identifier in $w$.
    pub normal: [f32; 4],
}

impl GpuTriangle {
    /// Converts a CPU [`Triangle`] into a GPU-compatible [`GpuTriangle`].
    #[must_use]
    pub fn from_cpu(tri: &Triangle) -> Self {
        Self {
            v0: [tri.v0.x as f32, tri.v0.y as f32, tri.v0.z as f32, 1.0],
            v1: [tri.v1.x as f32, tri.v1.y as f32, tri.v1.z as f32, 1.0],
            v2: [tri.v2.x as f32, tri.v2.y as f32, tri.v2.z as f32, 1.0],
            normal: [
                tri.normal.x as f32,
                tri.normal.y as f32,
                tri.normal.z as f32,
                tri.material_id as f32,
            ],
        }
    }
}

/// Packs an array of CPU [`BvhNode`]s into a byte buffer suitable for a GPU storage buffer.
#[must_use]
pub fn pack_bvh_to_gpu_buffer(nodes: &[BvhNode]) -> Vec<u8> {
    let gpu_nodes: Vec<GpuBvhNode> = nodes.iter().map(GpuBvhNode::from_cpu).collect();
    bytemuck::cast_slice(&gpu_nodes).to_vec()
}

/// Packs an array of CPU [`Triangle`]s into a byte buffer suitable for a GPU storage buffer.
#[must_use]
pub fn pack_triangles_to_gpu_buffer(triangles: &[Triangle]) -> Vec<u8> {
    let gpu_tris: Vec<GpuTriangle> = triangles.iter().map(GpuTriangle::from_cpu).collect();
    bytemuck::cast_slice(&gpu_tris).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aabb::Aabb;
    use optics_core::Vec3;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_gpu_bvh_node_layout_and_alignment() -> TestResult {
        assert_eq!(
            std::mem::size_of::<GpuBvhNode>(),
            32,
            "GpuBvhNode must be exactly 32 bytes"
        );
        assert_eq!(
            std::mem::align_of::<GpuBvhNode>(),
            16,
            "GpuBvhNode must be 16-byte aligned"
        );
        Ok(())
    }

    #[test]
    fn test_gpu_triangle_layout_and_alignment() -> TestResult {
        assert_eq!(
            std::mem::size_of::<GpuTriangle>(),
            64,
            "GpuTriangle must be exactly 64 bytes"
        );
        assert_eq!(
            std::mem::align_of::<GpuTriangle>(),
            16,
            "GpuTriangle must be 16-byte aligned"
        );
        Ok(())
    }

    #[test]
    fn test_pack_bvh_and_triangles_to_bytes() -> TestResult {
        let node = BvhNode::leaf(
            Aabb {
                min: Vec3::new(-1.0, -2.0, -3.0),
                max: Vec3::new(1.0, 2.0, 3.0),
            },
            42,
            3,
        );

        let bytes = pack_bvh_to_gpu_buffer(&[node]);
        assert_eq!(bytes.len(), 32);

        let tri = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            7,
        );
        let tri_bytes = pack_triangles_to_gpu_buffer(&[tri]);
        assert_eq!(tri_bytes.len(), 64);

        Ok(())
    }
}
