//! Optimization merit function operand definitions and evaluation.

use serde::{Deserialize, Serialize};

use optics_core::aiming::{RayAimingMode, trace_aimed_ray};
use optics_core::analysis::{
    PupilPattern, compute_diffraction_mtf, compute_field_curves, compute_seidel_coefficients,
    compute_wavefront_map, trace_spot_diagram,
};
use optics_core::conic::conic_sag;
use optics_core::prescription::{ResolvedPrescription, calculate_paraxial_properties};
use optics_core::types::{Field, Pupil, Wavelength};

use crate::error::OptimizeError;

/// Type of optical operand evaluated during merit calculation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OperandType {
    // --- First-Order Paraxial Operands ---
    /// System Effective Focal Length (EFL) in millimeters.
    EffectiveFocalLength {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// System working F-number ($F/\#$).
    WorkingFNumber {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Total track length (sum of all surface vertex separations) in millimeters.
    TotalTrack,
    /// Back Focal Length (BFL) from rear vertex to paraxial image plane in millimeters.
    BackFocalLength {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Paraxial transverse magnification.
    ParaxialMagnification {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },

    // --- Aberration Operands ---
    /// Geometric RMS spot radius in micrometers.
    RmsSpotRadius {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
        /// Number of radial rings for hexapolar pupil sampling (default: 3).
        hex_rings: usize,
    },
    /// Wavefront RMS error in waves.
    RmsWavefrontError {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
        /// Pupil grid dimension (e.g. 16 or 32).
        grid_size: usize,
    },
    /// Seidel 3rd-order spherical aberration $S_I$.
    SeidelSpherical {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Seidel 3rd-order coma $S_{II}$.
    SeidelComa {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Seidel 3rd-order astigmatism $S_{III}$.
    SeidelAstigmatism {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Seidel Petzval field curvature sum.
    SeidelPetzval {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Seidel 3rd-order distortion $S_V$.
    SeidelDistortion {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Primary axial chromatic aberration $C_L$.
    SeidelAxialColor {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Primary lateral chromatic aberration $C_T$.
    SeidelLateralColor {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Diffraction MTF at a specified spatial frequency in cycles/mm.
    MtfAtFrequency {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Spatial frequency in lp/mm.
        frequency_lpm: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
        /// Whether tangential (true) or sagittal (false) MTF is targeted.
        tangential: bool,
    },
    /// Optical distortion percentage at a specified field angle.
    DistortionPercentage {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },

    // --- Ray Coordinate Operands ---
    /// Transverse $y$ ray intercept at a specified surface.
    RayCoordinateY {
        /// Target surface index.
        surface_index: usize,
        /// Normalized entrance pupil $P_x$.
        px: f64,
        /// Normalized entrance pupil $P_y$.
        py: f64,
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },

    // --- Physical Constraints ---
    /// Minimum center thickness between surface $i$ and $i+1$.
    MinCenterThickness {
        /// Surface index.
        surface_index: usize,
        /// Minimum allowable thickness in millimeters.
        min_thickness_mm: f64,
    },
    /// Maximum center thickness between surface $i$ and $i+1$.
    MaxCenterThickness {
        /// Surface index.
        surface_index: usize,
        /// Maximum allowable thickness in millimeters.
        max_thickness_mm: f64,
    },
    /// Minimum edge thickness for element between surface $i$ and $i+1$.
    MinEdgeThickness {
        /// Front surface index of the optical element.
        surface_index: usize,
        /// Minimum allowable physical edge thickness in millimeters.
        min_edge_mm: f64,
    },
}

/// An individual operand within the merit function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeritOperand {
    /// Human-readable identifier or tag.
    pub id: String,
    /// Physical operand type and configuration.
    pub operand_type: OperandType,
    /// Desired target numerical value.
    pub target: f64,
    /// Statistical weight ($w_i \ge 0$).
    pub weight: f64,
}

impl MeritOperand {
    /// Create a new merit operand.
    pub fn new(id: impl Into<String>, operand_type: OperandType, target: f64, weight: f64) -> Self {
        Self {
            id: id.into(),
            operand_type,
            target,
            weight,
        }
    }

    /// Evaluate the current optical value $v_i$ for this operand against a resolved prescription.
    pub fn evaluate(&self, resolved: &ResolvedPrescription) -> Result<f64, OptimizeError> {
        match self.operand_type {
            OperandType::EffectiveFocalLength { wavelength_nm } => {
                match calculate_paraxial_properties(resolved, wavelength_nm) {
                    Ok(paraxial) if paraxial.efl_mm.is_finite() && paraxial.efl_mm.abs() > 0.01 => {
                        Ok(paraxial.efl_mm)
                    }
                    _ => Ok(10_000.0), // Finite penalty for concentric / zero-power / singular paraxial trace
                }
            }
            OperandType::WorkingFNumber { wavelength_nm } => {
                match calculate_paraxial_properties(resolved, wavelength_nm) {
                    Ok(paraxial) if paraxial.f_number.is_finite() => Ok(paraxial.f_number),
                    _ => Ok(100.0),
                }
            }
            OperandType::TotalTrack => {
                let mut totr = 0.0;
                for surf in &resolved.surfaces {
                    totr += surf.thickness_mm;
                }
                Ok(totr)
            }
            OperandType::BackFocalLength { wavelength_nm } => {
                match calculate_paraxial_properties(resolved, wavelength_nm) {
                    Ok(paraxial) if paraxial.bfl_mm.is_finite() => Ok(paraxial.bfl_mm),
                    _ => Ok(10_000.0),
                }
            }
            OperandType::ParaxialMagnification { wavelength_nm } => {
                match calculate_paraxial_properties(resolved, wavelength_nm) {
                    Ok(paraxial) if paraxial.bfl_mm.abs() > 1e-12 && paraxial.efl_mm.abs() > 1e-6 => {
                        Ok(paraxial.bfl_mm / paraxial.efl_mm)
                    }
                    _ => Ok(0.0),
                }
            }
            OperandType::RmsSpotRadius {
                field_angle_deg,
                wavelength_nm,
                hex_rings,
            } => {
                let pattern = PupilPattern::Hexapolar {
                    num_rings: hex_rings.max(1),
                };
                match trace_spot_diagram(resolved, (0.0, field_angle_deg), wavelength_nm, pattern) {
                    Ok(spot) if spot.metrics.passed_rays_count > 0 && spot.metrics.rms_radius_um.is_finite() => {
                        Ok(spot.metrics.rms_radius_um)
                    }
                    _ => Ok(100_000.0), // Standard Zemax-style 100 mm penalty for failed ray trace / TIR
                }
            }
            OperandType::RmsWavefrontError {
                field_angle_deg,
                wavelength_nm,
                grid_size,
            } => {
                match compute_wavefront_map(
                    resolved,
                    grid_size.max(8),
                    field_angle_deg,
                    wavelength_nm,
                    true,
                ) {
                    Ok(wf) if wf.rms_waves.is_finite() => Ok(wf.rms_waves),
                    _ => Ok(100.0), // Finite penalty in waves for wavefront failure
                }
            }
            OperandType::SeidelSpherical {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.s1_spherical)
            }
            OperandType::SeidelComa {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.s2_coma)
            }
            OperandType::SeidelAstigmatism {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.s3_astigmatism)
            }
            OperandType::SeidelPetzval {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.petzval_sum)
            }
            OperandType::SeidelDistortion {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.s5_distortion)
            }
            OperandType::SeidelAxialColor {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.c1_axial_color)
            }
            OperandType::SeidelLateralColor {
                field_angle_deg,
                wavelength_nm,
            } => {
                let s = compute_seidel_coefficients(resolved, field_angle_deg, wavelength_nm)?;
                Ok(s.c2_lateral_color)
            }
            OperandType::MtfAtFrequency {
                field_angle_deg,
                frequency_lpm,
                wavelength_nm,
                tangential,
            } => {
                let mtf = compute_diffraction_mtf(resolved, 16, field_angle_deg, wavelength_nm)?;
                // Find closest point or interpolate
                let mut best_val = 0.0;
                let mut min_diff = f64::INFINITY;
                for pt in &mtf.points {
                    let diff = (pt.spatial_frequency_lpm - frequency_lpm).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_val = if tangential {
                            pt.tangential_mtf
                        } else {
                            pt.sagittal_mtf
                        };
                    }
                }
                Ok(best_val)
            }
            OperandType::DistortionPercentage {
                field_angle_deg,
                wavelength_nm,
            } => {
                let max_field = field_angle_deg.abs().max(1.0);
                let fc = compute_field_curves(resolved, 5, max_field, wavelength_nm)?;
                let mut best_dist = 0.0;
                let mut min_diff = f64::INFINITY;
                for pt in &fc.points {
                    let diff = (pt.field_angle_deg - field_angle_deg).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_dist = pt.distortion_percent;
                    }
                }
                Ok(best_dist)
            }
            OperandType::RayCoordinateY {
                surface_index,
                px,
                py,
                field_angle_deg,
                wavelength_nm,
            } => {
                let aimed_ray = trace_aimed_ray(
                    resolved,
                    Field::AngleDegrees {
                        x: 0.0,
                        y: field_angle_deg,
                    },
                    Pupil::new(px, py)?,
                    Wavelength::from_nanometers(wavelength_nm)?,
                    RayAimingMode::Paraxial,
                )?;
                let path = optics_core::trace_sequential(resolved, &aimed_ray)?;
                let hit = path.hits.get(surface_index).ok_or_else(|| {
                    OptimizeError::OperandEvaluation(format!(
                        "Surface {} not reached in ray path",
                        surface_index
                    ))
                })?;
                Ok(hit.point.y)
            }
            OperandType::MinCenterThickness {
                surface_index,
                min_thickness_mm,
            } => {
                let surf = resolved.surfaces.get(surface_index).ok_or_else(|| {
                    OptimizeError::OperandEvaluation(format!("Invalid surface {}", surface_index))
                })?;
                // If thickness is below minimum, value violates constraint
                let t = surf.thickness_mm;
                if t < min_thickness_mm {
                    // Penalty increases quadratically
                    Ok(t - min_thickness_mm)
                } else {
                    Ok(0.0)
                }
            }
            OperandType::MaxCenterThickness {
                surface_index,
                max_thickness_mm,
            } => {
                let surf = resolved.surfaces.get(surface_index).ok_or_else(|| {
                    OptimizeError::OperandEvaluation(format!("Invalid surface {}", surface_index))
                })?;
                let t = surf.thickness_mm;
                if t > max_thickness_mm {
                    Ok(t - max_thickness_mm)
                } else {
                    Ok(0.0)
                }
            }
            OperandType::MinEdgeThickness {
                surface_index,
                min_edge_mm,
            } => {
                let surf1 = resolved.surfaces.get(surface_index).ok_or_else(|| {
                    OptimizeError::OperandEvaluation(format!("Invalid surface {}", surface_index))
                })?;
                let surf2 = resolved.surfaces.get(surface_index + 1).ok_or_else(|| {
                    OptimizeError::OperandEvaluation(format!(
                        "Invalid back surface {}",
                        surface_index + 1
                    ))
                })?;
                let y_max = surf1.conic.semi_diam_mm.min(surf2.conic.semi_diam_mm);
                let z1 = conic_sag(y_max, surf1.conic.curvature, surf1.conic.conic_constant)?;
                let z2 = conic_sag(y_max, surf2.conic.curvature, surf2.conic.conic_constant)?;
                let edge_thickness = surf1.thickness_mm + z2 - z1;
                if edge_thickness < min_edge_mm {
                    Ok(edge_thickness - min_edge_mm)
                } else {
                    Ok(0.0)
                }
            }
        }
    }
}
