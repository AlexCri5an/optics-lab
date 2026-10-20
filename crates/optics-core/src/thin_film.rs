//! Optical thin film coatings and Transfer-Matrix Method (TMM) solver.
//!
//! Evaluates complex amplitude reflection and transmission coefficients ($r_s, r_p, t_s, t_p$),
//! power reflectance and transmittance ($R_s, R_p, T_s, T_p$), and phase shifts for arbitrary
//! multilayer dielectric and absorbing stacks as a function of wavelength and angle of incidence.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;

/// Lightweight self-contained complex number for thin-film transfer matrix operations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    /// Real component.
    pub re: f64,
    /// Imaginary component.
    pub im: f64,
}

impl Complex {
    /// Constructs a complex number $z = x + i y$.
    #[must_use]
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Real scalar value $z = x + 0 i$.
    #[must_use]
    pub const fn real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    /// Zero value $0 + 0 i$.
    #[must_use]
    pub const fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    /// Complex scalar multiplication.
    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self {
            re: self.re * s,
            im: self.im * s,
        }
    }

    /// Complex squared magnitude $|z|^2 = x^2 + y^2$.
    #[must_use]
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Complex absolute value / modulus $|z| = \sqrt{x^2 + y^2}$.
    #[must_use]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Complex phase / argument $\text{atan2}(y, x)$.
    #[must_use]
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    /// Complex exponential $e^z = e^x (\cos y + i \sin y)$.
    #[must_use]
    pub fn exp(self) -> Self {
        let exp_x = self.re.exp();
        Self {
            re: exp_x * self.im.cos(),
            im: exp_x * self.im.sin(),
        }
    }

    /// Complex square root using principal branch.
    #[must_use]
    pub fn sqrt(self) -> Self {
        let r = self.norm();
        let u = ((r + self.re) / 2.0).max(0.0).sqrt();
        let v = if self.im >= 0.0 {
            ((r - self.re) / 2.0).max(0.0).sqrt()
        } else {
            -((r - self.re) / 2.0).max(0.0).sqrt()
        };
        Self { re: u, im: v }
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let denom = rhs.re * rhs.re + rhs.im * rhs.im;
        if denom < 1e-28 {
            return Self::zero();
        }
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / denom,
            im: (self.im * rhs.re - self.re * rhs.im) / denom,
        }
    }
}

/// A single isotropic thin-film layer within a multilayer stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinFilmLayer {
    /// Real part of refractive index $n$.
    pub n: f64,
    /// Extinction coefficient $\kappa$ ($\tilde{n} = n - i \kappa$).
    pub k: f64,
    /// Physical film thickness in nanometers.
    pub thickness_nm: f64,
}

impl ThinFilmLayer {
    /// Creates a dielectric lossless layer ($k = 0$).
    #[must_use]
    pub fn dielectric(n: f64, thickness_nm: f64) -> Self {
        Self {
            n,
            k: 0.0,
            thickness_nm,
        }
    }

    /// Creates an absorbing or metallic thin film layer.
    #[must_use]
    pub fn absorbing(n: f64, k: f64, thickness_nm: f64) -> Self {
        Self { n, k, thickness_nm }
    }
}

/// Optical performance response of a thin-film coating stack at a specific wavelength and angle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoatingResponse {
    /// Complex reflection amplitude coefficient for s-polarization $r_s$.
    pub rs: Complex,
    /// Complex reflection amplitude coefficient for p-polarization $r_p$.
    pub rp: Complex,
    /// Complex transmission amplitude coefficient for s-polarization $t_s$.
    pub ts: Complex,
    /// Complex transmission amplitude coefficient for p-polarization $t_p$.
    pub tp: Complex,
    /// Power reflectance for s-polarization $R_s = |r_s|^2 \in [0, 1]$.
    pub reflectance_s: f64,
    /// Power reflectance for p-polarization $R_p = |r_p|^2 \in [0, 1]$.
    pub reflectance_p: f64,
    /// Power transmittance for s-polarization $T_s \in [0, 1]$.
    pub transmittance_s: f64,
    /// Power transmittance for p-polarization $T_p \in [0, 1]$.
    pub transmittance_p: f64,
    /// Phase of reflection coefficient for s-polarization in radians.
    pub phase_rs_rad: f64,
    /// Phase of reflection coefficient for p-polarization in radians.
    pub phase_rp_rad: f64,
}

