//! Biconic and Toroidal optical surface profiles and analytical gradients.
//!
//! # Optical Equations
//! - **Biconic Surface**:
//!   $$z(x, y) = \frac{c_x x^2 + c_y y^2}{1 + \sqrt{1 - (1+\kappa_x)c_x^2 x^2 - (1+\kappa_y)c_y^2 y^2}}$$
//! - **Toroidal Surface**:
//!   $$z(x, y) = R_x - \text{sign}(R_x) \sqrt{(R_x - z_y(y))^2 - x^2}$$
//!   where $z_y(y) = \frac{c_y y^2}{1 + \sqrt{1 - (1+\kappa_y)c_y^2 y^2}}$.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::Vec3;

/// Biconic optical surface with independent $x$ and $y$ curvatures and conic constants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiconicSurface {
    /// Curvature along the $x$-meridian $c_x = 1 / R_x$ in $\text{mm}^{-1}$.
    pub cx: f64,
    /// Conic constant along the $x$-meridian $\kappa_x$.
    pub kx: f64,
    /// Curvature along the $y$-meridian $c_y = 1 / R_y$ in $\text{mm}^{-1}$.
    pub cy: f64,
    /// Conic constant along the $y$-meridian $\kappa_y$.
    pub ky: f64,
    /// Maximum semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl BiconicSurface {
    /// Creates a new biconic surface with verified finite inputs.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `semi_diameter_mm < 0.0`.
    pub fn new(cx: f64, kx: f64, cy: f64, ky: f64, semi_diameter_mm: f64) -> Result<Self, OpticsError> {
        if !cx.is_finite()
            || !kx.is_finite()
            || !cy.is_finite()
            || !ky.is_finite()
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
        {
            return Err(OpticsError::InvalidParameter(
                "BiconicSurface parameters must be finite and semi-diameter non-negative".to_string(),
            ));
        }

        Ok(Self {
            cx,
            kx,
            cy,
            ky,
            semi_diameter_mm,
        })
    }

    /// Creates a biconic surface from radii of curvature in millimeters.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `semi_diameter_mm < 0.0`.
    pub fn from_radii(rx_mm: f64, kx: f64, ry_mm: f64, ky: f64, semi_diameter_mm: f64) -> Result<Self, OpticsError> {
        let cx = if rx_mm.is_infinite() || rx_mm.abs() < 1e-15 {
            0.0
        } else {
            1.0 / rx_mm
        };
        let cy = if ry_mm.is_infinite() || ry_mm.abs() < 1e-15 {
            0.0
        } else {
            1.0 / ry_mm
        };
        Self::new(cx, kx, cy, ky, semi_diameter_mm)
    }

    /// Radius of curvature along $x$-meridian in millimeters ($R_x = 1/c_x$).
    #[must_use]
    pub fn rx_mm(&self) -> f64 {
        if self.cx.abs() < 1e-15 {
            f64::INFINITY
        } else {
            1.0 / self.cx
        }
    }

    /// Radius of curvature along $y$-meridian in millimeters ($R_y = 1/c_y$).
    #[must_use]
    pub fn ry_mm(&self) -> f64 {
        if self.cy.abs() < 1e-15 {
            f64::INFINITY
        } else {
            1.0 / self.cy
        }
    }

    /// Evaluates the surface sag $z(x, y)$ at point $(x, y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $(x, y)$ falls outside the real surface domain.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(OpticsError::InvalidParameter("Coordinates must be finite".to_string()));
        }

        let num = self.cx * x * x + self.cy * y * y;
        let radicand = 1.0 - (1.0 + self.kx) * self.cx * self.cx * x * x - (1.0 + self.ky) * self.cy * self.cy * y * y;

        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }

        let s = radicand.max(0.0).sqrt();
        Ok(num / (1.0 + s))
    }

    /// Analytical partial derivatives $(\partial z / \partial x, \partial z / \partial y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] or [`OpticsError::NumericalSingularity`] if at or beyond boundary.
    pub fn derivatives(&self, x: f64, y: f64) -> Result<(f64, f64), OpticsError> {
        if x.abs() < 1e-15 && y.abs() < 1e-15 {
            return Ok((0.0, 0.0));
        }

        let num = self.cx * x * x + self.cy * y * y;
        let radicand = 1.0 - (1.0 + self.kx) * self.cx * self.cx * x * x - (1.0 + self.ky) * self.cy * self.cy * y * y;

        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }

        let s = radicand.max(0.0).sqrt();
        if s <= 1e-14 {
            return Err(OpticsError::NumericalSingularity(
                "Biconic surface normal singularity at boundary".to_string(),
            ));
        }

        let one_plus_s = 1.0 + s;
        let denom_sq = one_plus_s * one_plus_s;

        // dz/dx = (2 * cx * x) / (1 + s) + (num * (1 + kx) * cx^2 * x) / (s * (1 + s)^2)
        let dz_dx = (2.0 * self.cx * x) / one_plus_s + (num * (1.0 + self.kx) * self.cx * self.cx * x) / (s * denom_sq);

        // dz/dy = (2 * cy * y) / (1 + s) + (num * (1 + ky) * cy^2 * y) / (s * denom_sq)
        let dz_dy = (2.0 * self.cy * y) / one_plus_s + (num * (1.0 + self.ky) * self.cy * self.cy * y) / (s * denom_sq);

        Ok((dz_dx, dz_dy))
    }

    /// Evaluates the outward unit normal directed toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain or singular.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let (dz_dx, dz_dy) = self.derivatives(x, y)?;
        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }

    /// Intersects a ray with the biconic surface using Newton-Raphson iteration.
    ///
    /// # Errors
    /// Returns [`OpticsError::RayMissedSurface`] if no intersection is found within tolerances.
    pub fn intersect_ray(&self, ray: &crate::types::Ray, vertex_z: f64) -> Result<(f64, Vec3), OpticsError> {
        if ray.direction.z.abs() < 1e-15 {
            return Err(OpticsError::RayMissedSurface(
                "Ray parallel to vertex tangent plane".to_string(),
            ));
        }

        let mut t = (vertex_z - ray.origin.z) / ray.direction.z;
        if t < 0.0 {
            t = 0.0;
        }

        let max_iters = 35;
        let tol = 1e-11; // 10 picometers

        for _ in 0..max_iters {
            let p = ray.at(t);
            let r = (p.x * p.x + p.y * p.y).sqrt();

            if r > self.semi_diameter_mm * 1.5 {
                return Err(OpticsError::RayMissedSurface(
                    "Ray exceeded biconic radial aperture boundary".to_string(),
                ));
            }

            let sag = match self.sag(p.x, p.y) {
                Ok(s) => s,
                Err(_) => {
                    return Err(OpticsError::RayMissedSurface("Outside biconic sag domain".to_string()));
                }
            };

            let f = (p.z - vertex_z) - sag;
            if f.abs() < tol {
                if r > self.semi_diameter_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index: 0,
                        r,
                        max_r: self.semi_diameter_mm,
                    });
                }
                return Ok((t, p));
            }

            let (dz_dx, dz_dy) = match self.derivatives(p.x, p.y) {
                Ok(d) => d,
                Err(_) => return Err(OpticsError::RayMissedSurface("Singular biconic slope".to_string())),
            };

            let f_prime = ray.direction.z - (dz_dx * ray.direction.x + dz_dy * ray.direction.y);
            if f_prime.abs() < 1e-15 {
                return Err(OpticsError::NumericalSingularity(
                    "Newton-Raphson zero gradient derivative in biconic intersection".to_string(),
                ));
            }

            let delta_t = f / f_prime;
            t -= delta_t;
            if delta_t.abs() < 1e-12 {
                let final_p = ray.at(t);
                let final_r = (final_p.x * final_p.x + final_p.y * final_p.y).sqrt();
                if final_r > self.semi_diameter_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index: 0,
                        r: final_r,
                        max_r: self.semi_diameter_mm,
                    });
                }
                return Ok((t, final_p));
            }
        }

        Err(OpticsError::RayMissedSurface(
            "Newton-Raphson failed to converge on biconic surface".to_string(),
        ))
    }

    /// Paraxial curvatures $(c_x, c_y)$ along the principal meridians.
    #[must_use]
    pub const fn paraxial_curvatures(&self) -> (f64, f64) {
        (self.cx, self.cy)
    }
}

