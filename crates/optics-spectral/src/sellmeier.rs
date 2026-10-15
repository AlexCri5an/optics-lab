//! Sellmeier and Cauchy empirical dispersion equations for transparent media.
//!
//! # Sellmeier Formula 1 (Standard Schott Formulation)
//!
//! $$n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{B_i \lambda^2}{\lambda^2 - C_i}$$
//!
//! where wavelength $\lambda$ is in **micrometers** ($\mu\text{m}$), $B_i$ are dimensionless oscillator strengths,
//! and $C_i$ are resonance wavelength squares ($\mu\text{m}^2$).
//!
//! # Sellmeier Formula 2
//!
//! $$n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{A_i \lambda^2}{\lambda^2 - B_i^2}$$
//!
//! where $B_i$ are the resonance wavelengths themselves ($\mu\text{m}$).
//!
//! # Cauchy Formula
//!
//! $$n(\lambda) = A + \frac{B}{\lambda^2}$$

use optics_core::OpticsError;

/// Evaluates Sellmeier Formula 1 for refractive index $n(\lambda)$ at wavelength $\lambda$ in micrometers.
///
/// # Arguments
/// * `wavelength_um` — Wavelength in vacuum in micrometers ($\mu\text{m}$).
/// * `b` — Array of 3 oscillator strength coefficients $[B_1, B_2, B_3]$.
/// * `c` — Array of 3 resonance wavelength square coefficients $[C_1, C_2, C_3]$ in $\mu\text{m}^2$.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if `wavelength_um` is non-finite, $\le 0$,
/// or if $\lambda^2 \approx C_i$.
pub fn sellmeier_formula_1(
    wavelength_um: f64,
    b: [f64; 3],
    c: [f64; 3],
) -> Result<f64, OpticsError> {
    if !wavelength_um.is_finite() || wavelength_um <= 0.0 {
        return Err(OpticsError::NumericalSingularity(
            "Wavelength must be positive and finite".to_string(),
        ));
    }

    let l2 = wavelength_um * wavelength_um;
    let mut sum = 0.0;

    for i in 0..3 {
        let denom = l2 - c[i];
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier resonance singularity at lambda={wavelength_um} um, C_{i}={}",
                c[i]
            )));
        }
        sum += (b[i] * l2) / denom;
    }

    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier formula 1"
        )));
    }

    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 2 for refractive index $n(\lambda)$.
///
/// $$n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{A_i \lambda^2}{\lambda^2 - B_i^2}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if wavelength is non-finite or resonance is hit.
pub fn sellmeier_formula_2(
    wavelength_um: f64,
    a: [f64; 3],
    b: [f64; 3],
) -> Result<f64, OpticsError> {
    if !wavelength_um.is_finite() || wavelength_um <= 0.0 {
        return Err(OpticsError::NumericalSingularity(
            "Wavelength must be positive and finite".to_string(),
        ));
    }

    let l2 = wavelength_um * wavelength_um;
    let mut sum = 0.0;

    for i in 0..3 {
        let b2 = b[i] * b[i];
        let denom = l2 - b2;
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier resonance singularity at lambda={wavelength_um} um, B_{i}={}",
                b[i]
            )));
        }
        sum += (a[i] * l2) / denom;
    }

    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier formula 2"
        )));
    }

    Ok(n2.sqrt())
}

/// Evaluates Cauchy's 2-term empirical dispersion equation: $n(\lambda) = A + B / \lambda^2$.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if wavelength is $\le 0$ or non-finite.
pub fn cauchy_dispersion(wavelength_um: f64, a: f64, b: f64) -> Result<f64, OpticsError> {
    if !wavelength_um.is_finite() || wavelength_um <= 0.0 {
        return Err(OpticsError::NumericalSingularity(
            "Wavelength must be positive and finite".to_string(),
        ));
    }

    let l2 = wavelength_um * wavelength_um;
    let n = a + b / l2;
    if n <= 0.0 || !n.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Invalid refractive index n={n} from Cauchy dispersion"
        )));
    }

    Ok(n)
}