impl CoatingResponse {
    /// Unpolarized average power reflectance $R = (R_s + R_p) / 2$.
    #[must_use]
    pub fn unpolarized_reflectance(&self) -> f64 {
        (self.reflectance_s + self.reflectance_p) * 0.5
    }

    /// Unpolarized average power transmittance $T = (T_s + T_p) / 2$.
    #[must_use]
    pub fn unpolarized_transmittance(&self) -> f64 {
        (self.transmittance_s + self.transmittance_p) * 0.5
    }
}

/// Multilayer optical coating stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Coating {
    /// Idealized anti-reflection coating ($R = 0, T = 1$).
    IdealAr,
    /// Idealized mirror coating ($R = 1, T = 0$).
    IdealMirror,
    /// Idealized 50/50 non-polarizing beamsplitter ($R = 0.5, T = 0.5$).
    IdealBeamsplitter,
    /// Explicit multilayer thin-film stack calculated via transfer-matrix method.
    Multilayer {
        /// Sequence of layers ordered from incident medium to substrate.
        layers: Vec<ThinFilmLayer>,
    },
}

impl Coating {
    /// Preset: Single quarter-wave anti-reflection coating on a dielectric substrate.
    ///
    /// Film index is chosen as $n = \sqrt{n_{\text{ambient}} \cdot n_{\text{substrate}}}$
    /// and optical thickness is $\lambda_0 / 4$.
    #[must_use]
    pub fn quarter_wave_ar(n_ambient: f64, n_substrate: f64, design_wavelength_nm: f64) -> Self {
        let n_opt = (n_ambient * n_substrate).sqrt();
        let thickness_nm = design_wavelength_nm / (4.0 * n_opt);
        Self::Multilayer {
            layers: vec![ThinFilmLayer::dielectric(n_opt, thickness_nm)],
        }
    }

    /// Preset: Dielectric Distributed Bragg Reflector (Quarter-Wave Stack) high-reflectance mirror.
    ///
    /// Alternates pairs of high and low index layers: $(H L)^N H$.
    #[must_use]
    pub fn bragg_mirror(n_high: f64, n_low: f64, pairs: usize, design_wavelength_nm: f64) -> Self {
        let d_h = design_wavelength_nm / (4.0 * n_high);
        let d_l = design_wavelength_nm / (4.0 * n_low);

        let mut layers = Vec::with_capacity(pairs * 2 + 1);
        for _ in 0..pairs {
            layers.push(ThinFilmLayer::dielectric(n_high, d_h));
            layers.push(ThinFilmLayer::dielectric(n_low, d_l));
        }
        layers.push(ThinFilmLayer::dielectric(n_high, d_h));

        Self::Multilayer { layers }
    }

    /// Evaluates the coating response using the Transfer-Matrix Method (TMM).
    ///
    /// # Arguments
    /// * `n_incident` — Refractive index of incident medium (e.g. 1.0 for air).
    /// * `n_substrate` — Refractive index of exit / substrate medium.
    /// * `angle_incident_rad` — Angle of incidence $\theta_0$ in radians ($[0, \pi/2)$).
    /// * `wavelength_nm` — Wavelength in vacuum in nanometers.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if inputs are non-finite or non-physical.
    pub fn evaluate(
        &self,
        n_incident: f64,
        n_substrate: f64,
        angle_incident_rad: f64,
        wavelength_nm: f64,
    ) -> Result<CoatingResponse, OpticsError> {
        if !n_incident.is_finite()
            || n_incident <= 0.0
            || !n_substrate.is_finite()
            || n_substrate <= 0.0
            || !angle_incident_rad.is_finite()
            || !wavelength_nm.is_finite()
            || wavelength_nm <= 0.0
        {
            return Err(OpticsError::InvalidParameter(
                "Invalid parameters to coating evaluate".to_string(),
            ));
        }

        match self {
            Self::IdealAr => Ok(CoatingResponse {
                rs: Complex::zero(),
                rp: Complex::zero(),
                ts: Complex::real(1.0),
                tp: Complex::real(1.0),
                reflectance_s: 0.0,
                reflectance_p: 0.0,
                transmittance_s: 1.0,
                transmittance_p: 1.0,
                phase_rs_rad: 0.0,
                phase_rp_rad: 0.0,
            }),
            Self::IdealMirror => Ok(CoatingResponse {
                rs: Complex::real(-1.0),
                rp: Complex::real(1.0),
                ts: Complex::zero(),
                tp: Complex::zero(),
                reflectance_s: 1.0,
                reflectance_p: 1.0,
                transmittance_s: 0.0,
                transmittance_p: 0.0,
                phase_rs_rad: std::f64::consts::PI,
                phase_rp_rad: 0.0,
            }),
            Self::IdealBeamsplitter => {
                let r_amp = (0.5_f64).sqrt();
                let t_amp = (0.5_f64).sqrt();
                Ok(CoatingResponse {
                    rs: Complex::real(r_amp),
                    rp: Complex::real(r_amp),
                    ts: Complex::real(t_amp),
                    tp: Complex::real(t_amp),
                    reflectance_s: 0.5,
                    reflectance_p: 0.5,
                    transmittance_s: 0.5,
                    transmittance_p: 0.5,
                    phase_rs_rad: 0.0,
                    phase_rp_rad: 0.0,
                })
            }
            Self::Multilayer { layers } => evaluate_tmm_multilayer(
                n_incident,
                n_substrate,
                angle_incident_rad,
                wavelength_nm,
                layers,
            ),
        }
    }
}

