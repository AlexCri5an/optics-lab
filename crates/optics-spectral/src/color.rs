//! CIE 1931 Color Matching Functions, D65 Standard Illuminant, and sRGB color space conversions.
//!
//! # Official Data Sources
//! - CIE 1931 2-degree Standard Observer (x_bar, y_bar, z_bar) tabulated at 5 nm from 380 nm to 780 nm:
//!   Colour & Vision Research Laboratory (CVRL), University College London:
//!   <http://www.cvrl.org/database/data/cmfs/ciexyz31.csv>
//! - CIE Standard Illuminant D65 relative spectral power distribution tabulated at 5 nm:
//!   ISO/CIE 11664-2:2014(E) / CIE S 014-2/E:2006.
//!
//! # Formulas
//! - CIE 1931 XYZ tristimulus integration.
//! - Linear sRGB transformation matrix (D65 white point, ITU-R BT.709-6 / IEC 61966-2-1).
//! - IEC 61966-2-1 piecewise electro-optical transfer function (sRGB gamma curve).

/// Tabulated CIE 1931 2-degree color matching functions at 5 nm increments from 380 nm to 780 nm.
/// Source: <http://www.cvrl.org/database/data/cmfs/ciexyz31.csv>
pub const CIE_1931_CMF_5NM: [(f64, f64, f64); 81] = [
    (0.001368, 3.9e-05, 0.006450001),    // 380 nm
    (0.002236, 6.4e-05, 0.01054999),     // 385 nm
    (0.004243, 0.00012, 0.02005001),     // 390 nm
    (0.00765, 0.000217, 0.03621),        // 395 nm
    (0.01431, 0.000396, 0.06785001),     // 400 nm
    (0.02319, 0.00064, 0.1102),          // 405 nm
    (0.04351, 0.00121, 0.2074),          // 410 nm
    (0.07763, 0.00218, 0.3713),          // 415 nm
    (0.13438, 0.004, 0.6456),            // 420 nm
    (0.21477, 0.0073, 1.0390501),        // 425 nm
    (0.2839, 0.0116, 1.3856),            // 430 nm
    (0.3285, 0.01684, 1.62296),          // 435 nm
    (0.34828, 0.023, 1.74706),           // 440 nm
    (0.34806, 0.0298, 1.7826),           // 445 nm
    (0.3362, 0.038, 1.77211),            // 450 nm
    (0.3187, 0.048, 1.7441),             // 455 nm
    (0.2908, 0.06, 1.6692),              // 460 nm
    (0.2511, 0.0739, 1.5281),            // 465 nm
    (0.19536, 0.09098, 1.28764),         // 470 nm
    (0.1421, 0.1126, 1.0419),            // 475 nm
    (0.09564, 0.13902, 0.8129501),       // 480 nm
    (0.05795001, 0.1693, 0.6162),        // 485 nm
    (0.03201, 0.20802, 0.46518),         // 490 nm
    (0.0147, 0.2586, 0.3533),            // 495 nm
    (0.0049, 0.323, 0.272),              // 500 nm
    (0.0024, 0.4073, 0.2123),            // 505 nm
    (0.0093, 0.503, 0.1582),             // 510 nm
    (0.0291, 0.6082, 0.1117),            // 515 nm
    (0.06327, 0.71, 0.07824999),         // 520 nm
    (0.1096, 0.7932, 0.05725001),        // 525 nm
    (0.1655, 0.862, 0.04216),            // 530 nm
    (0.2257499, 0.9148501, 0.02984),     // 535 nm
    (0.2904, 0.954, 0.0203),             // 540 nm
    (0.3597, 0.9803, 0.0134),            // 545 nm
    (0.4334499, 0.9949501, 0.008749999), // 550 nm
    (0.5120501, 1.0, 0.005749999),       // 555 nm
    (0.5945, 0.995, 0.0039),             // 560 nm
    (0.6784, 0.9786, 0.002749999),       // 565 nm
    (0.7621, 0.952, 0.0021),             // 570 nm
    (0.8425, 0.9154, 0.0018),            // 575 nm
    (0.9163, 0.87, 0.001650001),         // 580 nm
    (0.9786, 0.8163, 0.0014),            // 585 nm
    (1.0263, 0.757, 0.0011),             // 590 nm
    (1.0567, 0.6949, 0.001),             // 595 nm
    (1.0622, 0.631, 0.0008),             // 600 nm
    (1.0456, 0.5668, 0.0006),            // 605 nm
    (1.0026, 0.503, 0.00034),            // 610 nm
    (0.9384, 0.4412, 0.00024),           // 615 nm
    (0.8544499, 0.381, 0.00019),         // 620 nm
    (0.7514, 0.321, 0.0001),             // 625 nm
    (0.6424, 0.265, 4.999999e-05),       // 630 nm
    (0.5419, 0.217, 3e-05),              // 635 nm
    (0.4479, 0.175, 2e-05),              // 640 nm
    (0.3608, 0.1382, 1e-05),             // 645 nm
    (0.2835, 0.107, 0.0),                // 650 nm
    (0.2187, 0.0816, 0.0),               // 655 nm
    (0.1649, 0.061, 0.0),                // 660 nm
    (0.1212, 0.04458, 0.0),              // 665 nm
    (0.0874, 0.032, 0.0),                // 670 nm
    (0.0636, 0.0232, 0.0),               // 675 nm
    (0.04677, 0.017, 0.0),               // 680 nm
    (0.0329, 0.01192, 0.0),              // 685 nm
    (0.0227, 0.00821, 0.0),              // 690 nm
    (0.01584, 0.005723, 0.0),            // 695 nm
    (0.01135916, 0.004102, 0.0),         // 700 nm
    (0.008110916, 0.002929, 0.0),        // 705 nm
    (0.005790346, 0.002091, 0.0),        // 710 nm
    (0.004109457, 0.001484, 0.0),        // 715 nm
    (0.002899327, 0.001047, 0.0),        // 720 nm
    (0.00204919, 0.00074, 0.0),          // 725 nm
    (0.001439971, 0.00052, 0.0),         // 730 nm
    (0.0009999493, 0.0003611, 0.0),      // 735 nm
    (0.0006900786, 0.0002492, 0.0),      // 740 nm
    (0.0004760213, 0.0001719, 0.0),      // 745 nm
    (0.0003323011, 0.00012, 0.0),        // 750 nm
    (0.0002348261, 8.48e-05, 0.0),       // 755 nm
    (0.0001661505, 6e-05, 0.0),          // 760 nm
    (0.000117413, 4.24e-05, 0.0),        // 765 nm
    (8.307527e-05, 3e-05, 0.0),          // 770 nm
    (5.870652e-05, 2.12e-05, 0.0),       // 775 nm
    (4.150994e-05, 1.499e-05, 0.0),      // 780 nm
];

