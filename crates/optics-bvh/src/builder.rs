//! Surface Area Heuristic (SAH) BVH builder with 16-bin spatial partitioning.

use crate::aabb::Aabb;
use crate::triangle::Triangle;

/// Cost constants for the Surface Area Heuristic.
pub const SAH_TRAVERSAL_COST: f64 = 1.0;
/// Intersection cost per triangle primitive.
pub const SAH_INTERSECTION_COST: f64 = 1.5;
/// Number of evaluation bins per axis.
pub const SAH_BINS: usize = 16;
/// Maximum primitive count per leaf node.
pub const MAX_LEAF_PRIMITIVES: usize = 4;

/// Flat BVH node representing either an internal split or a primitive leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BvhNode {
    /// Spatial bounding box enclosing all descendants or leaf primitives.
    pub aabb: Aabb,
    /// If leaf: index of the first primitive in index list. If internal: index of the left child node.
    pub left_or_first: u32,
    /// If leaf: number of primitives. If internal: 0.
    pub count: u32,
    /// Whether this node is a leaf containing primitives.
    pub is_leaf: bool,
}

impl BvhNode {
    /// Creates a new leaf node.
    #[must_use]
    pub const fn leaf(aabb: Aabb, first_primitive: u32, count: u32) -> Self {
        Self {
            aabb,
            left_or_first: first_primitive,
            count,
            is_leaf: true,
        }
    }

    /// Creates a new internal split node.
    #[must_use]
    pub const fn internal(aabb: Aabb, left_child_index: u32) -> Self {
        Self {
            aabb,
            left_or_first: left_child_index,
            count: 0,
            is_leaf: false,
        }
    }
}

/// Result of evaluating an axis-aligned split candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitDecision {
    /// Partition along `axis` (0: X, 1: Y, 2: Z) at the given spatial coordinate.
    Split {
        /// Partition axis (0: X, 1: Y, 2: Z).
        axis: usize,
        /// Spatial split coordinate.
        split_pos: f64,
        /// Evaluated SAH cost.
        cost: f64,
    },
    /// Make a leaf node because splitting does not reduce cost.
    MakeLeaf,
}

/// Evaluates the optimal split plane for a set of primitives using 16-bin SAH.
#[must_use]
pub fn find_best_split(
    triangles: &[Triangle],
    indices: &[usize],
    parent_aabb: &Aabb,
) -> SplitDecision {
    let count = indices.len();
    if count <= MAX_LEAF_PRIMITIVES {
        return SplitDecision::MakeLeaf;
    }

    let parent_sa = parent_aabb.surface_area();
    if parent_sa <= 1e-12 {
        return SplitDecision::MakeLeaf;
    }
    let leaf_cost = count as f64 * SAH_INTERSECTION_COST;
    let mut best_cost = leaf_cost;
    let mut best_axis = 0;
    let mut best_split_pos = 0.0;

    // Evaluate each 3D axis (X, Y, Z)
    for axis in 0..3 {
        let mut axis_min = f64::INFINITY;
        let mut axis_max = f64::NEG_INFINITY;

        for &idx in indices {
            let c = triangles[idx].centroid()[axis];
            axis_min = axis_min.min(c);
            axis_max = axis_max.max(c);
        }

        if (axis_max - axis_min) < 1e-6 {
            continue; // Degenerate centroids along this axis
        }

        // Initialize 16 bins
        struct Bin {
            count: usize,
            bounds: Aabb,
        }
        let mut bins: [Bin; SAH_BINS] = std::array::from_fn(|_| Bin {
            count: 0,
            bounds: Aabb::empty(),
        });

        let bin_scale = SAH_BINS as f64 / (axis_max - axis_min);

        for &idx in indices {
            let tri = &triangles[idx];
            let c = tri.centroid()[axis];
            let bin_idx = (((c - axis_min) * bin_scale) as usize).min(SAH_BINS - 1);
            bins[bin_idx].count += 1;
            bins[bin_idx].bounds = bins[bin_idx].bounds.union(&tri.aabb());
        }

        // Prefix and suffix cumulative sweeps
        let mut left_bounds = [Aabb::empty(); SAH_BINS - 1];
        let mut left_counts = [0_usize; SAH_BINS - 1];
        let mut running_box = Aabb::empty();
        let mut running_count = 0;

        for i in 0..(SAH_BINS - 1) {
            running_count += bins[i].count;
            running_box = running_box.union(&bins[i].bounds);
            left_counts[i] = running_count;
            left_bounds[i] = running_box;
        }

        let mut right_bounds = [Aabb::empty(); SAH_BINS - 1];
        let mut right_counts = [0_usize; SAH_BINS - 1];
        running_box = Aabb::empty();
        running_count = 0;

        for i in (1..SAH_BINS).rev() {
            running_count += bins[i].count;
            running_box = running_box.union(&bins[i].bounds);
            right_counts[i - 1] = running_count;
            right_bounds[i - 1] = running_box;
        }

        // Evaluate SAH cost at bin boundaries
        for i in 0..(SAH_BINS - 1) {
            let nl = left_counts[i];
            let nr = right_counts[i];

            if nl == 0 || nr == 0 {
                continue;
            }

            let sa_l = left_bounds[i].surface_area();
            let sa_r = right_bounds[i].surface_area();

            let cost = SAH_TRAVERSAL_COST
                + (sa_l * nl as f64 * SAH_INTERSECTION_COST
                    + sa_r * nr as f64 * SAH_INTERSECTION_COST)
                    / parent_sa;

            if cost < best_cost {
                best_cost = cost;
                best_axis = axis;
                best_split_pos = axis_min + (i + 1) as f64 * (1.0 / bin_scale);
            }
        }
    }

    if best_cost < leaf_cost {
        SplitDecision::Split {
            axis: best_axis,
            split_pos: best_split_pos,
            cost: best_cost,
        }
    } else {
        SplitDecision::MakeLeaf
    }
}