/// High-level surface coating specification applied to an optical interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceCoating {
    /// Idealized 100% Anti-Reflection coating ($R = 0\%, T = 100\%$).
    IdealAr,
    /// Standard single-layer Magnesium Fluoride ($\text{MgF}_2$, $n \approx 1.38$) $\lambda/4$ coating.
    MgF2 {
        /// Center design wavelength in nanometers (standard 550.0 nm).
        design_wavelength_nm: f64,
    },
    /// High-performance 4-layer Broadband Anti-Reflection (BBAR) dielectric stack for 400–700 nm.
    BroadbandAr,
    /// High-reflectance dielectric mirror coating ($R \ge 99.9\%, T \le 0.1\%$).
    HighReflectanceMirror,
    /// Explicit user-defined multi-layer thin film stack.
    Custom(Coating),
}

impl SurfaceCoating {
    /// Creates a standard MgF2 single-layer quarter-wave AR coating at 550.0 nm.
    #[must_use]
    pub const fn mgf2_default() -> Self {
        Self::MgF2 {
            design_wavelength_nm: 550.0,
        }
    }

    /// Converts this high-level coating specification to a concrete [`Coating`] model.
    #[must_use]
    pub fn to_coating(&self, _n_substrate: f64) -> Coating {
        match self {
            Self::IdealAr => Coating::IdealAr,
            Self::MgF2 {
                design_wavelength_nm,
            } => {
                let n_mgf2 = 1.38;
                let d_nm = design_wavelength_nm / (4.0 * n_mgf2);
                Coating::Multilayer {
                    layers: vec![ThinFilmLayer::dielectric(n_mgf2, d_nm)],
                }
            }
            Self::BroadbandAr => {
                // 4-layer BBAR dielectric stack (outermost layer incident medium to substrate):
                // Layer 4 (SiO2, n=1.46, 88.0 nm)
                // Layer 3 (TiO2, n=2.30, 112.0 nm)
                // Layer 2 (SiO2, n=1.46, 32.0 nm)
                // Layer 1 (TiO2, n=2.30, 15.0 nm)
                Coating::Multilayer {
                    layers: vec![
                        ThinFilmLayer::dielectric(1.46, 88.0),
                        ThinFilmLayer::dielectric(2.30, 112.0),
                        ThinFilmLayer::dielectric(1.46, 32.0),
                        ThinFilmLayer::dielectric(2.30, 15.0),
                    ],
                }
            }
            Self::HighReflectanceMirror => {
                // 15-layer dielectric quarter-wave Bragg reflector (TiO2 n=2.30, SiO2 n=1.46) at 550nm
                Coating::bragg_mirror(2.30, 1.46, 7, 550.0)
            }
            Self::Custom(c) => c.clone(),
        }
    }

    /// Evaluates the coating response using the Transfer-Matrix Method (TMM).
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if inputs are invalid.
    pub fn evaluate(
        &self,
        n_incident: f64,
        n_substrate: f64,
        angle_incident_rad: f64,
        wavelength_nm: f64,
    ) -> Result<CoatingResponse, OpticsError> {
        let mut coating = self.to_coating(n_substrate);
        // When light propagates from a dense medium (e.g. glass) to a rare medium (e.g. air),
        // the physical coating stack is deposited on the dense substrate with its outermost layer facing air.
        // In TMM layer sequence (incident -> exit), the stack must be traversed in reverse order.
        if n_incident > n_substrate {
            if let Coating::Multilayer { ref mut layers } = coating {
                layers.reverse();
            }
        }
        coating.evaluate(n_incident, n_substrate, angle_incident_rad, wavelength_nm)
    }

