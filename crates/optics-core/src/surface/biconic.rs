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
    pub fn new(
        cx: f64,
        kx: f64,
        cy: f64,
        ky: f64,
        semi_diameter_mm: f64,
    ) -> Result<Self, OpticsError> {
        if !cx.is_finite()
            || !kx.is_finite()
            || !cy.is_finite()
            || !ky.is_finite()
            || !semi_diameter_mm.is_finite()
            || semi_diameter_mm < 0.0
        {
            return Err(OpticsError::InvalidParameter(
                "BiconicSurface parameters must be finite and semi-diameter non-negative"
                    .to_string(),
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

    /// Evaluates the surface sag $z(x, y)$ at point $(x, y)$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $(x, y)$ falls outside the real surface domain.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(OpticsError::InvalidParameter(
                "Coordinates must be finite".to_string(),
            ));
        }

        let num = self.cx * x * x + self.cy * y * y;
        let radicand = 1.0
            - (1.0 + self.kx) * self.cx * self.cx * x * x
            - (1.0 + self.ky) * self.cy * self.cy * y * y;

        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }

        let s = radicand.max(0.0).sqrt();
        Ok(num / (1.0 + s))
    }

    /// Evaluates the outward unit normal directed toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain or singular.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        if x.abs() < 1e-15 && y.abs() < 1e-15 {
            return Ok(Vec3::new(0.0, 0.0, -1.0));
        }

        let radicand = 1.0
            - (1.0 + self.kx) * self.cx * self.cx * x * x
            - (1.0 + self.ky) * self.cy * self.cy * y * y;

        if radicand < -1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }

        let s = radicand.max(0.0).sqrt();
        if s <= 1e-15 {
            return Err(OpticsError::NumericalSingularity(
                "Biconic surface normal singularity at boundary".to_string(),
            ));
        }

        let dz_dx = (self.cx * x) / s;
        let dz_dy = (self.cy * y) / s;

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }

    /// Paraxial curvatures $(c_x, c_y)$ along the principal meridians.
    #[must_use]
    pub const fn paraxial_curvatures(&self) -> (f64, f64) {
        (self.cx, self.cy)
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
                "Toroidal surface requires finite parameters and non-zero rotation radius"
                    .to_string(),
            ));
        }

        Ok(Self {
            rx_mm,
            cy,
            ky,
            semi_diameter_mm,
        })
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

    /// Evaluates outward unit normal pointing toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let zy = self.base_curve_sag(y)?;
        let rx = self.rx_mm;
        let term = rx - zy;
        let radicand = term * term - x * x;
        if radicand <= 1e-14 {
            return Err(OpticsError::GeometryOutsideAperture);
        }
        let s = radicand.sqrt();

        // dz/dx = x / s
        let dz_dx = x / s;

        // dzy/dy = (cy * y) / sqrt(1 - (1+ky)*cy^2*y^2)
        let base_rad = 1.0 - (1.0 + self.ky) * self.cy * self.cy * y * y;
        let base_s = base_rad.max(0.0).sqrt();
        let dzy_dy = if base_s > 1e-14 {
            (self.cy * y) / base_s
        } else {
            0.0
        };

        // dz/dy = (term / s) * dzy/dy
        let dz_dy = (term / s) * dzy_dy;

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_biconic_normal_unit_length() -> TestResult {
        let biconic = BiconicSurface::new(1.0 / 60.0, -0.8, 1.0 / 40.0, 0.2, 15.0)?;
        let n = biconic.normal(5.0, 4.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }

    #[test]
    fn test_toroidal_normal_unit_length() -> TestResult {
        let toroid = ToroidalSurface::new(100.0, 1.0 / 50.0, 0.0, 20.0)?;
        let sag = toroid.sag(4.0, 6.0)?;
        assert!(sag > 0.0);
        let n = toroid.normal(4.0, 6.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }
}
