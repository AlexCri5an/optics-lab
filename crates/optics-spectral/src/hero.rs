//! Hero-wavelength spectral sampling and Monte Carlo color reconstruction.
//!
//! Implements uniform stratified spectral sampling as reverse-engineered from
//! pbrt-v4, Mitsuba 3, and three-gpu-pathtracer.
//!
//! A single primary "hero" wavelength guides spatial ray paths (e.g. through refractive
//! prisms and lenses), while $N$ correlated companion wavelengths are traced simultaneously
//! to eliminate chromatic noise in GPU path tracing.

use crate::color::{
    SPECTRUM_MAX_NM, SPECTRUM_MIN_NM, cie_1931_cmf, d65_normalization_factor, d65_spd,
    xyz_to_linear_srgb, xyz_to_srgb,
};

/// A bundle of $N$ stratified hero wavelengths and their Monte Carlo integration weights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroSpectrum<const N: usize> {
    /// Sampled wavelengths in nanometers (nm).
    pub wavelengths_nm: [f64; N],
    /// Spectral weights including probability density normalization: $\Delta \lambda / N$.
    pub weights: [f64; N],
}

/// Canonical 4-channel hero wavelength bundle matching 4-component GPU texture buffers and SIMD lanes.
pub type HeroBundle4 = HeroSpectrum<4>;

impl<const N: usize> HeroSpectrum<N> {
    /// Creates a new hero spectrum bundle from explicit wavelengths and weights.
    #[must_use]
    pub const fn new(wavelengths_nm: [f64; N], weights: [f64; N]) -> Self {
        Self {
            wavelengths_nm,
            weights,
        }
    }

    /// Returns the primary hero wavelength guiding the geometric ray path.
    #[must_use]
    pub const fn hero_wavelength(&self) -> f64 {
        self.wavelengths_nm[0]
    }

    /// Evaluates the CIE 1931 XYZ tristimulus values for the bundle given spectral radiances.
    ///
    /// Integrates using standard Monte Carlo summation:
    /// $$X = \sum_{k=0}^{N-1} L_k \cdot \bar{x}(\lambda_k) \cdot w_k$$
    #[must_use]
    pub fn to_xyz(&self, radiances: &[f64; N]) -> [f64; 3] {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;

        for (k, &rad) in radiances.iter().enumerate().take(N) {
            let cmf = cie_1931_cmf(self.wavelengths_nm[k]);
            let weight = self.weights[k] * rad;
            x += weight * cmf[0];
            y += weight * cmf[1];
            z += weight * cmf[2];
        }

        [x, y, z]
    }

    /// Converts spectral radiances through this hero bundle directly to linear sRGB.
    #[must_use]
    pub fn to_linear_srgb(&self, radiances: &[f64; N]) -> [f64; 3] {
        let xyz = self.to_xyz(radiances);
        xyz_to_linear_srgb(xyz)
    }

    /// Converts spectral radiances through this hero bundle to gamma-compressed sRGB.
    #[must_use]
    pub fn to_srgb(&self, radiances: &[f64; N]) -> [f32; 3] {
        let xyz = self.to_xyz(radiances);
        xyz_to_srgb(xyz)
    }

    /// Returns the relative spectral power multipliers for CIE Standard Illuminant D65.
    #[must_use]
    pub fn d65_weights(&self) -> [f64; N] {
        let norm = d65_normalization_factor();
        let mut weights = [0.0; N];
        for (k, w) in weights.iter_mut().enumerate().take(N) {
            *w = (d65_spd(self.wavelengths_nm[k]) / 100.0) / norm;
        }
        weights
    }
}

/// Samples an $N$-component stratified hero wavelength bundle in $[\lambda_{\min}, \lambda_{\max}]$.
///
/// Given a primary hero wavelength $\lambda_0 \in [\lambda_{\min}, \lambda_{\max}]$,
/// generates $N$ uniformly shifted wavelengths:
/// $$\lambda_k = \lambda_{\min} + \left( (\lambda_0 - \lambda_{\min}) + k \cdot \frac{\Delta \lambda}{N} \right) \pmod{\Delta \lambda}$$
///
/// # Panics
/// Panics in debug mode if $N == 0$ or if $\lambda_{\max} \le \lambda_{\min}$.
#[must_use]
pub fn sample_hero_bundle<const N: usize>(
    hero_nm: f64,
    lambda_min: f64,
    lambda_max: f64,
) -> HeroSpectrum<N> {
    assert!(
        N > 0,
        "HeroSpectrum bundle must contain at least 1 wavelength"
    );
    assert!(
        lambda_max > lambda_min,
        "Spectral range maximum must exceed minimum"
    );

    let delta_lambda = lambda_max - lambda_min;
    let step = delta_lambda / N as f64;
    let u_hero = (hero_nm - lambda_min).rem_euclid(delta_lambda);

    let mut wavelengths = [0.0; N];
    let mut weights = [0.0; N];
    let weight_val = step;

    for k in 0..N {
        let offset = (u_hero + k as f64 * step).rem_euclid(delta_lambda);
        wavelengths[k] = lambda_min + offset;
        weights[k] = weight_val;
    }

    HeroSpectrum::new(wavelengths, weights)
}