/// Tabulated CIE Standard Illuminant D65 relative spectral power distribution at 5 nm increments from 380 nm to 780 nm.
/// Source: ISO/CIE 11664-2:2014(E).
pub const CIE_D65_SPD_5NM: [f64; 81] = [
    49.9755, // 380 nm
    52.3118, // 385 nm
    54.6482, // 390 nm
    68.7015, // 395 nm
    82.7549, // 400 nm
    87.1204, // 405 nm
    91.486,  // 410 nm
    92.4589, // 415 nm
    93.4318, // 420 nm
    90.057,  // 425 nm
    86.6823, // 430 nm
    95.7736, // 435 nm
    104.865, // 440 nm
    110.936, // 445 nm
    117.008, // 450 nm
    117.41,  // 455 nm
    117.812, // 460 nm
    116.336, // 465 nm
    114.861, // 470 nm
    115.392, // 475 nm
    115.923, // 480 nm
    112.367, // 485 nm
    108.811, // 490 nm
    109.082, // 495 nm
    109.354, // 500 nm
    108.578, // 505 nm
    107.802, // 510 nm
    106.296, // 515 nm
    104.79,  // 520 nm
    106.239, // 525 nm
    107.689, // 530 nm
    106.047, // 535 nm
    104.405, // 540 nm
    104.225, // 545 nm
    104.046, // 550 nm
    102.023, // 555 nm
    100.0,   // 560 nm
    98.1671, // 565 nm
    96.3342, // 570 nm
    96.0611, // 575 nm
    95.788,  // 580 nm
    92.2368, // 585 nm
    88.6856, // 590 nm
    89.3459, // 595 nm
    90.0062, // 600 nm
    89.8026, // 605 nm
    89.5991, // 610 nm
    88.6489, // 615 nm
    87.6987, // 620 nm
    85.4936, // 625 nm
    83.2886, // 630 nm
    83.4939, // 635 nm
    83.6992, // 640 nm
    81.863,  // 645 nm
    80.0268, // 650 nm
    80.1207, // 655 nm
    80.2146, // 660 nm
    81.2462, // 665 nm
    82.2778, // 670 nm
    80.281,  // 675 nm
    78.2842, // 680 nm
    74.0027, // 685 nm
    69.7213, // 690 nm
    70.6652, // 695 nm
    71.6091, // 700 nm
    72.979,  // 705 nm
    74.349,  // 710 nm
    67.9765, // 715 nm
    61.604,  // 720 nm
    65.7448, // 725 nm
    69.8856, // 730 nm
    72.4863, // 735 nm
    75.087,  // 740 nm
    69.3398, // 745 nm
    63.5927, // 750 nm
    55.0054, // 755 nm
    46.4182, // 760 nm
    56.6118, // 765 nm
    66.8054, // 770 nm
    65.0941, // 775 nm
    63.3828, // 780 nm
];

