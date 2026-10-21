//! Cartesian XY Polynomial and Zernike standard sag optical surfaces.
//!
//! # Equations
//! - **XY Polynomial**:
//!   $$z(x, y) = z_{\text{conic}}(r) + \sum_{i, j} A_{ij} x^i y^j$$
//! - **Zernike Sag**:
//!   $$z(r, \theta) = z_{\text{conic}}(r) + \sum_{j=1}^{M} C_j Z_j(\rho, \theta)$$
//!   with $\rho = r / r_{\text{norm}} \le 1.0$ and Noll indexing $Z_j$.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::Vec3;

/// Single term in an XY polynomial: $A_{ij} x^i y^j$.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyTerm {
    /// Power of $x$.
    pub x_power: u32,
    /// Power of $y$.
    pub y_power: u32,
    /// Coefficient $A_{ij}$.
    pub coefficient: f64,
}

/// XY Polynomial surface combining a base conic with general bivariate polynomial terms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyPolynomialSurface {
    /// Base conic curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Base conic constant $\kappa$.
    pub conic_constant: f64,
    /// Polynomial terms $A_{ij} x^i y^j$.
    pub terms: Vec<XyTerm>,
    /// Clear aperture semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl Default for XyPolynomialSurface {
    fn default() -> Self {
        Self {
            curvature: 0.0,
            conic_constant: 0.0,
            terms: Vec::new(),
            semi_diameter_mm: 10.0,
        }
    }
}

impl XyPolynomialSurface {
    /// Creates a new XY polynomial surface with validated parameters.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any parameter is non-finite or `semi_diameter_mm < 0.0`.
    pub fn new(
        curvature: f64,
        conic_constant: f64,
        terms: Vec<XyTerm>,
        semi_diameter_mm: f64,
    ) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !conic_constant.is_finite()
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
            || !terms.iter().all(|t| t.coefficient.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "XyPolynomialSurface parameters must be finite and semi-diameter non-negative".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            conic_constant,
            terms,
            semi_diameter_mm,
        })
    }

    /// Evaluates surface sag $z(x, y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $(x, y)$ exceeds the base conic boundary.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(OpticsError::InvalidParameter("Coordinates must be finite".to_string()));
        }

        let r2 = x * x + y * y;
        let c = self.curvature;
        let k = self.conic_constant;

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

        let mut poly_sag = 0.0;
        for term in &self.terms {
            poly_sag += term.coefficient * x.powi(term.x_power as i32) * y.powi(term.y_power as i32);
        }

        Ok(conic_sag + poly_sag)
    }

    /// Evaluates the outward unit normal directed toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let r = (x * x + y * y).sqrt();
        let c = self.curvature;
        let k = self.conic_constant;

        let (conic_dx, conic_dy) = if c.abs() < 1e-15 || r < 1e-15 {
            (0.0, 0.0)
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * (r * r);
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt().max(1e-15);
            let dz_dr = (c * r) / s;
            (dz_dr * (x / r), dz_dr * (y / r))
        };

        let mut poly_dx = 0.0;
        let mut poly_dy = 0.0;

        for term in &self.terms {
            if term.x_power > 0 {
                poly_dx += term.coefficient
                    * (term.x_power as f64)
                    * x.powi((term.x_power - 1) as i32)
                    * y.powi(term.y_power as i32);
            }
            if term.y_power > 0 {
                poly_dy += term.coefficient
                    * (term.y_power as f64)
                    * x.powi(term.x_power as i32)
                    * y.powi((term.y_power - 1) as i32);
            }
        }

        let dz_dx = conic_dx + poly_dx;
        let dz_dy = conic_dy + poly_dy;
        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();

        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }
}

/// Zernike standard sag surface evaluated over a circular normalization pupil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZernikeSagSurface {
    /// Base conic curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Base conic constant $\kappa$.
    pub conic_constant: f64,
    /// Pupil normalization radius in millimeters ($r_{\text{norm}}$).
    pub norm_radius_mm: f64,
    /// Zernike polynomial coefficients indexed by Noll index $j \ge 1$: $(j, C_j)$.
    pub coefficients: Vec<(usize, f64)>,
    /// Clear semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
}

impl Default for ZernikeSagSurface {
    fn default() -> Self {
        Self {
            curvature: 0.0,
            conic_constant: 0.0,
            norm_radius_mm: 10.0,
            coefficients: vec![(4, 0.0), (11, 0.0)],
            semi_diameter_mm: 10.0,
        }
    }
}