/// Dynamically sized hero spectrum for arbitrary channel counts.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicHeroSpectrum {
    /// Sampled wavelengths in nanometers (nm).
    pub wavelengths_nm: Vec<f64>,
    /// Integration weights.
    pub weights: Vec<f64>,
}

impl DynamicHeroSpectrum {
    /// Evaluates the CIE 1931 XYZ tristimulus values.
    #[must_use]
    pub fn to_xyz(&self, radiances: &[f64]) -> [f64; 3] {
        let n = self.wavelengths_nm.len();
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;

        for k in 0..n {
            let rad = radiances.get(k).copied().unwrap_or(0.0);
            let cmf = cie_1931_cmf(self.wavelengths_nm[k]);
            let weight = self.weights[k] * rad;
            x += weight * cmf[0];
            y += weight * cmf[1];
            z += weight * cmf[2];
        }

        [x, y, z]
    }
}

/// Samples a dynamically sized hero wavelength spectrum across the visible spectrum ($380–780\,\text{nm}$).
///
/// Implements `Phase 3.3` of the master plan:
/// - Stratified uniform sampling across visible band with primary hero wavelength.
///
/// # Panics
/// Panics if `n_wavelengths == 0`.
#[must_use]
pub fn hero_sample_wavelengths(primary_nm: f64, n_wavelengths: usize) -> DynamicHeroSpectrum {
    assert!(n_wavelengths > 0, "n_wavelengths must be strictly positive");

    let lambda_min = SPECTRUM_MIN_NM;
    let lambda_max = SPECTRUM_MAX_NM;
    let delta_lambda = lambda_max - lambda_min;
    let step = delta_lambda / n_wavelengths as f64;
    let u_hero = (primary_nm - lambda_min).rem_euclid(delta_lambda);

    let mut wavelengths_nm = Vec::with_capacity(n_wavelengths);
    let mut weights = Vec::with_capacity(n_wavelengths);

    for k in 0..n_wavelengths {
        let offset = (u_hero + k as f64 * step).rem_euclid(delta_lambda);
        wavelengths_nm.push(lambda_min + offset);
        weights.push(step);
    }

    DynamicHeroSpectrum {
        wavelengths_nm,
        weights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_hero_sample_stratification_and_wrap_around() -> TestResult {
        let hero = 420.0;
        let bundle: HeroBundle4 = sample_hero_bundle(hero, 380.0, 780.0);

        // Expected spacing: (780 - 380) / 4 = 100 nm
        assert_eq!(bundle.wavelengths_nm[0], 420.0);
        assert_eq!(bundle.wavelengths_nm[1], 520.0);
        assert_eq!(bundle.wavelengths_nm[2], 620.0);
        assert_eq!(bundle.wavelengths_nm[3], 720.0);

        // Wrap around test: hero near upper boundary (720 nm)
        let hero_high = 720.0;
        let bundle_high: HeroBundle4 = sample_hero_bundle(hero_high, 380.0, 780.0);
        assert_eq!(bundle_high.wavelengths_nm[0], 720.0);
        assert_eq!(bundle_high.wavelengths_nm[1], 420.0); // 720 + 100 = 820 -> 380 + 40 = 420
        assert_eq!(bundle_high.wavelengths_nm[2], 520.0);
        assert_eq!(bundle_high.wavelengths_nm[3], 620.0);

        Ok(())
    }

    #[test]
    fn test_hero_bundle_constant_spectrum_reconstruction() -> TestResult {
        // Average over stratified hero offsets across visible spectrum for flat white (L = 1.0)
        let steps = 64;
        let mut sum_xyz = [0.0; 3];

        for i in 0..steps {
            let hero_nm = 380.0 + (i as f64 / steps as f64) * 100.0;
            let bundle: HeroBundle4 = sample_hero_bundle(hero_nm, 380.0, 780.0);
            let radiances = [1.0; 4];
            let xyz = bundle.to_xyz(&radiances);
            sum_xyz[0] += xyz[0];
            sum_xyz[1] += xyz[1];
            sum_xyz[2] += xyz[2];
        }

        let mean_xyz = [
            sum_xyz[0] / steps as f64,
            sum_xyz[1] / steps as f64,
            sum_xyz[2] / steps as f64,
        ];

        // Theoretical Riemann integral of CMF over 380..780 nm with step 1 nm
        // Integral of y_bar for equal-energy spectrum is ~106.8
        assert!(
            (mean_xyz[1] - 106.8).abs() < 1.0,
            "Mean Y integral error: got {}, expected ~106.8",
            mean_xyz[1]
        );

        Ok(())
    }

    #[test]
    fn test_dynamic_hero_spectrum() -> TestResult {
        let dynamic = hero_sample_wavelengths(550.0, 8);
        assert_eq!(dynamic.wavelengths_nm.len(), 8);
        assert_eq!(dynamic.weights.len(), 8);

        // All wavelengths within [380, 780]
        for &wl in &dynamic.wavelengths_nm {
            assert!(
                (380.0..=780.0).contains(&wl),
                "Wavelength {wl} outside visible spectrum"
            );
        }

        let rads = vec![1.0; 8];
        let xyz = dynamic.to_xyz(&rads);
        assert!(xyz[1] > 50.0, "Integrated luminance must be positive");

        Ok(())
    }
}
