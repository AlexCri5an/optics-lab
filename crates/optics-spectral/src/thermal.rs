//! Thermal dispersion modeling, atmospheric air refractive index, and internal absorption.
//!
//! # Schott Thermal Model
//! Describes the temperature dependence of the relative refractive index $n_{\text{rel}}(\lambda, T)$
//! relative to reference temperature $T_0 = 20^\circ\text{C}$:
//!
//! $$\Delta n_{\text{rel}}(\lambda, \Delta T) = \frac{n^2(\lambda, T_0) - 1}{2 n(\lambda, T_0)} \left( D_0 \Delta T + D_1 \Delta T^2 + D_2 \Delta T^3 + \frac{E_0 \Delta T + E_1 \Delta T^2}{\lambda^2 - \lambda_{\text{TK}}^2} \right)$$
//!
//! # Edlén / Birch & Downs Atmospheric Air Model
//! Calculates standard and environmental air refractive index $n_{\text{air}}(\lambda, T, P)$
//! as a function of temperature ($^\circ\text{C}$) and pressure ($\text{Pa}$).

use optics_core::OpticsError;
use serde::{Deserialize, Serialize};

/// Schott temperature dispersion coefficients for optical glasses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SchottThermalModel {
    /// Coefficient $D_0$ in $\text{K}^{-1}$.
    pub d0: f64,
    /// Coefficient $D_1$ in $\text{K}^{-2}$.
    pub d1: f64,
    /// Coefficient $D_2$ in $\text{K}^{-3}$.
    pub d2: f64,
    /// Coefficient $E_0$ in $\text{K}^{-1}$.
    pub e0: f64,
    /// Coefficient $E_1$ in $\text{K}^{-2}$.
    pub e1: f64,
    /// Thermal resonance wavelength $\lambda_{\text{TK}}$ in $\mu\text{m}$.
    pub lambda_tk: f64,
}

impl SchottThermalModel {
    /// Computes $\Delta n_{\text{rel}}$ for temperature deviation $\Delta T = T - T_0$ in Kelvin ($^\circ\text{C}$).
    ///
    /// # Arguments
    /// * `n_ref` — Baseline refractive index at $T_0 = 20^\circ\text{C}$.
    /// * `wavelength_um` — Wavelength in vacuum in micrometers ($\mu\text{m}$).
    /// * `delta_t` — Temperature difference $T - 20.0$ in $^\circ\text{C}$.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] or [`OpticsError::NumericalSingularity`]
    /// if wavelength is near thermal resonance $\lambda_{\text{TK}}$.
    pub fn delta_n(
        &self,
        n_ref: f64,
        wavelength_um: f64,
        delta_t: f64,
    ) -> Result<f64, OpticsError> {
        if !n_ref.is_finite() || n_ref <= 0.0 || !wavelength_um.is_finite() || !delta_t.is_finite()
        {
            return Err(OpticsError::InvalidParameter(
                "Inputs to thermal model must be finite and n_ref positive".to_string(),
            ));
        }

        let l2 = wavelength_um * wavelength_um;
        let tk2 = self.lambda_tk * self.lambda_tk;
        let denom = l2 - tk2;
        if denom.abs() < 1e-12 {
            return Err(OpticsError::NumericalSingularity(format!(
                "Thermal resonance singularity at lambda={wavelength_um} um, lambda_tk={}",
                self.lambda_tk
            )));
        }

        let dt2 = delta_t * delta_t;
        let dt3 = dt2 * delta_t;

        let bracket = self.d0 * delta_t
            + self.d1 * dt2
            + self.d2 * dt3
            + (self.e0 * delta_t + self.e1 * dt2) / denom;

        let factor = (n_ref * n_ref - 1.0) / (2.0 * n_ref);
        Ok(factor * bracket)
    }
}

/// Evaluates refractive index of dry air via updated Edlén / Birch & Downs (1993) formulation.
///
/// # Arguments
/// * `wavelength_um` — Wavelength in micrometers ($\mu\text{m}$, valid $0.2 \dots 2.0\mu\text{m}$).
/// * `temperature_c` — Ambient temperature in Celsius (standard $15^\circ\text{C}$ or $20^\circ\text{C}$).
/// * `pressure_pa` — Ambient atmospheric pressure in Pascals (standard $101325\text{ Pa}$).
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if parameters are outside physical domain.
pub fn edlen_air_refractive_index(
    wavelength_um: f64,
    temperature_c: f64,
    pressure_pa: f64,
) -> Result<f64, OpticsError> {
    if !wavelength_um.is_finite() || wavelength_um <= 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Wavelength must be positive and finite".to_string(),
        ));
    }
    if !temperature_c.is_finite() || temperature_c < -273.15 {
        return Err(OpticsError::InvalidParameter(
            "Temperature must be above absolute zero".to_string(),
        ));
    }
    if !pressure_pa.is_finite() || pressure_pa < 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Pressure must be non-negative".to_string(),
        ));
    }

    // Vacuum wavenumber in um^-1
    let sigma = 1.0 / wavelength_um;
    let sigma2 = sigma * sigma;

    // Standard dry air refractivity (15 C, 101325 Pa, 450 ppm CO2)
    let denom1 = 130.0 - sigma2;
    let denom2 = 38.9 - sigma2;
    if denom1.abs() < 1e-10 || denom2.abs() < 1e-10 {
        return Err(OpticsError::NumericalSingularity(
            "Air dispersion resonance hit".to_string(),
        ));
    }

    let n_s_minus_1_e8 = 8342.54 + 2_406_147.0 / denom1 + 15_998.0 / denom2;
    let n_s_minus_1 = n_s_minus_1_e8 * 1e-8;

    // Conversion for temperature and pressure
    let p_standard = 101_325.0;
    let t_standard = 288.15; // 15 C in Kelvin
    let t_actual = temperature_c + 273.15;

    // Virial compressibility correction factor
    let alpha = 1.0 + pressure_pa * (5.79e-7 - 9.325e-9 * temperature_c) * 1e-5;
    let alpha_s = 1.0 + p_standard * (5.79e-7 - 9.325e-9 * 15.0) * 1e-5;

    let p_factor = (pressure_pa * alpha) / (p_standard * alpha_s);
    let t_factor = t_standard / t_actual;

    let n_air = 1.0 + n_s_minus_1 * p_factor * t_factor;
    Ok(n_air)
}

