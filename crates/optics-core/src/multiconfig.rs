//! Multi-configuration optical systems, parameter pickups, and paraxial solves.
//!
//! # Multi-Configuration (Zoom & Multi-State)
//! A [`MultiConfigSystem`] holds a base [`OpticalPrescription`] alongside discrete configuration
//! states (e.g. zoom positions, conjugate distances, temperature environments). Each configuration
//! overrides specific surface thicknesses, curvatures, materials, or aperture dimensions.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::prescription::{OpticalPrescription, calculate_paraxial_properties};

/// Parameter override targeting a specific surface or system-level setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigOverride {
    /// Overrides surface curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    Curvature {
        /// 1-based or 0-based surface index.
        surface_index: usize,
        /// New surface curvature.
        curvature: f64,
    },
    /// Overrides surface thickness to next vertex in millimeters.
    Thickness {
        /// Surface index.
        surface_index: usize,
        /// New axial separation in mm.
        thickness_mm: f64,
    },
    /// Overrides surface conic constant $\kappa$.
    Conic {
        /// Surface index.
        surface_index: usize,
        /// New conic constant.
        conic_constant: f64,
    },
    /// Overrides surface glass material designation.
    Glass {
        /// Surface index.
        surface_index: usize,
        /// New material name (e.g. "SF11", "AIR").
        material_name: String,
    },
    /// Overrides system entrance pupil diameter in millimeters.
    EntrancePupilDiameter {
        /// New entrance pupil diameter.
        epd_mm: f64,
    },
}

/// A discrete configuration state (e.g. "Wide", "Tele", "Macro", "20C", "60C").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    /// Human-readable configuration identifier.
    pub name: String,
    /// List of parameter overrides applied when this configuration is active.
    pub overrides: Vec<ConfigOverride>,
}

/// Parametric relationship tying a target surface parameter to a source surface parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pickup {
    /// Curvature pickup: $c_{\text{target}} = s \cdot c_{\text{source}} + \delta$.
    Curvature {
        /// Target surface index receiving the solved curvature.
        target_surface: usize,
        /// Source surface index.
        source_surface: usize,
        /// Dimensionless scaling factor $s$ (default 1.0 or -1.0 for symmetric lenses).
        scale: f64,
        /// Curvature offset in $\text{mm}^{-1}$.
        offset: f64,
    },
    /// Thickness pickup: $t_{\text{target}} = s \cdot t_{\text{source}} + \delta$.
    Thickness {
        /// Target surface index receiving the solved thickness.
        target_surface: usize,
        /// Source surface index.
        source_surface: usize,
        /// Scaling factor $s$.
        scale: f64,
        /// Thickness offset in mm.
        offset_mm: f64,
    },
    /// Material pickup: copies material designation from source surface.
    Glass {
        /// Target surface index.
        target_surface: usize,
        /// Source surface index.
        source_surface: usize,
    },
}

impl Pickup {
    /// Applies this pickup to mutate an optical prescription in place.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if source or target surface indices are invalid.
    pub fn apply(&self, prescription: &mut OpticalPrescription) -> Result<(), OpticsError> {
        match self {
            Self::Curvature {
                target_surface,
                source_surface,
                scale,
                offset,
            } => {
                let src_curv = prescription
                    .surfaces
                    .iter()
                    .find(|s| s.surface_index == *source_surface)
                    .map(|s| {
                        if s.radius_mm.abs() < 1e-12 || s.radius_mm.is_infinite() {
                            0.0
                        } else {
                            1.0 / s.radius_mm
                        }
                    })
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup source surface {source_surface} not found"
                        ))
                    })?;

                let target_curv = scale * src_curv + offset;
                let target_surf = prescription
                    .surfaces
                    .iter_mut()
                    .find(|s| s.surface_index == *target_surface)
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup target surface {target_surface} not found"
                        ))
                    })?;

                target_surf.radius_mm = if target_curv.abs() < 1e-12 {
                    f64::INFINITY
                } else {
                    1.0 / target_curv
                };
            }
            Self::Thickness {
                target_surface,
                source_surface,
                scale,
                offset_mm,
            } => {
                let src_thick = prescription
                    .surfaces
                    .iter()
                    .find(|s| s.surface_index == *source_surface)
                    .map(|s| s.thickness_mm)
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup source surface {source_surface} not found"
                        ))
                    })?;

                let target_thick = scale * src_thick + offset_mm;
                let target_surf = prescription
                    .surfaces
                    .iter_mut()
                    .find(|s| s.surface_index == *target_surface)
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup target surface {target_surface} not found"
                        ))
                    })?;

                target_surf.thickness_mm = target_thick;
            }
            Self::Glass {
                target_surface,
                source_surface,
            } => {
                let src_glass = prescription
                    .surfaces
                    .iter()
                    .find(|s| s.surface_index == *source_surface)
                    .map(|s| s.material_name.clone())
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup source surface {source_surface} not found"
                        ))
                    })?;

                let target_surf = prescription
                    .surfaces
                    .iter_mut()
                    .find(|s| s.surface_index == *target_surface)
                    .ok_or_else(|| {
                        OpticsError::InvalidParameter(format!(
                            "Pickup target surface {target_surface} not found"
                        ))
                    })?;

                target_surf.material_name = src_glass;
            }
        }
        Ok(())
    }
}

