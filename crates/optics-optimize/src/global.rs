//! Global optimization: multi-start random restarts, simulated annealing, and basin hopping.

use serde::{Deserialize, Serialize};
use std::time::Instant;

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::dls::{ControlFlow, DlsConfig, DlsOptimizer, OptimizationCallback, OptimizationResult};
use crate::error::OptimizeError;
use crate::merit::MeritFunction;
use crate::variables::VariableSet;

/// Lightweight deterministic 64-bit XorShift pseudo-random number generator.
#[derive(Debug, Clone)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Initialize with a non-zero seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    /// Generate next pseudo-random u64.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform float in $[0.0, 1.0)$.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform float in $[a, b]$.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next_f64()
    }
}

/// Configuration settings for multi-start global search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiStartConfig {
    /// Number of random restart trials.
    pub num_trials: usize,
    /// Maximum execution time in seconds.
    pub time_budget_secs: f64,
    /// DLS configuration for local basin minimization.
    pub local_dls_config: DlsConfig,
    /// Random seed for repeatability.
    pub seed: u64,
}

impl Default for MultiStartConfig {
    fn default() -> Self {
        Self {
            num_trials: 10,
            time_budget_secs: 30.0,
            local_dls_config: DlsConfig {
                max_iterations: 20,
                ..Default::default()
            },
            seed: 42,
        }
    }
}

/// Multi-start global optimizer using random restarts followed by local DLS refinement.
pub struct MultiStartOptimizer {
    config: MultiStartConfig,
}

impl MultiStartOptimizer {
    /// Create a new multi-start optimizer.
    pub fn new(config: MultiStartConfig) -> Self {
        Self { config }
    }

    /// Execute multi-start global search over active variable bounds.
    pub fn optimize(
        &self,
        base_prescription: &OpticalPrescription,
        variables: &VariableSet,
        merit_function: &MeritFunction,
        catalog: &GlassCatalogDb,
        mut callback: Option<&mut dyn OptimizationCallback>,
    ) -> Result<OptimizationResult, OptimizeError> {
        let start_time = Instant::now();
        let mut rng = Rng64::new(self.config.seed);
        let local_optimizer = DlsOptimizer::new(self.config.local_dls_config.clone());

        // First evaluate baseline system
        let mut best_result = local_optimizer.optimize(base_prescription, variables, merit_function, catalog, None)?;

        let num_vars = variables.num_active();
        if num_vars == 0 {
            return Ok(best_result);
        }

        for trial in 0..self.config.num_trials {
            // Check time budget
            if start_time.elapsed().as_secs_f64() >= self.config.time_budget_secs {
                break;
            }

            // Generate random starting parameter vector
            let mut random_params = Vec::with_capacity(num_vars);
            for var in &variables.variables {
                if !var.enabled {
                    continue;
                }
                let val = rng.uniform(var.min_value, var.max_value);
                random_params.push(val);
            }

            let mut trial_presc = base_prescription.clone();
            if variables.apply_values(&mut trial_presc, &random_params).is_err() {
                continue;
            }

            // Perform local minimization
            if let Ok(result) = local_optimizer.optimize(&trial_presc, variables, merit_function, catalog, None) {
                if result.final_merit < best_result.final_merit {
                    best_result = result;
                }
            }

            // Report progress
            if let Some(cb) = callback.as_deref_mut() {
                if cb.on_iteration(trial, best_result.final_merit, &[]) == ControlFlow::Stop {
                    break;
                }
            }
        }

        Ok(best_result)
    }
}

/// Configuration settings for simulated annealing global search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulatedAnnealingConfig {
    /// Initial temperature $T_0$.
    pub initial_temperature: f64,
    /// Geometric cooling factor $\alpha \in (0, 1)$ ($T \leftarrow \alpha T$).
    pub cooling_rate: f64,
    /// Number of steps per temperature level.
    pub steps_per_temperature: usize,
    /// Maximum temperature cooling iterations.
    pub max_cooling_cycles: usize,
    /// Random seed for repeatability.
    pub seed: u64,
}

