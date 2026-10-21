//! Diffractive optical surfaces and phase polynomial ray deflection.
//!
//! # Equations
//! The rotationally symmetric diffractive phase function $\Phi(r)$ adds an optical phase (in radians):
//! $$\Phi(r) = \sum_{i=1}^{K} A_i r^{2i}$$
//! The phase gradient on the surface tangent plane is:
//! $$\nabla \Phi(x, y) = \frac{d\Phi}{dr} \left(\frac{x}{r} \hat{\mathbf{x}} + \frac{y}{r} \hat{\mathbf{y}}\right)$$
//! Ray redirection follows the vector grating equation:
//! $$\hat{\mathbf{t}} \times \hat{\mathbf{n}} = \eta (\hat{\mathbf{d}} \times \hat{\mathbf{n}}) + \frac{m \lambda}{2\pi n_2} (\nabla \Phi \times \hat{\mathbf{n}})$$

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::Vec3;

/// Diffractive surface with polynomial phase profile $\Phi(r) = \sum_{i=1}^K A_i r^{2i}$.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffractivePhaseSurface {
    /// Diffraction order $m$ (typically $+1$ or $-1$).
    pub diffraction_order: i32,
    /// Phase coefficients $[A_1, A_2, \dots]$ in $\text{rad} / \text{mm}^{2i}$.
    pub coefficients: Vec<f64>,
    /// Design wavelength in nanometers.
    pub design_wavelength_nm: f64,
}

impl DiffractivePhaseSurface {
    /// Creates a new diffractive phase surface.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any parameter is non-finite or design wavelength non-positive.
    pub fn new(diffraction_order: i32, coefficients: Vec<f64>, design_wavelength_nm: f64) -> Result<Self, OpticsError> {
        if !design_wavelength_nm.is_finite()
            || design_wavelength_nm <= 0.0
            || !coefficients.iter().all(|c| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "Diffractive surface parameters must be finite and wavelength positive".to_string(),
            ));
        }

        Ok(Self {
            diffraction_order,
            coefficients,
            design_wavelength_nm,
        })
    }

    /// Evaluates phase addition $\Phi(r)$ in radians.
    #[must_use]
    pub fn phase(&self, r: f64) -> f64 {
        let r2 = r * r;
        let mut sum = 0.0;
        let mut r_pow = r2;
        for &a in &self.coefficients {
            sum += a * r_pow;
            r_pow *= r2;
        }
        sum
    }

    /// Evaluates phase derivative with respect to radius $d\Phi / dr$.
    #[must_use]
    pub fn dphase_dr(&self, r: f64) -> f64 {
        let r2 = r * r;
        let mut sum = 0.0;
        let mut r_pow = r;
        for (i, &a) in self.coefficients.iter().enumerate() {
            let power = 2.0 * (i as f64 + 1.0);
            sum += power * a * r_pow;
            r_pow *= r2;
        }
        sum
    }

    /// Computes the diffracted unit ray direction $\hat{\mathbf{t}}$ using the vector grating equation.
    ///
    /// # Parameters
    /// - `d`: Incident unit ray direction.
    /// - `n`: Interface unit normal pointing toward incident medium ($\mathbf{d} \cdot \mathbf{n} \le 0$).
    /// - `hit_point`: 3D position of ray intersection on surface.
    /// - `n1`: Incident medium refractive index.
    /// - `n2`: Transmitted medium refractive index.
    /// - `wavelength_nm`: Ray wavelength in vacuum in nanometers.
    ///
    /// # Errors
    /// Returns [`OpticsError::TotalInternalReflection`] if diffracted evanescent wave occurs.
    pub fn diffract(
        &self,
        d: Vec3,
        n: Vec3,
        hit_point: Vec3,
        n1: f64,
        n2: f64,
        wavelength_nm: f64,
    ) -> Result<Vec3, OpticsError> {
        let r = (hit_point.x * hit_point.x + hit_point.y * hit_point.y).sqrt();
        let dphi_dr = self.dphase_dr(r);

        let grad_phi = if r > 1e-14 {
            Vec3::new(dphi_dr * (hit_point.x / r), dphi_dr * (hit_point.y / r), 0.0)
        } else {
            Vec3::zeros()
        };

        // Convert wavelength to mm: 1 nm = 1e-6 mm
        let lambda_mm = wavelength_nm * 1e-6;
        let m = self.diffraction_order as f64;

        // Vector grating equation in tangential space:
        // p_trans = (n1/n2) * (d - (d.n)*n) + (m * lambda / (2*pi * n2)) * grad_phi_tangential
        let cos_i = -(d.dot(&n));
        let eta = n1 / n2;

        let d_tangential = d + n * cos_i;
        let grad_phi_tangential = grad_phi - n * grad_phi.dot(&n);

        let phase_term = (m * lambda_mm) / (2.0 * std::f64::consts::PI * n2);
        let t_tangential = d_tangential * eta + grad_phi_tangential * phase_term;

        let t_tan_sq = t_tangential.norm_squared();
        if t_tan_sq > 1.0 {
            return Err(OpticsError::TotalInternalReflection);
        }

        let cos_t = (1.0 - t_tan_sq).max(0.0).sqrt();
        // Transmitted direction moves forward through surface: t_tangential - n * cos_t
        let t = t_tangential - n * cos_t;
        let norm = t.norm();
        if norm <= 1e-15 {
            return Err(OpticsError::NumericalSingularity(
                "Diffracted ray direction norm is zero".to_string(),
            ));
        }

        Ok(t / norm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_diffractive_phase_and_grating_bending() -> TestResult {
        // Linear phase gradient (constant dispersion/deflection)
        let diff = DiffractivePhaseSurface::new(1, vec![100.0], 550.0)?;
        let d = Vec3::new(0.0, 0.0, 1.0);
        let n = Vec3::new(0.0, 0.0, -1.0);
        let pt = Vec3::new(2.0, 0.0, 0.0);

        let t = diff.diffract(d, n, pt, 1.0, 1.0, 550.0)?;
        assert!((t.norm() - 1.0).abs() < 1e-12);
        // Should produce deflection in +x direction
        assert!(t.x > 0.0);
        assert!(t.z > 0.0);
        Ok(())
    }
}