/// Wavelength range start for tabulated data (nm).
pub const SPECTRUM_MIN_NM: f64 = 380.0;
/// Wavelength range end for tabulated data (nm).
pub const SPECTRUM_MAX_NM: f64 = 780.0;
/// Step size between tabulated samples (nm).
pub const SPECTRUM_STEP_NM: f64 = 5.0;

/// Interpolates CIE 1931 (x_bar, y_bar, z_bar) for any wavelength between 380 nm and 780 nm.
///
/// Returns `[0.0, 0.0, 0.0]` for wavelengths outside the visible band [380 nm, 780 nm].
#[inline]
pub fn cie_1931_cmf(wavelength_nm: f64) -> [f64; 3] {
    if !(SPECTRUM_MIN_NM..=SPECTRUM_MAX_NM).contains(&wavelength_nm) {
        return [0.0, 0.0, 0.0];
    }
    let offset = (wavelength_nm - SPECTRUM_MIN_NM) / SPECTRUM_STEP_NM;
    let idx = offset.floor() as usize;
    if idx >= CIE_1931_CMF_5NM.len() - 1 {
        let (x, y, z) = CIE_1931_CMF_5NM[CIE_1931_CMF_5NM.len() - 1];
        return [x, y, z];
    }
    let t = offset - idx as f64;
    let (x0, y0, z0) = CIE_1931_CMF_5NM[idx];
    let (x1, y1, z1) = CIE_1931_CMF_5NM[idx + 1];
    [x0 + t * (x1 - x0), y0 + t * (y1 - y0), z0 + t * (z1 - z0)]
}

/// Evaluates the relative spectral power distribution of CIE Standard Illuminant D65.
///
/// Returns 0.0 for wavelengths outside [380 nm, 780 nm].
#[inline]
pub fn d65_spd(wavelength_nm: f64) -> f64 {
    if !(SPECTRUM_MIN_NM..=SPECTRUM_MAX_NM).contains(&wavelength_nm) {
        return 0.0;
    }
    let offset = (wavelength_nm - SPECTRUM_MIN_NM) / SPECTRUM_STEP_NM;
    let idx = offset.floor() as usize;
    if idx >= CIE_D65_SPD_5NM.len() - 1 {
        return CIE_D65_SPD_5NM[CIE_D65_SPD_5NM.len() - 1];
    }
    let t = offset - idx as f64;
    let p0 = CIE_D65_SPD_5NM[idx];
    let p1 = CIE_D65_SPD_5NM[idx + 1];
    p0 + t * (p1 - p0)
}

/// Converts monochromatic radiance at a given wavelength to unnormalized CIE 1931 XYZ tristimulus values.
#[inline]
pub fn wavelength_to_xyz(wavelength_nm: f64, radiance: f64) -> [f64; 3] {
    let cmf = cie_1931_cmf(wavelength_nm);
    [cmf[0] * radiance, cmf[1] * radiance, cmf[2] * radiance]
}

/// Computes the normalization factor k = 1 / integral(D65 * y_bar * d_lambda)
/// such that illuminating an ideal white reflector with D65 produces Y = 1.0.
#[inline]
pub fn d65_normalization_factor() -> f64 {
    let mut sum_y = 0.0;
    for (spd, (_, y, _)) in CIE_D65_SPD_5NM.iter().zip(CIE_1931_CMF_5NM.iter()) {
        sum_y += spd * y * SPECTRUM_STEP_NM;
    }
    1.0 / sum_y
}

/// Converts CIE 1931 XYZ coordinates to linear sRGB components using the standard D65-adapted matrix.
///
/// Matrix specification: IEC 61966-2-1 / ITU-R BT.709.
#[inline]
pub fn xyz_to_linear_srgb(xyz: [f64; 3]) -> [f64; 3] {
    let x = xyz[0];
    let y = xyz[1];
    let z = xyz[2];
    [
        3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
        -0.9692660 * x + 1.8760108 * y + 0.0415560 * z,
        0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
    ]
}

