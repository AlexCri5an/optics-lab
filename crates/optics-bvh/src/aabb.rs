//! Axis-Aligned Bounding Box (AABB) and ray-slab intersection testing.

use optics_core::{Ray, Vec3};

/// Axis-Aligned Bounding Box (AABB) in 3D world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// Minimum corner coordinate (lower bound).
    pub min: Vec3,
    /// Maximum corner coordinate (upper bound).
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::empty()
    }
}

impl Aabb {
    /// Creates an empty (inverted) bounding box suitable for growing.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min: Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    /// Creates an AABB enclosing a set of 3D points.
    #[must_use]
    pub fn from_points(points: &[Vec3]) -> Self {
        let mut aabb = Self::empty();
        for &pt in points {
            aabb.grow(pt);
        }
        aabb
    }

    /// Expands the AABB to enclose the given 3D point.
    pub fn grow(&mut self, pt: Vec3) {
        self.min.x = self.min.x.min(pt.x);
        self.min.y = self.min.y.min(pt.y);
        self.min.z = self.min.z.min(pt.z);

        self.max.x = self.max.x.max(pt.x);
        self.max.y = self.max.y.max(pt.y);
        self.max.z = self.max.z.max(pt.z);
    }

    /// Returns the union of this AABB with another AABB.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Returns the center point of the bounding box.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Returns the diagonal extent vector $(dx, dy, dz)$.
    #[must_use]
    pub fn extent(&self) -> Vec3 {
        self.max - self.min
    }

    /// Computes the total surface area of the bounding box.
    ///
    /// Formula:
    /// $$\text{SA} = 2 \cdot (dx \cdot dy + dy \cdot dz + dz \cdot dx)$$
    #[must_use]
    pub fn surface_area(&self) -> f64 {
        let d = self.extent();
        if d.x < 0.0 || d.y < 0.0 || d.z < 0.0 {
            return 0.0;
        }
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Tests ray intersection using the Kay–Kajiya / Smits slab method.
    ///
    /// Handles divisions by zero and NaNs safely by comparing $t_{\min}$ and $t_{\max}$.
    /// Returns `Some((t_enter, t_exit))` if a valid intersection occurs along $t \ge 0$.
    #[must_use]
    pub fn intersect_ray(&self, ray: &Ray) -> Option<(f64, f64)> {
        ray_aabb_slab_test(ray, self)
    }
}

/// Evaluates ray-AABB intersection using the exact slab method.
///
/// Returns `Some((t_enter, t_exit))` where $t_{\text{enter}} \le t_{\text{exit}}$ and $t_{\text{exit}} \ge 0.0$.
#[must_use]
pub fn ray_aabb_slab_test(ray: &Ray, aabb: &Aabb) -> Option<(f64, f64)> {
    let mut t_enter = 0.0_f64;
    let mut t_exit = f64::INFINITY;

    for i in 0..3 {
        let origin = ray.origin[i];
        let dir = ray.direction[i];
        let b_min = aabb.min[i];
        let b_max = aabb.max[i];

        if dir.abs() < 1e-12 {
            // Ray parallel to slab: must be inside slab boundaries
            if origin < b_min || origin > b_max {
                return None;
            }
        } else {
            let inv_d = 1.0 / dir;
            let mut t0 = (b_min - origin) * inv_d;
            let mut t1 = (b_max - origin) * inv_d;

            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }

            t_enter = t_enter.max(t0);
            t_exit = t_exit.min(t1);

            if t_enter > t_exit {
                return None;
            }
        }
    }

    if t_exit < 0.0 {
        return None;
    }

    Some((t_enter, t_exit))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_aabb_surface_area() -> TestResult {
        let aabb = Aabb {
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(2.0, 3.0, 4.0),
        };
        // SA = 2 * (2*3 + 3*4 + 4*2) = 2 * (6 + 12 + 8) = 2 * 26 = 52.0
        assert_eq!(aabb.surface_area(), 52.0);
        Ok(())
    }

    #[test]
    fn test_ray_slab_intersection_hit_and_miss() -> TestResult {
        let aabb = Aabb {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        };

        // 1. Ray through center
        let hit_ray = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;
        let res = ray_aabb_slab_test(&hit_ray, &aabb);
        assert!(res.is_some());
        let (t0, t1) = res.ok_or("expected hit")?;
        assert!((t0 - 4.0).abs() < 1e-9);
        assert!((t1 - 6.0).abs() < 1e-9);

        // 2. Ray missing AABB
        let miss_ray = Ray::new(Vec3::new(5.0, 5.0, -5.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;
        assert!(ray_aabb_slab_test(&miss_ray, &aabb).is_none());

        // 3. Ray pointing away from AABB (miss before)
        let away_ray = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, -1.0), 587.56, 1.0)?;
        assert!(ray_aabb_slab_test(&away_ray, &aabb).is_none());

        // 4. Ray past AABB pointing away (+z beyond max.z)
        let past_ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;
        assert!(ray_aabb_slab_test(&past_ray, &aabb).is_none());

        Ok(())
    }
}
