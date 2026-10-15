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
/// Returns [`OpticsError::NumericalSingularity`] if the gradient norm is near-zero or non-finite.
pub fn conic_normal(hit_point: Vec3, surface: &ConicSurface) -> Result<Vec3, OpticsError> {
    let local_z = hit_point.z - surface.vertex_z;
    let c = surface.curvature;
    let k = surface.conic_constant;

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
        let t = -pz / dz;
        if t <= 1e-9 {
            return Err(OpticsError::RayMissedSurface(
                "Plane intersection lies behind ray origin".to_string(),
            ));
        }

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
        if t1 > 1e-9 {
            candidate_t.push(t1);
        }
    }
    if a.abs() > 1e-30 && q.abs() > 1e-30 {
        let t2 = q / a;
        if t2 > 1e-9 {
            candidate_t.push(t2);
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

    #[test]
    fn test_plane_conic_sag_and_vertex() {
        let plane = ConicSurface::plane(10.0).expect("Plane should be valid");
        assert_eq!(conic_sag(0.0, plane.curvature, 0.0).unwrap(), 0.0);
        assert_eq!(conic_sag(5.0, plane.curvature, 0.0).unwrap(), 0.0);
        assert_eq!(plane.radius_mm(), f64::INFINITY);
    }

    #[test]
    fn test_parabola_sag_known_value() {
        // For a paraboloid (k = -1), the sag equation simplifies analytically:
        // z(r) = c * r^2 / (1 + sqrt(1 - 0)) = 0.5 * c * r^2 = r^2 / (2 * R)
        let radius = 100.0;
        let c = 1.0 / radius;
        let r = 10.0;
        let expected_sag = (r * r) / (2.0 * radius); // 100 / 200 = 0.5 mm

        let computed_sag = conic_sag(r, c, -1.0).expect("Parabola sag should succeed");
        assert!(
            (computed_sag - expected_sag).abs() < 1e-12,
            "Parabola sag mismatch: got {computed_sag}, expected {expected_sag}"
        );
    }

    #[test]
    fn test_sphere_intersection_equals_analytic_circle() {
        // In 2D meridian plane (y, z), a sphere centered at (0, 0, R) with radius R has equation:
        // y^2 + (z - R)^2 = R^2  => z^2 - 2 R z + y^2 = 0 => z = R - sqrt(R^2 - y^2)
        let radius = 50.0;
        let sphere = ConicSurface::sphere(radius, 25.0).expect("Sphere should be valid");

        let y_ray = 15.0;
        let ray = Ray::new(
            Vec3::new(0.0, y_ray, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )
        .expect("Ray should be valid");

        let hit = intersect_conic(&ray, &sphere, 1).expect("Ray must intersect sphere");
        let expected_z = radius - (radius * radius - y_ray * y_ray).sqrt();

        assert!(
            (hit.point.z - expected_z).abs() < 1e-12,
            "Sphere hit z mismatch: got {}, expected {}",
            hit.point.z,
            expected_z
        );
        assert_eq!(hit.point.y, y_ray);
    }

    #[test]
    fn test_near_zero_curvature_continuity() {
        // Test that c = 1e-12 produces nearly identical sag and intersection to c = 0.0
        let r = 8.0;
        let sag_zero = conic_sag(r, 0.0, 0.0).expect("c=0 sag");
        let sag_tiny = conic_sag(r, 1e-12, 0.0).expect("c=1e-12 sag");
        assert!((sag_tiny - sag_zero).abs() < 1e-10);

        let ray = Ray::new(
            Vec3::new(0.0, 5.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )
        .expect("Ray");

        let plane = ConicSurface::plane(20.0).expect("Plane");
        let tiny_sphere =
            ConicSurface::from_curvature(1e-12, 0.0, 20.0).expect("Tiny curvature surface");

        let hit_plane = intersect_conic(&ray, &plane, 0).expect("Plane hit");
        let hit_tiny = intersect_conic(&ray, &tiny_sphere, 0).expect("Tiny hit");

        assert!(
            (hit_plane.t - hit_tiny.t).abs() < 1e-9,
            "Plane vs tiny curvature t mismatch"
        );
    }

    #[test]
    fn test_conic_implicit_equation_satisfaction_1e12() {
        // Test that intersection points for random k values satisfy the implicit quadric:
        // c * (x^2 + y^2) - 2 * z + (1+k) * c * z^2 = 0
        let test_cases = [
            (50.0, 0.0),   // sphere
            (80.0, -1.0),  // parabola
            (60.0, -2.5),  // hyperbola
            (40.0, 0.5),   // oblate spheroid
        ];

        for (radius, k) in test_cases {
            let surf = ConicSurface::from_radius(radius, k, 20.0).expect("Surface");
            let c = surf.curvature;

            for y_val in [2.0, 5.0, 9.0, 12.0] {
                let ray = Ray::new(
                    Vec3::new(1.0, y_val, -15.0),
                    Vec3::new(0.05, 0.02, 0.9984988733).normalize(),
                    587.56,
                    1.0,
                )
                .expect("Ray");

                let hit = intersect_conic(&ray, &surf, 0).expect("Intersection must succeed");
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
    }

    #[test]
    fn test_normal_orthogonality_to_surface_tangent() {
        // Check that conic_normal is perpendicular to tangents computed via finite difference
        let surf = ConicSurface::from_radius(45.0, -0.5, 20.0).expect("Conic surface");
        let x = 4.0_f64;
        let y = 6.0_f64;
        let r = (x * x + y * y).sqrt();
        let z = conic_sag(r, surf.curvature, surf.conic_constant).expect("Sag");
        let hit_pt = Vec3::new(x, y, z);

        let normal = conic_normal(hit_pt, &surf).expect("Normal");

        // Finite difference tangents along x and y
        let eps = 1e-7;
        let rx = ((x + eps) * (x + eps) + y * y).sqrt();
        let zx = conic_sag(rx, surf.curvature, surf.conic_constant).expect("Sag x");
        let tangent_x = Vec3::new(eps, 0.0, zx - z).normalize();

        let ry = (x * x + (y + eps) * (y + eps)).sqrt();
        let zy = conic_sag(ry, surf.curvature, surf.conic_constant).expect("Sag y");
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
    }

    #[test]
    fn test_vignetted_and_missing_rays() {
        let surf = ConicSurface::sphere(60.0, 10.0).expect("Sphere");

        // Ray striking beyond semi-diameter of 10.0 mm
        let vignetted_ray = Ray::new(
            Vec3::new(0.0, 12.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )
        .expect("Ray");
        match intersect_conic(&vignetted_ray, &surf, 3) {
            Err(OpticsError::RayVignetted { surface_index, r, max_r }) => {
                assert_eq!(surface_index, 3);
                assert!((r - 12.0).abs() < 1e-6);
                assert_eq!(max_r, 10.0);
            }
            res => panic!("Expected RayVignetted, got {res:?}"),
        }

        // Ray missing the surface entirely (traveling backwards away from the lens)
        let backward_ray = Ray::new(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, -1.0),
            587.56,
            1.0,
        )
        .expect("Ray");
        assert!(matches!(
            intersect_conic(&backward_ray, &surf, 0),
            Err(OpticsError::RayMissedSurface(_))
        ));
    }
}