/// Applies the exact piecewise IEC 61966-2-1 electro-optical transfer function (gamma curve) to a linear channel.
///
/// - For c <= 0.0031308: 12.92 * c
/// - For c > 0.0031308: 1.055 * c^(1/2.4) - 0.055
///
/// Negative inputs are clamped to 0.0 before conversion.
#[inline]
pub fn linear_to_srgb(linear: f64) -> f32 {
    if linear <= 0.0 {
        0.0
    } else if linear <= 0.0031308 {
        (12.92 * linear) as f32
    } else {
        (1.055 * linear.powf(1.0 / 2.4) - 0.055) as f32
    }
}

/// Applies the exact inverse IEC 61966-2-1 electro-optical transfer function.
#[inline]
pub fn srgb_to_linear(srgb: f64) -> f64 {
    if srgb <= 0.0 {
        0.0
    } else if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts CIE 1931 XYZ tristimulus values to non-linear sRGB in [0.0, 1.0].
#[inline]
pub fn xyz_to_srgb(xyz: [f64; 3]) -> [f32; 3] {
    let [r_lin, g_lin, b_lin] = xyz_to_linear_srgb(xyz);
    [
        linear_to_srgb(r_lin).clamp(0.0, 1.0),
        linear_to_srgb(g_lin).clamp(0.0, 1.0),
        linear_to_srgb(b_lin).clamp(0.0, 1.0),
    ]
}

/// Integrates a spectral power distribution sampled at arbitrary (wavelength_nm, power) points
/// over the visible spectrum [380, 780 nm], normalized against standard illuminant D65 (Y=1.0),
/// and converts to non-linear sRGB [0.0, 1.0].
pub fn spectrum_to_srgb(samples: &[(f64, f64)]) -> [f32; 3] {
    if samples.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let k = d65_normalization_factor();
    let mut x_acc = 0.0;
    let mut y_acc = 0.0;
    let mut z_acc = 0.0;

    if samples.len() == 1 {
        let (wl, p) = samples[0];
        let [x, y, z] = wavelength_to_xyz(wl, p);
        return xyz_to_srgb([x * k, y * k, z * k]);
    }

    // Trapezoidal integration between sample points
    for window in samples.windows(2) {
        let (wl0, p0) = window[0];
        let (wl1, p1) = window[1];
        if wl1 <= wl0 {
            continue;
        }
        let delta = wl1 - wl0;
        let [x0, y0, z0] = wavelength_to_xyz(wl0, p0);
        let [x1, y1, z1] = wavelength_to_xyz(wl1, p1);
        x_acc += 0.5 * (x0 + x1) * delta;
        y_acc += 0.5 * (y0 + y1) * delta;
        z_acc += 0.5 * (z0 + z1) * delta;
    }

    xyz_to_srgb([x_acc * k, y_acc * k, z_acc * k])
}

/// Converts a monochromatic spectral line to an 8-bit sRGB color [R, G, B] normalized for display.
///
/// # Note on Radiometry
/// This helper is intended for visual display and rendering UI rays only. It is **not**
/// radiometric: monochromatic lines at the spectral edges (e.g. 400 nm or 700 nm) have very low
/// luminous efficacy and would appear near-black in true radiometric units. This function normalizes
/// the peak linear color channel to 1.0 so every visible wavelength is vibrant and distinguishable.
pub fn display_color_for_wavelength(wavelength_nm: f64) -> [u8; 3] {
    let [x, y, z] = wavelength_to_xyz(wavelength_nm, 1.0);
    let [r_lin, g_lin, b_lin] = xyz_to_linear_srgb([x, y, z]);

    // Clamp negative out-of-gamut components to 0
    let r_clamped = r_lin.max(0.0);
    let g_clamped = g_lin.max(0.0);
    let b_clamped = b_lin.max(0.0);

    let max_val = r_clamped.max(g_clamped).max(b_clamped);
    if max_val <= 1e-9 {
        return [0, 0, 0];
    }

    // Normalize peak channel to 1.0 for vivid display
    let r_norm = r_clamped / max_val;
    let g_norm = g_clamped / max_val;
    let b_norm = b_clamped / max_val;

    [
        (linear_to_srgb(r_norm) * 255.0).round().clamp(0.0, 255.0) as u8,
        (linear_to_srgb(g_norm) * 255.0).round().clamp(0.0, 255.0) as u8,
        (linear_to_srgb(b_norm) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}
