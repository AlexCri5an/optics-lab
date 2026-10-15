//! 2-Level BVH Architecture: Top-Level (TLAS) and Bottom-Level (BLAS) Acceleration Structures.

use nalgebra::{Matrix4, Point3, Vector4};
use optics_core::{HitRecord, Ray, Vec3};

use crate::aabb::Aabb;
use crate::builder::{BvhNode, build_bvh};
use crate::triangle::Triangle;

/// An instance of a BLAS mesh placed in the world with an affine transform.
#[derive(Clone, Debug, PartialEq)]
pub struct BlasInstance {
    /// Index of the referenced BLAS mesh in `BvhScene.blas`.
    pub blas_index: usize,
    /// Local-to-world transformation matrix.
    pub transform: Matrix4<f64>,
    /// World-to-local inverse transformation matrix.
    pub inv_transform: Matrix4<f64>,
    /// Transformed bounding box in world space.
    pub world_aabb: Aabb,
}

impl BlasInstance {
    /// Creates a new BLAS instance with an affine transform and computes its world-space AABB.
    #[must_use]
    pub fn new(blas_index: usize, local_aabb: &Aabb, transform: Matrix4<f64>) -> Self {
        let inv_transform = transform.try_inverse().unwrap_or_else(Matrix4::identity);

        // Transform the 8 corners of the local AABB into world space
        let corners = [
            Point3::new(local_aabb.min.x, local_aabb.min.y, local_aabb.min.z),
            Point3::new(local_aabb.max.x, local_aabb.min.y, local_aabb.min.z),
            Point3::new(local_aabb.min.x, local_aabb.max.y, local_aabb.min.z),
            Point3::new(local_aabb.max.x, local_aabb.max.y, local_aabb.min.z),
            Point3::new(local_aabb.min.x, local_aabb.min.y, local_aabb.max.z),
            Point3::new(local_aabb.max.x, local_aabb.min.y, local_aabb.max.z),
            Point3::new(local_aabb.min.x, local_aabb.max.y, local_aabb.max.z),
            Point3::new(local_aabb.max.x, local_aabb.max.y, local_aabb.max.z),
        ];

        let mut world_aabb = Aabb::empty();
        for &c in &corners {
            let p4 = transform * Vector4::new(c.x, c.y, c.z, 1.0);
            let pt = Vec3::new(p4.x / p4.w, p4.y / p4.w, p4.z / p4.w);
            world_aabb.grow(pt);
        }

        Self {
            blas_index,
            transform,
            inv_transform,
            world_aabb,
        }
    }

    /// Transforms a world-space ray into the instance's local coordinate system.
    #[must_use]
    pub fn transform_ray(&self, ray: &Ray) -> Option<Ray> {
        let o4 = self.inv_transform * Vector4::new(ray.origin.x, ray.origin.y, ray.origin.z, 1.0);
        let d4 = self.inv_transform
            * Vector4::new(ray.direction.x, ray.direction.y, ray.direction.z, 0.0);

        let local_origin = Vec3::new(o4.x / o4.w, o4.y / o4.w, o4.z / o4.w);
        let local_dir = Vec3::new(d4.x, d4.y, d4.z);
        let len = local_dir.norm();
        let normalized_dir = if len > 1e-12 {
            local_dir / len
        } else {
            return None;
        };

        Ray::new(local_origin, normalized_dir, ray.wavelength_nm, ray.energy).ok()
    }
}

/// A complete 2-level BVH scene containing TLAS, BLAS meshes, and triangle pools.
#[derive(Clone, Debug, PartialEq)]
pub struct BvhScene {
    /// Top-Level Acceleration Structure nodes over scene instances.
    pub tlas: Vec<BvhNode>,
    /// Bottom-Level Acceleration Structure nodes for each mesh geometry.
    pub blas: Vec<Vec<BvhNode>>,
    /// Primitive index mappings for each BLAS mesh.
    pub blas_indices: Vec<Vec<usize>>,
    /// Triangle primitive pools for each BLAS mesh.
    pub triangles: Vec<Vec<Triangle>>,
    /// Scene instances referenced by TLAS leaves.
    pub instances: Vec<BlasInstance>,
}

impl BvhScene {
    /// Builds a 2-level acceleration structure from individual mesh geometries and world instances.
    #[must_use]
    pub fn build(mesh_geometries: Vec<Vec<Triangle>>, instances: Vec<BlasInstance>) -> Self {
        let mut blas = Vec::with_capacity(mesh_geometries.len());
        let mut blas_indices = Vec::with_capacity(mesh_geometries.len());

        for mesh in &mesh_geometries {
            let (nodes, indices) = build_bvh(mesh);
            blas.push(nodes);
            blas_indices.push(indices);
        }

        // Build TLAS over instance bounding boxes
        let tlas = if instances.is_empty() {
            vec![BvhNode::leaf(Aabb::empty(), 0, 0)]
        } else {
            let mut pseudo_triangles = Vec::with_capacity(instances.len());
            for inst in &instances {
                // A proxy triangle whose AABB matches the instance world AABB
                let min = inst.world_aabb.min;
                let max = inst.world_aabb.max;
                pseudo_triangles.push(Triangle::new(min, max, (min + max) * 0.5, 0));
            }
            let (tlas_nodes, _) = build_bvh(&pseudo_triangles);
            tlas_nodes
        };

        Self {
            tlas,
            blas,
            blas_indices,
            triangles: mesh_geometries,
            instances,
        }
    }

