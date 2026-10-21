//! Optical media, dispersion formulas, bulk absorption, and surface finish models.

use crate::error::NonSeqError;
use optics_glass::catalog::GlassCatalogDb;
use serde::{Deserialize, Serialize};

/// Dispersion model for an optical medium.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OpticalMedium {
    /// Vacuum or ambient air ($n = 1.0$).
    Air,
    /// Constant nondispersive index.
    Constant {
        /// Refractive index value.
        n: f64,
    },
    /// Glass material resolved from glass catalog database.
    GlassCatalog {
        /// Catalog glass name (e.g. "N-BK7", "SF11", "Fused Silica").
        name: String,
    },
    /// Standard 3-term Sellmeier dispersion equation.
    Sellmeier1 {
        /// $B_1$ coefficient.
        b1: f64,
        /// $C_1$ coefficient in $\mu\text{m}^2$.
        c1: f64,
        /// $B_2$ coefficient.
        b2: f64,
        /// $C_2$ coefficient in $\mu\text{m}^2$.
        c2: f64,
        /// $B_3$ coefficient.
        b3: f64,
        /// $C_3$ coefficient in $\mu\text{m}^2$.
        c3: f64,
    },
    /// Schott dispersion polynomial ($A_0 \dots A_5$).
    Schott {
        /// $A_0$ constant coefficient.
        a0: f64,
        /// $A_1 \cdot \lambda^2$ coefficient.
        a1: f64,
        /// $A_2 \cdot \lambda^{-2}$ coefficient.
        a2: f64,
        /// $A_3 \cdot \lambda^{-4}$ coefficient.
        a3: f64,
        /// $A_4 \cdot \lambda^{-6}$ coefficient.
        a4: f64,
        /// $A_5 \cdot \lambda^{-8}$ coefficient.
        a5: f64,
    },
}

impl Default for OpticalMedium {
    fn default() -> Self {
        Self::Air
    }
}

impl OpticalMedium {
    /// Compute refractive index at specified wavelength in nanometers.
    pub fn refractive_index(&self, wavelength_nm: f64, catalog: Option<&GlassCatalogDb>) -> Result<f64, NonSeqError> {
        if wavelength_nm <= 0.0 || !wavelength_nm.is_finite() {
            return Err(NonSeqError::InvalidGeometry(format!(
                "Invalid wavelength: {wavelength_nm}"
            )));
        }

        match *self {
            OpticalMedium::Air => Ok(1.0),
            OpticalMedium::Constant { n } => {
                if n < 1.0 || !n.is_finite() {
                    return Err(NonSeqError::Material(format!("Invalid refractive index: {n}")));
                }
                Ok(n)
            }
            OpticalMedium::GlassCatalog { ref name } => {
                let cat = catalog
                    .ok_or_else(|| NonSeqError::Catalog("Glass catalog database required for lookup".to_string()))?;
                let glass = cat
                    .lookup(name)
                    .ok_or_else(|| NonSeqError::Catalog(format!("Glass '{name}' not found in catalog")))?;
                Ok(glass.refractive_index(wavelength_nm))
            }
            OpticalMedium::Sellmeier1 { b1, c1, b2, c2, b3, c3 } => {
                let l_um = wavelength_nm * 1e-3;
                let l2 = l_um * l_um;
                let term1 = (b1 * l2) / (l2 - c1);
                let term2 = (b2 * l2) / (l2 - c2);
                let term3 = (b3 * l2) / (l2 - c3);
                let n2 = 1.0 + term1 + term2 + term3;
                if n2 <= 0.0 || !n2.is_finite() {
                    return Err(NonSeqError::Material(format!(
                        "Non-physical Sellmeier index squared: {n2}"
                    )));
                }
                Ok(n2.sqrt())
            }
            OpticalMedium::Schott { a0, a1, a2, a3, a4, a5 } => {
                let l_um = wavelength_nm * 1e-3;
                let l2 = l_um * l_um;
                let inv_l2 = 1.0 / l2;
                let inv_l4 = inv_l2 * inv_l2;
                let inv_l6 = inv_l4 * inv_l2;
                let inv_l8 = inv_l6 * inv_l2;
                let n2 = a0 + a1 * l2 + a2 * inv_l2 + a3 * inv_l4 + a4 * inv_l6 + a5 * inv_l8;
                if n2 <= 0.0 || !n2.is_finite() {
                    return Err(NonSeqError::Material(format!(
                        "Non-physical Schott index squared: {n2}"
                    )));
                }
                Ok(n2.sqrt())
            }
        }
    }
}

