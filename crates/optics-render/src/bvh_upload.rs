//! GPU buffer allocation and upload routines for BVH nodes and triangle primitives.

use optics_bvh::{BvhScene, pack_bvh_to_gpu_buffer, pack_triangles_to_gpu_buffer};
use wgpu::util::DeviceExt;

use crate::context::GpuContext;

/// Allocated GPU storage buffers holding the flattened BVH hierarchy and primitive pools.
#[derive(Debug)]
pub struct BvhBuffer {
    /// Storage buffer containing flat 16-byte aligned `GpuBvhNode` elements.
    pub node_buf: wgpu::Buffer,
    /// Storage buffer containing 16-byte aligned `GpuTriangle` elements.
    pub triangle_buf: wgpu::Buffer,
    /// Number of BVH nodes in `node_buf`.
    pub node_count: u32,
    /// Number of triangles in `triangle_buf`.
    pub triangle_count: u32,
}

impl BvhBuffer {
    /// Uploads a [`BvhScene`] into GPU storage buffers.
    #[must_use]
    pub fn upload(ctx: &GpuContext, scene: &BvhScene) -> Self {
        // Collect all TLAS and BLAS nodes
        let mut all_nodes = scene.tlas.clone();
        for mesh_nodes in &scene.blas {
            all_nodes.extend_from_slice(mesh_nodes);
        }

        // Collect all mesh triangles
        let mut all_triangles = Vec::new();
        for mesh_tris in &scene.triangles {
            all_triangles.extend_from_slice(mesh_tris);
        }

        let node_bytes = pack_bvh_to_gpu_buffer(&all_nodes);
        let tri_bytes = pack_triangles_to_gpu_buffer(&all_triangles);

        // Ensure buffers have at least minimal size (32 bytes) to satisfy wgpu alignment rules
        let node_bytes = if node_bytes.is_empty() {
            vec![0_u8; 32]
        } else {
            node_bytes
        };
        let tri_bytes = if tri_bytes.is_empty() {
            vec![0_u8; 64]
        } else {
            tri_bytes
        };

        let node_buf = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BVH Node Storage Buffer"),
                contents: &node_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        let triangle_buf = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Triangle Storage Buffer"),
                contents: &tri_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            node_buf,
            triangle_buf,
            node_count: all_nodes.len() as u32,
            triangle_count: all_triangles.len() as u32,
        }
    }
}
