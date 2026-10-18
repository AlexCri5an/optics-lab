//! Modulation Transfer Function (MTF) analysis: Diffraction FFT MTF,
//! Geometric spot MTF, and through-focus frequency sweeps.
//!
//! # Optical Foundations
//! - **Optical Transfer Function (OTF):**
//!   $$\mathcal{H}(\nu_x, \nu_y) = \text{MTF}(\nu_x, \nu_y) \cdot \exp\left( i \text{PTF}(\nu_x, \nu_y) \right) = \frac{\mathcal{F}\{\text{PSF}(x, y)\}}{\iint \text{PSF}(x, y) \, dx dy}$$
//!   where $\text{MTF}$ is the Modulation Transfer Function (contrast ratio) and $\text{PTF}$ is the Phase Transfer Function.
//! - **Diffraction Cutoff Frequency:**
//!   For an optical system with focal ratio $F/\#$ operating at wavelength $\lambda$:
//!   $$\nu_0 = \frac{1}{\lambda \cdot F/\#} \quad (\text{cycles/mm or line-pairs/mm})$$
//! - **Closed-Form Diffraction Limit for Circular Pupil:**
//!   At normalized spatial frequency $f = \nu / \nu_0 \in [0, 1]$:
//!   $$\text{MTF}_{\text{diff}}(f) = \frac{2}{\pi} \left[ \arccos(f) - f \sqrt{1 - f^2} \right]$$
//!   and $\text{MTF}_{\text{diff}}(f) = 0$ for $f > 1$.
//! - **Geometric MTF (from Spot Diagram):**
//!   Discrete Fourier sum of image ray intersection coordinates $(x_k, y_k)$:
//!   $$\mathcal{H}_{\text{geom}}(\nu_x, \nu_y) = \frac{1}{N} \sum_{k=1}^N \exp\left( -i 2\pi (\nu_x x_k + \nu_y y_k) \right)$$
//!   $$\text{MTF}_{\text{geom}} = |\mathcal{H}_{\text{geom}}|$$

use serde::{Deserialize, Serialize};

use crate::analysis::psf::compute_fft_psf;
use crate::analysis::spot_fan::{SpotDiagram, trace_spot_diagram};
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties};

/// A single point along an MTF frequency response curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MtfPoint {
    /// Spatial frequency in cycles per millimeter ($\text{lp/mm}$).
    pub spatial_frequency_lpm: f64,
    /// Tangential (meridional) modulation transfer factor in $[0, 1]$.
    pub tangential_mtf: f64,
    /// Sagittal modulation transfer factor in $[0, 1]$.
    pub sagittal_mtf: f64,
    /// Theoretical diffraction limit MTF for an unaberrated circular aperture.
    pub diffraction_limit_mtf: f64,
}

/// Complete MTF frequency response curve for a given field and wavelength.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MtfCurve {
    /// Spatial frequency points from 0 to cutoff $\nu_0$.
    pub points: Vec<MtfPoint>,
    /// Diffraction optical cutoff frequency $\nu_0 = 1 / (\lambda F/\#)$ in $\text{lp/mm}$.
    pub cutoff_frequency_lpm: f64,
    /// Evaluation wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Field angle in degrees.
    pub field_angle_deg: f64,
}

/// Evaluates the closed-form diffraction-limited MTF of an ideal circular aperture.
pub fn diffraction_limit_mtf(spatial_frequency_lpm: f64, cutoff_frequency_lpm: f64) -> f64 {
    if cutoff_frequency_lpm <= 1e-15 || spatial_frequency_lpm <= 0.0 {
        return 1.0;
    }
    let f = spatial_frequency_lpm / cutoff_frequency_lpm;
    if f >= 1.0 {
        return 0.0;
    }
    (2.0 / std::f64::consts::PI) * (f.acos() - f * (1.0 - f * f).sqrt())
}

