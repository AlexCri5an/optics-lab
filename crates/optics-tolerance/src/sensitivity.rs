//! Sensitivity analysis: perturbation to ± limits, optional compensator re-focusing, and RSS degradation estimation.

use serde::{Deserialize, Serialize};

use optics_core::analysis::PupilPattern;
use optics_core::analysis::spot_fan::trace_spot_diagram;
use optics_core::analysis::wavefront::compute_wavefront_map;
use optics_core::prescription::{OpticalPrescription, calculate_paraxial_properties};
use optics_glass::catalog::GlassCatalogDb;

use crate::error::ToleranceError;
use crate::operand::ToleranceOperand;

/// Evaluation criterion monitored during tolerancing simulations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToleranceCriterion {
    /// Geometric RMS spot radius in micrometers at designated field angle.
    RmsSpotRadius {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Wavefront RMS OPD error in waves at designated field angle.
    RmsWavefront {
        /// Field angle in degrees.
        field_angle_deg: f64,
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Paraxial Effective Focal Length (EFL) in millimeters.
    EffectiveFocalLength {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Paraxial Back Focal Length (BFL) in millimeters.
    BackFocalLength {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
}

impl Default for ToleranceCriterion {
    fn default() -> Self {
        Self::RmsSpotRadius {
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
        }
    }
}

impl ToleranceCriterion {
    /// Evaluate criterion value on an optical prescription.
    pub fn evaluate(
        &self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) -> Result<f64, ToleranceError> {
        let resolved = prescription
            .resolve(catalog)
            .map_err(|e| ToleranceError::Catalog(e.to_string()))?;

        match *self {
            ToleranceCriterion::RmsSpotRadius {
                field_angle_deg,
                wavelength_nm,
            } => {
                let spot = trace_spot_diagram(
                    &resolved,
                    (0.0, field_angle_deg),
                    wavelength_nm,
                    PupilPattern::Hexapolar { num_rings: 3 },
                )?;
                Ok(spot.metrics.rms_radius_um)
            }
            ToleranceCriterion::RmsWavefront {
                field_angle_deg,
                wavelength_nm,
            } => {
                let wf =
                    compute_wavefront_map(&resolved, 16, field_angle_deg, wavelength_nm, true)?;
                Ok(wf.rms_waves)
            }
            ToleranceCriterion::EffectiveFocalLength { wavelength_nm } => {
                let p = calculate_paraxial_properties(&resolved, wavelength_nm)?;
                Ok(p.efl_mm)
            }
            ToleranceCriterion::BackFocalLength { wavelength_nm } => {
                let p = calculate_paraxial_properties(&resolved, wavelength_nm)?;
                Ok(p.bfl_mm)
            }
        }
    }
}

/// Optical compensator mechanism adjusted during tolerancing to recover performance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Compensator {
    /// No compensator applied.
    None,
    /// Adjust back focal distance (thickness of surface preceding image plane) to paraxial best focus.
    ParaxialRefocus {
        /// Evaluation wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Adjust thickness of specified surface index to minimize criterion.
    VariableThickness {
        /// Surface index whose thickness is adjusted.
        surface_index: usize,
    },
}

impl Default for Compensator {
    fn default() -> Self {
        Self::None
    }
}

impl Compensator {
    /// Apply compensator adjustment to a perturbed prescription.
    pub fn compensate(
        &self,
        prescription: &mut OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) -> Result<(), ToleranceError> {
        match *self {
            Compensator::None => Ok(()),
            Compensator::ParaxialRefocus { wavelength_nm } => {
                let resolved = prescription
                    .resolve(catalog)
                    .map_err(|e| ToleranceError::Catalog(e.to_string()))?;
                let props = calculate_paraxial_properties(&resolved, wavelength_nm)?;
                // Adjust rear thickness to match paraxial back focal length to detector plane
                let n_surfs = prescription.surfaces.len();
                if n_surfs >= 2 {
                    prescription.surfaces[n_surfs - 2].thickness_mm = props.bfl_mm.max(0.001);
                }
                Ok(())
            }
            Compensator::VariableThickness { surface_index } => {
                let resolved = prescription
                    .resolve(catalog)
                    .map_err(|e| ToleranceError::Catalog(e.to_string()))?;
                let props = calculate_paraxial_properties(&resolved, 587.56)?;
                if let Some(surf) = prescription.surfaces.get_mut(surface_index) {
                    surf.thickness_mm = props.bfl_mm.max(0.001);
                }
                Ok(())
            }
        }
    }
}

/// Individual sensitivity record for a single tolerance operand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitivityRecord {
    /// Tolerance operand identifier.
    pub id: String,
    /// Nominal criterion value.
    pub nominal_value: f64,
    /// Criterion value under positive perturbation $+T_k$.
    pub value_plus: f64,
    /// Criterion value under negative perturbation $-T_k$.
    pub value_minus: f64,
    /// Change in criterion $\Delta \Phi_k^+ = \Phi_k^+ - \Phi_0$.
    pub delta_plus: f64,
    /// Change in criterion $\Delta \Phi_k^- = \Phi_k^- - \Phi_0$.
    pub delta_minus: f64,
    /// Maximum absolute degradation $\max(|\Delta \Phi^+|, |\Delta \Phi^-|)$.
    pub max_degradation: f64,
    /// Normalized sensitivity rate $\frac{\partial \Phi}{\partial T} \approx \frac{\Phi^+ - \Phi^-}{2 T}$.
    pub sensitivity_rate: f64,
}

/// Comprehensive report of sensitivity analysis across all tolerance operands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitivityReport {
    /// Nominal system performance value.
    pub nominal_value: f64,
    /// Root-Sum-Square (RSS) estimated statistical performance degradation.
    pub estimated_rss_degradation: f64,
    /// Estimated worst-case performance sum $\Phi_0 + \sum |\Delta \Phi_k|$.
    pub worst_case_degradation: f64,
    /// Ranked sensitivity records sorted from most sensitive to least sensitive.
    pub records: Vec<SensitivityRecord>,
}

/// Sensitivity analysis engine.
pub struct SensitivityAnalyzer;

impl SensitivityAnalyzer {
    /// Execute sensitivity analysis across all provided tolerance operands.
    pub fn analyze(
        nominal_prescription: &OpticalPrescription,
        operands: &[ToleranceOperand],
        criterion: &ToleranceCriterion,
        compensator: &Compensator,
        catalog: &GlassCatalogDb,
    ) -> Result<SensitivityReport, ToleranceError> {
        let mut nominal_presc = nominal_prescription.clone();
        compensator.compensate(&mut nominal_presc, catalog)?;
        let nominal_val = criterion.evaluate(&nominal_presc, catalog)?;
        let mut records = Vec::with_capacity(operands.len());

        let mut sum_rss_sq = 0.0_f64;
        let mut sum_worst_case = 0.0_f64;

        for op in operands {
            let half_w = op.half_width();
            if half_w <= 0.0 {
                continue;
            }

            // 1. Positive perturbation
            let mut presc_plus = nominal_prescription.clone();
            op.apply_perturbation(&mut presc_plus, half_w)?;
            compensator.compensate(&mut presc_plus, catalog)?;
            let val_plus = criterion.evaluate(&presc_plus, catalog)?;
            let delta_plus = val_plus - nominal_val;

            // 2. Negative perturbation
            let mut presc_minus = nominal_prescription.clone();
            op.apply_perturbation(&mut presc_minus, -half_w)?;
            compensator.compensate(&mut presc_minus, catalog)?;
            let val_minus = criterion.evaluate(&presc_minus, catalog)?;
            let delta_minus = val_minus - nominal_val;

            let max_deg = delta_plus.abs().max(delta_minus.abs());
            let rate = (val_plus - val_minus) / (2.0 * half_w);

            sum_rss_sq += max_deg * max_deg;
            sum_worst_case += max_deg;

            records.push(SensitivityRecord {
                id: op.id.clone(),
                nominal_value: nominal_val,
                value_plus: val_plus,
                value_minus: val_minus,
                delta_plus,
                delta_minus,
                max_degradation: max_deg,
                sensitivity_rate: rate,
            });
        }

        // Sort descending by max degradation (most sensitive first)
        records.sort_by(|a, b| {
            b.max_degradation
                .partial_cmp(&a.max_degradation)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(SensitivityReport {
            nominal_value: nominal_val,
            estimated_rss_degradation: sum_rss_sq.sqrt(),
            worst_case_degradation: nominal_val + sum_worst_case,
            records,
        })
    }
}
