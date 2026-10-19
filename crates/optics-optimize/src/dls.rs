//! Damped Least Squares (Levenberg-Marquardt) optical optimization algorithm.

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::error::OptimizeError;
use crate::merit::{MeritEvaluation, MeritFunction};
use crate::variables::VariableSet;

/// Flow control signal returned by progress callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFlow {
    /// Proceed to the next optimization iteration.
    Continue,
    /// Terminate optimization gracefully and retain current best system.
    Stop,
}

/// Trait for observing optimization progress or requesting early cancellation.
pub trait OptimizationCallback: Send + Sync {
    /// Invoked at the completion of each optimization iteration.
    fn on_iteration(
        &mut self,
        iteration: usize,
        merit_value: f64,
        parameter_vector: &[f64],
    ) -> ControlFlow;
}

/// Configuration settings for the Damped Least Squares optimizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlsConfig {
    /// Maximum number of parameter update iterations.
    pub max_iterations: usize,
    /// Initial Levenberg-Marquardt damping factor $\lambda_0$.
    pub initial_damping: f64,
    /// Relative merit improvement threshold for convergence termination.
    pub relative_merit_tolerance: f64,
    /// Gradient norm threshold for stationary point termination.
    pub gradient_tolerance: f64,
    /// Minimum relative finite-difference perturbation step.
    pub finite_difference_relative_step: f64,
}

impl Default for DlsConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            initial_damping: 1.0,
            relative_merit_tolerance: 1e-6,
            gradient_tolerance: 1e-7,
            finite_difference_relative_step: 1e-5,
        }
    }
}

/// Summary report returned upon completion of an optimization run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// Initial merit function value prior to optimization.
    pub initial_merit: f64,
    /// Final merit function value achieved.
    pub final_merit: f64,
    /// Total number of iterations executed.
    pub iterations: usize,
    /// Whether optimization satisfied convergence tolerances.
    pub converged: bool,
    /// Final optimized optical prescription.
    pub optimized_prescription: OpticalPrescription,
    /// Final detailed evaluation breakdown.
    pub final_evaluation: MeritEvaluation,
}

/// Damped Least Squares (Levenberg-Marquardt) optimizer for optical prescriptions.
pub struct DlsOptimizer {
    config: DlsConfig,
}

impl DlsOptimizer {
    /// Create a new DLS optimizer with specified configuration.
    pub fn new(config: DlsConfig) -> Self {
        Self { config }
    }

