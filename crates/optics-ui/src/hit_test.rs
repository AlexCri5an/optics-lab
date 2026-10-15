//! Hit-testing routines for interactive ray selection in screen space.

use egui::Pos2;

/// Calculates the shortest Euclidean distance from point `p` to the line segment `ab`,
/// along with the closest point on the segment.
///
/// Handles degenerate segments ($a \approx b$), internal perpendicular projections,
/// and endpoint clamps gracefully.
#[must_use]
pub fn point_to_segment_distance(p: Pos2, a: Pos2, b: Pos2) -> (f32, Pos2) {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let len_sq = ab_x * ab_x + ab_y * ab_y;

    if len_sq <= 1e-8 {
        // Degenerate segment: a and b coincide
        let d = p.distance(a);
        return (d, a);
    }

    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;

    // Parameter t represents projection onto vector ab: t in [0.0, 1.0]
    let t = ((ap_x * ab_x + ap_y * ab_y) / len_sq).clamp(0.0, 1.0);
    let closest = Pos2::new(a.x + t * ab_x, a.y + t * ab_y);
    let dist = p.distance(closest);

    (dist, closest)
}

/// Finds the ray index whose polyline segment passes closest to `screen_pos`
/// within a pixel threshold `threshold_px`.
///
/// Each ray is represented as a slice of 2D screen positions.
#[must_use]
pub fn find_hovered_ray_index(
    screen_pos: Pos2,
    ray_paths: &[Vec<Pos2>],
    threshold_px: f32,
) -> Option<usize> {
    let mut closest_idx = None;
    let mut min_dist = threshold_px;

    for (idx, path) in ray_paths.iter().enumerate() {
        if path.len() < 2 {
            continue;
        }
        for window in path.windows(2) {
            let (dist, _) = point_to_segment_distance(screen_pos, window[0], window[1]);
            if dist < min_dist {
                min_dist = dist;
                closest_idx = Some(idx);
            }
        }
    }

    closest_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection_interior() {
        let a = Pos2::new(0.0, 0.0);
        let b = Pos2::new(100.0, 0.0);
        let p = Pos2::new(50.0, 20.0);

        let (dist, closest) = point_to_segment_distance(p, a, b);
        assert!((dist - 20.0).abs() < 1e-4);
        assert!((closest.x - 50.0).abs() < 1e-4);
        assert!((closest.y - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_projection_endpoints() {
        let a = Pos2::new(10.0, 10.0);
        let b = Pos2::new(50.0, 10.0);

        // Before endpoint A
        let p1 = Pos2::new(0.0, 10.0);
        let (d1, c1) = point_to_segment_distance(p1, a, b);
        assert!((d1 - 10.0).abs() < 1e-4);
        assert_eq!(c1, a);

        // Past endpoint B
        let p2 = Pos2::new(70.0, 10.0);
        let (d2, c2) = point_to_segment_distance(p2, a, b);
        assert!((d2 - 20.0).abs() < 1e-4);
        assert_eq!(c2, b);
    }

    #[test]
    fn test_degenerate_segment() {
        let a = Pos2::new(25.0, 30.0);
        let b = Pos2::new(25.0, 30.0); // same point
        let p = Pos2::new(25.0, 40.0);

        let (dist, closest) = point_to_segment_distance(p, a, b);
        assert!((dist - 10.0).abs() < 1e-4);
        assert_eq!(closest, a);
    }

    #[test]
    fn test_hit_test_threshold_and_tie() {
        let path1 = vec![Pos2::new(0.0, 0.0), Pos2::new(100.0, 0.0)];
        let path2 = vec![Pos2::new(0.0, 10.0), Pos2::new(100.0, 10.0)];
        let rays = vec![path1, path2];

        // Cursor at (50, 2) is closer to ray 0 (dist 2) than ray 1 (dist 8)
        let hit = find_hovered_ray_index(Pos2::new(50.0, 2.0), &rays, 5.0);
        assert_eq!(hit, Some(0));

        // Cursor at (50, 9) is closer to ray 1 (dist 1)
        let hit = find_hovered_ray_index(Pos2::new(50.0, 9.0), &rays, 5.0);
        assert_eq!(hit, Some(1));

        // Cursor at (50, 50) exceeds threshold 5.0
        let hit = find_hovered_ray_index(Pos2::new(50.0, 50.0), &rays, 5.0);
        assert_eq!(hit, None);
    }
}