/// Multi-configuration optical system containing a base prescription and multiple config states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiConfigSystem {
    /// Baseline optical prescription.
    pub base_prescription: OpticalPrescription,
    /// Available configurations (e.g. zoom positions).
    pub configurations: Vec<Configuration>,
    /// Global parametric pickups applied across all configurations.
    pub pickups: Vec<Pickup>,
    /// Active configuration index.
    pub active_index: usize,
}

impl MultiConfigSystem {
    /// Creates a new multi-configuration system with default single configuration.
    #[must_use]
    pub fn new(base_prescription: OpticalPrescription) -> Self {
        Self {
            base_prescription,
            configurations: vec![Configuration {
                name: "Config 1".to_string(),
                overrides: Vec::new(),
            }],
            pickups: Vec::new(),
            active_index: 0,
        }
    }

    /// Evaluates the complete [`OpticalPrescription`] for the currently active configuration,
    /// applying config overrides followed by parametric pickups.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if active configuration index is invalid.
    pub fn active_prescription(&self) -> Result<OpticalPrescription, OpticsError> {
        let config = self.configurations.get(self.active_index).ok_or_else(|| {
            OpticsError::InvalidParameter(format!(
                "Active configuration index {} out of bounds (count {})",
                self.active_index,
                self.configurations.len()
            ))
        })?;

        let mut presc = self.base_prescription.clone();
        presc.name = format!("{} [{}]", self.base_prescription.name, config.name);

        // Apply parameter overrides
        for override_item in &config.overrides {
            match override_item {
                ConfigOverride::Curvature {
                    surface_index,
                    curvature,
                } => {
                    if let Some(surf) = presc
                        .surfaces
                        .iter_mut()
                        .find(|s| s.surface_index == *surface_index)
                    {
                        surf.radius_mm = if curvature.abs() < 1e-12 {
                            f64::INFINITY
                        } else {
                            1.0 / curvature
                        };
                    }
                }
                ConfigOverride::Thickness {
                    surface_index,
                    thickness_mm,
                } => {
                    if let Some(surf) = presc
                        .surfaces
                        .iter_mut()
                        .find(|s| s.surface_index == *surface_index)
                    {
                        surf.thickness_mm = *thickness_mm;
                    }
                }
                ConfigOverride::Conic {
                    surface_index,
                    conic_constant,
                } => {
                    if let Some(surf) = presc
                        .surfaces
                        .iter_mut()
                        .find(|s| s.surface_index == *surface_index)
                    {
                        surf.conic_constant = *conic_constant;
                    }
                }
                ConfigOverride::Glass {
                    surface_index,
                    material_name,
                } => {
                    if let Some(surf) = presc
                        .surfaces
                        .iter_mut()
                        .find(|s| s.surface_index == *surface_index)
                    {
                        surf.material_name = material_name.clone();
                    }
                }
                ConfigOverride::EntrancePupilDiameter { epd_mm } => {
                    presc.entrance_pupil_diameter_mm = *epd_mm;
                }
            }
        }

        // Apply pickups
        for pickup in &self.pickups {
            pickup.apply(&mut presc)?;
        }

        Ok(presc)
    }
}