    /// Run damped least-squares optimization on a prescription with variables and merit function.
    pub fn optimize(
        &self,
        initial_prescription: &OpticalPrescription,
        variables: &VariableSet,
        merit_function: &MeritFunction,
        catalog: &GlassCatalogDb,
        mut callback: Option<&mut dyn OptimizationCallback>,
    ) -> Result<OptimizationResult, OptimizeError> {
        let p_dim = variables.num_active();
        if p_dim == 0 {
            return Err(OptimizeError::InvalidVariable(
                "No active variables enabled for optimization".to_string(),
            ));
        }
        let m_dim = merit_function.len();
        if m_dim == 0 {
            return Err(OptimizeError::OperandEvaluation(
                "Merit function has zero operands".to_string(),
            ));
        }

        let mut current_prescription = initial_prescription.clone();
        let mut params = variables.extract_values(&current_prescription)?;

        let mut current_eval = merit_function.evaluate(&current_prescription, catalog)?;
        let initial_merit = current_eval.total_merit;
        let mut current_merit = initial_merit;

        let mut damping = self.config.initial_damping;
        let mut converged = false;
        let mut iteration = 0;

        while iteration < self.config.max_iterations {
            iteration += 1;

            // 1. Compute Jacobian matrix J (M x P) via central differences
            let mut j_mat = DMatrix::<f64>::zeros(m_dim, p_dim);
            let mut param_idx = 0;

            for var in &variables.variables {
                if !var.enabled {
                    continue;
                }
                let x0 = params[param_idx];
                let delta = (x0.abs() * self.config.finite_difference_relative_step)
                    .max(var.step_size)
                    .max(1e-7);

                // Forward perturbation
                let mut p_plus = params.clone();
                p_plus[param_idx] = (x0 + delta).clamp(var.min_value, var.max_value);
                let actual_delta_plus = p_plus[param_idx] - x0;

                let mut presc_plus = current_prescription.clone();
                variables.apply_values(&mut presc_plus, &p_plus)?;
                let eval_plus = merit_function.evaluate(&presc_plus, catalog)?;

                // Backward perturbation
                let mut p_minus = params.clone();
                p_minus[param_idx] = (x0 - delta).clamp(var.min_value, var.max_value);
                let actual_delta_minus = x0 - p_minus[param_idx];

                let mut presc_minus = current_prescription.clone();
                variables.apply_values(&mut presc_minus, &p_minus)?;
                let eval_minus = merit_function.evaluate(&presc_minus, catalog)?;

                let denom = actual_delta_plus + actual_delta_minus;
                for i in 0..m_dim {
                    let d_res = if denom.abs() > 1e-14 {
                        let diff = eval_plus.residuals[i] - eval_minus.residuals[i];
                        if diff.is_finite() {
                            (diff / denom).clamp(-1e8, 1e8)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    j_mat[(i, param_idx)] = d_res;
                }

                param_idx += 1;
            }

            // 2. Normal equations: H = J^T J, g = J^T r
            let r_vec = DVector::<f64>::from_row_slice(&current_eval.residuals);
            let h_mat = j_mat.transpose() * &j_mat;
            let g_vec = j_mat.transpose() * &r_vec;

            // Gradient norm termination check
            let grad_norm = g_vec.norm();
            if !grad_norm.is_finite() || grad_norm < self.config.gradient_tolerance {
                converged = true;
                break;
            }

            // 3. Solve Levenberg-Marquardt damped system: (H + lambda * diag(H)) dx = -g
            let mut step_accepted = false;

            for _attempt in 0..10 {
                let mut a_mat = h_mat.clone();
                for i in 0..p_dim {
                    let diag_val = a_mat[(i, i)].abs().max(1e-6);
                    a_mat[(i, i)] += damping * diag_val;
                }

                let lu = a_mat.lu();
                if let Some(dx) = lu.solve(&(-&g_vec)) {
                    if dx.iter().all(|&v| v.is_finite()) {
                        let mut candidate_params = params.clone();
                        for i in 0..p_dim {
                            candidate_params[i] += dx[i];
                        }
                        variables.clamp_values(&mut candidate_params);

                        let mut test_presc = current_prescription.clone();
                        if variables
                            .apply_values(&mut test_presc, &candidate_params)
                            .is_ok()
                        {
                            if let Ok(test_eval) = merit_function.evaluate(&test_presc, catalog) {
                                if test_eval.total_merit.is_finite() && test_eval.total_merit < current_merit {
                                    // Successful reduction!
                                    let improvement = current_merit - test_eval.total_merit;
                                    let rel_improvement = improvement / current_merit.max(1e-12);

                                    current_merit = test_eval.total_merit;
                                    current_eval = test_eval;
                                    current_prescription = test_presc;
                                    params = candidate_params;

                                    // Reduce damping
                                    damping = (damping / 3.0).max(1e-7);
                                    step_accepted = true;

                                    if rel_improvement < self.config.relative_merit_tolerance {
                                        converged = true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }

                // If step rejected or singular solve, increase damping
                damping = (damping * 5.0).min(1e8);
            }

            // Invoke progress callback
            if let Some(cb) = callback.as_deref_mut() {
                if cb.on_iteration(iteration, current_merit, &params) == ControlFlow::Stop {
                    break;
                }
            }

            if converged || !step_accepted {
                break;
            }
        }

        Ok(OptimizationResult {
            initial_merit,
            final_merit: current_merit,
            iterations: iteration,
            converged,
            optimized_prescription: current_prescription,
            final_evaluation: current_eval,
        })
    }
}