    /// Short technical display label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::IdealAr => "Ideal AR",
            Self::MgF2 { .. } => "MgF2 (λ/4)",
            Self::BroadbandAr => "BBAR (Multi)",
            Self::HighReflectanceMirror => "Mirror (99.9%)",
            Self::Custom(_) => "Custom Stack",
        }
    }

    /// Parses a Zemax OpticStudio coating name (e.g. `AR`, `MGF2`, `BBAR`, `MIRROR`, `NONE`).
    #[must_use]
    pub fn from_zemax_name(name: &str) -> Option<Self> {
        let clean = name.trim().to_uppercase();
        if clean.is_empty() || clean == "NONE" || clean == "UNCOATED" {
            return None;
        }
        if clean == "MGF2" || clean.contains("MGF2") {
            Some(Self::mgf2_default())
        } else if clean == "BBAR" || clean == "HE-NE" || clean.contains("BBAR") {
            Some(Self::BroadbandAr)
        } else if clean == "MIRROR" || clean.contains("REFL") || clean == "HR" {
            Some(Self::HighReflectanceMirror)
        } else if clean == "AR" || clean.contains("AR_") || clean.contains("AR") {
            Some(Self::IdealAr)
        } else {
            // Default unknown coating to Ideal AR for conservative estimation
            Some(Self::IdealAr)
        }
    }

    /// Formats the coating name for Zemax OpticStudio export.
    #[must_use]
    pub fn to_zemax_name(&self) -> &'static str {
        match self {
            Self::IdealAr => "AR",
            Self::MgF2 { .. } => "MGF2",
            Self::BroadbandAr => "BBAR",
            Self::HighReflectanceMirror => "MIRROR",
            Self::Custom(_) => "CUSTOM",
        }
    }
}

/// Evaluates characteristic matrix multiplication for multilayer thin films.
fn evaluate_tmm_multilayer(
    n_incident: f64,
    n_substrate: f64,
    angle_incident_rad: f64,
    wavelength_nm: f64,
    layers: &[ThinFilmLayer],
) -> Result<CoatingResponse, OpticsError> {
    let sin_theta0 = angle_incident_rad.sin();
    let cos_theta0 = angle_incident_rad.cos();

    // Incident admittance
    let eta0_s = Complex::real(n_incident * cos_theta0);
    let eta0_p = Complex::real(n_incident / cos_theta0.max(1e-12));

    // Substrate admittance
    // Snell: n0 * sin(theta0) = n_sub * sin(theta_sub)
    let sin_theta_sub = (n_incident * sin_theta0) / n_substrate;
    let cos_theta_sub = if sin_theta_sub.abs() <= 1.0 {
        Complex::real((1.0 - sin_theta_sub * sin_theta_sub).max(0.0).sqrt())
    } else {
        // Total internal reflection into substrate
        Complex::new(0.0, -(sin_theta_sub * sin_theta_sub - 1.0).sqrt())
    };

    let n_sub_c = Complex::real(n_substrate);
    let eta_sub_s = n_sub_c * cos_theta_sub;
    let eta_sub_p = n_sub_c / cos_theta_sub;

    // Initial 2x2 identity matrices: [ [m11, m12], [m21, m22] ]
    let mut ms = [
        [Complex::real(1.0), Complex::zero()],
        [Complex::zero(), Complex::real(1.0)],
    ];
    let mut mp = [
        [Complex::real(1.0), Complex::zero()],
        [Complex::zero(), Complex::real(1.0)],
    ];

    for layer in layers {
        let n_c = Complex::new(layer.n, -layer.k);

        // Snell angle in layer: n_c * sin(theta) = n0 * sin(theta0)
        let sin_theta = Complex::real(n_incident * sin_theta0) / n_c;
        let cos_theta = (Complex::real(1.0) - sin_theta * sin_theta).sqrt();

        // Admittance in layer
        let eta_l_s = n_c * cos_theta;
        let eta_l_p = n_c / cos_theta;

        // Phase thickness delta = 2*pi * d / lambda * n_c * cos(theta)
        let factor = 2.0 * std::f64::consts::PI * layer.thickness_nm / wavelength_nm;
        let delta = (n_c * cos_theta).scale(factor);

        // Layer characteristic matrices
        let exp_pos = Complex::new(-delta.im, delta.re).exp(); // e^(i delta)
        let exp_neg = Complex::new(delta.im, -delta.re).exp(); // e^(-i delta)

        let cos_delta = (exp_pos + exp_neg).scale(0.5);
        let sin_delta = (exp_pos - exp_neg) * Complex::new(0.0, -0.5); // (e^(i d) - e^(-i d)) / (2 i)

        let i_sin_delta = Complex::new(-sin_delta.im, sin_delta.re); // i * sin(delta)

        // S-polarization matrix
        let m_layer_s = [
            [cos_delta, i_sin_delta / eta_l_s],
            [i_sin_delta * eta_l_s, cos_delta],
        ];

        // P-polarization matrix
        let m_layer_p = [
            [cos_delta, i_sin_delta / eta_l_p],
            [i_sin_delta * eta_l_p, cos_delta],
        ];

        // Matrix multiplication: M = M * M_layer
        ms = mat_mul_2x2(ms, m_layer_s);
        mp = mat_mul_2x2(mp, m_layer_p);
    }

    // Solve for s-polarization
    // [B; C] = M * [1; eta_sub]
    let bs = ms[0][0] + ms[0][1] * eta_sub_s;
    let cs = ms[1][0] + ms[1][1] * eta_sub_s;

    let num_rs = eta0_s * bs - cs;
    let den_rs = eta0_s * bs + cs;
    let rs = num_rs / den_rs;
    let ts = eta0_s.scale(2.0) / den_rs;

    // Solve for p-polarization
    let bp = mp[0][0] + mp[0][1] * eta_sub_p;
    let cp = mp[1][0] + mp[1][1] * eta_sub_p;

    let num_rp = eta0_p * bp - cp;
    let den_rp = eta0_p * bp + cp;
    let rp = num_rp / den_rp;
    let tp = eta0_p.scale(2.0) / den_rp;

    let reflectance_s = rs.norm_sq().clamp(0.0, 1.0);
    let reflectance_p = rp.norm_sq().clamp(0.0, 1.0);

    let transmittance_s = (eta_sub_s.re / eta0_s.re * ts.norm_sq()).clamp(0.0, 1.0);
    let transmittance_p = (eta_sub_p.re / eta0_p.re * tp.norm_sq()).clamp(0.0, 1.0);

    Ok(CoatingResponse {
        rs,
        rp,
        ts,
        tp,
        reflectance_s,
        reflectance_p,
        transmittance_s,
        transmittance_p,
        phase_rs_rad: rs.arg(),
        phase_rp_rad: rp.arg(),
    })
}

