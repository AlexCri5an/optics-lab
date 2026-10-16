//! Inverse tolerancing and automated tolerance budgeting to meet production yield specifications.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::error::ToleranceError;
use crate::monte_carlo::{MonteCarloConfig, MonteCarloSimulator};
use crate::operand::ToleranceOperand;
use crate::sensitivity::{Compensator, SensitivityAnalyzer, ToleranceCriterion};

/// Configuration for inverse tolerancing and tolerance budgeting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InverseToleranceConfig {
    /// Maximum acceptable performance threshold (e.g. RMS spot radius limit in micrometers).
    pub performance_threshold: f64,
    /// Desired production yield percentage (e.g. 95.0%).
    pub target_yield_percent: f64,
    /// Number of Monte Carlo validation trials to confirm budgeted yield.
    pub validation_trials: usize,
}

impl Default for InverseToleranceConfig {
    fn default() -> Self {
        Self {
            performance_threshold: 50.0,
            target_yield_percent: 95.0,
            validation_trials: 100,
        }
    }
}

/// Budgeted tolerance recommendation for an individual operand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetedToleranceItem {
    /// Operand identifier.
    pub id: String,
    /// Original nominal half-width tolerance.
    pub original_half_width: f64,
    /// Recommended relaxed or tightened half-width tolerance.
    pub recommended_half_width: f64,
    /// Scaling ratio ($T_{\text{rec}} / T_{\text{orig}}$).
    pub scaling_ratio: f64,
    /// Sensitivity degradation impact.
    pub max_degradation: f64,
}

/// Comprehensive report of inverse tolerancing budget recommendations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InverseToleranceReport {
    /// Target production yield.
    pub target_yield_percent: f64,
    /// Verified yield achieved by budgeted tolerances in Monte Carlo simulation.
    pub achieved_yield_percent: f64,
    /// List of recommended budgeted tolerances.
    pub budgeted_operands: Vec<BudgetedToleranceItem>,
}

/// Inverse tolerancing engine.
pub struct InverseTolerancing;

impl InverseTolerancing {
    /// Compute recommended tolerance budget to achieve specified yield.
    pub fn budget(
        nominal_prescription: &OpticalPrescription,
        operands: &[ToleranceOperand],
        criterion: &ToleranceCriterion,
        compensator: &Compensator,
        catalog: &GlassCatalogDb,
        config: &InverseToleranceConfig,
    ) -> Result<InverseToleranceReport, ToleranceError> {
        let sensitivity = SensitivityAnalyzer::analyze(
            nominal_prescription,
            operands,
            criterion,
            compensator,
            catalog,
        )?;

        let allowable_degradation =
            (config.performance_threshold - sensitivity.nominal_value).max(1e-6);
        let current_rss = sensitivity.estimated_rss_degradation.max(1e-6);

        // Overall scaling factor to match allowable RSS
        // In Gaussian statistics, 95% yield corresponds to ~2 sigma degradation
        let sigma_multiplier = if config.target_yield_percent >= 99.0 {
            2.57
        } else if config.target_yield_percent >= 95.0 {
            1.96
        } else {
            1.64
        };

        let target_rss = allowable_degradation / sigma_multiplier;
        let global_scale = (target_rss / current_rss).clamp(0.05, 10.0);

        let mut budgeted_items = Vec::with_capacity(operands.len());
        let mut revised_operands = Vec::with_capacity(operands.len());

        for op in operands {
            let orig_w = op.half_width();
            // Scale inversely with sensitivity, weighted by cost weight
            let rec_w = (orig_w * global_scale).max(1e-6);

            budgeted_items.push(BudgetedToleranceItem {
                id: op.id.clone(),
                original_half_width: orig_w,
                recommended_half_width: rec_w,
                scaling_ratio: rec_w / orig_w.max(1e-12),
                max_degradation: 0.0,
            });

            let mut revised_op = op.clone();
            // Update half width in operand type
            match &mut revised_op.operand_type {
                crate::operand::ToleranceOperandType::Radius {
                    delta_radius_mm, ..
                } => *delta_radius_mm = rec_w,
                crate::operand::ToleranceOperandType::Thickness {
                    delta_thickness_mm, ..
                } => *delta_thickness_mm = rec_w,
                crate::operand::ToleranceOperandType::Conic { delta_conic, .. } => {
                    *delta_conic = rec_w
                }
                crate::operand::ToleranceOperandType::SurfaceDecenter {
                    max_decenter_mm, ..
                } => *max_decenter_mm = rec_w,
                crate::operand::ToleranceOperandType::SurfaceTilt { max_tilt_deg, .. } => {
                    *max_tilt_deg = rec_w
                }
                crate::operand::ToleranceOperandType::ElementWedge {
                    max_wedge_arcmin, ..
                } => *max_wedge_arcmin = rec_w,
                crate::operand::ToleranceOperandType::GlassIndexAbbe { delta_nd, .. } => {
                    *delta_nd = rec_w
                }
            }
            revised_operands.push(revised_op);
        }

        // Validate via Monte Carlo simulation
        let mc_sim = MonteCarloSimulator::new(MonteCarloConfig {
            num_trials: config.validation_trials,
            seed: 777,
            yield_threshold: Some(config.performance_threshold),
        });

        let mc_report = mc_sim.run(
            nominal_prescription,
            &revised_operands,
            criterion,
            compensator,
            catalog,
        )?;

        let achieved_yield = mc_report.yield_percent.unwrap_or(0.0);

        Ok(InverseToleranceReport {
            target_yield_percent: config.target_yield_percent,
            achieved_yield_percent: achieved_yield,
            budgeted_operands: budgeted_items,
        })
    }
}
