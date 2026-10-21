//! Polarization ray tracing, Jones calculus, Stokes vectors, and Mueller matrices.
//!
//! Provides representation and propagation of polarized electromagnetic field states
//! through birefringent elements, linear polarizers, retarders, and optical thin-film interfaces.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::thin_film::{CoatingResponse, Complex};

/// A 2D complex Jones vector representing the transverse electric field $\mathbf{E} = [E_x, E_y]^T$.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JonesVector {
    /// Transverse $x$ (or $s$-polarization) electric field amplitude.
    pub ex: Complex,
    /// Transverse $y$ (or $p$-polarization) electric field amplitude.
    pub ey: Complex,
}

impl JonesVector {
    /// Creates a Jones vector from complex components.
    #[must_use]
    pub const fn new(ex: Complex, ey: Complex) -> Self {
        Self { ex, ey }
    }

    /// Linearly polarized along horizontal axis ($x$ / $s$).
    #[must_use]
    pub fn linear_horizontal() -> Self {
        Self::new(Complex::real(1.0), Complex::zero())
    }

    /// Linearly polarized along vertical axis ($y$ / $p$).
    #[must_use]
    pub fn linear_vertical() -> Self {
        Self::new(Complex::zero(), Complex::real(1.0))
    }

    /// Linearly polarized at $+45^\circ$.
    #[must_use]
    pub fn linear_45() -> Self {
        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        Self::new(Complex::real(inv_sqrt2), Complex::real(inv_sqrt2))
    }

    /// Right-hand circular polarization (RCP, IEEE convention): $\frac{1}{\sqrt{2}} [1, -i]^T$.
    #[must_use]
    pub fn circular_right() -> Self {
        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        Self::new(Complex::real(inv_sqrt2), Complex::new(0.0, -inv_sqrt2))
    }

    /// Left-hand circular polarization (LCP): $\frac{1}{\sqrt{2}} [1, i]^T$.
    #[must_use]
    pub fn circular_left() -> Self {
        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        Self::new(Complex::real(inv_sqrt2), Complex::new(0.0, inv_sqrt2))
    }

    /// Total electric field optical intensity $I = |E_x|^2 + |E_y|^2$.
    #[must_use]
    pub fn intensity(&self) -> f64 {
        self.ex.norm_sq() + self.ey.norm_sq()
    }

    /// Normalizes the Jones vector such that total intensity equals 1.0.
    #[must_use]
    pub fn normalize(&self) -> Self {
        let i = self.intensity();
        if i <= 1e-28 {
            return *self;
        }
        let inv_sqrt_i = 1.0 / i.sqrt();
        Self {
            ex: self.ex.scale(inv_sqrt_i),
            ey: self.ey.scale(inv_sqrt_i),
        }
    }

    /// Converts this fully polarized Jones state into a 4-element Stokes vector.
    ///
    /// $$S_0 = |E_x|^2 + |E_y|^2$$
    /// $$S_1 = |E_x|^2 - |E_y|^2$$
    /// $$S_2 = 2 \text{Re}(E_x E_y^*)$$
    /// $$S_3 = 2 \text{Im}(E_x^* E_y)$$
    #[must_use]
    pub fn to_stokes(&self) -> StokesVector {
        let i_x = self.ex.norm_sq();
        let i_y = self.ey.norm_sq();

        let s0 = i_x + i_y;
        let s1 = i_x - i_y;

        // E_x * E_y^* = (x1 + i y1)(x2 - i y2) = (x1 x2 + y1 y2) + i (y1 x2 - x1 y2)
        let ex_ey_conj_re = self.ex.re * self.ey.re + self.ex.im * self.ey.im;
        let s2 = 2.0 * ex_ey_conj_re;

        // E_x^* * E_y = (x1 - i y1)(x2 + i y2) = (x1 x2 + y1 y2) + i (x1 y2 - y1 x2)
        let ex_conj_ey_im = self.ex.re * self.ey.im - self.ex.im * self.ey.re;
        let s3 = 2.0 * ex_conj_ey_im;

        StokesVector { s0, s1, s2, s3 }
    }
}

/// 4-component real Stokes vector $\mathbf{S} = [S_0, S_1, S_2, S_3]^T$.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StokesVector {
    /// Total irradiance / intensity $S_0 \ge 0$.
    pub s0: f64,
    /// Linear horizontal vs vertical preference $S_1 \in [-S_0, S_0]$.
    pub s1: f64,
    /// Linear $+45^\circ$ vs $-45^\circ$ preference $S_2 \in [-S_0, S_0]$.
    pub s2: f64,
    /// Circular right vs left preference $S_3 \in [-S_0, S_0]$.
    pub s3: f64,
}

