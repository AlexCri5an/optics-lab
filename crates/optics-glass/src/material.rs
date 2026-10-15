//! Optical glass material data model and verified preset glasses.

use optics_core::RefractiveIndex;
use optics_spectral::sellmeier::{cauchy_dispersion, sellmeier_formula_1, sellmeier_formula_2};
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
    /// Sellmeier Formula 2: $n^2 - 1 = \sum \frac{A_i \lambda^2}{\lambda^2 - B_i^2}$.
    Sellmeier2 {
        /// Coefficients $A_i$.
        a: [f64; 3],
        /// Resonance wavelengths $B_i$ in $\mu\text{m}$.
        b: [f64; 3],
    },
    /// Cauchy dispersion: $n(\lambda) = A + B / \lambda^2$.
    Cauchy {
        /// Offset coefficient $A$.
        a: f64,
        /// Dispersion coefficient $B$ in $\mu\text{m}^2$.
        b: f64,
    },
    /// Constant refractive index across all wavelengths.
    Constant {
        /// Fixed index $n$.
        n: f64,
    },
    /// Air medium with constant $n = 1.0000$.
    Air,
}

/// Optical glass material with physical dispersion formula and catalog parameters.
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
}

impl GlassMaterial {
    /// Computes refractive index $n$ at the specified wavelength in nanometers.
    ///
    /// Internally converts nanometers to micrometers ($\mu\text{m} = \text{nm} / 1000.0$)
    /// when calling Sellmeier formulas.
    #[must_use]
    pub fn refractive_index(&self, wavelength_nm: f64) -> f64 {
        let wavelength_um = wavelength_nm / 1000.0;
        match &self.dispersion {
            DispersionModel::Sellmeier1 { b, c } => {
                sellmeier_formula_1(wavelength_um, *b, *c).unwrap_or(self.nd)
            }
            DispersionModel::Sellmeier2 { a, b } => {
                sellmeier_formula_2(wavelength_um, *a, *b).unwrap_or(self.nd)
            }
            DispersionModel::Cauchy { a, b } => {
                cauchy_dispersion(wavelength_um, *a, *b).unwrap_or(self.nd)
            }
            DispersionModel::Constant { n } => *n,
            DispersionModel::Air => 1.0,
        }
    }

    /// Preset Schott N-BK7 crown glass ($n_d = 1.51680, V_d = 64.17$).
    ///
    /// Sellmeier 1 coefficients from Schott catalog:
    /// - $B = [1.03961212, 0.231792344, 1.01046945]$
    /// - $C = [0.00600069867, 0.0200179144, 103.560653]\;\mu\text{m}^2$
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
        }
    }

    /// Preset Schott SF11 dense flint glass ($n_d = 1.78472, V_d = 25.76$).
    ///
    /// Sellmeier 1 coefficients from Schott catalog:
    /// - $B = [1.73848403, 0.311168974, 1.17490871]$
    /// - $C = [0.0136068604, 0.0615960463, 121.922711]\;\mu\text{m}^2$
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
        }
    }

    /// Preset Schott F2 flint glass ($n_d = 1.62004, V_d = 36.37$).
    ///
    /// Sellmeier 1 coefficients from Schott catalog:
    /// - $B = [1.34533359, 0.209073176, 0.937357162]$
    /// - $C = [0.00997743871, 0.0470450767, 111.886764]\;\mu\text{m}^2$
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
        }
    }

    /// Preset synthetic Fused Silica (Schott LITHOSIL-Q, $n_d = 1.45844, V_d = 67.83$).
    ///
    /// Sellmeier coefficients from catalog:
    /// - $B = [0.67071081, 0.433322857, 0.877379057]$
    /// - $C = [0.00449192312, 0.0132812976, 95.8899878]\;\mu\text{m}^2$
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
        }
    }
}

impl RefractiveIndex for GlassMaterial {
    #[inline]
    fn n(&self, wavelength_nm: f64) -> f64 {
        self.refractive_index(wavelength_nm)
    }
}