/// Bulk absorption parameters obeying the Beer-Lambert law.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulkAbsorption {
    /// Internal transmittance $\tau_0 \in (0, 1]$ across reference thickness $t_0$.
    pub internal_transmittance: f64,
    /// Reference sample thickness $t_0$ in millimeters.
    pub reference_thickness_mm: f64,
}

impl BulkAbsorption {
    /// Create a bulk absorption model from internal transmittance.
    pub fn new(internal_transmittance: f64, reference_thickness_mm: f64) -> Result<Self, NonSeqError> {
        if internal_transmittance <= 0.0 || internal_transmittance > 1.0 || !internal_transmittance.is_finite() {
            return Err(NonSeqError::Material(
                "Internal transmittance must be in (0, 1]".to_string(),
            ));
        }
        if reference_thickness_mm <= 0.0 || !reference_thickness_mm.is_finite() {
            return Err(NonSeqError::Material(
                "Reference thickness must be positive".to_string(),
            ));
        }
        Ok(Self {
            internal_transmittance,
            reference_thickness_mm,
        })
    }

    /// Linear attenuation coefficient $\alpha = -\ln(\tau_0)/t_0$ in $\text{mm}^{-1}$.
    pub fn alpha(&self) -> f64 {
        -self.internal_transmittance.ln() / self.reference_thickness_mm
    }

    /// Compute transmission factor $\exp(-\alpha L)$ for ray segment length $L$ in millimeters.
    pub fn transmission(&self, length_mm: f64) -> f64 {
        if length_mm <= 0.0 {
            1.0
        } else {
            (-self.alpha() * length_mm).exp()
        }
    }
}

/// Surface optical interaction behavior at object boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceFinish {
    /// Ideal smooth specular dielectric interface (Snell refraction + Fresnel reflection).
    Specular,
    /// Mirror reflection with specified power reflectance $R \in [0, 1]$.
    Mirror {
        /// Power reflectance coefficient.
        reflectance: f64,
    },
    /// Diffuse Lambertian scattering with specified albedo $\rho \in [0, 1]$.
    Lambertian {
        /// Diffuse reflection albedo.
        albedo: f64,
    },
    /// Ideal blackbody absorber: completely terminates incident ray energy.
    Absorber,
}

impl Default for SurfaceFinish {
    fn default() -> Self {
        Self::Specular
    }
}

/// Combined material specification defining interior volume and bounding surface finish.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqMaterial {
    /// Interior optical medium.
    pub medium: OpticalMedium,
    /// Optional bulk absorption profile.
    pub absorption: Option<BulkAbsorption>,
    /// Bounding surface interaction finish.
    pub finish: SurfaceFinish,
}

impl Default for NonSeqMaterial {
    fn default() -> Self {
        Self {
            medium: OpticalMedium::Air,
            absorption: None,
            finish: SurfaceFinish::Specular,
        }
    }
}

impl NonSeqMaterial {
    /// Create a standard dielectric glass material.
    pub fn dielectric(medium: OpticalMedium) -> Self {
        Self {
            medium,
            absorption: None,
            finish: SurfaceFinish::Specular,
        }
    }

    /// Create an ideal planar or curved mirror.
    pub fn mirror(reflectance: f64) -> Self {
        Self {
            medium: OpticalMedium::Air,
            absorption: None,
            finish: SurfaceFinish::Mirror { reflectance },
        }
    }

    /// Create a diffuse Lambertian surface.
    pub fn lambertian(albedo: f64) -> Self {
        Self {
            medium: OpticalMedium::Air,
            absorption: None,
            finish: SurfaceFinish::Lambertian { albedo },
        }
    }

    /// Create an ideal ray absorbing surface or beam dump.
    pub fn absorber() -> Self {
        Self {
            medium: OpticalMedium::Air,
            absorption: None,
            finish: SurfaceFinish::Absorber,
        }
    }
}