impl Default for SimulatedAnnealingConfig {
    fn default() -> Self {
        Self {
            initial_temperature: 100.0,
            cooling_rate: 0.90,
            steps_per_temperature: 15,
            max_cooling_cycles: 25,
            seed: 12345,
        }
    }
}

/// Simulated annealing global search optimizer.
pub struct SimulatedAnnealingOptimizer {
    config: SimulatedAnnealingConfig,
}

impl SimulatedAnnealingOptimizer {
    /// Create a new simulated annealing optimizer.
    pub fn new(config: SimulatedAnnealingConfig) -> Self {
        Self { config }
    }

    /// Execute simulated annealing optimization on an optical prescription.
    pub fn optimize(
        &self,
        base_prescription: &OpticalPrescription,
        variables: &VariableSet,
        merit_function: &MeritFunction,
        catalog: &GlassCatalogDb,
        mut callback: Option<&mut dyn OptimizationCallback>,
    ) -> Result<OptimizationResult, OptimizeError> {
        let mut rng = Rng64::new(self.config.seed);
        let mut current_presc = base_prescription.clone();
        let mut current_params = variables.extract_values(&current_presc)?;

        let current_eval = merit_function.evaluate(&current_presc, catalog)?;
        let initial_merit = current_eval.total_merit;
        let mut current_merit = initial_merit;

        let mut best_presc = current_presc.clone();
        let mut best_merit = current_merit;

        let mut temp = self.config.initial_temperature;
        let mut total_iterations = 0;

        for cycle in 0..self.config.max_cooling_cycles {
            for _step in 0..self.config.steps_per_temperature {
                total_iterations += 1;

                // Random bounded perturbation
                let mut trial_params = current_params.clone();
                let mut p_idx = 0;
                for var in &variables.variables {
                    if !var.enabled {
                        continue;
                    }
                    let range = var.max_value - var.min_value;
                    let perturb = (rng.next_f64() - 0.5) * 0.1 * range;
                    trial_params[p_idx] = (trial_params[p_idx] + perturb).clamp(var.min_value, var.max_value);
                    p_idx += 1;
                }

                let mut trial_presc = current_presc.clone();
                if variables.apply_values(&mut trial_presc, &trial_params).is_err() {
                    continue;
                }

                if let Ok(trial_eval) = merit_function.evaluate(&trial_presc, catalog) {
                    let delta_merit = trial_eval.total_merit - current_merit;

                    // Metropolis acceptance criterion
                    let accept = if delta_merit <= 0.0 {
                        true
                    } else {
                        let prob = (-delta_merit / temp.max(1e-12)).exp();
                        rng.next_f64() < prob
                    };

                    if accept {
                        current_merit = trial_eval.total_merit;
                        current_params = trial_params;
                        current_presc = trial_presc;

                        if current_merit < best_merit {
                            best_merit = current_merit;
                            best_presc = current_presc.clone();
                        }
                    }
                }
            }

            temp *= self.config.cooling_rate;

            if let Some(cb) = callback.as_deref_mut() {
                if cb.on_iteration(cycle, best_merit, &[]) == ControlFlow::Stop {
                    break;
                }
            }
        }

        // Final local DLS polishing from best state
        let local_dls = DlsOptimizer::new(DlsConfig {
            max_iterations: 25,
            ..Default::default()
        });
        local_optimizer_polish(
            &local_dls,
            &best_presc,
            variables,
            merit_function,
            catalog,
            initial_merit,
            total_iterations,
        )
    }
}

fn local_optimizer_polish(
    dls: &DlsOptimizer,
    presc: &OpticalPrescription,
    vars: &VariableSet,
    mf: &MeritFunction,
    catalog: &GlassCatalogDb,
    initial_merit: f64,
    prior_iters: usize,
) -> Result<OptimizationResult, OptimizeError> {
    let polished = dls.optimize(presc, vars, mf, catalog, None)?;
    Ok(OptimizationResult {
        initial_merit,
        final_merit: polished.final_merit,
        iterations: prior_iters + polished.iterations,
        converged: polished.converged,
        optimized_prescription: polished.optimized_prescription,
        final_evaluation: polished.final_evaluation,
    })
}