impl StokesVector {
    /// Creates a Stokes vector from individual elements.
    #[must_use]
    pub const fn new(s0: f64, s1: f64, s2: f64, s3: f64) -> Self {
        Self { s0, s1, s2, s3 }
    }

    /// Completely unpolarized beam with specified intensity.
    #[must_use]
    pub fn unpolarized(intensity: f64) -> Self {
        Self {
            s0: intensity.max(0.0),
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
        }
    }

    /// Total Degree of Polarization (DOP): $\text{DOP} = \frac{\sqrt{S_1^2 + S_2^2 + S_3^2}}{S_0} \in [0, 1]$.
    #[must_use]
    pub fn degree_of_polarization(&self) -> f64 {
        if self.s0 <= 1e-15 {
            return 0.0;
        }
        let p_sq = self.s1 * self.s1 + self.s2 * self.s2 + self.s3 * self.s3;
        (p_sq.sqrt() / self.s0).clamp(0.0, 1.0)
    }

    /// Degree of Linear Polarization (DOLP): $\text{DOLP} = \frac{\sqrt{S_1^2 + S_2^2}}{S_0} \in [0, 1]$.
    #[must_use]
    pub fn degree_of_linear_polarization(&self) -> f64 {
        if self.s0 <= 1e-15 {
            return 0.0;
        }
        let lp = (self.s1 * self.s1 + self.s2 * self.s2).sqrt();
        (lp / self.s0).clamp(0.0, 1.0)
    }

    /// Degree of Circular Polarization (DOCP): $\text{DOCP} = \frac{|S_3|}{S_0} \in [0, 1]$.
    #[must_use]
    pub fn degree_of_circular_polarization(&self) -> f64 {
        if self.s0 <= 1e-15 {
            return 0.0;
        }
        (self.s3.abs() / self.s0).clamp(0.0, 1.0)
    }

    /// Azimuthal polarization orientation angle $\psi = \frac{1}{2} \text{atan2}(S_2, S_1)$ in radians.
    #[must_use]
    pub fn orientation_angle_rad(&self) -> f64 {
        0.5 * self.s2.atan2(self.s1)
    }

    /// Ellipticity angle $\chi = \frac{1}{2} \arcsin\left(\frac{S_3}{S_0}\right)$ in radians.
    #[must_use]
    pub fn ellipticity_angle_rad(&self) -> f64 {
        if self.s0 <= 1e-15 {
            return 0.0;
        }
        let ratio = (self.s3 / self.s0).clamp(-1.0, 1.0);
        0.5 * ratio.asin()
    }
}

/// A $2 \times 2$ complex Jones matrix acting on Jones vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JonesMatrix {
    /// Matrix elements $[ [m_{00}, m_{01}], [m_{10}, m_{11}] ]$.
    pub m: [[Complex; 2]; 2],
}

