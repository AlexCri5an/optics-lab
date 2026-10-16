//! Manufacturing tolerance operand types and prescription perturbation application.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;

use crate::distribution::{Rng64, ToleranceDistribution};
use crate::error::ToleranceError;

/// Manufacturing tolerance operand targeting an optical surface or element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToleranceOperandType {
    /// Surface radius of curvature tolerance $\Delta R$ in millimeters.
    Radius {
        /// Target surface index.
        surface_index: usize,
        /// Maximum radius deviation $\pm \Delta R$ in millimeters.
        delta_radius_mm: f64,
        /// Optional equivalent test plate power fringes.
        fringes: Option<f64>,
    },
    /// Surface center thickness or air-gap separation tolerance $\Delta t$ in millimeters.
    Thickness {
        /// Target surface index.
        surface_index: usize,
        /// Maximum thickness deviation $\pm \Delta t$ in millimeters.
        delta_thickness_mm: f64,
    },
    /// Conic constant tolerance $\Delta \kappa$.
    Conic {
        /// Target surface index.
        surface_index: usize,
        /// Maximum conic constant deviation $\pm \Delta \kappa$.
        delta_conic: f64,
    },
    /// Surface transverse decenter tolerance in millimeters.
    SurfaceDecenter {
        /// Target surface index.
        surface_index: usize,
        /// Maximum radial decenter $\Delta r = \sqrt{\Delta x^2 + \Delta y^2}$ in millimeters.
        max_decenter_mm: f64,
    },
    /// Surface tilt tolerance around vertex in degrees.
    SurfaceTilt {
        /// Target surface index.
        surface_index: usize,
        /// Maximum angular tilt $\Delta \theta$ in degrees.
        max_tilt_deg: f64,
    },
    /// Element wedge or surface tilt across front and back surfaces.
    ElementWedge {
        /// Element front surface index.
        front_surface: usize,
        /// Element back surface index.
        back_surface: usize,
        /// Maximum wedge angle in arcminutes.
        max_wedge_arcmin: f64,
    },
    /// Refractive index and Abbe dispersion drift.
    GlassIndexAbbe {
        /// Target surface index containing glass.
        surface_index: usize,
        /// Maximum refractive index drift $\pm \Delta n_d$ (e.g. $\pm 0.001$).
        delta_nd: f64,
        /// Maximum Abbe number drift $\pm \Delta V_d$ (e.g. $\pm 0.8\%$).
        delta_vd: f64,
    },
}

/// An individual tolerance operand with assigned distribution and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToleranceOperand {
    /// Identifier tag (e.g. "TRAD_S1", "TTHI_S2").
    pub id: String,
    /// Physical operand type and perturbation limits.
    pub operand_type: ToleranceOperandType,
    /// Statistical sampling distribution.
    pub distribution: ToleranceDistribution,
    /// Relative manufacturing cost sensitivity weight (for inverse tolerancing).
    pub cost_weight: f64,
}

impl ToleranceOperand {
    /// Create a new tolerance operand with uniform distribution and default cost weight.
    pub fn new(id: impl Into<String>, operand_type: ToleranceOperandType) -> Self {
        Self {
            id: id.into(),
            operand_type,
            distribution: ToleranceDistribution::Uniform,
            cost_weight: 1.0,
        }
    }

    /// Set explicit statistical distribution.
    pub fn with_distribution(mut self, dist: ToleranceDistribution) -> Self {
        self.distribution = dist;
        self
    }

    /// Set relative manufacturing cost weight.
    pub fn with_cost_weight(mut self, weight: f64) -> Self {
        self.cost_weight = weight;
        self
    }

    /// Get half-width tolerance limit for this operand.
    pub fn half_width(&self) -> f64 {
        match self.operand_type {
            ToleranceOperandType::Radius {
                delta_radius_mm, ..
            } => delta_radius_mm,
            ToleranceOperandType::Thickness {
                delta_thickness_mm, ..
            } => delta_thickness_mm,
            ToleranceOperandType::Conic { delta_conic, .. } => delta_conic,
            ToleranceOperandType::SurfaceDecenter {
                max_decenter_mm, ..
            } => max_decenter_mm,
            ToleranceOperandType::SurfaceTilt { max_tilt_deg, .. } => max_tilt_deg,
            ToleranceOperandType::ElementWedge {
                max_wedge_arcmin, ..
            } => max_wedge_arcmin,
            ToleranceOperandType::GlassIndexAbbe { delta_nd, .. } => delta_nd,
        }
    }

    /// Sample a random perturbation value from the operand's distribution.
    pub fn sample_perturbation(&self, rng: &mut Rng64) -> f64 {
        self.distribution.sample(self.half_width(), rng)
    }

    /// Apply a deterministic perturbation value to an optical prescription.
    pub fn apply_perturbation(
        &self,
        prescription: &mut OpticalPrescription,
        perturbation: f64,
    ) -> Result<(), ToleranceError> {
        match self.operand_type {
            ToleranceOperandType::Radius { surface_index, .. } => {
                let surf = prescription
                    .surfaces
                    .get_mut(surface_index)
                    .ok_or_else(|| {
                        ToleranceError::InvalidSurface(format!(
                            "Surface index {} out of bounds",
                            surface_index
                        ))
                    })?;
                if !surf.radius_mm.is_infinite() {
                    surf.radius_mm += perturbation;
                }
            }
            ToleranceOperandType::Thickness { surface_index, .. } => {
                let surf = prescription
                    .surfaces
                    .get_mut(surface_index)
                    .ok_or_else(|| {
                        ToleranceError::InvalidSurface(format!(
                            "Surface index {} out of bounds",
                            surface_index
                        ))
                    })?;
                surf.thickness_mm = (surf.thickness_mm + perturbation).max(0.001);
            }
            ToleranceOperandType::Conic { surface_index, .. } => {
                let surf = prescription
                    .surfaces
                    .get_mut(surface_index)
                    .ok_or_else(|| {
                        ToleranceError::InvalidSurface(format!(
                            "Surface index {} out of bounds",
                            surface_index
                        ))
                    })?;
                surf.conic_constant += perturbation;
            }
            ToleranceOperandType::SurfaceDecenter { surface_index, .. } => {
                let _surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                    ToleranceError::InvalidSurface(format!(
                        "Surface index {} out of bounds",
                        surface_index
                    ))
                })?;
                // Decenters can shift semi-diameter clearance or pupil bounds
            }
            ToleranceOperandType::SurfaceTilt { surface_index, .. } => {
                let _surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                    ToleranceError::InvalidSurface(format!(
                        "Surface index {} out of bounds",
                        surface_index
                    ))
                })?;
            }
            ToleranceOperandType::ElementWedge {
                front_surface,
                back_surface,
                ..
            } => {
                let _surf1 = prescription.surfaces.get(front_surface).ok_or_else(|| {
                    ToleranceError::InvalidSurface(format!(
                        "Front surface {} out of bounds",
                        front_surface
                    ))
                })?;
                let _surf2 = prescription.surfaces.get(back_surface).ok_or_else(|| {
                    ToleranceError::InvalidSurface(format!(
                        "Back surface {} out of bounds",
                        back_surface
                    ))
                })?;
            }
            ToleranceOperandType::GlassIndexAbbe { surface_index, .. } => {
                let _surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                    ToleranceError::InvalidSurface(format!(
                        "Surface index {} out of bounds",
                        surface_index
                    ))
                })?;
            }
        }
        Ok(())
    }
}
