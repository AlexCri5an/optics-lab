//! Abbe number and standard Fraunhofer spectral line definitions.
//!
//! # Abbe Number ($V_d$)
//!
//! The Abbe number quantifies chromatic dispersion of a transparent medium:
//!
//! $$V_d = \frac{n_d - 1}{n_F - n_C}$$
//!
//! Higher $V_d$ denotes lower dispersion (crown glasses, e.g. N-BK7 with $V_d \approx 64.17$),
//! whereas lower $V_d$ denotes high dispersion (flint glasses, e.g. SF11 with $V_d \approx 25.68$).

use optics_core::OpticsError;

/// Helium yellow $d$ spectral line wavelength in nanometers (587.56 nm).
pub const LAMBDA_D_NM: f64 = 587.56;
/// Helium yellow $d$ spectral line wavelength in micrometers (0.58756 $\mu\text{m}$).
pub const LAMBDA_D_UM: f64 = 0.58756;

/// Hydrogen blue $F$ spectral line wavelength in nanometers (486.13 nm).
pub const LAMBDA_F_NM: f64 = 486.13;
/// Hydrogen blue $F$ spectral line wavelength in micrometers (0.48613 $\mu\text{m}$).
pub const LAMBDA_F_UM: f64 = 0.48613;

/// Hydrogen red $C$ spectral line wavelength in nanometers (656.27 nm).
pub const LAMBDA_C_NM: f64 = 656.27;
/// Hydrogen red $C$ spectral line wavelength in micrometers (0.65627 $\mu\text{m}$).
pub const LAMBDA_C_UM: f64 = 0.65627;

/// Computes the Abbe number $V_d = (n_d - 1) / (n_F - n_C)$.
///
/// # Arguments
/// * `n_d` — Refractive index at helium yellow $d$ line (587.56 nm).
/// * `n_f` — Refractive index at hydrogen blue $F$ line (486.13 nm).
/// * `n_c` — Refractive index at hydrogen red $C$ line (656.27 nm).
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if $n_F \le n_C$ (anomalous or zero dispersion denominator)
/// or if any index is non-finite.
pub fn abbe_vd(n_d: f64, n_f: f64, n_c: f64) -> Result<f64, OpticsError> {
    if !n_d.is_finite() || !n_f.is_finite() || !n_c.is_finite() {
        return Err(OpticsError::NumericalSingularity(
            "Refractive indices must be finite to compute Abbe number".to_string(),
        ));
    }

    let delta_n = n_f - n_c;
    if delta_n.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Zero dispersion difference n_F - n_C in Abbe number calculation".to_string(),
        ));
    }

    Ok((n_d - 1.0) / delta_n)
}
