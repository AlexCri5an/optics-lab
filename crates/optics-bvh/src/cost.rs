//! Traversal cost evaluation for performance diagnostics and heatmaps.

use optics_core::Ray;

use crate::scene::BvhScene;

/// Traces a ray through the 2-level BVH hierarchy and returns the total intersection test cost.
///
/// Returns the sum of AABB slab tests and triangle intersection evaluations.
/// For an early-exit miss (ray missing the scene's root bounding box entirely),
/// returns 0.
#[must_use]
pub fn trace_bvh_cost(ray: &Ray, bvh: &BvhScene) -> u32 {
    if bvh.tlas.is_empty() || bvh.instances.is_empty() {
        return 0;
    }

    // Early-exit check against root AABB
    if bvh.tlas[0].aabb.intersect_ray(ray).is_none() {
        return 0;
    }

    let mut cost: u32 = 1; // Counted the root AABB test

    let mut stack = [0_u32; 64];
    let mut stack_ptr = 1;
    stack[0] = 0;

    while stack_ptr > 0 {
        stack_ptr -= 1;
        let node_idx = stack[stack_ptr] as usize;
        let node = &bvh.tlas[node_idx];

        if node_idx != 0 {
            cost = cost.saturating_add(1);
            if node.aabb.intersect_ray(ray).is_none() {
                continue;
            }
        }

        if node.is_leaf {
            for i in 0..node.count {
                let inst_idx = (node.left_or_first + i) as usize;
                if let Some(inst) = bvh.instances.get(inst_idx) {
                    if let Some(local_ray) = inst.transform_ray(ray) {
                        if let Some(blas_nodes) = bvh.blas.get(inst.blas_index) {
                            let tris = &bvh.triangles[inst.blas_index];
                            let tri_indices = &bvh.blas_indices[inst.blas_index];

                            let mut b_stack = [0_u32; 64];
                            let mut b_stack_ptr = 1;
                            b_stack[0] = 0;

                            while b_stack_ptr > 0 {
                                b_stack_ptr -= 1;
                                let b_idx = b_stack[b_stack_ptr] as usize;
                                if let Some(b_node) = blas_nodes.get(b_idx) {
                                    cost = cost.saturating_add(1);
                                    if b_node.aabb.intersect_ray(&local_ray).is_none() {
                                        continue;
                                    }

                                    if b_node.is_leaf {
                                        for j in 0..b_node.count {
                                            cost = cost.saturating_add(1);
                                            let tri_idx =
                                                tri_indices[(b_node.left_or_first + j) as usize];
                                            let _ = tris[tri_idx].intersect(&local_ray, inst_idx);
                                        }
                                    } else {
                                        b_stack[b_stack_ptr] = b_node.left_or_first + 1;
                                        b_stack_ptr += 1;
                                        b_stack[b_stack_ptr] = b_node.left_or_first;
                                        b_stack_ptr += 1;
                                    }
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

    cost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::BlasInstance;
    use crate::triangle::Triangle;
    use nalgebra::Matrix4;
    use optics_core::Vec3;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_traversal_cost_hit_and_early_exit_miss() -> TestResult {
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0,
        );
        let mesh = vec![tri];
        let local_aabb = tri.aabb();
        let transform = Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 0.0, 5.0));
        let instance = BlasInstance::new(0, &local_aabb, transform);
        let scene = BvhScene::build(vec![mesh], vec![instance]);

        // 1. Hit ray
        let hit_ray = Ray::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;
        let cost_hit = trace_bvh_cost(&hit_ray, &scene);
        assert!(
            cost_hit > 0,
            "Cost for hit ray must be > 0, got {}",
            cost_hit
        );

        // 2. Early-exit miss ray (pointing far away from scene)
        let miss_ray = Ray::new(
            Vec3::new(100.0, 100.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            587.56,
            1.0,
        )?;
        let cost_miss = trace_bvh_cost(&miss_ray, &scene);
        assert_eq!(
            cost_miss, 0,
            "Cost for early-exit miss must be 0, got {}",
            cost_miss
        );

        Ok(())
    }
}
