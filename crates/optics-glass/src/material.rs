//! Optical glass material data model, verified preset glasses, and extended dispersion equations.

use optics_core::RefractiveIndex;
use optics_spectral::dispersion::{
    conrady_dispersion, extended_1, extended_2, extended_cauchy, handbook_of_optics_1,
    handbook_of_optics_2, herzberger_dispersion, schott_dispersion, sellmeier_2, sellmeier_3,
    sellmeier_4, sellmeier_5,
};
use optics_spectral::sellmeier::{cauchy_dispersion, sellmeier_formula_1};
use optics_spectral::thermal::SchottThermalModel;
use serde::{Deserialize, Serialize};

/// Glass manufacturer or catalog family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GlassCatalog {
    /// Schott Glass (Germany).
    Schott,
    /// Ohara Corporation (Japan).
    Ohara,
    /// CDGM Glass Co. (China).
    Cdgm,
    /// Hoya Corporation (Japan).
    Hoya,
    /// Custom, idealized, or non-catalog media.
    Custom,
}

impl GlassCatalog {
    /// Returns the human-readable manufacturer or catalog name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Schott => "Schott",
            Self::Ohara => "OHARA",
            Self::Cdgm => "CDGM",
            Self::Hoya => "HOYA",
            Self::Custom => "Custom",
        }
    }
}

/// Dispersion equation representation for refractive index calculation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DispersionModel {
    /// Sellmeier Formula 1: $n^2 - 1 = \sum \frac{B_i \lambda^2}{\lambda^2 - C_i}$ with $\lambda$ in $\mu\text{m}$.
    Sellmeier1 {
        /// Dimensionless oscillator strengths.
        b: [f64; 3],
        /// Resonance wavelength squares in $\mu\text{m}^2$.
        c: [f64; 3],
    },
    /// Sellmeier Formula 2: $n^2 - 1 = A + \sum \frac{B_i \lambda^2}{\lambda^2 - C_i}$.
    Sellmeier2 {
        /// Constant term $A$.
        a: f64,
        /// Oscillator strengths.
        b: [f64; 2],
        /// Resonance wavelength squares.
        c: [f64; 2],
    },
    /// Sellmeier Formula 3 (4-term Sellmeier).
    Sellmeier3 {
        /// Oscillator strengths $K_i$.
        k: [f64; 4],
        /// Resonance wavelength squares $L_i$.
        l: [f64; 4],
    },
    /// Sellmeier Formula 4: $n^2 = A + \frac{B \lambda^2}{\lambda^2 - C} + \frac{D \lambda^2}{\lambda^2 - E}$.
    Sellmeier4 {
        /// Offset $A$.
        a: f64,
        /// Coefficient $B$.
        b: f64,
        /// Resonance $C$.
        c: f64,
        /// Coefficient $D$.
        d: f64,
        /// Resonance $E$.
        e: f64,
    },
    /// Sellmeier Formula 5 (5-term Sellmeier).
    Sellmeier5 {
        /// Oscillator strengths $K_i$.
        k: [f64; 5],
        /// Resonance wavelength squares $L_i$.
        l: [f64; 5],
    },
    /// Schott empirical equation: $n^2 = a_0 + a_1 \lambda^2 + a_2 \lambda^{-2} + \dots + a_5 \lambda^{-8}$.
    Schott {
        /// Six polynomial coefficients.
        a: [f64; 6],
    },
    /// Herzberger empirical equation.
    Herzberger {
        /// Five Herzberger coefficients.
        coeffs: [f64; 5],
    },
    /// Conrady equation: $n = n_0 + A / \lambda + B / \lambda^{3.5}$.
    Conrady {
        /// Baseline index $n_0$.
        n0: f64,
        /// Coefficient $A$.
        a: f64,
        /// Coefficient $B$.
        b: f64,
    },
    /// Handbook of Optics Formula 1.
    HandbookOfOptics1 {
        /// Offset $A$.
        a: f64,
        /// Coefficient $B$.
        b: f64,
        /// Resonance $C$.
        c: f64,
        /// Dispersion slope $D$.
        d: f64,
    },
    /// Handbook of Optics Formula 2.
    HandbookOfOptics2 {
        /// Offset $A$.
        a: f64,
        /// Coefficient $B$.
        b: f64,
        /// Resonance $C$.
        c: f64,
        /// Dispersion slope $D$.
        d: f64,
    },
    /// Extended Formula 1 (high-order infrared expansion).
    Extended1 {
        /// Eight polynomial coefficients.
        a: [f64; 8],
    },
    /// Extended Formula 2.
    Extended2 {
        /// Seven polynomial coefficients.
        a: [f64; 7],
    },
    /// Cauchy dispersion: $n(\lambda) = A + B / \lambda^2$.
    Cauchy {
        /// Offset coefficient $A$.
        a: f64,
        /// Dispersion coefficient $B$ in $\mu\text{m}^2$.
        b: f64,
    },
    /// Extended Cauchy dispersion: $n(\lambda) = A + B / \lambda^2 + C / \lambda^4 + D / \lambda^6$.
    ExtendedCauchy {
        /// Offset $A$.
        a: f64,
        /// $B$ coefficient.
        b: f64,
        /// $C$ coefficient.
        c: f64,
        /// $D$ coefficient.
        d: f64,
    },
    /// Synthetic model glass defined by $(n_d, V_d, \Delta P_{g,F})$.
    ModelGlass {
        /// Refractive index at helium yellow $d$-line.
        nd: f64,
        /// Abbe number $V_d$.
        vd: f64,
        /// Deviation from normal partial dispersion $\Delta P_{g,F}$.
        d_pgf: f64,
        /// Cached solved Cauchy coefficients $[A, B, C, D]$.
        fitted_cauchy: [f64; 4],
    },
    /// Constant refractive index across all wavelengths.
    Constant {
        /// Fixed index $n$.
        n: f64,
    },
    /// Air medium with constant $n = 1.0000$.
    Air,
    /// Idealized reflective mirror surface ($n = -1.0$).
    Mirror,
}

