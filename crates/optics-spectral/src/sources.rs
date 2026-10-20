//! Standard discrete spectral line sources and continuous Planck blackbody radiator.
//!
//! Provides rigorously tabulated discharge lamp emission lines according to
//! ISO 7944 / NIST / Zemax OpticStudio / Optiland standards:
//! - Helium (He)
//! - Neon (Ne)
//! - Mercury (Hg)
//! - Cadmium (Cd)
//! - Hydrogen / Deuterium (H / D)
//! - Sodium (Na)
//!
//! And continuous spectral radiance models:
//! - Planck Blackbody Radiator: $B_\lambda(\lambda, T) = \frac{2hc^2}{\lambda^5} \frac{1}{\exp\left(\frac{hc}{\lambda k_B T}\right) - 1}$
//! - Standard color temperatures: 2856 K (CIE Illuminant A), 3000 K (Halogen), 5500 K (Daylight D55), 6500 K (Daylight D65).

use optics_core::error::OpticsError;

/// Fundamental physical constants in SI units (CODATA recommended values).
/// Planck constant $h$ in $\text{J} \cdot \text{s}$.
pub const PLANCK_CONSTANT_H: f64 = 6.626_070_15e-34;
/// Speed of light in vacuum $c$ in $\text{m} / \text{s}$.
pub const SPEED_OF_LIGHT_C: f64 = 299_792_458.0;
/// Boltzmann constant $k_B$ in $\text{J} / \text{K}$.
pub const BOLTZMANN_CONSTANT_K: f64 = 1.380_649e-23;

/// Standard gas discharge spectral lamp species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpectralLamp {
    /// Helium (He) gas discharge lamp.
    Helium,
    /// Neon (Ne) glow discharge lamp.
    Neon,
    /// Mercury (Hg) low-pressure vapor lamp (primary optical metrology & UV calibration standard).
    Mercury,
    /// Cadmium (Cd) metal vapor lamp (interferometry standard).
    Cadmium,
    /// Hydrogen / Deuterium (H / D) discharge lamp (Fraunhofer $C, F$ line standard).
    Hydrogen,
    /// Sodium (Na) vapor lamp (Fraunhofer $D$ doublet line standard).
    Sodium,
}

/// A discrete spectral emission line emitted by a calibration lamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectralLine {
    /// Fraunhofer or spectroscopic designation (e.g. "d", "e", "F", "C", "h", "i").
    pub name: &'static str,
    /// Vacuum/air wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Relative emission intensity (normalized to ~1.0 or 100 for primary peak).
    pub relative_intensity: f64,
    /// Element / emitter species.
    pub element: SpectralLamp,
}

impl SpectralLamp {
    /// Returns the standard tabulated spectral lines for this lamp species.
    ///
    /// Wavelengths adhere to ISO 7944, NIST Atomic Spectra Database, and Zemax OpticStudio standards.
    #[must_use]
    pub fn spectral_lines(self) -> &'static [SpectralLine] {
        match self {
            Self::Helium => &HELIUM_LINES,
            Self::Neon => &NEON_LINES,
            Self::Mercury => &MERCURY_LINES,
            Self::Cadmium => &CADMIUM_LINES,
            Self::Hydrogen => &HYDROGEN_LINES,
            Self::Sodium => &SODIUM_LINES,
        }
    }
}

/// Helium (He) discharge lamp emission lines.
pub static HELIUM_LINES: [SpectralLine; 6] = [
    SpectralLine { name: "He 447.1", wavelength_nm: 447.148, relative_intensity: 0.35, element: SpectralLamp::Helium },
    SpectralLine { name: "He 501.6", wavelength_nm: 501.567, relative_intensity: 0.40, element: SpectralLamp::Helium },
    SpectralLine { name: "d",        wavelength_nm: 587.5618, relative_intensity: 1.00, element: SpectralLamp::Helium }, // Standard helium yellow d-line
    SpectralLine { name: "He 667.8", wavelength_nm: 667.815, relative_intensity: 0.55, element: SpectralLamp::Helium },
    SpectralLine { name: "r",        wavelength_nm: 706.519, relative_intensity: 0.45, element: SpectralLamp::Helium }, // Fraunhofer r-line
    SpectralLine { name: "He 1083",  wavelength_nm: 1083.03, relative_intensity: 0.85, element: SpectralLamp::Helium },
];