impl JonesMatrix {
    /// Identity transformation.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            m: [
                [Complex::real(1.0), Complex::zero()],
                [Complex::zero(), Complex::real(1.0)],
            ],
        }
    }

    /// Ideal linear polarizer oriented at angle $\theta$ relative to the $x$-axis.
    #[must_use]
    pub fn linear_polarizer(angle_rad: f64) -> Self {
        let c = angle_rad.cos();
        let s = angle_rad.sin();
        Self {
            m: [
                [Complex::real(c * c), Complex::real(c * s)],
                [Complex::real(c * s), Complex::real(s * s)],
            ],
        }
    }

    /// Phase retarder with retardance $\delta$ (radians) and fast axis at angle $\theta$.
    #[must_use]
    pub fn retarder(retardance_rad: f64, fast_axis_rad: f64) -> Self {
        let half_delta = retardance_rad * 0.5;
        let exp_pos = Complex::new(half_delta.cos(), half_delta.sin());
        let exp_neg = Complex::new(half_delta.cos(), -half_delta.sin());

        let c = fast_axis_rad.cos();
        let s = fast_axis_rad.sin();
        let c2 = c * c;
        let s2 = s * s;
        let cs = c * s;

        let m00 = exp_pos.scale(c2) + exp_neg.scale(s2);
        let m01 = (exp_pos - exp_neg).scale(cs);
        let m10 = m01;
        let m11 = exp_pos.scale(s2) + exp_neg.scale(c2);

        Self {
            m: [[m00, m01], [m10, m11]],
        }
    }

    /// Quarter-wave plate ($\delta = \pi/2$) with fast axis at angle $\theta$.
    #[must_use]
    pub fn quarter_wave_plate(fast_axis_rad: f64) -> Self {
        Self::retarder(std::f64::consts::FRAC_PI_2, fast_axis_rad)
    }

    /// Half-wave plate ($\delta = \pi$) with fast axis at angle $\theta$.
    #[must_use]
    pub fn half_wave_plate(fast_axis_rad: f64) -> Self {
        Self::retarder(std::f64::consts::PI, fast_axis_rad)
    }

    /// Pure diagonal diattenuator (e.g. Fresnel reflection or transmission).
    #[must_use]
    pub fn diattenuator(tx: Complex, ty: Complex) -> Self {
        Self {
            m: [[tx, Complex::zero()], [Complex::zero(), ty]],
        }
    }

    /// Transforms a Jones vector: $\mathbf{E}' = \mathbf{J} \mathbf{E}$.
    #[must_use]
    pub fn apply(&self, v: JonesVector) -> JonesVector {
        let ex = self.m[0][0] * v.ex + self.m[0][1] * v.ey;
        let ey = self.m[1][0] * v.ex + self.m[1][1] * v.ey;
        JonesVector { ex, ey }
    }

    /// Matrix product: $\mathbf{J}_{\text{res}} = \mathbf{J}_1 \mathbf{J}_2$.
    #[must_use]
    pub fn mul(&self, rhs: &Self) -> Self {
        let mut res = [[Complex::zero(); 2]; 2];
        for (i, row) in res.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = self.m[i][0] * rhs.m[0][j] + self.m[i][1] * rhs.m[1][j];
            }
        }
        Self { m: res }
    }

    /// Converts a non-depolarizing Jones matrix into its equivalent $4 \times 4$ Mueller matrix.
    #[must_use]
    pub fn to_mueller(&self) -> MuellerMatrix {
        // Evaluate response to 4 canonical Stokes basis vectors
        let h = JonesVector::linear_horizontal();
        let v = JonesVector::linear_vertical();
        let p45 = JonesVector::linear_45();
        let rcp = JonesVector::circular_right();

        let s_h = self.apply(h).to_stokes();
        let s_v = self.apply(v).to_stokes();
        let s_p45 = self.apply(p45).to_stokes();
        let s_rcp = self.apply(rcp).to_stokes();

        // Reconstruct columns of Mueller matrix
        let m00 = 0.5 * (s_h.s0 + s_v.s0);
        let m10 = 0.5 * (s_h.s1 + s_v.s1);
        let m20 = 0.5 * (s_h.s2 + s_v.s2);
        let m30 = 0.5 * (s_h.s3 + s_v.s3);

        let m01 = 0.5 * (s_h.s0 - s_v.s0);
        let m11 = 0.5 * (s_h.s1 - s_v.s1);
        let m21 = 0.5 * (s_h.s2 - s_v.s2);
        let m31 = 0.5 * (s_h.s3 - s_v.s3);

        let m02 = s_p45.s0 - m00;
        let m12 = s_p45.s1 - m10;
        let m22 = s_p45.s2 - m20;
        let m32 = s_p45.s3 - m30;

        let m03 = s_rcp.s0 - m00;
        let m13 = s_rcp.s1 - m10;
        let m23 = s_rcp.s2 - m20;
        let m33 = s_rcp.s3 - m30;

        MuellerMatrix {
            m: [
                [m00, m01, m02, m03],
                [m10, m11, m12, m13],
                [m20, m21, m22, m23],
                [m30, m31, m32, m33],
            ],
        }
    }
}

/// A $4 \times 4$ Mueller matrix for partially polarized or depolarizing media.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MuellerMatrix {
    /// 16 real matrix entries.
    pub m: [[f64; 4]; 4],
}

impl MuellerMatrix {
    /// Identity Mueller matrix.
    #[must_use]
    pub fn identity() -> Self {
        let mut m = [[0.0; 4]; 4];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        Self { m }
    }

    /// Transforms a Stokes vector: $\mathbf{S}' = \mathbf{M} \mathbf{S}$.
    #[must_use]
    pub fn apply(&self, s: StokesVector) -> StokesVector {
        let s_arr = [s.s0, s.s1, s.s2, s.s3];
        let mut res = [0.0; 4];
        for (i, row) in self.m.iter().enumerate() {
            for (j, &val) in s_arr.iter().enumerate() {
                res[i] += row[j] * val;
            }
        }
        StokesVector {
            s0: res[0],
            s1: res[1],
            s2: res[2],
            s3: res[3],
        }
    }
}

/// Constructs reflection and transmission Jones matrices for a coated optical surface.
#[must_use]
pub fn surface_coating_jones(coating: &CoatingResponse) -> (JonesMatrix, JonesMatrix) {
    let reflection = JonesMatrix::diattenuator(coating.rs, coating.rp);
    let transmission = JonesMatrix::diattenuator(coating.ts, coating.tp);
    (reflection, transmission)
}