/// Optical glass material with physical dispersion formula, thermal coefficients, and catalog parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlassMaterial {
    /// Glass commercial designation (e.g. "N-BK7").
    pub name: String,
    /// Catalog manufacturer.
    pub catalog: GlassCatalog,
    /// Refractive index at helium yellow $d$ line (587.56 nm).
    pub nd: f64,
    /// Abbe dispersion number $V_d = (n_d - 1) / (n_F - n_C)$.
    pub vd: f64,
    /// Analytical dispersion model.
    pub dispersion: DispersionModel,
    /// Optional Schott thermal dispersion coefficients $(D_0, D_1, D_2, E_0, E_1, \lambda_{\text{TK}})$.
    pub thermal: Option<SchottThermalModel>,
    /// Internal spectral transmittances as pairs of `(wavelength_nm, internal_transmittance)`.
    pub internal_transmittance: Vec<(f64, f64)>,
    /// Reference glass sample thickness for internal transmittance (typically 10.0 or 25.0 mm).
    pub test_thickness_mm: f64,
}

impl GlassMaterial {
    /// Computes refractive index $n$ at reference temperature $20^\circ\text{C}$
    /// for specified wavelength in nanometers.
    #[must_use]
    pub fn refractive_index(&self, wavelength_nm: f64) -> f64 {
        let wavelength_um = wavelength_nm / 1000.0;
        match &self.dispersion {
            DispersionModel::Sellmeier1 { b, c } => {
                sellmeier_formula_1(wavelength_um, *b, *c).unwrap_or(self.nd)
            }
            DispersionModel::Sellmeier2 { a, b, c } => {
                sellmeier_2(wavelength_um, *a, *b, *c).unwrap_or(self.nd)
            }
            DispersionModel::Sellmeier3 { k, l } => {
                sellmeier_3(wavelength_um, *k, *l).unwrap_or(self.nd)
            }
            DispersionModel::Sellmeier4 { a, b, c, d, e } => {
                sellmeier_4(wavelength_um, *a, *b, *c, *d, *e).unwrap_or(self.nd)
            }
            DispersionModel::Sellmeier5 { k, l } => {
                sellmeier_5(wavelength_um, *k, *l).unwrap_or(self.nd)
            }
            DispersionModel::Schott { a } => {
                schott_dispersion(wavelength_um, *a).unwrap_or(self.nd)
            }
            DispersionModel::Herzberger { coeffs } => {
                herzberger_dispersion(wavelength_um, *coeffs).unwrap_or(self.nd)
            }
            DispersionModel::Conrady { n0, a, b } => {
                conrady_dispersion(wavelength_um, *n0, *a, *b).unwrap_or(self.nd)
            }
            DispersionModel::HandbookOfOptics1 { a, b, c, d } => {
                handbook_of_optics_1(wavelength_um, *a, *b, *c, *d).unwrap_or(self.nd)
            }
            DispersionModel::HandbookOfOptics2 { a, b, c, d } => {
                handbook_of_optics_2(wavelength_um, *a, *b, *c, *d).unwrap_or(self.nd)
            }
            DispersionModel::Extended1 { a } => extended_1(wavelength_um, *a).unwrap_or(self.nd),
            DispersionModel::Extended2 { a } => extended_2(wavelength_um, *a).unwrap_or(self.nd),
            DispersionModel::Cauchy { a, b } => {
                cauchy_dispersion(wavelength_um, *a, *b).unwrap_or(self.nd)
            }
            DispersionModel::ExtendedCauchy { a, b, c, d } => {
                extended_cauchy(wavelength_um, *a, *b, *c, *d).unwrap_or(self.nd)
            }
            DispersionModel::ModelGlass { fitted_cauchy, .. } => extended_cauchy(
                wavelength_um,
                fitted_cauchy[0],
                fitted_cauchy[1],
                fitted_cauchy[2],
                fitted_cauchy[3],
            )
            .unwrap_or(self.nd),
            DispersionModel::Constant { n } => *n,
            DispersionModel::Air => 1.0,
            DispersionModel::Mirror => -1.0,
        }
    }

