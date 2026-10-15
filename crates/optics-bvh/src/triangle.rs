//! 3D Triangle primitive definition and Möller–Trumbore intersection.

use optics_core::{HitRecord, Ray, Vec3};

use crate::aabb::Aabb;

/// 3D triangle mesh primitive for BLAS geometry acceleration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    /// First vertex position.
    pub v0: Vec3,
    /// Second vertex position.
    pub v1: Vec3,
    /// Third vertex position.
    pub v2: Vec3,
    /// Surface normal vector.
    pub normal: Vec3,
    /// Assigned optical material identifier.
    pub material_id: u32,
}

impl Triangle {
    /// Creates a new triangle from vertices with automatic geometric normal calculation.
    #[must_use]
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3, material_id: u32) -> Self {
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let cross = edge1.cross(&edge2);
        let normal = if cross.norm_squared() > 1e-12 {
            cross.normalize()
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };

        Self {
            v0,
            v1,
            v2,
            normal,
            material_id,
        }
    }

    /// Computes the bounding box enclosing the triangle.
    #[must_use]
    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(&[self.v0, self.v1, self.v2])
    }

    /// Computes the triangle centroid coordinate.
    #[must_use]
    pub fn centroid(&self) -> Vec3 {
        (self.v0 + self.v1 + self.v2) / 3.0
    }

    /// Evaluates ray-triangle intersection via the Möller–Trumbore algorithm.
    #[must_use]
    pub fn intersect(&self, ray: &Ray, surface_index: usize) -> Option<HitRecord> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let pvec = ray.direction.cross(&edge2);
        let det = edge1.dot(&pvec);

        if det.abs() < 1e-10 {
            return None; // Parallel ray
        }

        let inv_det = 1.0 / det;
        let tvec = ray.origin - self.v0;
        let u = tvec.dot(&pvec) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = tvec.cross(&edge1);
        let v = ray.direction.dot(&qvec) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = edge2.dot(&qvec) * inv_det;
        if t <= 1e-6 {
            return None;
        }

        let point = ray.at(t);
        let is_front_face = ray.direction.dot(&self.normal) < 0.0;
        let normal = if is_front_face {
            self.normal
        } else {
            -self.normal
        };

        Some(HitRecord {
            t,
            point,
            normal,
            is_front_face,
            surface_index,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_triangle_centroid_and_aabb() -> TestResult {
        let tri = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            0,
        );

        assert_eq!(tri.centroid(), Vec3::new(1.0, 1.0, 0.0));
        let bbox = tri.aabb();
        assert_eq!(bbox.min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(bbox.max, Vec3::new(3.0, 3.0, 0.0));
        assert_eq!(tri.normal, Vec3::new(0.0, 0.0, 1.0));

        Ok(())
    }

    #[test]
    fn test_moller_trumbore_hit() -> TestResult {
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            1,
        );

        let ray = Ray::new(
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let hit = tri.intersect(&ray, 1);
        assert!(hit.is_some());
        let h = hit.ok_or("expected hit")?;
        assert!((h.t - 2.0).abs() < 1e-9);
        assert_eq!(h.point, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(h.normal, Vec3::new(0.0, 0.0, -1.0)); // front-face points toward origin

        Ok(())
    }
}