fn mat_mul_2x2(a: [[Complex; 2]; 2], b: [[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1],
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_uncoated_substrate_reflectance_matches_fresnel() -> TestResult {
        // Air (1.0) to Glass (1.5) at normal incidence
        let bare = Coating::Multilayer { layers: vec![] };
        let resp = bare.evaluate(1.0, 1.5, 0.0, 550.0)?;

        // Analytic normal Fresnel: ((1.5 - 1.0) / (1.5 + 1.0))^2 = (0.5 / 2.5)^2 = 0.04 (4%)
        assert!((resp.reflectance_s - 0.04).abs() < 1e-6);
        assert!((resp.reflectance_p - 0.04).abs() < 1e-6);
        assert!((resp.transmittance_s - 0.96).abs() < 1e-6);
        assert!((resp.transmittance_p - 0.96).abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn test_quarter_wave_ar_zero_reflectance() -> TestResult {
        // Quarter-wave coating: n_film = sqrt(1.0 * 1.5) = 1.22474487
        let ar = Coating::quarter_wave_ar(1.0, 1.5, 550.0);
        let resp = ar.evaluate(1.0, 1.5, 0.0, 550.0)?;

        // Exactly at design wavelength and normal incidence, reflectance must be zero
        assert!(
            resp.reflectance_s < 1e-6,
            "Expected zero reflectance with QW AR, got {}",
            resp.reflectance_s
        );
        assert!(resp.reflectance_p < 1e-6);
        assert!((resp.transmittance_s - 1.0).abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn test_bragg_mirror_high_reflectance() -> TestResult {
        // High index 2.3 (TiO2), Low index 1.46 (SiO2), 8 pairs
        let bragg = Coating::bragg_mirror(2.3, 1.46, 8, 550.0);
        let resp = bragg.evaluate(1.0, 1.52, 0.0, 550.0)?;

        // Reflectance of 8-pair Bragg mirror should be > 99%
        assert!(
            resp.reflectance_s > 0.99,
            "Bragg reflectance should exceed 99%, got {}",
            resp.reflectance_s
        );
        assert!(resp.reflectance_p > 0.99);
        Ok(())
    }
}