impl Default for BiconicSurface {
    fn default() -> Self {
        Self {
            cx: 1.0 / 50.0,
            kx: 0.0,
            cy: 1.0 / 100.0,
            ky: 0.0,
            semi_diameter_mm: 15.0,
        }
    }
}

/// Toroidal optical surface with base cross-cylinder curve and rotation radius.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToroidalSurface {
    /// Radius of rotation about axis parallel to $y$ ($R_x$ in mm).
    pub rx_mm: f64,
    /// Base curve curvature in $y$-$z$ plane $c_y = 1 / R_y$ in $\text{mm}^{-1}$.
    pub cy: f64,
    /// Base curve conic constant $\kappa_y$.
    pub ky: f64,
    /// Clear aperture semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl ToroidalSurface {
    /// Creates a new toroidal surface.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `rx_mm.abs() < 1e-12`.
    pub fn new(rx_mm: f64, cy: f64, ky: f64, semi_diameter_mm: f64) -> Result<Self, OpticsError> {
        if !rx_mm.is_finite()
            || !cy.is_finite()
            || !ky.is_finite()
            || !semi_diameter_mm.is_finite()
            || rx_mm.abs() < 1e-12
            || semi_diameter_mm < 0.0
        {
            return Err(OpticsError::InvalidParameter(
                "Toroidal surface requires finite parameters and non-zero rotation radius".to_string(),
            ));
        }

        Ok(Self {
            rx_mm,
            cy,
            ky,
            semi_diameter_mm,
        })
    }

    /// Creates a toroidal surface from rotation radius $R_x$ and base curve radius $R_y$ in mm.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any parameter is non-finite or invalid.
    pub fn from_radii(rx_mm: f64, ry_mm: f64, ky: f64, semi_diameter_mm: f64) -> Result<Self, OpticsError> {
        let cy = if ry_mm.is_infinite() || ry_mm.abs() < 1e-15 {
            0.0
        } else {
            1.0 / ry_mm
        };
        Self::new(rx_mm, cy, ky, semi_diameter_mm)
    }

    /// Base curve radius $R_y = 1 / c_y$ in millimeters.
    #[must_use]
    pub fn ry_mm(&self) -> f64 {
        if self.cy.abs() < 1e-15 {
            f64::INFINITY
        } else {
            1.0 / self.cy
        }
    }

    /// Evaluates base curve sag in the $y$-$z$ plane.
    fn base_curve_sag(&self, y: f64) -> Result<f64, OpticsError> {
        let y2 = y * y;
        let radicand = 1.0 - (1.0 + self.ky) * self.cy * self.cy * y2;
        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }
        let s = radicand.max(0.0).sqrt();
        Ok((self.cy * y2) / (1.0 + s))
    }

    /// Evaluates surface sag $z(x, y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if point lies outside the toroidal surface envelope.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        let zy = self.base_curve_sag(y)?;
        let rx = self.rx_mm;
        let term = rx - zy;
        let radicand = term * term - x * x;
        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }
        let s = radicand.max(0.0).sqrt();
        let sign = if rx >= 0.0 { 1.0 } else { -1.0 };
        Ok(rx - sign * s)
    }

    /// Analytical partial derivatives $(\partial z / \partial x, \partial z / \partial y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain.
    pub fn derivatives(&self, x: f64, y: f64) -> Result<(f64, f64), OpticsError> {
        let zy = self.base_curve_sag(y)?;
        let rx = self.rx_mm;
        let term = rx - zy;
        let radicand = term * term - x * x;
        if radicand <= 1e-14 {
            return Err(OpticsError::GeometryOutsideAperture);
        }
        let s = radicand.sqrt();
        let sign = if rx >= 0.0 { 1.0 } else { -1.0 };

        // dz/dx = sign * x / s
        let dz_dx = sign * x / s;

        // dzy/dy = (cy * y) / sqrt(1 - (1+ky)*cy^2*y^2)
        let base_rad = 1.0 - (1.0 + self.ky) * self.cy * self.cy * y * y;
        let base_s = base_rad.max(0.0).sqrt();
        let dzy_dy = if base_s > 1e-14 { (self.cy * y) / base_s } else { 0.0 };

        // dz/dy = sign * (term / s) * dzy/dy
        let dz_dy = sign * (term / s) * dzy_dy;

        Ok((dz_dx, dz_dy))
    }

    /// Evaluates outward unit normal pointing toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let (dz_dx, dz_dy) = self.derivatives(x, y)?;
        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }

    /// Intersects a ray with the toroidal surface using Newton-Raphson iteration.
    ///
    /// # Errors
    /// Returns [`OpticsError::RayMissedSurface`] if no intersection is found within tolerances.
    pub fn intersect_ray(&self, ray: &crate::types::Ray, vertex_z: f64) -> Result<(f64, Vec3), OpticsError> {
        if ray.direction.z.abs() < 1e-15 {
            return Err(OpticsError::RayMissedSurface(
                "Ray parallel to vertex tangent plane".to_string(),
            ));
        }

        let mut t = (vertex_z - ray.origin.z) / ray.direction.z;
        if t < 0.0 {
            t = 0.0;
        }

        let max_iters = 35;
        let tol = 1e-11; // 10 picometers

        for _ in 0..max_iters {
            let p = ray.at(t);
            let r = (p.x * p.x + p.y * p.y).sqrt();

            if r > self.semi_diameter_mm * 1.5 {
                return Err(OpticsError::RayMissedSurface(
                    "Ray exceeded toroidal radial aperture boundary".to_string(),
                ));
            }

            let sag = match self.sag(p.x, p.y) {
                Ok(s) => s,
                Err(_) => {
                    return Err(OpticsError::RayMissedSurface("Outside toroidal sag domain".to_string()));
                }
            };

            let f = (p.z - vertex_z) - sag;
            if f.abs() < tol {
                if r > self.semi_diameter_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index: 0,
                        r,
                        max_r: self.semi_diameter_mm,
                    });
                }
                return Ok((t, p));
            }

            let (dz_dx, dz_dy) = match self.derivatives(p.x, p.y) {
                Ok(d) => d,
                Err(_) => return Err(OpticsError::RayMissedSurface("Singular toroidal slope".to_string())),
            };

            let f_prime = ray.direction.z - (dz_dx * ray.direction.x + dz_dy * ray.direction.y);
            if f_prime.abs() < 1e-15 {
                return Err(OpticsError::NumericalSingularity(
                    "Newton-Raphson zero gradient derivative in toroidal intersection".to_string(),
                ));
            }

            let delta_t = f / f_prime;
            t -= delta_t;
            if delta_t.abs() < 1e-12 {
                let final_p = ray.at(t);
                let final_r = (final_p.x * final_p.x + final_p.y * final_p.y).sqrt();
                if final_r > self.semi_diameter_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index: 0,
                        r: final_r,
                        max_r: self.semi_diameter_mm,
                    });
                }
                return Ok((t, final_p));
            }
        }

        Err(OpticsError::RayMissedSurface(
            "Newton-Raphson failed to converge on toroidal surface".to_string(),
        ))
    }
}