/// Builds a flat BVH node array for the given triangle primitives using SAH.
///
/// Returns `(nodes, primitive_indices)` where leaf nodes reference contiguous slices of `primitive_indices`.
#[must_use]
pub fn build_bvh(triangles: &[Triangle]) -> (Vec<BvhNode>, Vec<usize>) {
    if triangles.is_empty() {
        return (vec![BvhNode::leaf(Aabb::empty(), 0, 0)], Vec::new());
    }

    let mut indices: Vec<usize> = (0..triangles.len()).collect();
    let mut nodes = Vec::with_capacity(triangles.len() * 2);

    subdivide_bvh_node(triangles, &mut indices, 0, triangles.len(), &mut nodes);

    (nodes, indices)
}

fn subdivide_bvh_node(
    triangles: &[Triangle],
    indices: &mut [usize],
    start: usize,
    count: usize,
    nodes: &mut Vec<BvhNode>,
) -> u32 {
    let node_idx = nodes.len() as u32;
    nodes.push(BvhNode::leaf(Aabb::empty(), start as u32, count as u32));

    // Compute bounding box of all primitives in this node
    let mut node_aabb = Aabb::empty();
    for &idx in &indices[start..start + count] {
        node_aabb = node_aabb.union(&triangles[idx].aabb());
    }
    nodes[node_idx as usize].aabb = node_aabb;

    let decision = find_best_split(triangles, &indices[start..start + count], &node_aabb);

    match decision {
        SplitDecision::Split {
            axis, split_pos, ..
        } => {
            // Partition primitives around split_pos
            let mut left = start;
            let mut right = start + count - 1;

            while left <= right {
                if triangles[indices[left]].centroid()[axis] < split_pos {
                    left += 1;
                } else {
                    indices.swap(left, right);
                    if right == 0 {
                        break;
                    }
                    right -= 1;
                }
            }

            let left_count = left - start;
            if left_count == 0 || left_count == count {
                // Median spatial fallback if partitioning failed
                let mid = start + count / 2;
                indices[start..start + count].select_nth_unstable_by(count / 2, |&a, &b| {
                    triangles[a].centroid()[axis].total_cmp(&triangles[b].centroid()[axis])
                });

                let left_child = subdivide_bvh_node(triangles, indices, start, count / 2, nodes);
                let _right_child =
                    subdivide_bvh_node(triangles, indices, mid, count - count / 2, nodes);

                nodes[node_idx as usize] = BvhNode::internal(node_aabb, left_child);
                return node_idx;
            }

            let left_child = subdivide_bvh_node(triangles, indices, start, left_count, nodes);
            let _right_child = subdivide_bvh_node(
                triangles,
                indices,
                start + left_count,
                count - left_count,
                nodes,
            );

            nodes[node_idx as usize] = BvhNode::internal(node_aabb, left_child);
        }
        SplitDecision::MakeLeaf => {
            nodes[node_idx as usize] = BvhNode::leaf(node_aabb, start as u32, count as u32);
        }
    }

    node_idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::Vec3;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_build_bvh_on_cube() -> TestResult {
        // 12 triangles forming a unit cube
        let mut triangles = Vec::new();
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];

        // Bottom (z=0)
        triangles.push(Triangle::new(corners[0], corners[1], corners[2], 0));
        triangles.push(Triangle::new(corners[0], corners[2], corners[3], 0));
        // Top (z=1)
        triangles.push(Triangle::new(corners[4], corners[6], corners[5], 0));
        triangles.push(Triangle::new(corners[4], corners[7], corners[6], 0));
        // Front (y=0)
        triangles.push(Triangle::new(corners[0], corners[5], corners[1], 0));
        triangles.push(Triangle::new(corners[0], corners[4], corners[5], 0));
        // Back (y=1)
        triangles.push(Triangle::new(corners[3], corners[2], corners[6], 0));
        triangles.push(Triangle::new(corners[3], corners[6], corners[7], 0));
        // Left (x=0)
        triangles.push(Triangle::new(corners[0], corners[3], corners[7], 0));
        triangles.push(Triangle::new(corners[0], corners[7], corners[4], 0));
        // Right (x=1)
        triangles.push(Triangle::new(corners[1], corners[5], corners[6], 0));
        triangles.push(Triangle::new(corners[1], corners[6], corners[2], 0));

        let (nodes, indices) = build_bvh(&triangles);

        assert!(!nodes.is_empty());
        assert_eq!(indices.len(), 12);
        // Root node bounds must match unit cube [0..1]
        assert_eq!(nodes[0].aabb.min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(nodes[0].aabb.max, Vec3::new(1.0, 1.0, 1.0));

        Ok(())
    }

    #[test]
    fn test_single_triangle_bvh() -> TestResult {
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 5.0),
            Vec3::new(1.0, -1.0, 5.0),
            Vec3::new(0.0, 1.0, 5.0),
            0,
        );

        let (nodes, indices) = build_bvh(&[tri]);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].is_leaf);
        assert_eq!(nodes[0].count, 1);
        assert_eq!(indices.len(), 1);

        let ray = optics_core::Ray::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let hit = tri.intersect(&ray, 0);
        assert!(hit.is_some());
        let h = hit.ok_or("expected hit")?;
        assert!((h.t - 5.0).abs() < 1e-4);

        Ok(())
    }

    #[test]
    fn test_sah_split_valid_partition() -> TestResult {
        // 8 triangles: 4 on the left (x in [-10, -8]) and 4 on the right (x in [8, 10])
        let mut triangles = Vec::new();
        for i in 0..4 {
            let offset_y = i as f64 * 2.0;
            // Left cluster
            triangles.push(Triangle::new(
                Vec3::new(-10.0, offset_y, 0.0),
                Vec3::new(-8.0, offset_y, 0.0),
                Vec3::new(-9.0, offset_y + 1.0, 0.0),
                0,
            ));
            // Right cluster
            triangles.push(Triangle::new(
                Vec3::new(8.0, offset_y, 0.0),
                Vec3::new(10.0, offset_y, 0.0),
                Vec3::new(9.0, offset_y + 1.0, 0.0),
                0,
            ));
        }

        let (nodes, indices) = build_bvh(&triangles);
        assert_eq!(indices.len(), 8);
        assert!(
            !nodes[0].is_leaf,
            "Root node of 8 widely separated triangles must be an internal split"
        );

        // The left child is at left_or_first, right child is at left_or_first + 1
        let left_idx = nodes[0].left_or_first as usize;
        let right_idx = left_idx + 1;
        assert!(left_idx < nodes.len());
        assert!(right_idx < nodes.len());

        // Both children must have non-zero primitive counts (no empty sides)
        assert_eq!(nodes[left_idx].count, 4);
        assert_eq!(nodes[right_idx].count, 4);
        assert!(nodes[left_idx].aabb.min.x < 0.0);
        assert!(nodes[right_idx].aabb.max.x > 0.0);

        Ok(())
    }
}