/// Computes the diffraction MTF curve via 2D Fast Fourier Transform of the PSF.
pub fn compute_diffraction_mtf(
    prescription: &ResolvedPrescription,
    num_frequencies: usize,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<MtfCurve, OpticsError> {
    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm).ok();
    let f_number = paraxial
        .as_ref()
        .map(|p| p.f_number)
        .filter(|&f| f.is_finite() && f > 0.05)
        .unwrap_or(5.0);
    let lambda_mm = wavelength_nm * 1e-6; // nm to mm
    let cutoff_frequency_lpm = (1.0 / (lambda_mm * f_number)).clamp(5.0, 5000.0);

    let psf = compute_fft_psf(prescription, 64, 128, field_angle_deg, wavelength_nm)?;
    let n = psf.grid_size;
    let dx_mm = psf.pixel_size_um * 1e-3;

    let total_psf: f64 = psf.data.iter().sum();
    let norm = if total_psf > 1e-15 {
        1.0 / total_psf
    } else {
        1.0
    };

    let center = n / 2;
    let mut points = Vec::with_capacity(num_frequencies);

    for step in 0..num_frequencies {
        let freq = (step as f64 / (num_frequencies - 1).max(1) as f64) * cutoff_frequency_lpm;
        let diff_limit = diffraction_limit_mtf(freq, cutoff_frequency_lpm);

        if freq <= 0.0 {
            points.push(MtfPoint {
                spatial_frequency_lpm: 0.0,
                tangential_mtf: 1.0,
                sagittal_mtf: 1.0,
                diffraction_limit_mtf: 1.0,
            });
            continue;
        }

        let mut t_re = 0.0_f64;
        let mut t_im = 0.0_f64;

        for (i, &val) in psf.lsf_tangential.iter().enumerate() {
            let x_mm = (i as f64 - center as f64) * dx_mm;
            let phase = 2.0 * std::f64::consts::PI * freq * x_mm;
            t_re += val * phase.cos();
            t_im += val * phase.sin();
        }

        let tangential_mtf = ((t_re * t_re + t_im * t_im).sqrt() * norm).clamp(0.0, 1.0);

        let mut s_re = 0.0_f64;
        let mut s_im = 0.0_f64;

        for (j, &val) in psf.lsf_sagittal.iter().enumerate() {
            let y_mm = (j as f64 - center as f64) * dx_mm;
            let phase = 2.0 * std::f64::consts::PI * freq * y_mm;
            s_re += val * phase.cos();
            s_im += val * phase.sin();
        }

        let sagittal_mtf = ((s_re * s_re + s_im * s_im).sqrt() * norm).clamp(0.0, 1.0);

        points.push(MtfPoint {
            spatial_frequency_lpm: freq,
            tangential_mtf,
            sagittal_mtf,
            diffraction_limit_mtf: diff_limit,
        });
    }

    Ok(MtfCurve {
        points,
        cutoff_frequency_lpm,
        wavelength_nm,
        field_angle_deg,
    })
}

/// Computes the Geometric MTF directly from a real-ray [`SpotDiagram`].
pub fn compute_geometric_mtf(
    spot: &SpotDiagram,
    max_frequency_lpm: f64,
    num_frequencies: usize,
) -> Vec<MtfPoint> {
    let n_pts = spot.points.len() as f64;
    if n_pts == 0.0 || num_frequencies == 0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(num_frequencies);

    for step in 0..num_frequencies {
        let freq = (step as f64 / (num_frequencies - 1).max(1) as f64) * max_frequency_lpm;

        let mut t_re = 0.0_f64;
        let mut t_im = 0.0_f64;
        let mut s_re = 0.0_f64;
        let mut s_im = 0.0_f64;

        for pt in &spot.points {
            let dx = pt.x_mm - spot.metrics.centroid_x_mm;
            let dy = pt.y_mm - spot.metrics.centroid_y_mm;

            // Tangential (nu_x = 0, nu_y = freq)
            let phase_t = 2.0 * std::f64::consts::PI * freq * dy;
            t_re += phase_t.cos();
            t_im += phase_t.sin();

            // Sagittal (nu_x = freq, nu_y = 0)
            let phase_s = 2.0 * std::f64::consts::PI * freq * dx;
            s_re += phase_s.cos();
            s_im += phase_s.sin();
        }

        let tangential_mtf = ((t_re * t_re + t_im * t_im).sqrt() / n_pts).clamp(0.0, 1.0);
        let sagittal_mtf = ((s_re * s_re + s_im * s_im).sqrt() / n_pts).clamp(0.0, 1.0);

        result.push(MtfPoint {
            spatial_frequency_lpm: freq,
            tangential_mtf,
            sagittal_mtf,
            diffraction_limit_mtf: 1.0,
        });
    }

    result
}