impl Default for ToroidalSurface {
    fn default() -> Self {
        Self {
            rx_mm: 100.0,
            cy: 1.0 / 50.0,
            ky: 0.0,
            semi_diameter_mm: 15.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Ray;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_biconic_symmetric_matches_sphere() -> TestResult {
        let c = 1.0 / 50.0;
        let biconic = BiconicSurface::new(c, 0.0, c, 0.0, 20.0)?;

        for &x in &[0.0, 3.0, 7.5] {
            for &y in &[0.0, 4.0, 8.0] {
                let sag = biconic.sag(x, y)?;
                let r = (x * x + y * y).sqrt();
                let sphere_sag = (c * r * r) / (1.0 + (1.0 - c * c * r * r).sqrt());
                assert!(
                    (sag - sphere_sag).abs() < 1e-12,
                    "Symmetric biconic must match sphere sag"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_biconic_normal_matches_finite_differences() -> TestResult {
        let biconic = BiconicSurface::new(1.0 / 60.0, -0.8, 1.0 / 40.0, 0.2, 15.0)?;
        let x = 5.0;
        let y = 4.0;
        let eps = 1e-7;

        let z0 = biconic.sag(x, y)?;
        let z_xp = biconic.sag(x + eps, y)?;
        let z_yp = biconic.sag(x, y + eps)?;

        let num_dz_dx = (z_xp - z0) / eps;
        let num_dz_dy = (z_yp - z0) / eps;

        let (ana_dz_dx, ana_dz_dy) = biconic.derivatives(x, y)?;

        assert!((ana_dz_dx - num_dz_dx).abs() < 1e-6);
        assert!((ana_dz_dy - num_dz_dy).abs() < 1e-6);

        let n = biconic.normal(x, y)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }

    #[test]
    fn test_biconic_ray_intersection_symmetric() -> TestResult {
        let c = 1.0 / 100.0;
        let biconic = BiconicSurface::new(c, 0.0, c, 0.0, 25.0)?;

        // Ray launched from z = -50 towards vertex at z = 0, off-axis at (x=10, y=0)
        let ray = Ray::new(Vec3::new(10.0, 0.0, -50.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;

        let (t, hit_pt) = biconic.intersect_ray(&ray, 0.0)?;
        let expected_sag = biconic.sag(10.0, 0.0)?;

        assert!((hit_pt.x - 10.0).abs() < 1e-9);
        assert!(hit_pt.y.abs() < 1e-9);
        assert!((hit_pt.z - expected_sag).abs() < 1e-9);
        assert!((t - (50.0 + expected_sag)).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn test_toroidal_normal_matches_finite_differences() -> TestResult {
        let toroid = ToroidalSurface::new(100.0, 1.0 / 50.0, -0.5, 20.0)?;
        let x = 4.0;
        let y = 6.0;
        let eps = 1e-7;

        let z0 = toroid.sag(x, y)?;
        let z_xp = toroid.sag(x + eps, y)?;
        let z_yp = toroid.sag(x, y + eps)?;

        let num_dz_dx = (z_xp - z0) / eps;
        let num_dz_dy = (z_yp - z0) / eps;

        let (ana_dz_dx, ana_dz_dy) = toroid.derivatives(x, y)?;

        assert!((ana_dz_dx - num_dz_dx).abs() < 1e-6);
        assert!((ana_dz_dy - num_dz_dy).abs() < 1e-6);

        let n = toroid.normal(x, y)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }

    #[test]
    fn test_toroidal_ray_intersection() -> TestResult {
        let toroid = ToroidalSurface::new(120.0, 1.0 / 60.0, 0.0, 25.0)?;
        let ray = Ray::new(Vec3::new(5.0, 5.0, -40.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;

        let (t, hit_pt) = toroid.intersect_ray(&ray, 0.0)?;
        let expected_sag = toroid.sag(5.0, 5.0)?;

        assert!((hit_pt.x - 5.0).abs() < 1e-9);
        assert!((hit_pt.y - 5.0).abs() < 1e-9);
        assert!((hit_pt.z - expected_sag).abs() < 1e-9);
        assert!((t - (40.0 + expected_sag)).abs() < 1e-9);
        Ok(())
    }
}
