//! Comprehensive empirical dispersion formulas for optical glasses and crystalline media.
//!
//! Includes Schott, Sellmeier (Formulas 1 through 5), Herzberger, Conrady,
//! Handbook of Optics (Formulas 1 & 2), Extended formulas (1 & 2), and Cauchy.
//!
//! All calculations assume wavelength $\lambda$ is provided in micrometers ($\mu\text{m}$).

use optics_core::OpticsError;

/// Validates that wavelength is strictly positive, finite, and within physical range.
fn validate_wavelength(wavelength_um: f64) -> Result<(), OpticsError> {
    if !wavelength_um.is_finite() || wavelength_um <= 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Wavelength in micrometers must be positive and finite".to_string(),
        ));
    }
    Ok(())
}

/// Evaluates the Schott empirical dispersion formula:
///
/// $$n^2(\lambda) = a_0 + a_1 \lambda^2 + a_2 \lambda^{-2} + a_3 \lambda^{-4} + a_4 \lambda^{-6} + a_5 \lambda^{-8}$$
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if wavelength or coefficients are invalid,
/// or [`OpticsError::NumericalSingularity`] if $n^2 \le 0$.
pub fn schott_dispersion(wavelength_um: f64, a: [f64; 6]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    if !a.iter().all(|c| c.is_finite()) {
        return Err(OpticsError::InvalidParameter(
            "Schott coefficients must be finite".to_string(),
        ));
    }

    let l2 = wavelength_um * wavelength_um;
    let inv_l2 = 1.0 / l2;
    let inv_l4 = inv_l2 * inv_l2;
    let inv_l6 = inv_l4 * inv_l2;
    let inv_l8 = inv_l4 * inv_l4;

    let n2 = a[0] + a[1] * l2 + a[2] * inv_l2 + a[3] * inv_l4 + a[4] * inv_l6 + a[5] * inv_l8;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Schott dispersion"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 1 (standard 3-term oscillator equation):
///
/// $$n^2(\lambda) - 1 = \sum_{i=0}^2 \frac{K_i \lambda^2}{\lambda^2 - L_i}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $\lambda^2 \approx L_i$ or $n^2 \le 0$.
pub fn sellmeier_1(wavelength_um: f64, k: [f64; 3], l: [f64; 3]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let mut sum = 0.0;
    for i in 0..3 {
        let denom = l2 - l[i];
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier 1 singularity at lambda={wavelength_um} um, L_{i}={}",
                l[i]
            )));
        }
        sum += (k[i] * l2) / denom;
    }
    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier 1"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 2 (constant offset plus 2 oscillators):
///
/// $$n^2(\lambda) - 1 = A + \sum_{i=0}^1 \frac{B_i \lambda^2}{\lambda^2 - C_i}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] at resonance or if $n^2 \le 0$.
pub fn sellmeier_2(
    wavelength_um: f64,
    a: f64,
    b: [f64; 2],
    c: [f64; 2],
) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let mut sum = a;
    for i in 0..2 {
        let denom = l2 - c[i];
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier 2 singularity at lambda={wavelength_um} um, C_{i}={}",
                c[i]
            )));
        }
        sum += (b[i] * l2) / denom;
    }
    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier 2"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 3 (4-term oscillator equation):
///
/// $$n^2(\lambda) - 1 = \sum_{i=0}^3 \frac{K_i \lambda^2}{\lambda^2 - L_i}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] at resonance or if $n^2 \le 0$.
pub fn sellmeier_3(wavelength_um: f64, k: [f64; 4], l: [f64; 4]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let mut sum = 0.0;
    for i in 0..4 {
        let denom = l2 - l[i];
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier 3 singularity at lambda={wavelength_um} um, L_{i}={}",
                l[i]
            )));
        }
        sum += (k[i] * l2) / denom;
    }
    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier 3"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 4:
///
/// $$n^2(\lambda) = A + \frac{B \lambda^2}{\lambda^2 - C} + \frac{D \lambda^2}{\lambda^2 - E}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] at resonance or if $n^2 \le 0$.
pub fn sellmeier_4(
    wavelength_um: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let denom1 = l2 - c;
    let denom2 = l2 - e;
    if denom1.abs() < 1e-12 || denom2.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Sellmeier 4 resonance singularity".to_string(),
        ));
    }
    let n2 = a + (b * l2) / denom1 + (d * l2) / denom2;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier 4"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Sellmeier Formula 5 (5-term oscillator equation):
///
/// $$n^2(\lambda) - 1 = \sum_{i=0}^4 \frac{K_i \lambda^2}{\lambda^2 - L_i}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] at resonance or if $n^2 \le 0$.
pub fn sellmeier_5(wavelength_um: f64, k: [f64; 5], l: [f64; 5]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let mut sum = 0.0;
    for i in 0..5 {
        let denom = l2 - l[i];
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Sellmeier 5 singularity at lambda={wavelength_um} um, L_{i}={}",
                l[i]
            )));
        }
        sum += (k[i] * l2) / denom;
    }
    let n2 = 1.0 + sum;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Sellmeier 5"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates the Herzberger dispersion equation:
///
/// $$n(\lambda) = A + B \lambda^2 + C \lambda^4 + \frac{D}{\lambda^2 - 0.028} + \frac{E}{(\lambda^2 - 0.028)^2}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $\lambda^2 \approx 0.028$ or if $n \le 0$.
pub fn herzberger_dispersion(wavelength_um: f64, coeffs: [f64; 5]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let denom = l2 - 0.028;
    if denom.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Herzberger resonance singularity at lambda^2 = 0.028 um^2".to_string(),
        ));
    }
    let inv1 = 1.0 / denom;
    let inv2 = inv1 * inv1;
    let n = coeffs[0] + coeffs[1] * l2 + coeffs[2] * l2 * l2 + coeffs[3] * inv1 + coeffs[4] * inv2;
    if n <= 0.0 || !n.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Non-positive or non-finite index n={n} in Herzberger"
        )));
    }
    Ok(n)
}

/// Evaluates the Conrady dispersion equation:
///
/// $$n(\lambda) = n_0 + \frac{A}{\lambda} + \frac{B}{\lambda^{3.5}}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $n \le 0$.
pub fn conrady_dispersion(wavelength_um: f64, n0: f64, a: f64, b: f64) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let n = n0 + a / wavelength_um + b / wavelength_um.powf(3.5);
    if n <= 0.0 || !n.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Non-positive or non-finite index n={n} in Conrady"
        )));
    }
    Ok(n)
}

/// Evaluates Handbook of Optics Formula 1:
///
/// $$n^2(\lambda) = A + \frac{B}{\lambda^2 - C} - D \lambda^2$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if resonance is hit or $n^2 \le 0$.
pub fn handbook_of_optics_1(
    wavelength_um: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let denom = l2 - c;
    if denom.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Handbook of Optics 1 resonance singularity".to_string(),
        ));
    }
    let n2 = a + b / denom - d * l2;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Handbook of Optics 1"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Handbook of Optics Formula 2:
///
/// $$n^2(\lambda) = A + \frac{B \lambda^2}{\lambda^2 - C} - D \lambda^2$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if resonance is hit or $n^2 \le 0$.
pub fn handbook_of_optics_2(
    wavelength_um: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let denom = l2 - c;
    if denom.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Handbook of Optics 2 resonance singularity".to_string(),
        ));
    }
    let n2 = a + (b * l2) / denom - d * l2;
    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Handbook of Optics 2"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Extended Formula 1 (high-order infrared dispersion):