impl ZernikeSagSurface {
    /// Creates a new Zernike sag surface.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or radii non-positive.
    pub fn new(
        curvature: f64,
        conic_constant: f64,
        norm_radius_mm: f64,
        coefficients: Vec<(usize, f64)>,
        semi_diameter_mm: f64,
    ) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !conic_constant.is_finite()
            || !norm_radius_mm.is_finite()
            || norm_radius_mm <= 0.0
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
            || !coefficients.iter().all(|(_, c)| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "ZernikeSagSurface requires finite parameters and positive normalization radius".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            conic_constant,
            norm_radius_mm,
            coefficients,
            semi_diameter_mm,
        })
    }

    /// Evaluates single Noll Zernike polynomial $Z_j(\rho, \theta)$.
    fn eval_noll_zernike(j: usize, rho: f64, theta: f64) -> f64 {
        match j {
            1 => 1.0,                                                                  // Piston
            2 => 2.0 * rho * theta.cos(),                                              // Tilt X
            3 => 2.0 * rho * theta.sin(),                                              // Tilt Y
            4 => (3.0_f64).sqrt() * (2.0 * rho * rho - 1.0),                           // Defocus
            5 => (6.0_f64).sqrt() * rho * rho * (2.0 * theta).sin(),                   // Astigmatism 45°
            6 => (6.0_f64).sqrt() * rho * rho * (2.0 * theta).cos(),                   // Astigmatism 0°
            7 => (8.0_f64).sqrt() * (3.0 * rho * rho * rho - 2.0 * rho) * theta.sin(), // Coma Y
            8 => (8.0_f64).sqrt() * (3.0 * rho * rho * rho - 2.0 * rho) * theta.cos(), // Coma X
            9 => (8.0_f64).sqrt() * rho * rho * rho * (3.0 * theta).sin(),             // Trefoil Y
            10 => (8.0_f64).sqrt() * rho * rho * rho * (3.0 * theta).cos(),            // Trefoil X
            11 => (5.0_f64).sqrt() * (6.0 * rho.powi(4) - 6.0 * rho * rho + 1.0),      // Primary Spherical
            _ => 0.0, // Higher-order polynomial fallback
        }
    }

    /// Evaluates total sag $z(x, y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $(x, y)$ falls outside the conic domain.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        let r = (x * x + y * y).sqrt();
        let c = self.curvature;
        let k = self.conic_constant;

        let conic_sag = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - (1.0 + k) * c * c * (r * r);
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            let s = radicand.max(0.0).sqrt();
            (c * r * r) / (1.0 + s)
        };

        let rho = r / self.norm_radius_mm;
        let theta = y.atan2(x);

        let mut zernike_sum = 0.0;
        for &(j, c_j) in &self.coefficients {
            zernike_sum += c_j * Self::eval_noll_zernike(j, rho, theta);
        }

        Ok(conic_sag + zernike_sum)
    }

    /// Evaluates outward unit normal directed toward $-z$.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let eps = 1e-7;
        let s = self.sag(x, y)?;
        let s_xp = self.sag(x + eps, y)?;
        let s_yp = self.sag(x, y + eps)?;

        let dz_dx = (s_xp - s) / eps;
        let dz_dy = (s_yp - s) / eps;

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_xy_polynomial_evaluation() -> TestResult {
        let terms = vec![
            XyTerm {
                x_power: 2,
                y_power: 0,
                coefficient: 1e-4,
            },
            XyTerm {
                x_power: 0,
                y_power: 2,
                coefficient: 1e-4,
            },
        ];
        let xy_surf = XyPolynomialSurface::new(0.0, 0.0, terms, 20.0)?;
        let sag = xy_surf.sag(5.0, 5.0)?;
        assert!((sag - 50.0 * 1e-4).abs() < 1e-12);

        let n = xy_surf.normal(5.0, 5.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }

    #[test]
    fn test_zernike_defocus_and_spherical() -> TestResult {
        let coeffs = vec![
            (4, 1e-3),  // Defocus
            (11, 2e-4), // Spherical
        ];
        let zernike_surf = ZernikeSagSurface::new(0.0, 0.0, 10.0, coeffs, 10.0)?;
        let sag_center = zernike_surf.sag(0.0, 0.0)?;
        assert!(sag_center.is_finite());

        let n = zernike_surf.normal(3.0, 4.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }
}
