//! Conic optical surfaces, sag equations, analytic normals, and numerically stable ray intersection.
//!
//! # Optical Conventions
//! - Coordinate System: light travels predominantly along $+z$.
//! - Surface vertices are located on the optical axis at $(0, 0, z_v)$.
//! - Curvature: $c = 1 / R$, with flat planes represented by $c = 0.0$.
//! - Positive radius of curvature ($R > 0$, $c > 0$) means the center of curvature lies on the $+z$ side
//!   of the vertex (convex toward $-z$).
//! - Negative radius of curvature ($R < 0$, $c < 0$) means the center of curvature lies on the $-z$ side
//!   of the vertex (concave toward $-z$).
//! - Normals: unit normal vectors point **toward the incident medium** (against $+z$, $\mathbf{d} \cdot \mathbf{n} \le 0$).

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::{HitRecord, Ray, Vec3};

/// A conic optical surface rotationally symmetric about the $z$-axis.
///
/// Defined by vertex position $z_v$, curvature $c = 1/R$, conic constant $k$, and clear aperture semi-diameter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConicSurface {
    /// Axial vertex position along the optical axis $z$ in millimeters.
    pub vertex_z: f64,
    /// Surface curvature $c = 1 / R$ in $\text{mm}^{-1}$ ($c = 0.0$ for planar surfaces).
    pub curvature: f64,
    /// Conic constant $k$ (Schwarzschild parameter):
    /// - $k = 0$: Sphere
    /// - $-1 < k < 0$: Prolate ellipsoid
    /// - $k = -1$: Paraboloid
    /// - $k < -1$: Hyperboloid
    /// - $k > 0$: Oblate spheroid
    pub conic_constant: f64,
    /// Clear aperture semi-diameter in millimeters (maximum radial extent $r_{\max}$).
    pub semi_diam_mm: f64,
}

impl ConicSurface {
    /// Creates a new conic surface from a radius of curvature in millimeters.
    ///
    /// Planar surfaces should pass `f64::INFINITY` (or use [`ConicSurface::plane`]).
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if `radius_mm == 0.0` or if any input is NaN.
    /// Returns [`OpticsError::InvalidSurface`] if `semi_diam_mm < 0.0`.
    pub fn from_radius(
        radius_mm: f64,
        conic_constant: f64,
        semi_diam_mm: f64,
    ) -> Result<Self, OpticsError> {
        if radius_mm.is_nan() || conic_constant.is_nan() || semi_diam_mm.is_nan() {
            return Err(OpticsError::NumericalSingularity(
                "ConicSurface parameter is NaN".to_string(),
            ));
        }
        if semi_diam_mm < 0.0 {
            return Err(OpticsError::InvalidSurface(
                "Semi-diameter cannot be negative".to_string(),
            ));
        }
        if radius_mm.abs() < 1e-15 {
            return Err(OpticsError::NumericalSingularity(
                "Radius of curvature cannot be zero (zero radius singularity)".to_string(),
            ));
        }

        let curvature = if radius_mm.is_infinite() {
            0.0
        } else {
            1.0 / radius_mm
        };

        Ok(Self {
            vertex_z: 0.0,
            curvature,
            conic_constant,
            semi_diam_mm,
        })
    }

    /// Creates a new conic surface directly from curvature $c = 1/R$ in $\text{mm}^{-1}$.
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if any input is NaN.
    /// Returns [`OpticsError::InvalidSurface`] if `semi_diam_mm < 0.0`.
    pub fn from_curvature(
        curvature: f64,
        conic_constant: f64,
        semi_diam_mm: f64,
    ) -> Result<Self, OpticsError> {
        if curvature.is_nan() || conic_constant.is_nan() || semi_diam_mm.is_nan() {
            return Err(OpticsError::NumericalSingularity(
                "ConicSurface parameter is NaN".to_string(),
            ));
        }
        if semi_diam_mm < 0.0 {
            return Err(OpticsError::InvalidSurface(
                "Semi-diameter cannot be negative".to_string(),
            ));
        }

        Ok(Self {
            vertex_z: 0.0,
            curvature,
            conic_constant,
            semi_diam_mm,
        })
    }

    /// Creates a spherical optical surface ($k = 0$).
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if `radius_mm == 0.0` or NaN.
    pub fn sphere(radius_mm: f64, semi_diam_mm: f64) -> Result<Self, OpticsError> {
        Self::from_radius(radius_mm, 0.0, semi_diam_mm)
    }

