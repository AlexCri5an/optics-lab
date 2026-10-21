//! Even and odd aspheric optical surface profiles and analytical gradients.
//!
//! # Optical Equations
//! For an even asphere with vertex curvature $c = 1/R$ and conic constant $\kappa$:
//! $$z(r) = \frac{c r^2}{1 + \sqrt{1 - (1+\kappa) c^2 r^2}} + \sum_{i=1}^{N} \alpha_{2i} r^{2i}$$
//! where coefficients $\alpha_2, \alpha_4, \dots, \alpha_{20}$ denote deformation terms.
//!
//! An odd asphere additionally incorporates odd polynomial powers $\sum_{j=1}^{M} \beta_j r^j$.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::{Ray, Vec3};

/// Even asphere surface profile with deformation coefficients up to $r^{20}$.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvenAsphere {
    /// Vertex curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Conic constant $\kappa$.
    pub conic_constant: f64,
    /// Even polynomial coefficients $[\alpha_2, \alpha_4, \alpha_6, \dots, \alpha_{20}]$ (indices 0..10).
    pub coefficients: Vec<f64>,
    /// Clear semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl EvenAsphere {
    /// Creates a new even asphere with verified finite inputs.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `semi_diameter_mm < 0.0`.
    pub fn new(
        curvature: f64,
        conic_constant: f64,
        coefficients: Vec<f64>,
        semi_diameter_mm: f64,
    ) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !conic_constant.is_finite()
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
            || !coefficients.iter().all(|c| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "EvenAsphere parameter is non-finite or has negative semi-diameter".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            conic_constant,
            coefficients,
            semi_diameter_mm,
        })
    }

    /// Evaluates the surface sag $z(r)$ at radial distance $r \ge 0$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $r$ exceeds the mathematical conic domain.
    pub fn sag(&self, r: f64) -> Result<f64, OpticsError> {
        if !r.is_finite() || r < 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Radial distance must be a non-negative finite number".to_string(),
            ));
        }

        let c = self.curvature;
        let k = self.conic_constant;
        let r2 = r * r;

        // Evaluate base conic sag
        let conic_sag = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * r2;
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt();
            (c * r2) / (1.0 + s)
        };

        // Evaluate polynomial deformation sum: sum_{i=1}^N alpha_{2i} r^{2i}
        let mut poly_sum = 0.0;
        let mut current_r_power = r2; // r^2 for i=1
        for &alpha in &self.coefficients {
            poly_sum += alpha * current_r_power;
            current_r_power *= r2; // increment power by 2
        }

        Ok(conic_sag + poly_sum)
    }

    /// Evaluates the radial slope $dz / dr$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $r$ exceeds the conic domain.
    pub fn radial_derivative(&self, r: f64) -> Result<f64, OpticsError> {
        if !r.is_finite() || r < 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Radial distance must be non-negative and finite".to_string(),
            ));
        }

        let c = self.curvature;
        let k = self.conic_constant;
        let r2 = r * r;

        let conic_deriv = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * r2;
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt();
            if s <= 1e-15 {
                return Err(OpticsError::NumericalSingularity(
                    "Conic normal derivative singularity at domain boundary".to_string(),
                ));
            }
            (c * r) / s
        };

        // Polynomial derivative: sum_{i=1}^N 2i * alpha_{2i} * r^{2i - 1}
        let mut poly_deriv = 0.0;
        let mut r_pow = r; // r^1 for i=1
        for (idx, &alpha) in self.coefficients.iter().enumerate() {
            let power = 2.0 * (idx as f64 + 1.0);
            poly_deriv += power * alpha * r_pow;
            r_pow *= r2;
        }

        Ok(conic_deriv + poly_deriv)
    }

    /// Evaluates the outward unit surface normal directed toward $-z$ (incident medium).
    ///
    /// # Errors
    /// Returns [`OpticsError`] if $(x, y)$ is outside the aperture domain or produces a singularity.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let r = (x * x + y * y).sqrt();
        if r < 1e-15 {
            // At optical axis vertex, normal is purely along -z
            return Ok(Vec3::new(0.0, 0.0, -1.0));
        }

        let dz_dr = self.radial_derivative(r)?;
        let dz_dx = dz_dr * (x / r);
        let dz_dy = dz_dr * (y / r);

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        if norm <= 1e-15 || !norm.is_finite() {
            return Err(OpticsError::NumericalSingularity(
                "Degenerate aspheric surface normal vector".to_string(),
            ));
        }

        // Outward normal toward incident medium: (dz/dx, dz/dy, -1) normalized
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }

    /// Second derivative at the vertex ($r=0$) contributing to paraxial curvature:
    /// $$c_{\text{eff}} = c + 2 \alpha_2$$
    #[must_use]
    pub fn paraxial_curvature(&self) -> f64 {
        let alpha2 = self.coefficients.first().copied().unwrap_or(0.0);
        self.curvature + 2.0 * alpha2
    }

    /// Intersects a ray with the asphere using paraxial/conic guess and Newton-Raphson iteration.
    ///
    /// # Errors
    /// Returns [`OpticsError::RayMissedSurface`] if no intersection is found within tolerances.
    pub fn intersect_ray(&self, ray: &Ray, vertex_z: f64) -> Result<(f64, Vec3), OpticsError> {
        // Initial estimate from intersection with surface vertex plane z = vertex_z
        if ray.direction.z.abs() < 1e-15 {
            return Err(OpticsError::RayMissedSurface(
                "Ray parallel to vertex tangent plane".to_string(),
            ));
        }

        let mut t = (vertex_z - ray.origin.z) / ray.direction.z;
        if t < 0.0 {
            t = 0.0;
        }

        // Newton-Raphson along parameter t
        let max_iters = 25;
        let tol = 1e-11; // 10 picometers

        for _ in 0..max_iters {
            let p = ray.at(t);
            let r = (p.x * p.x + p.y * p.y).sqrt();

            if r > self.semi_diameter_mm * 1.5 {
                return Err(OpticsError::RayMissedSurface(
                    "Ray exceeded asphere radial aperture boundary".to_string(),
                ));
            }

            let sag = match self.sag(r) {
                Ok(s) => s,
                Err(_) => {
                    return Err(OpticsError::RayMissedSurface("Outside sag domain".to_string()));
                }
            };

            // Residual f(t) = (p.z - vertex_z) - sag(r)
            let f = (p.z - vertex_z) - sag;
            if f.abs() < tol {
                // Verify within clear aperture
                if r > self.semi_diameter_mm {
                    return Err(OpticsError::RayVignetted {
                        surface_index: 0,
                        r,
                        max_r: self.semi_diameter_mm,
                    });
                }
                return Ok((t, p));
            }

            // Derivative f'(t) = d.z - (dz/dx * d.x + dz/dy * d.y)
            let dz_dr = match self.radial_derivative(r) {
                Ok(d) => d,
                Err(_) => return Err(OpticsError::RayMissedSurface("Singular slope".to_string())),
            };

            let (dz_dx, dz_dy) = if r > 1e-14 {
                (dz_dr * (p.x / r), dz_dr * (p.y / r))
            } else {
                (0.0, 0.0)
            };

            let f_prime = ray.direction.z - (dz_dx * ray.direction.x + dz_dy * ray.direction.y);
            if f_prime.abs() < 1e-15 {
                return Err(OpticsError::NumericalSingularity(
                    "Newton-Raphson zero gradient derivative".to_string(),
                ));
            }

            let delta_t = f / f_prime;
            t -= delta_t;
            if delta_t.abs() < 1e-12 {
                let final_p = ray.at(t);
                return Ok((t, final_p));
            }
        }

        Err(OpticsError::RayMissedSurface(
            "Newton-Raphson did not converge for aspheric surface".to_string(),
        ))
    }
}

