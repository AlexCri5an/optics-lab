//! Optical optimization parameter variables and state mappings.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;

use crate::error::OptimizeError;

/// Type of optical parameter that can be varied during optimization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VariableType {
    /// Surface curvature $c = 1/R$ in $\text{mm}^{-1}$ (planar surface: $c = 0$).
    Curvature {
        /// Target surface index in the prescription.
        surface_index: usize,
    },
    /// Surface axial thickness to the subsequent surface in millimeters.
    Thickness {
        /// Target surface index in the prescription.
        surface_index: usize,
    },
    /// Conic constant $\kappa$ ($0$ = sphere, $-1$ = parabola).
    Conic {
        /// Target surface index in the prescription.
        surface_index: usize,
    },
    /// Surface clear aperture semi-diameter in millimeters.
    SemiDiameter {
        /// Target surface index in the prescription.
        surface_index: usize,
    },
}

/// Definition of an optimization variable, including bounds and finite-difference step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDef {
    /// Identifier index within the active variable set.
    pub id: usize,
    /// Parameter type and target surface.
    pub var_type: VariableType,
    /// Lower physical bound for the parameter value.
    pub min_value: f64,
    /// Upper physical bound for the parameter value.
    pub max_value: f64,
    /// Perturbation step for finite-difference Jacobian approximation.
    pub step_size: f64,
    /// Whether this variable is active in the current optimization.
    pub enabled: bool,
}

impl VariableDef {
    /// Create a new variable definition with sensible default bounds and step size.
    pub fn new(id: usize, var_type: VariableType) -> Self {
        let (min_val, max_val, step) = match var_type {
            VariableType::Curvature { .. } => (-0.5, 0.5, 1e-6),
            VariableType::Thickness { .. } => (0.01, 500.0, 1e-4),
            VariableType::Conic { .. } => (-10.0, 10.0, 1e-4),
            VariableType::SemiDiameter { .. } => (0.1, 500.0, 1e-3),
        };
        Self {
            id,
            var_type,
            min_value: min_val,
            max_value: max_val,
            step_size: step,
            enabled: true,
        }
    }

    /// Set explicit physical bounds on the variable.
    pub fn with_bounds(mut self, min_value: f64, max_value: f64) -> Self {
        self.min_value = min_value;
        self.max_value = max_value;
        self
    }
}

/// Collection of variables managed during an optimization run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableSet {
    /// List of variable definitions.
    pub variables: Vec<VariableDef>,
}

impl VariableSet {
    /// Create a new empty variable set.
    pub fn new() -> Self {
        Self { variables: Vec::new() }
    }

    /// Add a variable to the set.
    pub fn add(&mut self, var_type: VariableType) -> usize {
        let id = self.variables.len();
        self.variables.push(VariableDef::new(id, var_type));
        id
    }

    /// Add a variable with explicit bounds to the set.
    pub fn add_with_bounds(&mut self, var_type: VariableType, min_value: f64, max_value: f64) -> usize {
        let id = self.variables.len();
        self.variables
            .push(VariableDef::new(id, var_type).with_bounds(min_value, max_value));
        id
    }

    /// Returns the number of enabled active variables.
    pub fn num_active(&self) -> usize {
        self.variables.iter().filter(|v| v.enabled).count()
    }

    /// Extract current numerical parameter values $\mathbf{x} \in \mathbb{R}^P$ from the prescription.
    pub fn extract_values(&self, prescription: &OpticalPrescription) -> Result<Vec<f64>, OptimizeError> {
        let mut values = Vec::new();
        for var in &self.variables {
            if !var.enabled {
                continue;
            }
            let val = match var.var_type {
                VariableType::Curvature { surface_index } => {
                    let surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!(
                            "Surface index {} out of range (system has {} surfaces)",
                            surface_index,
                            prescription.surfaces.len()
                        ))
                    })?;
                    if surf.radius_mm.is_infinite() || surf.radius_mm.abs() < 1e-15 {
                        0.0
                    } else {
                        1.0 / surf.radius_mm
                    }
                }
                VariableType::Thickness { surface_index } => {
                    let surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.thickness_mm
                }
                VariableType::Conic { surface_index } => {
                    let surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.conic_constant
                }
                VariableType::SemiDiameter { surface_index } => {
                    let surf = prescription.surfaces.get(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.semi_diameter_mm
                }
            };
            values.push(val);
        }
        Ok(values)
    }

    /// Apply updated numerical parameter vector $\mathbf{x}$ to the prescription.
    pub fn apply_values(&self, prescription: &mut OpticalPrescription, values: &[f64]) -> Result<(), OptimizeError> {
        let mut val_idx = 0;
        for var in &self.variables {
            if !var.enabled {
                continue;
            }
            if val_idx >= values.len() {
                return Err(OptimizeError::InvalidVariable(
                    "Parameter vector length mismatch".to_string(),
                ));
            }
            let raw_val = values[val_idx];
            let clamped_val = raw_val.clamp(var.min_value, var.max_value);
            val_idx += 1;

            match var.var_type {
                VariableType::Curvature { surface_index } => {
                    let surf = prescription.surfaces.get_mut(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    if clamped_val.abs() < 1e-12 {
                        surf.radius_mm = f64::INFINITY;
                    } else {
                        surf.radius_mm = 1.0 / clamped_val;
                    }
                }
                VariableType::Thickness { surface_index } => {
                    let surf = prescription.surfaces.get_mut(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.thickness_mm = clamped_val;
                }
                VariableType::Conic { surface_index } => {
                    let surf = prescription.surfaces.get_mut(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.conic_constant = clamped_val;
                }
                VariableType::SemiDiameter { surface_index } => {
                    let surf = prescription.surfaces.get_mut(surface_index).ok_or_else(|| {
                        OptimizeError::InvalidVariable(format!("Surface index {} out of range", surface_index))
                    })?;
                    surf.semi_diameter_mm = clamped_val;
                }
            }
        }
        Ok(())
    }

    /// Clamp values in-place according to active variable bounds.
    pub fn clamp_values(&self, values: &mut [f64]) {
        let mut val_idx = 0;
        for var in &self.variables {
            if !var.enabled {
                continue;
            }
            if val_idx < values.len() {
                values[val_idx] = values[val_idx].clamp(var.min_value, var.max_value);
                val_idx += 1;
            }
        }
    }
}