    /// Traces a ray through the 2-level BVH hierarchy and returns the closest geometric hit.
    #[must_use]
    pub fn intersect(&self, ray: &Ray) -> Option<HitRecord> {
        if self.tlas.is_empty() || self.instances.is_empty() {
            return None;
        }

        let mut closest_hit: Option<HitRecord> = None;
        let mut closest_t = f64::INFINITY;

        // Traverse TLAS
        let mut stack = [0_u32; 64];
        let mut stack_ptr = 1;
        stack[0] = 0; // root node

        while stack_ptr > 0 {
            stack_ptr -= 1;
            let node_idx = stack[stack_ptr] as usize;
            let node = &self.tlas[node_idx];

            if node.aabb.intersect_ray(ray).is_none() {
                continue;
            }

            if node.is_leaf {
                for i in 0..node.count {
                    let inst_idx = (node.left_or_first + i) as usize;
                    if let Some(inst) = self.instances.get(inst_idx) {
                        if let Some(local_ray) = inst.transform_ray(ray) {
                            // Traverse BLAS
                            if let Some(blas_nodes) = self.blas.get(inst.blas_index) {
                                let tris = &self.triangles[inst.blas_index];
                                let tri_indices = &self.blas_indices[inst.blas_index];

                                if let Some(hit) = intersect_blas(
                                    &local_ray,
                                    blas_nodes,
                                    tris,
                                    tri_indices,
                                    inst_idx,
                                ) {
                                    // Transform hit back to world space
                                    let hit_pt4 = inst.transform
                                        * Vector4::new(hit.point.x, hit.point.y, hit.point.z, 1.0);
                                    let world_pt = Vec3::new(
                                        hit_pt4.x / hit_pt4.w,
                                        hit_pt4.y / hit_pt4.w,
                                        hit_pt4.z / hit_pt4.w,
                                    );

                                    let n4 = inst.inv_transform.transpose()
                                        * Vector4::new(
                                            hit.normal.x,
                                            hit.normal.y,
                                            hit.normal.z,
                                            0.0,
                                        );
                                    let world_normal = Vec3::new(n4.x, n4.y, n4.z).normalize();

                                    let world_t = (world_pt - ray.origin).dot(&ray.direction);

                                    if world_t > 1e-5 && world_t < closest_t {
                                        closest_t = world_t;
                                        closest_hit = Some(HitRecord {
                                            t: world_t,
                                            point: world_pt,
                                            normal: world_normal,
                                            is_front_face: hit.is_front_face,
                                            surface_index: hit.surface_index,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                stack[stack_ptr] = node.left_or_first + 1;
                stack_ptr += 1;
                stack[stack_ptr] = node.left_or_first;
                stack_ptr += 1;
            }
        }

        closest_hit
    }
}

fn intersect_blas(
    ray: &Ray,
    nodes: &[BvhNode],
    triangles: &[Triangle],
    indices: &[usize],
    instance_idx: usize,
) -> Option<HitRecord> {
    let mut closest_hit: Option<HitRecord> = None;
    let mut closest_t = f64::INFINITY;

    let mut stack = [0_u32; 64];
    let mut stack_ptr = 1;
    stack[0] = 0;

    while stack_ptr > 0 {
        stack_ptr -= 1;
        let node_idx = stack[stack_ptr] as usize;
        let node = &nodes[node_idx];

        if node.aabb.intersect_ray(ray).is_none() {
            continue;
        }

        if node.is_leaf {
            for i in 0..node.count {
                let tri_idx = indices[(node.left_or_first + i) as usize];
                if let Some(hit) = triangles[tri_idx].intersect(ray, instance_idx) {
                    if hit.t < closest_t {
                        closest_t = hit.t;
                        closest_hit = Some(hit);
                    }
                }
            }
        } else {
            stack[stack_ptr] = node.left_or_first + 1;
            stack_ptr += 1;
            stack[stack_ptr] = node.left_or_first;
            stack_ptr += 1;
        }
    }

    closest_hit
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_2level_bvh_scene_intersection() -> TestResult {
        // Create a single triangle mesh (BLAS)
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0,
        );
        let mesh = vec![tri];
        let local_aabb = tri.aabb();

        // Instance placed at z = 5.0 in world space
        let transform = Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 0.0, 5.0));
        let instance = BlasInstance::new(0, &local_aabb, transform);

        let scene = BvhScene::build(vec![mesh], vec![instance]);

        let ray = Ray::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let hit = scene.intersect(&ray);
        assert!(hit.is_some(), "Ray must intersect the instanced triangle");
        let h = hit.ok_or("Ray must intersect the instanced triangle")?;
        assert!((h.t - 5.0).abs() < 1e-4);
        assert!((h.point.z - 5.0).abs() < 1e-4);

        Ok(())
    }
}