    /// Computes refractive index $n(\lambda, T)$ accounting for thermal drift from $20^\circ\text{C}$.
    #[must_use]
    pub fn refractive_index_at_temp(&self, wavelength_nm: f64, temperature_c: f64) -> f64 {
        let n_base = self.refractive_index(wavelength_nm);
        if let Some(ref thermal) = self.thermal {
            let delta_t = temperature_c - 20.0;
            let dn = thermal
                .delta_n(n_base, wavelength_nm / 1000.0, delta_t)
                .unwrap_or(0.0);
            n_base + dn
        } else {
            n_base
        }
    }

    /// Creates a synthetic model glass parameterized by $(n_d, V_d, \Delta P_{g,F})$.
    #[must_use]
    pub fn model_glass(name: &str, nd: f64, vd: f64, d_pgf: f64) -> Self {
        let fitted = solve_model_glass_cauchy(nd, vd, d_pgf);
        Self {
            name: name.to_string(),
            catalog: GlassCatalog::Custom,
            nd,
            vd,
            dispersion: DispersionModel::ModelGlass {
                nd,
                vd,
                d_pgf,
                fitted_cauchy: fitted,
            },
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Preset Schott N-BK7 crown glass ($n_d = 1.51680, V_d = 64.17$).
    #[must_use]
    pub fn n_bk7() -> Self {
        Self {
            name: "N-BK7".to_string(),
            catalog: GlassCatalog::Schott,
            nd: 1.51680,
            vd: 64.17,
            dispersion: DispersionModel::Sellmeier1 {
                b: [1.03961212, 0.231792344, 1.01046945],
                c: [0.00600069867, 0.0200179144, 103.560653],
            },
            thermal: Some(SchottThermalModel {
                d0: 1.86e-6,
                d1: 1.31e-8,
                d2: -1.37e-11,
                e0: 4.34e-7,
                e1: 6.27e-10,
                lambda_tk: 0.170,
            }),
            internal_transmittance: vec![(400.0, 0.992), (550.0, 0.998), (700.0, 0.998)],
            test_thickness_mm: 10.0,
        }
    }

    /// Preset Schott SF11 dense flint glass ($n_d = 1.78472, V_d = 25.76$).
    #[must_use]
    pub fn sf11() -> Self {
        Self {
            name: "SF11".to_string(),
            catalog: GlassCatalog::Schott,
            nd: 1.78472,
            vd: 25.76,
            dispersion: DispersionModel::Sellmeier1 {
                b: [1.73848403, 0.311168974, 1.17490871],
                c: [0.0136068604, 0.0615960463, 121.922711],
            },
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Preset Schott N-SF11 lead-free dense flint glass ($n_d = 1.78472, V_d = 25.68$).
    #[must_use]
    pub fn n_sf11() -> Self {
        Self {
            name: "N-SF11".to_string(),
            catalog: GlassCatalog::Schott,
            nd: 1.78472,
            vd: 25.68,
            dispersion: DispersionModel::Sellmeier1 {
                b: [1.73759695, 0.313747346, 1.89878101],
                c: [0.013188707, 0.0623068142, 155.23629],
            },
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Preset Schott F2 flint glass ($n_d = 1.62004, V_d = 36.37$).
    #[must_use]
    pub fn f2() -> Self {
        Self {
            name: "F2".to_string(),
            catalog: GlassCatalog::Schott,
            nd: 1.62004,
            vd: 36.37,
            dispersion: DispersionModel::Sellmeier1 {
                b: [1.34533359, 0.209073176, 0.937357162],
                c: [0.00997743871, 0.0470450767, 111.886764],
            },
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Preset synthetic Fused Silica (Schott LITHOSIL-Q, $n_d = 1.45844, V_d = 67.83$).
    #[must_use]
    pub fn fused_silica() -> Self {
        Self {
            name: "Fused Silica".to_string(),
            catalog: GlassCatalog::Schott,
            nd: 1.45844,
            vd: 67.83,
            dispersion: DispersionModel::Sellmeier1 {
                b: [0.67071081, 0.433322857, 0.877379057],
                c: [0.00449192312, 0.0132812976, 95.8899878],
            },
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Idealized Air medium ($n = 1.0000$ constant).
    #[must_use]
    pub fn air() -> Self {
        Self {
            name: "Air".to_string(),
            catalog: GlassCatalog::Custom,
            nd: 1.0000,
            vd: f64::INFINITY,
            dispersion: DispersionModel::Air,
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 10.0,
        }
    }

    /// Idealized Mirror reflective surface ($n = -1.0$).
    #[must_use]
    pub fn mirror() -> Self {
        Self {
            name: "Mirror".to_string(),
            catalog: GlassCatalog::Custom,
            nd: -1.0,
            vd: f64::INFINITY,
            dispersion: DispersionModel::Mirror,
            thermal: None,
            internal_transmittance: Vec::new(),
            test_thickness_mm: 0.0,
        }
    }
}

impl RefractiveIndex for GlassMaterial {
    #[inline]
    fn n(&self, wavelength_nm: f64) -> f64 {
        self.refractive_index(wavelength_nm)
    }
}

/// Solves 4-term Cauchy expansion coefficients $[A, B, C, D]$ for model glass.
fn solve_model_glass_cauchy(nd: f64, vd: f64, d_pgf: f64) -> [f64; 4] {
    let ld = 0.58756;
    let lf = 0.48613;
    let lc = 0.65627;
    let lg = 0.43583;

    let delta_n = (nd - 1.0) / vd.max(1.0);
    let xd = 1.0 / (ld * ld);
    let xf = 1.0 / (lf * lf);
    let xc = 1.0 / (lc * lc);
    let xg = 1.0 / (lg * lg);

    // Approximate nc and nf based on Cauchy interpolation
    let frac = (xd - xc) / (xf - xc);
    let nc = nd - delta_n * frac;
    let nf = nc + delta_n;
    let p_normal = 0.6438 - 0.001682 * vd;
    let pgf = p_normal + d_pgf;
    let ng = nf + pgf * delta_n;

    // Set up linear system M * [A, B, C, D]^T = [nd, nf, nc, ng]^T
    let mut mat = [
        [1.0, xd, xd * xd, xd * xd * xd, nd],
        [1.0, xf, xf * xf, xf * xf * xf, nf],
        [1.0, xc, xc * xc, xc * xc * xc, nc],
        [1.0, xg, xg * xg, xg * xg * xg, ng],
    ];

    // Simple Gaussian elimination with partial pivoting for 4x4
    for i in 0..4 {
        let mut max_row = i;
        let mut max_val = mat[i][i].abs();
        for (k, row) in mat.iter().enumerate().skip(i + 1) {
            if row[i].abs() > max_val {
                max_val = row[i].abs();
                max_row = k;
            }
        }
        if max_row != i {
            mat.swap(i, max_row);
        }

        let pivot = mat[i][i];
        if pivot.abs() < 1e-14 {
            return [nd, 0.005, 0.0, 0.0];
        }

        for j in i..5 {
            mat[i][j] /= pivot;
        }

        for k in 0..4 {
            if k != i {
                let factor = mat[k][i];
                for j in i..5 {
                    mat[k][j] -= factor * mat[i][j];
                }
            }
        }
    }

    [mat[0][4], mat[1][4], mat[2][4], mat[3][4]]
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_model_glass_interpolation() -> TestResult {
        // Model glass with standard N-BK7 properties
        let model = GlassMaterial::model_glass("Model_Crown", 1.5168, 64.17, 0.0);
        let n_d = model.refractive_index(587.56);
        let n_f = model.refractive_index(486.13);
        let n_c = model.refractive_index(656.27);

        assert!((n_d - 1.5168).abs() < 1e-4);
        let vd_calc = (n_d - 1.0) / (n_f - n_c);
        assert!((vd_calc - 64.17).abs() < 0.1);
        Ok(())
    }

    #[test]
    fn test_nbk7_thermal_shift() -> TestResult {
        let nbk7 = GlassMaterial::n_bk7();
        let n_20c = nbk7.refractive_index_at_temp(587.56, 20.0);
        let n_40c = nbk7.refractive_index_at_temp(587.56, 40.0);

        assert!(n_40c > n_20c);
        assert!(n_40c - n_20c < 1e-4);
        Ok(())
    }

    #[test]
    fn test_mirror_material() {
        let mirror = GlassMaterial::mirror();
        assert_eq!(mirror.refractive_index(550.0), -1.0);
    }
}
