//! Discrete glass substitution search ("Glass Hammer") optimizer.

use serde::{Deserialize, Serialize};
use std::time::Instant;

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::dls::{ControlFlow, DlsConfig, DlsOptimizer, OptimizationCallback, OptimizationResult};
use crate::error::OptimizeError;
use crate::merit::MeritFunction;
use crate::variables::VariableSet;

/// Configuration settings for the discrete glass substitution optimizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlassHammerConfig {
    /// Surface indices permitted to undergo glass material substitution.
    pub target_surfaces: Vec<usize>,
    /// Candidate glass names to consider (if empty, uses common optical glasses).
    pub candidate_glasses: Vec<String>,
    /// Maximum number of full substitution passes across all target surfaces.
    pub max_cycles: usize,
    /// Maximum allowable run time in seconds.
    pub time_budget_secs: f64,
    /// DLS configuration for fast geometry re-optimization following each glass change.
    pub local_dls_config: DlsConfig,
}

impl Default for GlassHammerConfig {
    fn default() -> Self {
        Self {
            target_surfaces: Vec::new(),
            candidate_glasses: vec![
                "N-BK7".to_string(),
                "N-SK16".to_string(),
                "N-BAK4".to_string(),
                "F2".to_string(),
                "N-SF11".to_string(),
                "N-SF5".to_string(),
                "N-LAK9".to_string(),
                "N-FK51A".to_string(),
                "N-KZFS4".to_string(),
                "FUSED_SILICA".to_string(),
            ],
            max_cycles: 3,
            time_budget_secs: 60.0,
            local_dls_config: DlsConfig {
                max_iterations: 15,
                ..Default::default()
            },
        }
    }
}

/// Glass substitution search optimizer that iterates through candidate catalog glasses
/// and locally re-tunes lens geometry with DLS.
pub struct GlassHammerOptimizer {
    config: GlassHammerConfig,
}

impl GlassHammerOptimizer {
    /// Create a new glass hammer optimizer.
    pub fn new(config: GlassHammerConfig) -> Self {
        Self { config }
    }

    /// Execute discrete glass substitution search.
    pub fn optimize(
        &self,
        base_prescription: &OpticalPrescription,
        variables: &VariableSet,
        merit_function: &MeritFunction,
        catalog: &GlassCatalogDb,
        mut callback: Option<&mut dyn OptimizationCallback>,
    ) -> Result<OptimizationResult, OptimizeError> {
        let start_time = Instant::now();
        let local_optimizer = DlsOptimizer::new(self.config.local_dls_config.clone());

        // Baseline polish
        let mut best_result = local_optimizer.optimize(
            base_prescription,
            variables,
            merit_function,
            catalog,
            None,
        )?;

        let mut current_presc = best_result.optimized_prescription.clone();
        let candidate_list = if self.config.candidate_glasses.is_empty() {
            vec!["N-BK7".to_string(), "F2".to_string(), "N-SF11".to_string()]
        } else {
            self.config.candidate_glasses.clone()
        };

        let mut total_swaps = 0;

        for cycle in 0..self.config.max_cycles {
            let mut improved_in_cycle = false;

            for &surf_idx in &self.config.target_surfaces {
                if start_time.elapsed().as_secs_f64() >= self.config.time_budget_secs {
                    break;
                }

                if surf_idx >= current_presc.surfaces.len() {
                    continue;
                }

                let current_glass = current_presc.surfaces[surf_idx].material_name.clone();

                for candidate in &candidate_list {
                    if start_time.elapsed().as_secs_f64() >= self.config.time_budget_secs {
                        break;
                    }

                    if candidate == &current_glass {
                        continue;
                    }

                    total_swaps += 1;

                    // Test candidate glass
                    let mut trial_presc = current_presc.clone();
                    trial_presc.surfaces[surf_idx].material_name = candidate.clone();

                    // Re-optimize geometry
                    if let Ok(result) = local_optimizer.optimize(
                        &trial_presc,
                        variables,
                        merit_function,
                        catalog,
                        None,
                    ) {
                        if result.final_merit < best_result.final_merit {
                            best_result = result;
                            current_presc = best_result.optimized_prescription.clone();
                            improved_in_cycle = true;
                        }
                    }

                    // Progress reporting
                    if let Some(cb) = callback.as_deref_mut() {
                        if cb.on_iteration(total_swaps, best_result.final_merit, &[])
                            == ControlFlow::Stop
                        {
                            return Ok(best_result);
                        }
                    }
                }
            }

            if !improved_in_cycle {
                break;
            }

            if let Some(cb) = callback.as_deref_mut() {
                if cb.on_iteration(cycle, best_result.final_merit, &[]) == ControlFlow::Stop {
                    break;
                }
            }
        }

        Ok(best_result)
    }
}