/// Odd asphere surface profile containing both even and odd polynomial powers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OddAsphere {
    /// Vertex curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Conic constant $\kappa$.
    pub conic_constant: f64,
    /// Even power polynomial coefficients $[\alpha_2, \alpha_4, \dots]$.
    pub even_coefficients: Vec<f64>,
    /// Odd power polynomial coefficients $[\beta_1, \beta_3, \dots, \beta_{2m-1}]$ for powers $r^1, r^3, \dots$.
    pub odd_coefficients: Vec<f64>,
    /// Clear aperture semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl OddAsphere {
    /// Creates a new odd asphere with verified finite parameters.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `semi_diameter_mm < 0.0`.
    pub fn new(
        curvature: f64,
        conic_constant: f64,
        even_coefficients: Vec<f64>,
        odd_coefficients: Vec<f64>,
        semi_diameter_mm: f64,
    ) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !conic_constant.is_finite()
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
            || !even_coefficients.iter().all(|c| c.is_finite())
            || !odd_coefficients.iter().all(|c| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "OddAsphere parameter is non-finite or has negative semi-diameter".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            conic_constant,
            even_coefficients,
            odd_coefficients,
            semi_diameter_mm,
        })
    }

    /// Evaluates total sag $z(r)$ at radial distance $r \ge 0$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $r$ exceeds the conic base domain.
    pub fn sag(&self, r: f64) -> Result<f64, OpticsError> {
        if !r.is_finite() || r < 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Radius must be a non-negative finite number".to_string(),
            ));
        }

        let c = self.curvature;
        let k = self.conic_constant;
        let r2 = r * r;

        let conic_sag = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * r2;
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt();
            (c * r2) / (1.0 + s)
        };

        // Even terms
        let mut even_sum = 0.0;
        let mut r_pow2 = r2;
        for &alpha in &self.even_coefficients {
            even_sum += alpha * r_pow2;
            r_pow2 *= r2;
        }

        // Odd terms: beta_1 r^1, beta_3 r^3, ...
        let mut odd_sum = 0.0;
        let mut r_pow_odd = r;
        for &beta in &self.odd_coefficients {
            odd_sum += beta * r_pow_odd;
            r_pow_odd *= r2;
        }

        Ok(conic_sag + even_sum + odd_sum)
    }

    /// Evaluates outward unit normal directed toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if evaluation is outside valid domain or singular.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let r = (x * x + y * y).sqrt();
        if r < 1e-15 {
            let beta1 = self.odd_coefficients.first().copied().unwrap_or(0.0);
            if beta1.abs() < 1e-14 {
                return Ok(Vec3::new(0.0, 0.0, -1.0));
            }
        }

        let c = self.curvature;
        let k = self.conic_constant;
        let r2 = r * r;

        let conic_deriv = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * r2;
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt();
            (c * r) / s.max(1e-15)
        };

        let mut even_deriv = 0.0;
        let mut r_even = r;
        for (idx, &alpha) in self.even_coefficients.iter().enumerate() {
            let power = 2.0 * (idx as f64 + 1.0);
            even_deriv += power * alpha * r_even;
            r_even *= r2;
        }

        let mut odd_deriv = 0.0;
        let mut r_odd_pow = 1.0;
        for (idx, &beta) in self.odd_coefficients.iter().enumerate() {
            let power = 2.0 * idx as f64 + 1.0;
            odd_deriv += power * beta * r_odd_pow;
            r_odd_pow *= r2;
        }

        let dz_dr = conic_deriv + even_deriv + odd_deriv;
        let (dz_dx, dz_dy) = if r > 1e-14 {
            (dz_dr * (x / r), dz_dr * (y / r))
        } else {
            (dz_dr, 0.0)
        };

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_even_asphere_zero_coefficients_matches_conic() -> TestResult {
        let r_curv = 50.0;
        let c = 1.0 / r_curv;
        let k = -0.5;

        let asphere = EvenAsphere::new(c, k, vec![], 20.0)?;
        for &r in &[0.0, 2.5, 5.0, 10.0, 15.0] {
            let sag = asphere.sag(r)?;
            let analytic_conic = (c * r * r) / (1.0 + (1.0 - (1.0 + k) * c * c * r * r).sqrt());
            assert!(
                (sag - analytic_conic).abs() < 1e-12,
                "Asphere with 0 coeffs must match conic sag exactly at r={r}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_even_asphere_normal_unit_length() -> TestResult {
        let c = 1.0 / 40.0;
        let k = -1.0; // parabolic base
        let alpha = vec![0.0, 1.2e-6, -3.5e-10]; // r^4 and r^6 coefficients
        let asphere = EvenAsphere::new(c, k, alpha, 18.0)?;

        for &x in &[-10.0, -5.0, 0.0, 3.2, 8.5] {
            for &y in &[-8.0, 0.0, 4.1, 9.0] {
                if x * x + y * y <= 18.0 * 18.0 {
                    let n = asphere.normal(x, y)?;
                    let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
                    assert!(
                        (len - 1.0).abs() < 1e-12,
                        "Normal vector at ({x}, {y}) must have exact unit length, got {len}"
                    );
                    assert!(n.z < 0.0, "Normal must face incident medium along -z, got z={}", n.z);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_even_asphere_ray_intersection() -> TestResult {
        let c = 1.0 / 100.0;
        let k = 0.0;
        let alpha = vec![0.0, 1e-7]; // small 4th-order perturbation
        let asphere = EvenAsphere::new(c, k, alpha, 25.0)?;

        let ray = Ray::new(Vec3::new(5.0, 0.0, -10.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;

        let (t, hit_pt) = asphere.intersect_ray(&ray, 0.0)?;
        assert!(t > 0.0);
        let expected_sag = asphere.sag(5.0)?;
        assert!((hit_pt.z - expected_sag).abs() < 1e-8);
        Ok(())
    }

    #[test]
    fn test_odd_asphere_sag_and_normal() -> TestResult {
        let odd_asphere = OddAsphere::new(0.02, 0.0, vec![0.0, 1e-6], vec![1e-5], 15.0)?;
        let sag = odd_asphere.sag(5.0)?;
        assert!(sag > 0.0);
        let n = odd_asphere.normal(3.0, 4.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        Ok(())
    }
}
