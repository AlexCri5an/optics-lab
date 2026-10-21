//! Monte Carlo tolerancing simulation engine and statistical aggregation.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;

use crate::distribution::Rng64;
use crate::error::ToleranceError;
use crate::operand::ToleranceOperand;
use crate::sensitivity::{Compensator, ToleranceCriterion};

/// Configuration settings for Monte Carlo tolerancing simulations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonteCarloConfig {
    /// Total number of manufacturing trials to simulate.
    pub num_trials: usize,
    /// Random seed for deterministic simulation.
    pub seed: u64,
    /// Optional acceptable performance threshold limit to evaluate yield.
    pub yield_threshold: Option<f64>,
}

impl Default for MonteCarloConfig {
    fn default() -> Self {
        Self {
            num_trials: 100,
            seed: 42,
            yield_threshold: None,
        }
    }
}

/// Statistical percentile summary of Monte Carlo simulation results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PercentileSummary {
    /// 10th percentile performance.
    pub p10: f64,
    /// 50th percentile (median) performance.
    pub p50: f64,
    /// 90th percentile performance.
    pub p90: f64,
    /// 99th percentile performance.
    pub p99: f64,
}

/// Comprehensive summary report of Monte Carlo tolerancing simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonteCarloReport {
    /// Nominal system performance value.
    pub nominal_value: f64,
    /// Total trials executed.
    pub total_trials: usize,
    /// Mean performance across all trials.
    pub mean: f64,
    /// Standard deviation of performance across all trials.
    pub std_dev: f64,
    /// Minimum performance observed.
    pub min_value: f64,
    /// Maximum performance observed (worst degradation).
    pub max_value: f64,
    /// Percentile distribution summary.
    pub percentiles: PercentileSummary,
    /// Estimated manufacturing yield percentage against specification threshold.
    pub yield_percent: Option<f64>,
    /// Worst-case perturbed optical prescription.
    pub worst_case_prescription: Option<OpticalPrescription>,
    /// Evaluated performance values for all simulated trials.
    pub trial_values: Vec<f64>,
}

impl MonteCarloReport {
    /// Compute histogram bin counts given a requested number of bins.
    ///
    /// Returns a list of tuples `(bin_min, bin_max, count)`.
    pub fn compute_histogram(&self, num_bins: usize) -> Vec<(f64, f64, usize)> {
        let n = num_bins.max(1);
        if self.trial_values.is_empty() {
            return Vec::new();
        }

        let min_v = self.min_value;
        let max_v = self.max_value;
        let span = max_v - min_v;

        if span <= 1e-12 {
            return vec![(min_v, max_v, self.trial_values.len())];
        }

        let bin_w = span / (n as f64);
        let mut bins = Vec::with_capacity(n);
        for i in 0..n {
            let b_start = min_v + (i as f64) * bin_w;
            let b_end = if i == n - 1 { max_v } else { b_start + bin_w };
            bins.push((b_start, b_end, 0usize));
        }

        for &val in &self.trial_values {
            let mut idx = ((val - min_v) / bin_w).floor() as usize;
            if idx >= n {
                idx = n - 1;
            }
            if let Some(entry) = bins.get_mut(idx) {
                entry.2 += 1;
            }
        }

        bins
    }
}

/// Monte Carlo tolerancing simulation engine.
pub struct MonteCarloSimulator {
    config: MonteCarloConfig,
}

impl MonteCarloSimulator {
    /// Create a new Monte Carlo simulator with specified configuration.
    pub fn new(config: MonteCarloConfig) -> Self {
        Self { config }
    }

    /// Run Monte Carlo tolerancing simulation across all tolerance operands.
    pub fn run(
        &self,
        nominal_prescription: &OpticalPrescription,
        operands: &[ToleranceOperand],
        criterion: &ToleranceCriterion,
        compensator: &Compensator,
        catalog: &GlassCatalogDb,
    ) -> Result<MonteCarloReport, ToleranceError> {
        if self.config.num_trials == 0 {
            return Err(ToleranceError::EmptySpecification(
                "Number of trials must be greater than zero".to_string(),
            ));
        }

        let mut nominal_presc = nominal_prescription.clone();
        compensator.compensate(&mut nominal_presc, catalog)?;
        let nominal_val = criterion.evaluate(&nominal_presc, catalog)?;
        let mut rng = Rng64::new(self.config.seed);

        let mut trial_values = Vec::with_capacity(self.config.num_trials);
        let mut worst_val = f64::NEG_INFINITY;
        let mut worst_presc = None;

        for _trial in 0..self.config.num_trials {
            let mut perturbed_presc = nominal_prescription.clone();

            // Sample and apply all operand perturbations
            for op in operands {
                let delta = op.sample_perturbation(&mut rng);
                op.apply_perturbation(&mut perturbed_presc, delta)?;
            }

            // Apply compensator (e.g. refocus)
            compensator.compensate(&mut perturbed_presc, catalog)?;

            // Evaluate criterion
            let val = criterion.evaluate(&perturbed_presc, catalog)?;
            trial_values.push(val);

            if val > worst_val {
                worst_val = val;
                worst_presc = Some(perturbed_presc);
            }
        }

        // Statistical calculation
        let n = trial_values.len() as f64;
        let sum: f64 = trial_values.iter().sum();
        let mean = sum / n;

        let var: f64 = if trial_values.len() > 1 {
            trial_values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let std_dev = var.sqrt();

        // Sort for percentiles
        let mut sorted = trial_values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p10 = sorted[((0.10 * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1)];
        let p50 = sorted[((0.50 * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1)];
        let p90 = sorted[((0.90 * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1)];
        let p99 = sorted[((0.99 * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1)];

        let min_val = sorted.first().copied().unwrap_or(nominal_val);
        let max_val = sorted.last().copied().unwrap_or(nominal_val);

        // Calculate yield if threshold provided
        let yield_percent = self.config.yield_threshold.map(|thresh| {
            let passed = trial_values.iter().filter(|&&v| v <= thresh).count();
            (passed as f64 / n) * 100.0
        });

        Ok(MonteCarloReport {
            nominal_value: nominal_val,
            total_trials: self.config.num_trials,
            mean,
            std_dev,
            min_value: min_val,
            max_value: max_val,
            percentiles: PercentileSummary { p10, p50, p90, p99 },
            yield_percent,
            worst_case_prescription: worst_presc,
            trial_values,
        })
    }
}
