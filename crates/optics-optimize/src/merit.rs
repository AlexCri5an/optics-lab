//! Merit function evaluation, residual vectors, and default merit generators.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::error::OptimizeError;
use crate::operands::{MeritOperand, OperandType};

/// Result of evaluating a merit function against an optical prescription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeritEvaluation {
    /// Overall scalar merit value $\Phi^2 = \sum w_i (v_i - t_i)^2$.
    pub total_merit: f64,
    /// Vector of weighted residuals $r_i = \sqrt{w_i} (v_i - t_i)$.
    pub residuals: Vec<f64>,
    /// Actual evaluated physical values $v_i$ for each operand.
    pub values: Vec<f64>,
    /// Target values $t_i$ for each operand.
    pub targets: Vec<f64>,
    /// Statistical weights $w_i$ for each operand.
    pub weights: Vec<f64>,
    /// Identifier tags for each operand.
    pub operand_ids: Vec<String>,
}

/// Optical merit function definition containing an operand table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MeritFunction {
    /// List of operands comprising the merit function.
    pub operands: Vec<MeritOperand>,
}

impl MeritFunction {
    /// Create a new empty merit function.
    pub fn new() -> Self {
        Self {
            operands: Vec::new(),
        }
    }

    /// Add an operand to the merit function.
    pub fn add_operand(&mut self, operand: MeritOperand) {
        self.operands.push(operand);
    }

    /// Number of operands in the merit function.
    pub fn len(&self) -> usize {
        self.operands.len()
    }

    /// Whether the merit function contains zero operands.
    pub fn is_empty(&self) -> bool {
        self.operands.is_empty()
    }

    /// Evaluate the complete merit function on a prescription using the provided glass catalog.
    pub fn evaluate(
        &self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) -> Result<MeritEvaluation, OptimizeError> {
        let resolved = prescription
            .resolve(catalog)
            .map_err(|e| OptimizeError::Catalog(e.to_string()))?;

        let mut total_merit = 0.0_f64;
        let mut residuals = Vec::with_capacity(self.operands.len());
        let mut values = Vec::with_capacity(self.operands.len());
        let mut targets = Vec::with_capacity(self.operands.len());
        let mut weights = Vec::with_capacity(self.operands.len());
        let mut operand_ids = Vec::with_capacity(self.operands.len());

        for op in &self.operands {
            let val = op.evaluate(&resolved)?;
            let diff = val - op.target;
            let res = op.weight.sqrt() * diff;
            let term = op.weight * diff * diff;

            total_merit += term;
            residuals.push(res);
            values.push(val);
            targets.push(op.target);
            weights.push(op.weight);
            operand_ids.push(op.id.clone());
        }

        Ok(MeritEvaluation {
            total_merit,
            residuals,
            values,
            targets,
            weights,
            operand_ids,
        })
    }

    /// Generate a default RMS Spot Radius merit function targeting zero spot size and optional EFL constraint.
    pub fn default_rms_spot(prescription: &OpticalPrescription, target_efl: Option<f64>) -> Self {
        let mut mf = Self::new();
        let primary_wl = prescription
            .wavelengths_nm
            .get(prescription.primary_wavelength_index)
            .copied()
            .unwrap_or(587.56);

        // Constrain EFL if specified
        if let Some(efl) = target_efl {
            mf.add_operand(MeritOperand::new(
                "TARGET_EFL",
                OperandType::EffectiveFocalLength {
                    wavelength_nm: primary_wl,
                },
                efl,
                100.0,
            ));
        }

        // Add RMS Spot radius operands across all defined field angles
        for &field in &prescription.field_angles_deg {
            let weight = if field.abs() < 1e-6 { 2.0 } else { 1.0 };
            for &wl in &prescription.wavelengths_nm {
                mf.add_operand(MeritOperand::new(
                    format!("SPOT_FLD{:.1}_WL{:.0}", field, wl),
                    OperandType::RmsSpotRadius {
                        field_angle_deg: field,
                        wavelength_nm: wl,
                        hex_rings: 3,
                    },
                    0.0,
                    weight,
                ));
            }
        }

        mf
    }

    /// Generate a default RMS Wavefront Error merit function targeting zero OPD error and optional EFL constraint.
    pub fn default_rms_wavefront(
        prescription: &OpticalPrescription,
        target_efl: Option<f64>,
    ) -> Self {
        let mut mf = Self::new();
        let primary_wl = prescription
            .wavelengths_nm
            .get(prescription.primary_wavelength_index)
            .copied()
            .unwrap_or(587.56);

        if let Some(efl) = target_efl {
            mf.add_operand(MeritOperand::new(
                "TARGET_EFL",
                OperandType::EffectiveFocalLength {
                    wavelength_nm: primary_wl,
                },
                efl,
                100.0,
            ));
        }

        for &field in &prescription.field_angles_deg {
            let weight = if field.abs() < 1e-6 { 2.0 } else { 1.0 };
            mf.add_operand(MeritOperand::new(
                format!("OPD_FLD{:.1}", field),
                OperandType::RmsWavefrontError {
                    field_angle_deg: field,
                    wavelength_nm: primary_wl,
                    grid_size: 16,
                },
                0.0,
                weight,
            ));
        }

        mf
    }
}
