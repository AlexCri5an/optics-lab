//! Statistical sampling distributions for optical manufacturing tolerances.

use serde::{Deserialize, Serialize};

/// Lightweight deterministic 64-bit XorShift pseudo-random number generator.
#[derive(Debug, Clone)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Initialize with a non-zero seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x5a17_98e1_b3c2_4d5e } else { seed },
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

    /// Standard normal sample $\mathcal{N}(0, 1)$ via Box-Muller polar transform.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-15);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// Statistical probability distribution assigned to a manufacturing tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ToleranceDistribution {
    /// Uniform distribution over $[-T, +T]$.
    Uniform,
    /// Truncated Gaussian distribution with $\pm k\sigma = \pm T$.
    NormalTruncated {
        /// Ratio of half-width tolerance $T$ to standard deviation $\sigma$ (default: 3.0 for $3\sigma$).
        sigma_cutoff: f64,
    },
    /// Worst-case edge distribution sampling exclusively from $\{-T, +T\}$ with equal probability.
    WorstCaseEdge,
    /// Parabolic distribution over $[-T, +T]$ with peak density at nominal $P(x) \propto 1 - (x/T)^2$.
    Parabolic,
    /// Bimodal distribution over $[-T, +T]$ with peak density at tolerance extremes $P(x) \propto (x/T)^2$.
    Bimodal,
}

impl Default for ToleranceDistribution {
    fn default() -> Self {
        Self::Uniform
    }
}

impl ToleranceDistribution {
    /// Sample a random perturbation within $[-T, +T]$ from the specified distribution.
    pub fn sample(&self, half_width: f64, rng: &mut Rng64) -> f64 {
        if half_width <= 0.0 {
            return 0.0;
        }

        match *self {
            ToleranceDistribution::Uniform => rng.uniform(-half_width, half_width),
            ToleranceDistribution::NormalTruncated { sigma_cutoff } => {
                let cutoff = sigma_cutoff.max(1.0);
                let sigma = half_width / cutoff;
                // Rejection sampling within [-half_width, half_width]
                for _ in 0..20 {
                    let z = rng.normal();
                    if z.abs() <= cutoff {
                        return z * sigma;
                    }
                }
                // Fallback clamp
                rng.uniform(-half_width, half_width)
            }
            ToleranceDistribution::WorstCaseEdge => {
                if rng.next_f64() < 0.5 {
                    -half_width
                } else {
                    half_width
                }
            }
            ToleranceDistribution::Parabolic => {
                for _ in 0..50 {
                    let x = rng.uniform(-half_width, half_width);
                    let u = x / half_width;
                    if rng.next_f64() <= (1.0 - u * u) {
                        return x;
                    }
                }
                rng.uniform(-half_width, half_width)
            }
            ToleranceDistribution::Bimodal => {
                for _ in 0..50 {
                    let x = rng.uniform(-half_width, half_width);
                    let u = x / half_width;
                    if rng.next_f64() <= (u * u) {
                        return x;
                    }
                }
                if rng.next_f64() < 0.5 { -half_width } else { half_width }
            }
        }
    }
}