/// Constructs bare Fresnel dielectric reflection and transmission Jones matrices.
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if refractive indices are invalid.
pub fn surface_fresnel_jones(n1: f64, n2: f64, angle_i_rad: f64) -> Result<(JonesMatrix, JonesMatrix), OpticsError> {
    if !n1.is_finite() || n1 <= 0.0 || !n2.is_finite() || n2 <= 0.0 || !angle_i_rad.is_finite() {
        return Err(OpticsError::InvalidParameter(
            "Invalid parameters to surface_fresnel_jones".to_string(),
        ));
    }

    let sin_i = angle_i_rad.sin();
    let cos_i = angle_i_rad.cos();

    let sin_t = (n1 * sin_i) / n2;
    if sin_t.abs() > 1.0 {
        // Total internal reflection: rs and rp have magnitude 1.0 with phase shift
        let rs_phase = -2.0 * ((sin_i * sin_i - (n2 / n1).powi(2)).sqrt() / cos_i).atan();
        let rp_phase = -2.0 * (((n1 / n2).powi(2) * (sin_i * sin_i - (n2 / n1).powi(2)).sqrt()) / cos_i).atan();

        let rs = Complex::new(rs_phase.cos(), rs_phase.sin());
        let rp = Complex::new(rp_phase.cos(), rp_phase.sin());

        let r_mat = JonesMatrix::diattenuator(rs, rp);
        let t_mat = JonesMatrix::diattenuator(Complex::zero(), Complex::zero());
        return Ok((r_mat, t_mat));
    }

    let cos_t = (1.0 - sin_t * sin_t).max(0.0).sqrt();

    let rs = (n1 * cos_i - n2 * cos_t) / (n1 * cos_i + n2 * cos_t);
    let rp = (n2 * cos_i - n1 * cos_t) / (n2 * cos_i + n1 * cos_t);

    let ts = (2.0 * n1 * cos_i) / (n1 * cos_i + n2 * cos_t);
    let tp = (2.0 * n1 * cos_i) / (n2 * cos_i + n1 * cos_t);

    let r_mat = JonesMatrix::diattenuator(Complex::real(rs), Complex::real(rp));
    let t_mat = JonesMatrix::diattenuator(Complex::real(ts), Complex::real(tp));

    Ok((r_mat, t_mat))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_brewster_angle_p_reflectance_zero() -> TestResult {
        // Air (1.0) to Glass (1.5)
        let n1: f64 = 1.0;
        let n2: f64 = 1.5;
        let theta_b = (n2 / n1).atan(); // Brewster angle ~56.3 degrees

        let (r_mat, _) = surface_fresnel_jones(n1, n2, theta_b)?;
        let r_p = r_mat.m[1][1];

        // At Brewster angle, rp must be zero
        assert!(
            r_p.norm_sq() < 1e-12,
            "rp must vanish at Brewster angle, got norm_sq={}",
            r_p.norm_sq()
        );
        Ok(())
    }

    #[test]
    fn test_linear_polarizer_extinction() -> TestResult {
        // Horizontal linear polarizer
        let pol_h = JonesMatrix::linear_polarizer(0.0);

        // Vertical input ray
        let ray_v = JonesVector::linear_vertical();
        let transmitted = pol_h.apply(ray_v);

        // Crossed polarizers extinguish completely
        assert!(transmitted.intensity() < 1e-15);

        // Horizontal input passes unattenuated
        let ray_h = JonesVector::linear_horizontal();
        let passed = pol_h.apply(ray_h);
        assert!((passed.intensity() - 1.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_quarter_wave_plate_creates_circular() -> TestResult {
        // QWP with fast axis at 0, input at +45 deg produces circular polarization
        let qwp = JonesMatrix::quarter_wave_plate(0.0);
        let ray_45 = JonesVector::linear_45();
        let output = qwp.apply(ray_45);

        let stokes = output.to_stokes();
        assert!((stokes.s0 - 1.0).abs() < 1e-12);
        assert!(stokes.s1.abs() < 1e-12);
        assert!(stokes.s2.abs() < 1e-12);
        assert!((stokes.s3.abs() - 1.0).abs() < 1e-12);
        assert!((stokes.degree_of_circular_polarization() - 1.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_mueller_matrix_conversion() -> TestResult {
        let pol_45 = JonesMatrix::linear_polarizer(std::f64::consts::FRAC_PI_4);
        let m_pol = pol_45.to_mueller();

        // Unpolarized input beam through 45 deg polarizer becomes 50% intensity at +45 deg
        let unpolarized = StokesVector::unpolarized(1.0);
        let out_stokes = m_pol.apply(unpolarized);

        assert!((out_stokes.s0 - 0.5).abs() < 1e-12);
        assert!((out_stokes.s2 - 0.5).abs() < 1e-12);
        assert!(out_stokes.s1.abs() < 1e-12);
        assert!(out_stokes.s3.abs() < 1e-12);
        Ok(())
    }
}
