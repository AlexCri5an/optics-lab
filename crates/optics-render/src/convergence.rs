//! Online convergence tracking, sample variance (Welford algorithm), and RMSE estimation.

/// Tracks statistical variance and convergence rate across progressive render iterations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvergenceTracker {
    /// Number of accumulated samples.
    pub sample_count: u64,
    /// Running mean estimation ($M_k$).
    pub welford_mean: f64,
    /// Running sum of squared differences ($S_k$).
    pub welford_m2: f64,
}

impl Default for ConvergenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConvergenceTracker {
    /// Creates a new, empty convergence tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sample_count: 0,
            welford_mean: 0.0,
            welford_m2: 0.0,
        }
    }

    /// Resets sample count and statistical moments to zero.
    pub fn reset(&mut self) {
        self.sample_count = 0;
        self.welford_mean = 0.0;
        self.welford_m2 = 0.0;
    }

    /// Incorporates a new scalar observation (e.g. mean image luminance or per-pixel error)
    /// using Welford's online one-pass algorithm.
    ///
    /// Formulations:
    /// $$M_k = M_{k-1} + \frac{x_k - M_{k-1}}{k}$$
    /// $$S_k = S_{k-1} + (x_k - M_{k-1})(x_k - M_k)$$
    pub fn update(&mut self, val: f64) {
        self.sample_count += 1;
        let delta = val - self.welford_mean;
        self.welford_mean += delta / self.sample_count as f64;
        let delta2 = val - self.welford_mean;
        self.welford_m2 += delta * delta2;
    }

    /// Returns the sample variance $\sigma_k^2 = \frac{S_k}{k - 1}$ ($k \ge 2$).
    #[must_use]
    pub fn variance(&self) -> f64 {
        if self.sample_count < 2 {
            0.0
        } else {
            self.welford_m2 / (self.sample_count - 1) as f64
        }
    }

    /// Returns the sample standard deviation $\sigma_k = \sqrt{\sigma_k^2}$.
    #[must_use]
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Returns the Standard Error of the Mean $\text{SEM}_k = \frac{\sigma_k}{\sqrt{k}}$.
    #[must_use]
    pub fn sem(&self) -> f64 {
        if self.sample_count == 0 {
            0.0
        } else {
            self.std_dev() / (self.sample_count as f64).sqrt()
        }
    }
}

/// Evaluates Root-Mean-Square Error (RMSE) between a rendered image buffer and a reference buffer.
///
/// $$\text{RMSE} = \sqrt{\frac{1}{N} \sum_{i=0}^{N-1} (A_i - B_i)^2}$$
#[must_use]
pub fn compute_rmse(current: &[f32], reference: &[f32]) -> f64 {
    if current.is_empty() || current.len() != reference.len() {
        return 0.0;
    }

    let mut sum_sq = 0.0_f64;
    for (c, r) in current.iter().zip(reference.iter()) {
        let diff = (*c as f64) - (*r as f64);
        sum_sq += diff * diff;
    }

    (sum_sq / current.len() as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_welford_variance_known_series() -> TestResult {
        let mut tracker = ConvergenceTracker::new();
        // Known series: [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
        // Mean = 5.0, Sample Variance = 4.571428571428571
        let samples = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        for s in samples {
            tracker.update(s);
        }

        assert_eq!(tracker.sample_count, 8);
        assert!((tracker.welford_mean - 5.0).abs() < 1e-10);
        assert!((tracker.variance() - 32.0 / 7.0).abs() < 1e-10);
        assert!(tracker.sem() > 0.0);

        Ok(())
    }

    #[test]
    fn test_compute_rmse() -> TestResult {
        let a = [1.0_f32, 2.0, 3.0, 4.0];
        let b = [1.0_f32, 2.0, 3.0, 6.0]; // diff = [0, 0, 0, 2] -> diff^2 = 4 -> mean = 1 -> rmse = 1.0
        let rmse = compute_rmse(&a, &b);
        assert!((rmse - 1.0).abs() < 1e-6);

        Ok(())
    }
}