/// Converts internal transmittance $\tau_i$ measured over sample thickness $t_0$ (in mm)
/// into linear absorption coefficient $\alpha$ (in $\text{mm}^{-1}$).
///
/// $$\alpha = -\frac{\ln(\tau_i)}{t_0}$$
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if $\tau_i \le 0$, $\tau_i > 1.0$, or $t_0 \le 0$.
pub fn internal_transmittance_to_absorption_coeff(
    tau_internal: f64,
    thickness_mm: f64,
) -> Result<f64, OpticsError> {
    if !tau_internal.is_finite() || tau_internal <= 0.0 || tau_internal > 1.0 {
        return Err(OpticsError::InvalidParameter(
            "Internal transmittance must be in range (0.0, 1.0]".to_string(),
        ));
    }
    if !thickness_mm.is_finite() || thickness_mm <= 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Reference thickness must be positive".to_string(),
        ));
    }
    Ok(-tau_internal.ln() / thickness_mm)
}

/// Evaluates transmittance across a ray path segment of length $L$ in millimeters
/// given linear absorption coefficient $\alpha$ in $\text{mm}^{-1}$.
///
/// $$\tau(L) = \exp(-\alpha L)$$
#[must_use]
pub fn ray_path_transmittance(absorption_coeff_per_mm: f64, path_length_mm: f64) -> f64 {
    if !absorption_coeff_per_mm.is_finite() || absorption_coeff_per_mm <= 0.0 {
        return 1.0;
    }
    let exponent = -absorption_coeff_per_mm * path_length_mm.max(0.0);
    exponent.exp().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_schott_thermal_nbk7() -> TestResult {
        // N-BK7 thermal coefficients from Schott catalog:
        // D0 = 1.86e-6, D1 = 1.31e-8, D2 = -1.37e-11, E0 = 4.34e-7, E1 = 6.27e-10, lambda_tk = 0.170 um
        let thermal = SchottThermalModel {
            d0: 1.86e-6,
            d1: 1.31e-8,
            d2: -1.37e-11,
            e0: 4.34e-7,
            e1: 6.27e-10,
            lambda_tk: 0.170,
        };

        let n_ref = 1.5168; // d-line index at 20 C
        let dn = thermal.delta_n(n_ref, 0.58756, 20.0)?; // delta T = +20 C (at 40 C)
        assert!(dn > 0.0, "Glass index increases with temperature");
        assert!(
            dn < 1e-3,
            "Index shift should be on order of 10^-5 to 10^-4"
        );
        Ok(())
    }

    #[test]
    fn test_edlen_air_standard_conditions() -> TestResult {
        // Standard air at 15 C, 101325 Pa, visible 587.56 nm
        let n_air = edlen_air_refractive_index(0.58756, 15.0, 101_325.0)?;
        assert!(
            (n_air - 1.000277).abs() < 1e-5,
            "Air index mismatch: got {n_air}"
        );

        // At higher temperature (lower density), refractive index should decrease toward 1.0
        let n_air_hot = edlen_air_refractive_index(0.58756, 40.0, 101_325.0)?;
        assert!(n_air_hot < n_air);
        Ok(())
    }

    #[test]
    fn test_internal_transmittance_and_path() -> TestResult {
        // 99.8% internal transmittance over 10 mm sample
        let alpha = internal_transmittance_to_absorption_coeff(0.998, 10.0)?;
        assert!(alpha > 0.0);

        // 10 mm path gives exactly 0.998
        let t10 = ray_path_transmittance(alpha, 10.0);
        assert!((t10 - 0.998).abs() < 1e-12);

        // 20 mm path gives 0.998^2 = 0.996004
        let t20 = ray_path_transmittance(alpha, 20.0);
        assert!((t20 - 0.998 * 0.998).abs() < 1e-12);
        Ok(())
    }
}