///
/// $$n^2(\lambda) = a_0 + a_1 \lambda^2 + a_2 \lambda^{-2} + a_3 \lambda^{-4} + a_4 \lambda^{-6} + a_5 \lambda^{-8} + a_6 \lambda^{-10} + a_7 \lambda^{-12}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $n^2 \le 0$.
pub fn extended_1(wavelength_um: f64, a: [f64; 8]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let inv_l2 = 1.0 / l2;
    let inv_l4 = inv_l2 * inv_l2;
    let inv_l6 = inv_l4 * inv_l2;
    let inv_l8 = inv_l4 * inv_l4;
    let inv_l10 = inv_l8 * inv_l2;
    let inv_l12 = inv_l6 * inv_l6;

    let n2 = a[0]
        + a[1] * l2
        + a[2] * inv_l2
        + a[3] * inv_l4
        + a[4] * inv_l6
        + a[5] * inv_l8
        + a[6] * inv_l10
        + a[7] * inv_l12;

    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Extended 1"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Extended Formula 2:
///
/// $$n^2(\lambda) = a_0 + a_1 \lambda^2 + a_2 \lambda^{-2} + a_3 \lambda^{-4} + a_4 \lambda^{-6} + a_5 \lambda^4 + a_6 \lambda^6$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $n^2 \le 0$.
pub fn extended_2(wavelength_um: f64, a: [f64; 7]) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let l2 = wavelength_um * wavelength_um;
    let l4 = l2 * l2;
    let l6 = l4 * l2;
    let inv_l2 = 1.0 / l2;
    let inv_l4 = inv_l2 * inv_l2;
    let inv_l6 = inv_l4 * inv_l2;

    let n2 =
        a[0] + a[1] * l2 + a[2] * inv_l2 + a[3] * inv_l4 + a[4] * inv_l6 + a[5] * l4 + a[6] * l6;

    if n2 <= 0.0 || !n2.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Negative or non-finite n^2={n2} in Extended 2"
        )));
    }
    Ok(n2.sqrt())
}

/// Evaluates Extended Cauchy equation:
///
/// $$n(\lambda) = A + \frac{B}{\lambda^2} + \frac{C}{\lambda^4} + \frac{D}{\lambda^6}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $n \le 0$.
pub fn extended_cauchy(
    wavelength_um: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
) -> Result<f64, OpticsError> {
    validate_wavelength(wavelength_um)?;
    let inv_l2 = 1.0 / (wavelength_um * wavelength_um);
    let inv_l4 = inv_l2 * inv_l2;
    let inv_l6 = inv_l4 * inv_l2;

    let n = a + b * inv_l2 + c * inv_l4 + d * inv_l6;
    if n <= 0.0 || !n.is_finite() {
        return Err(OpticsError::NumericalSingularity(format!(
            "Non-positive or non-finite index n={n} in Extended Cauchy"
        )));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_schott_dispersion_evaluation() -> TestResult {
        // Typical Schott glass coefficients (e.g. SF11 vintage Schott representation)
        let a = [2.87, -0.01, 0.03, 0.002, 0.0001, 0.0];
        let n_550 = schott_dispersion(0.55, a)?;
        assert!(n_550 > 1.6 && n_550 < 1.9);

        // Shorter wavelength should exhibit higher refractive index
        let n_400 = schott_dispersion(0.40, a)?;
        let n_700 = schott_dispersion(0.70, a)?;
        assert!(n_400 > n_550);
        assert!(n_550 > n_700);
        Ok(())
    }

    #[test]
    fn test_herzberger_and_conrady() -> TestResult {
        let h_coeffs = [1.5, -0.001, 0.00001, 0.01, 0.0001];
        let n_h = herzberger_dispersion(0.6, h_coeffs)?;
        assert!(n_h > 1.4 && n_h < 1.7);

        let n_conrady = conrady_dispersion(0.587, 1.5, 0.005, 0.0002)?;
        assert!(n_conrady > 1.5);
        Ok(())
    }

    #[test]
    fn test_extended_cauchy() -> TestResult {
        let n = extended_cauchy(0.5, 1.5, 0.004, 0.0001, 0.0)?;
        let expected = 1.5 + 0.004 / 0.25 + 0.0001 / 0.0625;
        assert!((n - expected).abs() < 1e-10);
        Ok(())
    }
}