/// Neon (Ne) glow discharge lamp emission lines.
pub static NEON_LINES: [SpectralLine; 6] = [
    SpectralLine { name: "Ne 540.1", wavelength_nm: 540.056, relative_intensity: 0.25, element: SpectralLamp::Neon },
    SpectralLine { name: "Ne 585.3", wavelength_nm: 585.249, relative_intensity: 0.60, element: SpectralLamp::Neon },
    SpectralLine { name: "Ne 640.2", wavelength_nm: 640.225, relative_intensity: 1.00, element: SpectralLamp::Neon },
    SpectralLine { name: "Ne 650.6", wavelength_nm: 650.653, relative_intensity: 0.75, element: SpectralLamp::Neon },
    SpectralLine { name: "s",        wavelength_nm: 852.11,  relative_intensity: 0.70, element: SpectralLamp::Neon },
    SpectralLine { name: "Ne 703.2", wavelength_nm: 703.241, relative_intensity: 0.65, element: SpectralLamp::Neon },
];

/// Mercury (Hg) low-pressure vapor calibration lamp emission lines.
///
/// Primary metrology standard in optical engineering (includes i, h, g, e lines).
pub static MERCURY_LINES: [SpectralLine; 7] = [
    SpectralLine { name: "i",        wavelength_nm: 365.015, relative_intensity: 0.55, element: SpectralLamp::Mercury }, // UV i-line
    SpectralLine { name: "h",        wavelength_nm: 404.656, relative_intensity: 0.40, element: SpectralLamp::Mercury }, // Violet h-line
    SpectralLine { name: "g",        wavelength_nm: 435.833, relative_intensity: 0.80, element: SpectralLamp::Mercury }, // Blue g-line
    SpectralLine { name: "e",        wavelength_nm: 546.074, relative_intensity: 1.00, element: SpectralLamp::Mercury }, // Green e-line (ISO principal reference)
    SpectralLine { name: "Hg 577.0", wavelength_nm: 576.960, relative_intensity: 0.35, element: SpectralLamp::Mercury }, // Yellow doublet 1
    SpectralLine { name: "Hg 579.1", wavelength_nm: 579.066, relative_intensity: 0.35, element: SpectralLamp::Mercury }, // Yellow doublet 2
    SpectralLine { name: "t",        wavelength_nm: 1013.98, relative_intensity: 0.30, element: SpectralLamp::Mercury }, // IR t-line
];

/// Cadmium (Cd) metal vapor calibration lamp emission lines.
pub static CADMIUM_LINES: [SpectralLine; 4] = [
    SpectralLine { name: "Cd 467.8", wavelength_nm: 467.815, relative_intensity: 0.30, element: SpectralLamp::Cadmium },
    SpectralLine { name: "F'",       wavelength_nm: 479.991, relative_intensity: 0.70, element: SpectralLamp::Cadmium }, // Fraunhofer F'-line
    SpectralLine { name: "Cd 508.6", wavelength_nm: 508.582, relative_intensity: 0.85, element: SpectralLamp::Cadmium },
    SpectralLine { name: "C'",       wavelength_nm: 643.847, relative_intensity: 1.00, element: SpectralLamp::Cadmium }, // Fraunhofer C'-line
];

/// Hydrogen / Deuterium (H / D) calibration lamp emission lines.
pub static HYDROGEN_LINES: [SpectralLine; 3] = [
    SpectralLine { name: "F",        wavelength_nm: 486.1327, relative_intensity: 0.80, element: SpectralLamp::Hydrogen }, // Fraunhofer F-line (H-beta)
    SpectralLine { name: "C",        wavelength_nm: 656.2725, relative_intensity: 1.00, element: SpectralLamp::Hydrogen }, // Fraunhofer C-line (H-alpha)
    SpectralLine { name: "H-gamma",  wavelength_nm: 434.047,  relative_intensity: 0.40, element: SpectralLamp::Hydrogen },
];

/// Sodium (Na) vapor calibration lamp emission lines.
pub static SODIUM_LINES: [SpectralLine; 2] = [
    SpectralLine { name: "D2",       wavelength_nm: 588.995, relative_intensity: 1.00, element: SpectralLamp::Sodium },
    SpectralLine { name: "D1",       wavelength_nm: 589.592, relative_intensity: 0.50, element: SpectralLamp::Sodium },
];