/// Solves the curvature of the final optical power surface to ensure system focal length matches `target_efl_mm`.
///
/// Uses paraxial sensitivity derivative $d(\text{EFL}) / dc$ and Newton-Raphson iteration.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if focal length solve fails to converge.
pub fn solve_focal_length<R>(
    prescription: &mut OpticalPrescription,
    resolver: &R,
    target_efl_mm: f64,
    surface_index: usize,
) -> Result<(), OpticsError>
where
    R: crate::prescription::MaterialResolver,
{
    if !target_efl_mm.is_finite() || target_efl_mm.abs() < 1e-6 {
        return Err(OpticsError::InvalidParameter(
            "Target focal length must be finite and non-zero".to_string(),
        ));
    }

    let max_iters = 15;
    let tol = 1e-5; // 10 nm in focal length

    for _ in 0..max_iters {
        let resolved = prescription.resolve(resolver)?;
        let props = calculate_paraxial_properties(&resolved, 587.56)?;
        let cur_efl = props.efl_mm;

        let err = cur_efl - target_efl_mm;
        if err.abs() < tol {
            return Ok(());
        }

        // Finite difference derivative
        let dc = 1e-5;
        let surf = prescription
            .surfaces
            .iter_mut()
            .find(|s| s.surface_index == surface_index)
            .ok_or_else(|| {
                OpticsError::InvalidParameter(format!("Surface {surface_index} not found"))
            })?;

        let cur_c = if surf.radius_mm.abs() < 1e-12 || surf.radius_mm.is_infinite() {
            0.0
        } else {
            1.0 / surf.radius_mm
        };

        surf.radius_mm = 1.0 / (cur_c + dc);
        let resolved_dc = prescription.resolve(resolver)?;
        let efl_dc = calculate_paraxial_properties(&resolved_dc, 587.56)?.efl_mm;
        let deriv = (efl_dc - cur_efl) / dc;

        if deriv.abs() < 1e-10 {
            return Err(OpticsError::NumericalSingularity(
                "Focal length derivative near zero".to_string(),
            ));
        }

        let delta_c = err / deriv;
        let new_c = cur_c - delta_c;

        let surf_update = prescription
            .surfaces
            .iter_mut()
            .find(|s| s.surface_index == surface_index)
            .ok_or_else(|| OpticsError::InvalidParameter("Surface not found".to_string()))?;

        surf_update.radius_mm = if new_c.abs() < 1e-12 {
            f64::INFINITY
        } else {
            1.0 / new_c
        };
    }

    Err(OpticsError::NumericalSingularity(
        "Focal length solve did not converge".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prescription::OpticalSurface;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_multiconfig_overrides_and_pickups() -> TestResult {
        let base = OpticalPrescription {
            name: "Zoom System".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 12.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: -50.0,
                    conic_constant: 0.0,
                    thickness_mm: 10.0, // Air gap
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                },
                OpticalSurface {
                    surface_index: 3,
                    radius_mm: 40.0,
                    conic_constant: 0.0,
                    thickness_mm: 4.0,
                    semi_diameter_mm: 10.0,
                    material_name: "SF11".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                },
            ],
        };

        let mut mc = MultiConfigSystem::new(base);
        // Add zoom config 2 with air gap expanded to 25 mm
        mc.configurations.push(Configuration {
            name: "Tele Zoom".to_string(),
            overrides: vec![ConfigOverride::Thickness {
                surface_index: 2,
                thickness_mm: 25.0,
            }],
        });

        // Config 0 has thickness 10.0
        let p0 = mc.active_prescription()?;
        assert_eq!(p0.surfaces[1].thickness_mm, 10.0);

        // Switch to Config 1
        mc.active_index = 1;
        let p1 = mc.active_prescription()?;
        assert_eq!(p1.surfaces[1].thickness_mm, 25.0);

        // Add symmetric curvature pickup: Surface 2 curvature = -1.0 * Surface 1 curvature
        mc.pickups.push(Pickup::Curvature {
            target_surface: 2,
            source_surface: 1,
            scale: -1.0,
            offset: 0.0,
        });

        let p1_pickup = mc.active_prescription()?;
        assert!((p1_pickup.surfaces[1].radius_mm - -50.0).abs() < 1e-12);
        Ok(())
    }
}