    /// Creates a planar optical surface ($c = 0$, $k = 0$).
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidSurface`] if `semi_diam_mm < 0.0`.
    pub fn plane(semi_diam_mm: f64) -> Result<Self, OpticsError> {
        Self::from_curvature(0.0, 0.0, semi_diam_mm)
    }

    /// Sets the axial vertex position $z_v$ along the optical axis in millimeters.
    #[must_use]
    pub fn with_vertex_z(mut self, vertex_z: f64) -> Self {
        self.vertex_z = vertex_z;
        self
    }

    /// Returns the radius of curvature in millimeters ($R = 1/c$).
    ///
    /// Returns `f64::INFINITY` if curvature $c = 0.0$.
    #[must_use]
    pub fn radius_mm(&self) -> f64 {
        if self.curvature.abs() < 1e-15 {
            f64::INFINITY
        } else {
            1.0 / self.curvature
        }
    }
}

/// Evaluates the axial sag $z(r)$ of a conic surface at radial distance $r = \sqrt{x^2 + y^2}$.
///
/// # Formula
/// $$z(r) = \frac{c\, r^2}{1 + \sqrt{1 - (1+k)\, c^2 r^2}}$$
///
/// # Numerical Stability
/// The formula is well-conditioned for flat surfaces: as $c \to 0$, $z(r) \to 0$ continuously
/// without division by zero.
///
/// # Errors
/// Returns [`OpticsError::GeometryOutsideAperture`] if $(1+k) c^2 r^2 > 1$, which corresponds
/// to coordinates beyond the real turning point of the conic section (no real geometric solution).
pub fn conic_sag(r: f64, c: f64, k: f64) -> Result<f64, OpticsError> {
    if !r.is_finite() || !c.is_finite() || !k.is_finite() {
        return Err(OpticsError::NumericalSingularity(
            "Conic sag evaluated with non-finite parameters".to_string(),
        ));
    }

    if c.abs() < 1e-15 || r.abs() < 1e-15 {
        return Ok(0.0);
    }

    let r_sq = r * r;
    let radicand = 1.0 - (1.0 + k) * c * c * r_sq;
    if radicand < 0.0 {
        return Err(OpticsError::GeometryOutsideAperture);
    }

    let denom = 1.0 + radicand.sqrt();
    Ok((c * r_sq) / denom)
}

/// Evaluates the analytical surface normal unit vector at a point on a conic surface.
///
/// The surface is defined implicitly by the quadric:
/// $$F(x, y, z) = c (x^2 + y^2) - 2 (z - z_v) + (1+k) c (z - z_v)^2 = 0$$
///
/// The gradient $\nabla F$ is:
/// $$\nabla F = \left( 2 c\, x,\; 2 c\, y,\; 2 (1+k) c (z - z_v) - 2 \right)$$
///
/// Dividing by 2 yields the unnormalized vector:
/// $$\mathbf{g} = \left( c\, x,\; c\, y,\; (1+k) c (z - z_v) - 1 \right)$$
///
/// At $x=0, y=0$, $\mathbf{g} = (0, 0, -1)$, which points against $+z$ toward the incident side,
/// complying with the repository-wide normal convention ($\mathbf{i} \cdot \mathbf{n} \le 0$).
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if hit point coordinates or surface parameters are non-finite,
/// or if the gradient norm is near-zero.
/// Returns [`OpticsError::GeometryOutsideAperture`] if $(1+k) c^2 r^2 > 1$, which corresponds
/// to coordinates beyond the real turning point of the conic section (surface mathematically undefined).
pub fn conic_normal(hit_point: Vec3, surface: &ConicSurface) -> Result<Vec3, OpticsError> {
    if !hit_point.x.is_finite()
        || !hit_point.y.is_finite()
        || !hit_point.z.is_finite()
        || !surface.curvature.is_finite()
        || !surface.conic_constant.is_finite()
        || !surface.vertex_z.is_finite()
    {
        return Err(OpticsError::NumericalSingularity(
            "Conic normal hit point coordinates or surface parameters are non-finite".to_string(),
        ));
    }

    let c = surface.curvature;
    let k = surface.conic_constant;
    let r_sq = hit_point.x * hit_point.x + hit_point.y * hit_point.y;
    let radicand = 1.0 - (1.0 + k) * c * c * r_sq;
    if radicand < 0.0 {
        return Err(OpticsError::GeometryOutsideAperture);
    }

    let local_z = hit_point.z - surface.vertex_z;

    let gx = c * hit_point.x;
    let gy = c * hit_point.y;
    let gz = (1.0 + k) * c * local_z - 1.0;

    let norm_sq = gx * gx + gy * gy + gz * gz;
    if norm_sq < 1e-30 || !norm_sq.is_finite() {
        return Err(OpticsError::NumericalSingularity(
            "Conic normal gradient has zero or non-finite norm".to_string(),
        ));
    }

    let norm = norm_sq.sqrt();
    Ok(Vec3::new(gx / norm, gy / norm, gz / norm))
}

/// Intersects a 3D ray with a conic surface using the Citardauque method.
///
/// # Citardauque Quadratic Root Method
/// The intersection of a ray $\vec{p}(t) = \vec{p}_0 + t \vec{d}$ with the quadric
/// conic equation $c (x^2 + y^2) - 2 z + (1+k) c z^2 = 0$ yields the quadratic:
/// $$A t^2 + B t + C = 0$$
/// where:
/// - $A = c (d_x^2 + d_y^2 + (1+k) d_z^2)$
/// - $B = 2 [c (p_x d_x + p_y d_y + (1+k) p_z d_z) - d_z]$
/// - $C = c (p_x^2 + p_y^2 + (1+k) p_z^2) - 2 p_z$
///
/// with $\vec{p} = \vec{p}_0 - (0, 0, z_v)$.
///
/// To prevent catastrophic cancellation and division by zero when $A \to 0$ (such as flat surfaces
/// where $c \to 0$, or near-axial rays on parabolic surfaces), we use the Citardauque rearranged root:
/// $$t_1 = \frac{2 C}{-B - \operatorname{sign}(B) \sqrt{B^2 - 4 A C}}, \quad t_2 = \frac{C}{A\, t_1}$$
///
/// # Root Selection & Physical Validation
/// 1. Candidate roots $t > 10^{-9}$ are evaluated.
/// 2. For each candidate point, the local $z$ is checked against the physical vertex sag branch $z_{\text{sag}}(r)$.
/// 3. Semi-diameter clipping is enforced: if $r > r_{\text{semi-diameter}}$, [`OpticsError::RayVignetted`] is returned.
/// 4. The closest forward physical intersection along the ray is selected.
///
/// # Errors
/// Returns [`OpticsError::RayMissedSurface`] if the ray misses the quadric surface or vertex sheet.
/// Returns [`OpticsError::RayVignetted`] if the ray strikes outside the surface clear semi-diameter.
pub fn intersect_conic(
    ray: &Ray,
    surface: &ConicSurface,
    surface_index: usize,
) -> Result<HitRecord, OpticsError> {
    let c = surface.curvature;
    let k = surface.conic_constant;

    // Shift ray origin relative to surface vertex
    let px = ray.origin.x;
    let py = ray.origin.y;
    let pz = ray.origin.z - surface.vertex_z;

    let dx = ray.direction.x;
    let dy = ray.direction.y;
    let dz = ray.direction.z;

    // Planar surface optimization (c = 0)
    if c.abs() < 1e-15 {
        if dz.abs() < 1e-15 {
            return Err(OpticsError::RayMissedSurface(
                "Ray is parallel to planar surface".to_string(),
            ));
        }
        let t_raw = -pz / dz;
        if t_raw < -1e-7 {
            return Err(OpticsError::RayMissedSurface(
                "Plane intersection lies behind ray origin".to_string(),
            ));
        }
        let t = t_raw.max(0.0);

        let hit_pt = ray.at(t);
        let r = (hit_pt.x * hit_pt.x + hit_pt.y * hit_pt.y).sqrt();
        if r > surface.semi_diam_mm {
            return Err(OpticsError::RayVignetted {
                surface_index,
                r,
                max_r: surface.semi_diam_mm,
            });
        }

        let normal = Vec3::new(0.0, 0.0, -1.0);
        let is_front_face = ray.direction.dot(&normal) < 0.0;
        return Ok(HitRecord {
            t,
            point: hit_pt,
            normal,
            is_front_face,
            surface_index,
        });
    }

    // General conic quadric coefficients: A t^2 + B t + C = 0
    let one_plus_k = 1.0 + k;
    let a = c * (dx * dx + dy * dy + one_plus_k * dz * dz);
    let b = 2.0 * (c * (px * dx + py * dy + one_plus_k * pz * dz) - dz);
    let c_coeff = c * (px * px + py * py + one_plus_k * pz * pz) - 2.0 * pz;

    let discriminant = b * b - 4.0 * a * c_coeff;
    if discriminant < 0.0 {
        return Err(OpticsError::RayMissedSurface(
            "Negative quadric discriminant".to_string(),
        ));
    }

    let sqrt_disc = discriminant.sqrt();
    let sign_b = if b >= 0.0 { 1.0 } else { -1.0 };
    let q = -0.5 * (b + sign_b * sqrt_disc);

    // Citardauque stable roots
    let mut candidate_t: Vec<f64> = Vec::with_capacity(2);
    if q.abs() > 1e-30 {
        let t1 = c_coeff / q;
        if t1 > -1e-7 {
            candidate_t.push(t1.max(0.0));
        }
    }
    if a.abs() > 1e-30 && q.abs() > 1e-30 {
        let t2 = q / a;
        if t2 > -1e-7 {
            candidate_t.push(t2.max(0.0));
        }
    }

    // Sort candidate roots ascending (closest forward intersection first)
    candidate_t.sort_by(|x, y| x.total_cmp(y));

    for t in candidate_t {
        let hit_pt = ray.at(t);
        let local_z = hit_pt.z - surface.vertex_z;
        let r = (hit_pt.x * hit_pt.x + hit_pt.y * hit_pt.y).sqrt();

        // Verify that this root lies on the physical vertex sheet of the conic sag equation
        if let Ok(expected_sag) = conic_sag(r, c, k) {
            if (local_z - expected_sag).abs() < 1e-4 {
                // Point is on physical sheet. Check clear semi-diameter:
                if r > surface.semi_diam_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index,
                        r,
                        max_r: surface.semi_diam_mm,
                    });
                }

                let normal = conic_normal(hit_pt, surface)?;
                let is_front_face = ray.direction.dot(&normal) < 0.0;

                return Ok(HitRecord {
                    t,
                    point: hit_pt,
                    normal,
                    is_front_face,
                    surface_index,
                });
            }
        }
    }

    Err(OpticsError::RayMissedSurface(
        "No intersection with physical conic vertex sheet".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_plane_conic_sag_and_vertex() -> TestResult {
        let plane = ConicSurface::plane(10.0)?;
        assert_eq!(conic_sag(0.0, plane.curvature, 0.0)?, 0.0);
        assert_eq!(conic_sag(5.0, plane.curvature, 0.0)?, 0.0);
        assert_eq!(plane.radius_mm(), f64::INFINITY);
        Ok(())
    }

    #[test]
    fn test_parabola_sag_known_value() -> TestResult {
        let radius = 100.0;
        let c = 1.0 / radius;
        let r = 10.0;
        let expected_sag = (r * r) / (2.0 * radius); // 100 / 200 = 0.5 mm

        let computed_sag = conic_sag(r, c, -1.0)?;
        assert!(
            (computed_sag - expected_sag).abs() < 1e-12,
            "Parabola sag mismatch: got {computed_sag}, expected {expected_sag}"
        );
        Ok(())
    }

    #[test]
    fn test_sphere_intersection_equals_analytic_circle() -> TestResult {
        let radius = 50.0;
        let sphere = ConicSurface::sphere(radius, 25.0)?;

        let y_ray = 15.0;
        let ray = Ray::new(
            Vec3::new(0.0, y_ray, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let hit = intersect_conic(&ray, &sphere, 1)?;
        let expected_z = radius - (radius * radius - y_ray * y_ray).sqrt();

        assert!(
            (hit.point.z - expected_z).abs() < 1e-12,
            "Sphere hit z mismatch: got {}, expected {}",
            hit.point.z,
            expected_z
        );
        assert_eq!(hit.point.y, y_ray);
        Ok(())
    }

    #[test]
    fn test_near_zero_curvature_continuity() -> TestResult {
        let r = 8.0;
        let sag_zero = conic_sag(r, 0.0, 0.0)?;
        let sag_tiny = conic_sag(r, 1e-12, 0.0)?;
        assert!((sag_tiny - sag_zero).abs() < 1e-10);

        let ray = Ray::new(
            Vec3::new(0.0, 5.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let plane = ConicSurface::plane(20.0)?;
        let tiny_sphere = ConicSurface::from_curvature(1e-12, 0.0, 20.0)?;

        let hit_plane = intersect_conic(&ray, &plane, 0)?;
        let hit_tiny = intersect_conic(&ray, &tiny_sphere, 0)?;

        assert!(
            (hit_plane.t - hit_tiny.t).abs() < 1e-9,
            "Plane vs tiny curvature t mismatch"
        );
        Ok(())
    }

    #[test]
    fn test_conic_implicit_equation_satisfaction_1e12() -> TestResult {
        let test_cases = [
            (50.0, 0.0),  // sphere
            (80.0, -1.0), // parabola
            (60.0, -2.5), // hyperbola
            (40.0, 0.5),  // oblate spheroid
        ];

        for (radius, k) in test_cases {
            let surf = ConicSurface::from_radius(radius, k, 20.0)?;
            let c = surf.curvature;

            for y_val in [2.0, 5.0, 9.0, 12.0] {
                let ray = Ray::new(
                    Vec3::new(1.0, y_val, -15.0),
                    Vec3::new(0.05, 0.02, 0.9984988733).normalize(),
                    587.56,
                    1.0,
                )?;

                let hit = intersect_conic(&ray, &surf, 0)?;
                let x = hit.point.x;
                let y = hit.point.y;
                let z = hit.point.z - surf.vertex_z;

                let quadric_val = c * (x * x + y * y) - 2.0 * z + (1.0 + k) * c * z * z;
                assert!(
                    quadric_val.abs() < 1e-12,
                    "Implicit quadric error {quadric_val} too large for R={radius}, k={k}, y={y_val}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_normal_orthogonality_to_surface_tangent() -> TestResult {
        let surf = ConicSurface::from_radius(45.0, -0.5, 20.0)?;
        let x = 4.0_f64;
        let y = 6.0_f64;
        let r = (x * x + y * y).sqrt();
        let z = conic_sag(r, surf.curvature, surf.conic_constant)?;
        let hit_pt = Vec3::new(x, y, z);

        let normal = conic_normal(hit_pt, &surf)?;

        let eps = 1e-7;
        let rx = ((x + eps) * (x + eps) + y * y).sqrt();
        let zx = conic_sag(rx, surf.curvature, surf.conic_constant)?;
        let tangent_x = Vec3::new(eps, 0.0, zx - z).normalize();

        let ry = (x * x + (y + eps) * (y + eps)).sqrt();
        let zy = conic_sag(ry, surf.curvature, surf.conic_constant)?;
        let tangent_y = Vec3::new(0.0, eps, zy - z).normalize();

        assert!(
            normal.dot(&tangent_x).abs() < 1e-6,
            "Normal not perpendicular to tangent x: dot={}",
            normal.dot(&tangent_x)
        );
        assert!(
            normal.dot(&tangent_y).abs() < 1e-6,
            "Normal not perpendicular to tangent y: dot={}",
            normal.dot(&tangent_y)
        );
        Ok(())
    }

    #[test]
    fn test_vignetted_and_missing_rays() -> TestResult {
        let surf = ConicSurface::sphere(60.0, 10.0)?;

        let vignetted_ray = Ray::new(
            Vec3::new(0.0, 12.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;
        let res = intersect_conic(&vignetted_ray, &surf, 3);
        assert!(matches!(
            res,
            Err(OpticsError::RayVignetted {
                surface_index: 3,
                ..
            })
        ));

        let backward_ray = Ray::new(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, -1.0),
            587.56,
            1.0,
        )?;
        assert!(matches!(
            intersect_conic(&backward_ray, &surf, 0),
            Err(OpticsError::RayMissedSurface(_))
        ));
        Ok(())
    }

    #[test]
    fn test_conic_normal_unit_length_across_conic_types() -> TestResult {
        // Test suite covering spheres, ellipsoids, paraboloids, hyperboloids, and planes
        let test_surfaces = [
            ConicSurface::sphere(50.0, 25.0)?,            // Sphere (k = 0)
            ConicSurface::from_radius(60.0, -0.5, 25.0)?, // Prolate ellipsoid
            ConicSurface::from_radius(80.0, -1.0, 30.0)?, // Paraboloid
            ConicSurface::from_radius(70.0, -2.5, 30.0)?, // Hyperboloid
            ConicSurface::from_radius(45.0, 0.8, 20.0)?,  // Oblate spheroid
            ConicSurface::plane(30.0)?,                   // Flat plane (c = 0)
        ];

        for surf in test_surfaces {
            for (x, y) in [(0.0_f64, 0.0_f64), (3.0, 4.0), (-5.0, 8.0), (12.0, -7.0)] {
                let r: f64 = (x * x + y * y).sqrt();
                let z = conic_sag(r, surf.curvature, surf.conic_constant)?;
                let hit_pt = Vec3::new(x, y, z + surf.vertex_z);

                let normal = conic_normal(hit_pt, &surf)?;
                let norm = normal.norm();
                assert!(
                    (norm - 1.0).abs() < 1e-14,
                    "Unit normal length violation: got {norm} for R={}, k={}, r={r}",
                    surf.radius_mm(),
                    surf.conic_constant
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_conic_normal_agrees_with_independent_analytical_derivation() -> TestResult {
        // Independent analytical formula:
        // n = [c*x, c*y, -sqrt(1 - (1+k)*c^2*r^2)] / sqrt(1 - k*c^2*r^2)
        let test_cases: [(f64, f64, f64, f64); 5] = [
            (50.0, 0.0, 10.0, 15.0),   // Sphere
            (60.0, -0.6, 8.0, 12.0),   // Ellipsoid
            (100.0, -1.0, 14.0, 18.0), // Paraboloid
            (75.0, -2.0, 9.0, 16.0),   // Hyperboloid
            (40.0, 0.5, 5.0, 7.0),     // Oblate spheroid
        ];

        for (radius, k, x, y) in test_cases {
            let surf = ConicSurface::from_radius(radius, k, 30.0)?;
            let c = surf.curvature;
            let r_sq: f64 = x * x + y * y;
            let r = r_sq.sqrt();
            let z = conic_sag(r, c, k)?;
            let hit_pt = Vec3::new(x, y, z + surf.vertex_z);

            let normal = conic_normal(hit_pt, &surf)?;

            let radicand_z = 1.0 - (1.0 + k) * c * c * r_sq;
            assert!(radicand_z >= 0.0);
            let denom = (1.0 - k * c * c * r_sq).sqrt();

            let expected_nx = c * x / denom;
            let expected_ny = c * y / denom;
            let expected_nz = -radicand_z.sqrt() / denom;

            assert!(
                (normal.x - expected_nx).abs() < 1e-12,
                "Analytical nx mismatch: got {}, expected {}",
                normal.x,
                expected_nx
            );
            assert!(
                (normal.y - expected_ny).abs() < 1e-12,
                "Analytical ny mismatch: got {}, expected {}",
                normal.y,
                expected_ny
            );
            assert!(
                (normal.z - expected_nz).abs() < 1e-12,
                "Analytical nz mismatch: got {}, expected {}",
                normal.z,
                expected_nz
            );
        }
        Ok(())
    }

    #[test]
    fn test_conic_normal_tangent_plane_orthogonality_analytic_and_numeric() -> TestResult {
        let surf = ConicSurface::from_radius(55.0, -1.5, 25.0)?.with_vertex_z(12.0);
        let x = 6.0_f64;
        let y = 9.0_f64;
        let r_sq: f64 = x * x + y * y;
        let r = r_sq.sqrt();
        let c = surf.curvature;
        let k = surf.conic_constant;
        let z_local = conic_sag(r, c, k)?;
        let hit_pt = Vec3::new(x, y, z_local + surf.vertex_z);

        let normal = conic_normal(hit_pt, &surf)?;

        // 1. Analytical tangent plane vectors:
        // dz/dx = c*x / sqrt(1 - (1+k)*c^2*r^2)
        // dz/dy = c*y / sqrt(1 - (1+k)*c^2*r^2)
        let sqrt_rad = (1.0 - (1.0 + k) * c * c * r_sq).sqrt();
        let dz_dx = c * x / sqrt_rad;
        let dz_dy = c * y / sqrt_rad;
        let t_x_analytic = Vec3::new(1.0, 0.0, dz_dx);
        let t_y_analytic = Vec3::new(0.0, 1.0, dz_dy);

        assert!(
            normal.dot(&t_x_analytic).abs() < 1e-13,
            "Normal not orthogonal to analytic t_x: dot = {}",
            normal.dot(&t_x_analytic)
        );
        assert!(
            normal.dot(&t_y_analytic).abs() < 1e-13,
            "Normal not orthogonal to analytic t_y: dot = {}",
            normal.dot(&t_y_analytic)
        );

        // 2. Central finite difference numeric tangents:
        let h = 1e-6;
        let r_xp = ((x + h) * (x + h) + y * y).sqrt();
        let r_xm = ((x - h) * (x - h) + y * y).sqrt();
        let dz_dx_num = (conic_sag(r_xp, c, k)? - conic_sag(r_xm, c, k)?) / (2.0 * h);
        let t_x_num = Vec3::new(1.0, 0.0, dz_dx_num).normalize();

        let r_yp = (x * x + (y + h) * (y + h)).sqrt();
        let r_ym = (x * x + (y - h) * (y - h)).sqrt();
        let dz_dy_num = (conic_sag(r_yp, c, k)? - conic_sag(r_ym, c, k)?) / (2.0 * h);
        let t_y_num = Vec3::new(0.0, 1.0, dz_dy_num).normalize();

        assert!(
            normal.dot(&t_x_num).abs() < 1e-6,
            "Normal not orthogonal to numeric t_x: dot = {}",
            normal.dot(&t_x_num)
        );
        assert!(
            normal.dot(&t_y_num).abs() < 1e-6,
            "Normal not orthogonal to numeric t_y: dot = {}",
            normal.dot(&t_y_num)
        );
        Ok(())
    }

    #[test]
    fn test_spherical_normal_at_off_axis_points() -> TestResult {
        // Positive radius (convex toward -z, center of curvature at (0, 0, zv + R))
        let r_pos = 100.0;
        let zv = 15.0;
        let sphere_pos = ConicSurface::sphere(r_pos, 60.0)?.with_vertex_z(zv);
        let center_pos = Vec3::new(0.0, 0.0, zv + r_pos);

        let (x, y) = (28.0_f64, 35.0_f64);
        let r: f64 = (x * x + y * y).sqrt();
        let z_local = sphere_pos.radius_mm()
            - (sphere_pos.radius_mm() * sphere_pos.radius_mm() - r * r).sqrt();
        let hit_pt = Vec3::new(x, y, zv + z_local);

        let normal = conic_normal(hit_pt, &sphere_pos)?;

        // Vector from center of curvature toward surface hit point
        let expected_dir = (hit_pt - center_pos).normalize();
        assert!(
            (normal.x - expected_dir.x).abs() < 1e-14,
            "Sphere normal x mismatch: got {}, expected {}",
            normal.x,
            expected_dir.x
        );
        assert!(
            (normal.y - expected_dir.y).abs() < 1e-14,
            "Sphere normal y mismatch: got {}, expected {}",
            normal.y,
            expected_dir.y
        );
        assert!(
            (normal.z - expected_dir.z).abs() < 1e-14,
            "Sphere normal z mismatch: got {}, expected {}",
            normal.z,
            expected_dir.z
        );

        // Negative radius (concave toward -z, center of curvature at (0, 0, zv + R))
        let r_neg = -100.0;
        let sphere_neg = ConicSurface::from_radius(r_neg, 0.0, 60.0)?.with_vertex_z(zv);
        let center_neg = Vec3::new(0.0, 0.0, zv + r_neg);
        let z_local_neg = conic_sag(r, sphere_neg.curvature, 0.0)?;
        let hit_pt_neg = Vec3::new(x, y, zv + z_local_neg);

        let normal_neg = conic_normal(hit_pt_neg, &sphere_neg)?;
        // For R < 0, (hit_pt - center) has length |R| and z-component > 0.
        // The normal toward incident medium points against +z, so -(hit_pt - center)/|R|:
        let expected_dir_neg = (center_neg - hit_pt_neg).normalize();
        assert!(
            (normal_neg.x - expected_dir_neg.x).abs() < 1e-14,
            "Concave sphere normal x mismatch"
        );
        assert!(
            (normal_neg.y - expected_dir_neg.y).abs() < 1e-14,
            "Concave sphere normal y mismatch"
        );
        assert!(
            (normal_neg.z - expected_dir_neg.z).abs() < 1e-14,
            "Concave sphere normal z mismatch"
        );
        Ok(())
    }

    #[test]
    fn test_planar_conic_normal_exactness() -> TestResult {
        let plane = ConicSurface::plane(50.0)?.with_vertex_z(10.0);

        // Everywhere on a plane with light travelling +z, the unit normal must be (0, 0, -1)
        for (x, y) in [
            (0.0_f64, 0.0_f64),
            (10.0, 20.0),
            (-35.0, 15.0),
            (45.0, -45.0),
        ] {
            let hit_pt = Vec3::new(x, y, 10.0);
            let normal = conic_normal(hit_pt, &plane)?;

            assert_eq!(normal.x, 0.0);
            assert_eq!(normal.y, 0.0);
            assert_eq!(normal.z, -1.0);
            assert_eq!(normal.norm(), 1.0);
        }
        Ok(())
    }

    #[test]
    fn test_domain_boundary_valid_and_invalid_cases() -> TestResult {
        let radius = 50.0;
        let sphere = ConicSurface::sphere(radius, 50.0)?;

        // 1. Valid exact boundary: r = R = 50.0 mm
        let boundary_pt = Vec3::new(radius, 0.0, radius);
        let boundary_normal = conic_normal(boundary_pt, &sphere)?;
        assert!((boundary_normal.norm() - 1.0).abs() < 1e-14);
        assert!((boundary_normal.x - 1.0).abs() < 1e-14);
        assert_eq!(boundary_normal.y, 0.0);
        assert!(boundary_normal.z.abs() < 1e-14);

        // 2. Valid near-boundary: r = 49.999 mm
        let r_near = 49.999;
        let z_near = conic_sag(r_near, sphere.curvature, 0.0)?;
        let near_pt = Vec3::new(r_near, 0.0, z_near);
        let near_normal = conic_normal(near_pt, &sphere)?;
        assert!((near_normal.norm() - 1.0).abs() < 1e-14);

        // 3. Invalid beyond boundary: r = 50.001 mm and r = 60.0 mm ((1+k)*c^2*r^2 > 1)
        let outside_pt1 = Vec3::new(50.001, 0.0, 50.0);
        let res1 = conic_normal(outside_pt1, &sphere);
        assert!(matches!(res1, Err(OpticsError::GeometryOutsideAperture)));

        let outside_pt2 = Vec3::new(0.0, 60.0, 50.0);
        let res2 = conic_normal(outside_pt2, &sphere);
        assert!(matches!(res2, Err(OpticsError::GeometryOutsideAperture)));

        // 4. Oblate spheroid boundary: k = 1.0, r_max = R / sqrt(2)
        let oblate = ConicSurface::from_radius(50.0, 1.0, 40.0)?;
        let r_max = 50.0 / 2.0_f64.sqrt(); // ~35.3553 mm
        let oblate_outside = Vec3::new(r_max + 1.0, 0.0, 10.0);
        let res_oblate = conic_normal(oblate_outside, &oblate);
        assert!(matches!(
            res_oblate,
            Err(OpticsError::GeometryOutsideAperture)
        ));
        Ok(())
    }

    #[test]
    fn test_non_finite_inputs_and_numerical_singularities() -> TestResult {
        let sphere = ConicSurface::sphere(50.0, 25.0)?;

        // NaN hit point coordinates
        let nan_x = Vec3::new(f64::NAN, 5.0, 1.0);
        assert!(matches!(
            conic_normal(nan_x, &sphere),
            Err(OpticsError::NumericalSingularity(_))
        ));

        let nan_z = Vec3::new(5.0, 5.0, f64::NAN);
        assert!(matches!(
            conic_normal(nan_z, &sphere),
            Err(OpticsError::NumericalSingularity(_))
        ));

        // Infinite hit point coordinates
        let inf_x = Vec3::new(f64::INFINITY, 0.0, 0.0);
        assert!(matches!(
            conic_normal(inf_x, &sphere),
            Err(OpticsError::NumericalSingularity(_))
        ));

        // Vanishing gradient singularity: hit point at the center of curvature (0, 0, zv + R)
        let center_of_curv = Vec3::new(0.0, 0.0, 50.0);
        assert!(matches!(
            conic_normal(center_of_curv, &sphere),
            Err(OpticsError::NumericalSingularity(_))
        ));

        // Non-finite surface parameters
        let invalid_surf = ConicSurface {
            vertex_z: 0.0,
            curvature: f64::NAN,
            conic_constant: 0.0,
            semi_diam_mm: 10.0,
        };
        assert!(matches!(
            conic_normal(Vec3::new(0.0, 0.0, 0.0), &invalid_surf),
            Err(OpticsError::NumericalSingularity(_))
        ));
        Ok(())
    }
}