/// Evaluates Planck's blackbody spectral radiance $B_\lambda(\lambda, T)$ in $\text{W} / (\text{m}^2 \cdot \text{sr} \cdot \text{m})$.
///
/// # Formula
/// $$B_\lambda(\lambda, T) = \frac{2 h c^2}{\lambda^5} \frac{1}{\exp\left(\frac{h c}{\lambda k_B T}\right) - 1}$$
///
/// # Arguments
/// - `wavelength_nm`: Radiation wavelength in nanometers ($\lambda > 0$).
/// - `temperature_k`: Absolute blackbody temperature in Kelvin ($T > 0$).
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if $\lambda \le 0$ or $T \le 0$.
pub fn planck_spectral_radiance(wavelength_nm: f64, temperature_k: f64) -> Result<f64, OpticsError> {
    if !wavelength_nm.is_finite() || wavelength_nm <= 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Wavelength must be strictly positive".to_string(),
        ));
    }
    if !temperature_k.is_finite() || temperature_k <= 0.0 {
        return Err(OpticsError::InvalidParameter(
            "Temperature in Kelvin must be strictly positive".to_string(),
        ));
    }

    let lambda_m = wavelength_nm * 1e-9;
    let c1 = 2.0 * PLANCK_CONSTANT_H * SPEED_OF_LIGHT_C * SPEED_OF_LIGHT_C;
    let c2 = (PLANCK_CONSTANT_H * SPEED_OF_LIGHT_C) / (BOLTZMANN_CONSTANT_K * temperature_k);

    let exponent = c2 / lambda_m;
    if exponent > 700.0 {
        // Exponent overflow check: Wien tail approaches zero
        return Ok(0.0);
    }

    let denom = (exponent.exp() - 1.0) * lambda_m.powi(5);
    if denom <= 0.0 || !denom.is_finite() {
        return Ok(0.0);
    }

    Ok(c1 / denom)
}

/// Evaluates normalized Planck blackbody spectral power distribution $S(\lambda) \in [0, 1]$,
/// peaking at 1.0 at Wien's displacement wavelength $\lambda_{\text{peak}} = b / T$.
///
/// # Arguments
/// - `wavelength_nm`: Radiation wavelength in nanometers.
/// - `temperature_k`: Absolute blackbody temperature in Kelvin.
///
/// # Errors
/// Returns [`OpticsError::InvalidParameter`] if $\lambda \le 0$ or $T \le 0$.
pub fn planck_normalized_spd(wavelength_nm: f64, temperature_k: f64) -> Result<f64, OpticsError> {
    // Wien displacement constant b = 2.897771955e-3 m * K = 2.897771955e6 nm * K
    let peak_lambda_nm = 2.897_771_955e6 / temperature_k;
    let b_max = planck_spectral_radiance(peak_lambda_nm, temperature_k)?;
    if b_max <= 1e-30 {
        return Ok(0.0);
    }

    let b_val = planck_spectral_radiance(wavelength_nm, temperature_k)?;
    Ok((b_val / b_max).clamp(0.0, 1.0))
}

/// Standard Planck blackbody color temperatures in Kelvin.
pub struct StandardColorTemp;

impl StandardColorTemp {
    /// CIE Standard Illuminant A (Gas-filled tungsten lamp, 2856 K).
    pub const ILLUMINANT_A: f64 = 2856.0;
    /// Studio tungsten halogen lamp (3000 K).
    pub const TUNGSTEN_HALOGEN: f64 = 3000.0;
    /// Direct solar radiation / Daylight D55 (5500 K).
    pub const DAYLIGHT_D55: f64 = 5500.0;
    /// Standard average daylight / CIE Illuminant D65 (6500 K).
    pub const DAYLIGHT_D65: f64 = 6500.0;
    /// Overcast sky / North sky daylight (7500 K).
    pub const OVERCAST_D75: f64 = 7500.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_mercury_principal_lines() {
        let lines = SpectralLamp::Mercury.spectral_lines();
        assert_eq!(lines.len(), 7);

        // e-line is 546.074 nm
        let e_line = lines.iter().find(|l| l.name == "e").unwrap();
        assert!((e_line.wavelength_nm - 546.074).abs() < 1e-3);

        // i-line is 365.015 nm (UV)
        let i_line = lines.iter().find(|l| l.name == "i").unwrap();
        assert!((i_line.wavelength_nm - 365.015).abs() < 1e-3);
    }

    #[test]
    fn test_planck_radiation_wien_peak() -> TestResult {
        // For T = 5500 K, Wien's peak is at ~526.8 nm (green)
        let temp: f64 = 5500.0;
        let peak_nm: f64 = 2.897_771_955e6 / temp;
        assert!((peak_nm - 526.86).abs() < 0.1);

        let spd_at_peak = planck_normalized_spd(peak_nm, temp)?;
        assert!((spd_at_peak - 1.0).abs() < 1e-4, "Peak SPD must normalize to 1.0");

        // Wavelengths away from peak must have lower normalized SPD
        let spd_blue = planck_normalized_spd(400.0, temp)?;
        let spd_red = planck_normalized_spd(700.0, temp)?;
        assert!(spd_blue < 1.0 && spd_blue > 0.0);
        assert!(spd_red < 1.0 && spd_red > 0.0);

        Ok(())
    }

    #[test]
    fn test_planck_sun_radiance_order_of_magnitude() -> TestResult {
        // Sun photosphere roughly 5778 K, spectral radiance at 500 nm is order of ~10^13 W/(m^3 * sr)
        let b = planck_spectral_radiance(500.0, 5778.0)?;
        assert!(b > 1e13 && b < 1e14, "Expected sun radiance ~2.6e13, got {b:e}");
        Ok(())
    }
}