/// A through-focus MTF evaluation point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThroughFocusMtfPoint {
    /// Defocus shift from nominal Gaussian image plane (mm).
    pub defocus_shift_mm: f64,
    /// Tangential MTF at this defocus.
    pub tangential_mtf: f64,
    /// Sagittal MTF at this defocus.
    pub sagittal_mtf: f64,
}

/// Evaluates through-focus Geometric MTF at a fixed target spatial frequency.
pub fn compute_through_focus_geometric_mtf(
    prescription: &ResolvedPrescription,
    field_angle_deg: f64,
    wavelength_nm: f64,
    target_frequency_lpm: f64,
    defocus_range_mm: f64,
    num_focus_steps: usize,
) -> Result<Vec<ThroughFocusMtfPoint>, OpticsError> {
    let base_spot = trace_spot_diagram(
        prescription,
        (0.0, field_angle_deg),
        wavelength_nm,
        crate::analysis::spot_fan::PupilPattern::Hexapolar { num_rings: 8 },
    )?;

    let mut result = Vec::with_capacity(num_focus_steps);
    let step_size = (2.0 * defocus_range_mm) / (num_focus_steps.saturating_sub(1).max(1) as f64);

    for s in 0..num_focus_steps {
        let dz = -defocus_range_mm + s as f64 * step_size;

        let mut t_re = 0.0_f64;
        let mut t_im = 0.0_f64;
        let mut s_re = 0.0_f64;
        let mut s_im = 0.0_f64;
        let n_pts = base_spot.points.len() as f64;

        for pt in &base_spot.points {
            let x_defocused = pt.x_mm - base_spot.metrics.centroid_x_mm;
            let y_defocused = (pt.y_mm - base_spot.metrics.centroid_y_mm) + dz * 0.1;

            let phase_t = 2.0 * std::f64::consts::PI * target_frequency_lpm * y_defocused;
            t_re += phase_t.cos();
            t_im += phase_t.sin();

            let phase_s = 2.0 * std::f64::consts::PI * target_frequency_lpm * x_defocused;
            s_re += phase_s.cos();
            s_im += phase_s.sin();
        }

        let tangential_mtf = ((t_re * t_re + t_im * t_im).sqrt() / n_pts).clamp(0.0, 1.0);
        let sagittal_mtf = ((s_re * s_re + s_im * s_im).sqrt() / n_pts).clamp(0.0, 1.0);

        result.push(ThroughFocusMtfPoint {
            defocus_shift_mm: dz,
            tangential_mtf,
            sagittal_mtf,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffraction_limit_mtf_values() {
        let cutoff = 1000.0; // lp/mm

        assert!((diffraction_limit_mtf(0.0, cutoff) - 1.0).abs() < 1e-12);
        assert!((diffraction_limit_mtf(cutoff, cutoff) - 0.0).abs() < 1e-12);

        let mtf_half = diffraction_limit_mtf(500.0, cutoff);
        let expected_half = (2.0 / 3.0) - (3.0f64.sqrt() / (2.0 * std::f64::consts::PI));
        assert!(
            (mtf_half - expected_half).abs() < 1e-5,
            "MTF at half cutoff mismatch: got {}, expected {}",
            mtf_half,
            expected_half
        );
    }
}
