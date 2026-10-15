//! `optics-spectral` — Dispersion models (Sellmeier, Cauchy), Fraunhofer lines,
//! and spectral color transformations.
//!
//! All calculations use wavelengths specified in nanometers (`nm`) in high-level APIs
//! and micrometers (`μm`) in Sellmeier formulas.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod abbe;
pub mod color;
pub mod hero;
pub mod sellmeier;

pub use abbe::{
    LAMBDA_C_NM, LAMBDA_C_UM, LAMBDA_D_NM, LAMBDA_D_UM, LAMBDA_F_NM, LAMBDA_F_UM, abbe_vd,
};
pub use color::{
    CIE_1931_CMF_5NM, CIE_D65_SPD_5NM, SPECTRUM_MAX_NM, SPECTRUM_MIN_NM, SPECTRUM_STEP_NM,
    cie_1931_cmf, d65_normalization_factor, d65_spd, display_color_for_wavelength, linear_to_srgb,
    spectrum_to_srgb, srgb_to_linear, wavelength_to_xyz, xyz_to_linear_srgb, xyz_to_srgb,
};
pub use hero::{
    DynamicHeroSpectrum, HeroBundle4, HeroSpectrum, hero_sample_wavelengths, sample_hero_bundle,
};
pub use sellmeier::{cauchy_dispersion, sellmeier_formula_1, sellmeier_formula_2};

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_nbk7_sellmeier_and_abbe() -> TestResult {
        // Schott N-BK7 Sellmeier coefficients from Schott catalog / optical_glasses.json:
        // B1 = 1.03961212, C1 = 0.00600069867 um^2
        // B2 = 0.231792344, C2 = 0.0200179144 um^2
        // B3 = 1.01046945,  C3 = 103.560653 um^2
        let b = [1.03961212, 0.231792344, 1.01046945];
        let c = [0.00600069867, 0.0200179144, 103.560653];

        // 1. Refractive index at helium yellow d line (587.56 nm = 0.58756 um)
        let n_d = sellmeier_formula_1(LAMBDA_D_UM, b, c)?;
        assert!(
            (n_d - 1.51680).abs() < 0.0002,
            "N-BK7 n_d mismatch: got {n_d}, expected ~1.5168"
        );

        // 2. Refractive index at hydrogen blue F line (486.13 nm = 0.48613 um)
        let n_f = sellmeier_formula_1(LAMBDA_F_UM, b, c)?;
        assert!(
            (n_f - 1.52238).abs() < 0.0002,
            "N-BK7 n_f mismatch: got {n_f}, expected ~1.52238"
        );

        // 3. Refractive index at hydrogen red C line (656.27 nm = 0.65627 um)
        let n_c = sellmeier_formula_1(LAMBDA_C_UM, b, c)?;
        assert!(
            (n_c - 1.51432).abs() < 0.0002,
            "N-BK7 n_c mismatch: got {n_c}, expected ~1.51432"
        );

        // 4. Dispersion ordering: n_F > n_d > n_C
        assert!(
            n_f > n_d && n_d > n_c,
            "Normal dispersion ordering violated"
        );

        // 5. Abbe number Vd = (n_d - 1) / (n_f - n_c)
        let vd = abbe_vd(n_d, n_f, n_c)?;
        assert!(
            (vd - 64.17).abs() < 0.1,
            "N-BK7 Abbe Vd mismatch: got {vd}, expected ~64.17"
        );

        Ok(())
    }

    #[test]
    fn test_cauchy_dispersion() -> TestResult {
        // Simple crown-like Cauchy model: n(lambda) = 1.5 + 0.005 / lambda^2
        let a = 1.5;
        let b = 0.005;
        let n_blue = cauchy_dispersion(0.4, a, b)?;
        let n_red = cauchy_dispersion(0.7, a, b)?;

        assert!(n_blue > n_red);
        let expected_blue = 1.5 + 0.005 / (0.4 * 0.4);
        assert!((n_blue - expected_blue).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_d65_y_normalizes_to_one() {
        let k = d65_normalization_factor();
        let mut y_sum = 0.0;
        for (spd, (_, y, _)) in CIE_D65_SPD_5NM.iter().zip(CIE_1931_CMF_5NM.iter()) {
            y_sum += spd * y * SPECTRUM_STEP_NM;
        }
        let y_norm = y_sum * k;
        assert!(
            (y_norm - 1.0).abs() < 1e-10,
            "D65 Y should normalize to 1.0, got {y_norm}"
        );
    }

    #[test]
    fn test_d65_to_linear_srgb_within_one_percent() {
        let k = d65_normalization_factor();
        let mut x_sum = 0.0;
        let mut y_sum = 0.0;
        let mut z_sum = 0.0;
        for (p, (x, y, z)) in CIE_D65_SPD_5NM.iter().zip(CIE_1931_CMF_5NM.iter()) {
            x_sum += p * x * SPECTRUM_STEP_NM;
            y_sum += p * y * SPECTRUM_STEP_NM;
            z_sum += p * z * SPECTRUM_STEP_NM;
        }
        let xyz_d65 = [x_sum * k, y_sum * k, z_sum * k];
        let [r, g, b] = xyz_to_linear_srgb(xyz_d65);

        assert!(
            (r - 1.0).abs() < 0.01,
            "D65 linear sRGB R should be within 1% of 1.0, got {r}"
        );
        assert!(
            (g - 1.0).abs() < 0.01,
            "D65 linear sRGB G should be within 1% of 1.0, got {g}"
        );
        assert!(
            (b - 1.0).abs() < 0.01,
            "D65 linear sRGB B should be within 1% of 1.0, got {b}"
        );
    }

    #[test]
    fn test_monochromatic_ordering_550nm_green_700nm_red() {
        // Monochromatic 550 nm: green dominant
        let xyz_550 = wavelength_to_xyz(550.0, 1.0);
        let [r_550, g_550, b_550] = xyz_to_linear_srgb(xyz_550);
        assert!(
            g_550 > r_550 && g_550 > b_550,
            "550 nm should be green-dominant: R={r_550}, G={g_550}, B={b_550}"
        );

        // Monochromatic 700 nm: red dominant
        let xyz_700 = wavelength_to_xyz(700.0, 1.0);
        let [r_700, g_700, b_700] = xyz_to_linear_srgb(xyz_700);
        assert!(
            r_700 > g_700 && r_700 > b_700,
            "700 nm should be red-dominant: R={r_700}, G={g_700}, B={b_700}"
        );
    }

    #[test]
    fn test_srgb_curve_linearity_exponent_and_continuity() {
        let junction = 0.0031308;

        // 1. Linearity below junction: slope must be 12.92
        let val_linear = 0.001;
        let srgb_linear = linear_to_srgb(val_linear) as f64;
        let expected_linear = 12.92 * val_linear;
        assert!(
            (srgb_linear - expected_linear).abs() < 1e-6,
            "Linear segment below {junction} should have slope 12.92: got {srgb_linear}, expected {expected_linear}"
        );

        // 2. Exponential curve above junction: 1.055 * c^(1/2.4) - 0.055
        let val_pow = 0.5;
        let srgb_pow = linear_to_srgb(val_pow) as f64;
        let expected_pow = 1.055 * val_pow.powf(1.0 / 2.4) - 0.055;
        assert!(
            (srgb_pow - expected_pow).abs() < 1e-6,
            "Power segment above {junction} must match IEC 61966-2-1: got {srgb_pow}, expected {expected_pow}"
        );

        // 3. Continuity at junction
        let val_below = junction - 1e-8;
        let val_above = junction + 1e-8;
        let srgb_below = linear_to_srgb(val_below);
        let srgb_above = linear_to_srgb(val_above);
        assert!(
            (srgb_below - srgb_above).abs() < 1e-4,
            "sRGB transfer function must be continuous at junction: below={srgb_below}, above={srgb_above}"
        );
    }

    #[test]
    fn test_equal_energy_spectrum_is_not_one() {
        let mut x_sum = 0.0;
        let mut y_sum = 0.0;
        let mut z_sum = 0.0;
        for (x, y, z) in &CIE_1931_CMF_5NM {
            x_sum += 1.0 * x * SPECTRUM_STEP_NM;
            y_sum += 1.0 * y * SPECTRUM_STEP_NM;
            z_sum += 1.0 * z * SPECTRUM_STEP_NM;
        }

        // Normalize equal energy to Y = 1.0
        let k_ee = 1.0 / y_sum;
        let xyz_ee = [x_sum * k_ee, y_sum * k_ee, z_sum * k_ee];
        let [r, _g, b] = xyz_to_linear_srgb(xyz_ee);

        // Equal Energy Illuminant E has chromaticity (1/3, 1/3) which differs from D65 white point (0.3127, 0.3290).
        // In sRGB, Illuminant E is reddish-pink (R > 1.20, G ~ 0.95, B ~ 0.91).
        // Assert it differs from (1, 1, 1) by at least 15% on the red channel:
        assert!(
            (r - 1.0).abs() > 0.15,
            "Equal-energy illuminant E should NOT produce (1,1,1) in D65 sRGB: R was {r}"
        );
        assert!(
            (b - 1.0).abs() > 0.05,
            "Equal-energy illuminant E should NOT produce (1,1,1) in D65 sRGB: B was {b}"
        );
    }

    #[test]
    fn test_display_color_for_wavelength() {
        let green = display_color_for_wavelength(550.0);
        let red = display_color_for_wavelength(700.0);
        let blue = display_color_for_wavelength(450.0);

        // Green line: G is highest channel
        assert!(green[1] >= green[0] && green[1] >= green[2]);
        assert_eq!(green[1], 255);

        // Red line: R is highest channel
        assert!(red[0] >= red[1] && red[0] >= red[2]);
        assert_eq!(red[0], 255);

        // Blue line: B is highest channel
        assert!(blue[2] >= blue[0] && blue[2] >= blue[1]);
        assert_eq!(blue[2], 255);
    }
}
