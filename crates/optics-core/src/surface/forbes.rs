//! Forbes orthogonal aspheric surface formulations ($Q_{\text{con}}$ and $Q_{\text{bfs}}$).
//!
//! # Reference
//! - G. W. Forbes, "Shape specification for axially symmetric aspheric surfaces,"
//!   *Optics Express*, Vol. 15, No. 8, pp. 5218–5226 (2007).
//! - G. W. Forbes, "Robust and flexible characterization of axisymmetric aspheres,"
//!   *Optics Express*, Vol. 18, No. 19, pp. 19700–19712 (2010).

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::Vec3;

/// Evaluates Forbes $Q_m^{\text{con}}(u)$ polynomial over normalized coordinate $u = (r / r_{\max})^2 \in [0, 1]$.
fn eval_q_con(m: usize, u: f64) -> f64 {
    if m == 0 {
        return 1.0;
    }
    if m == 1 {
        return (13.0 - 16.0 * u) / 19.0_f64.sqrt();
    }

    // Stable forward three-term recurrence
    let mut q_prev2 = 1.0;
    let mut q_prev1 = (13.0 - 16.0 * u) / 19.0_f64.sqrt();
    let mut q_curr = q_prev1;

    for k in 2..=m {
        let k_f = k as f64;
        let c_k = 2.0 * (k_f - 1.0) / (2.0 * k_f + 1.0);
        let d_k = (1.0 - 2.0 * u) * (2.0 * k_f - 1.0) / k_f;
        q_curr = d_k * q_prev1 - c_k * q_prev2;
        q_prev2 = q_prev1;
        q_prev1 = q_curr;
    }

    q_curr
}

/// Forbes $Q_{\text{con}}$ asphere representing departures from a base conic in an orthogonal basis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForbesQconSurface {
    /// Base conic curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Base conic constant $\kappa$.
    pub conic_constant: f64,
    /// Maximum normalization radius in millimeters ($r_{\max}$).
    pub r_max_mm: f64,
    /// Orthogonal expansion coefficients $a_m$ ($m = 0, \dots, M$).
    pub coefficients: Vec<f64>,
}

impl ForbesQconSurface {
    /// Creates a new Forbes $Q_{\text{con}}$ asphere.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any input is non-finite or `r_max_mm <= 0.0`.
    pub fn new(
        curvature: f64,
        conic_constant: f64,
        r_max_mm: f64,
        coefficients: Vec<f64>,
    ) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !conic_constant.is_finite()
            || !r_max_mm.is_finite()
            || r_max_mm <= 0.0
            || !coefficients.iter().all(|c| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "ForbesQconSurface requires finite parameters and positive r_max".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            conic_constant,
            r_max_mm,
            coefficients,
        })
    }

    /// Evaluates surface sag $z(r)$ at radial distance $r$.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $r$ exceeds the conic base domain or $r_{\max}$.
    pub fn sag(&self, r: f64) -> Result<f64, OpticsError> {
        if !r.is_finite() || r < 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Radius must be non-negative and finite".to_string(),
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

        let u = (r / self.r_max_mm).powi(2);
        if u > 1.0 + 1e-12 {
            return Err(OpticsError::GeometryOutsideAperture);
        }
        let u_clamped = u.clamp(0.0, 1.0);

        let mut q_sum = 0.0;
        for (m, &a_m) in self.coefficients.iter().enumerate() {
            q_sum += a_m * eval_q_con(m, u_clamped);
        }

        let departure = u_clamped * (1.0 - u_clamped) * q_sum;
        Ok(conic_sag + departure)
    }

    /// Evaluates outward unit normal directed toward $-z$.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let eps = 1e-7;
        let r = (x * x + y * y).sqrt();
        let s = self.sag(r)?;
        let s_xp = self.sag((r + eps).min(self.r_max_mm))?;
        let dz_dr = (s_xp - s) / eps;

        let (dz_dx, dz_dy) = if r > 1e-14 {
            (dz_dr * (x / r), dz_dr * (y / r))
        } else {
            (0.0, 0.0)
        };

        let norm = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        Ok(Vec3::new(dz_dx / norm, dz_dy / norm, -1.0 / norm))
    }
}

/// Forbes $Q_{\text{bfs}}$ asphere representing departures from a best-fit sphere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForbesQbfsSurface {
    /// Best-fit sphere curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Maximum normalization radius in millimeters ($r_{\max}$).
    pub r_max_mm: f64,
    /// Departure coefficients $b_m$.
    pub coefficients: Vec<f64>,
}

impl ForbesQbfsSurface {
    /// Creates a new Forbes $Q_{\text{bfs}}$ asphere.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if any parameter is non-finite or $r_{\max} \le 0$.
    pub fn new(curvature: f64, r_max_mm: f64, coefficients: Vec<f64>) -> Result<Self, OpticsError> {
        if !curvature.is_finite()
            || !r_max_mm.is_finite()
            || r_max_mm <= 0.0
            || !coefficients.iter().all(|c| c.is_finite())
        {
            return Err(OpticsError::InvalidParameter(
                "ForbesQbfsSurface requires finite parameters and positive r_max".to_string(),
            ));
        }

        Ok(Self {
            curvature,
            r_max_mm,
            coefficients,
        })
    }

    /// Evaluates surface sag $z(r)$.
    pub fn sag(&self, r: f64) -> Result<f64, OpticsError> {
        let c = self.curvature;
        let r2 = r * r;

        let sphere_sag = if c.abs() < 1e-15 {
            0.0
        } else {
            let radicand = 1.0 - c * c * r2;
            if radicand < -1e-12 {
                return Err(OpticsError::GeometryOutsideAperture);
            }
            (c * r2) / (1.0 + radicand.max(0.0).sqrt())
        };

        let u = (r / self.r_max_mm).powi(2).clamp(0.0, 1.0);
        let mut q_sum = 0.0;
        for (m, &b_m) in self.coefficients.iter().enumerate() {
            q_sum += b_m * eval_q_con(m, u);
        }

        let departure = u * (1.0 - u) * q_sum;
        Ok(sphere_sag + departure)
    }

    /// Evaluates unit surface normal pointing toward $-z$.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        let eps = 1e-7;
        let r = (x * x + y * y).sqrt();
        let s = self.sag(r)?;
        let s_xp = self.sag((r + eps).min(self.r_max_mm))?;
        let dz_dr = (s_xp - s) / eps;

        let (dz_dx, dz_dy) = if r > 1e-14 {
            (dz_dr * (x / r), dz_dr * (y / r))
        } else {
            (0.0, 0.0)
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
    fn test_forbes_qcon_zero_departure_matches_conic() -> TestResult {
        let c = 1.0 / 50.0;
        let k = -1.0;
        let r_max = 20.0;
        let qcon = ForbesQconSurface::new(c, k, r_max, vec![])?;

        for &r in &[0.0, 5.0, 10.0, 15.0] {
            let s = qcon.sag(r)?;
            let parabola_sag = 0.5 * c * r * r;
            assert!((s - parabola_sag).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn test_forbes_qcon_normal_unit_length() -> TestResult {
        let qcon = ForbesQconSurface::new(1.0 / 80.0, 0.0, 25.0, vec![1e-3, -5e-4])?;
        let n = qcon.normal(8.0, 6.0)?;
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
        assert!(n.z < 0.0);
        Ok(())
    }
}
