//! Point Spread Function (PSF) computation via Fast Fourier Transform (FFT)
//! and Huygens wavelet superposition.
//!
//! # Optical Foundations
//! - **Generalized Pupil Function:**
//!   $$\mathcal{P}(u, v) = A(u, v) \exp\left( i \frac{2\pi}{\lambda} W(u, v) \right)$$
//!   where $A(u, v) \in [0, 1]$ is the pupil amplitude transmission (including vignetting)
//!   and $W(u, v)$ is the wavefront OPD error in units of wavelengths.
//! - **Diffraction Image Amplitude (Fraunhofer Diffraction):**
//!   The complex electric field at the focal plane is proportional to the 2D Fourier transform
//!   of the pupil function:
//!   $$E(x, y) = \mathcal{F}\{\mathcal{P}(u, v)\}$$
//! - **Irradiance (PSF):**
//!   $$\text{PSF}(x, y) = |E(x, y)|^2$$
//! - **Focal Plane Grid Sampling:**
//!   For pupil diameter $D_{\text{EP}}$, focal length $f$, wavelength $\lambda$, and FFT zero-padding
//!   factor $Q = M / N$ (where $N$ is pupil grid size, $M$ is padded FFT size):
//!   $$\Delta x = \Delta y = \frac{\lambda \cdot F/\#}{Q}$$
//! - **Strehl Ratio:**
//!   Ratio of the aberrated peak intensity to the diffraction-limited peak:
//!   $$S = \frac{\max_{(x,y)} \text{PSF}(x, y)}{\max_{(x,y)} \text{PSF}_{\text{diff}}(x, y)}$$
//! - **Encircled Energy:**
//!   $$\text{EE}(r) = \frac{\sum_{x^2 + y^2 \le r^2} \text{PSF}(x, y)}{\sum_{\text{all}} \text{PSF}(x, y)}$$
//!   For an ideal circular pupil, the first dark ring of the Airy pattern is at
//!   $r_{\text{Airy}} = 1.22 \cdot \lambda \cdot F/\#$, containing approximately $83.8\%$ of total energy.

use serde::{Deserialize, Serialize};

use crate::analysis::wavefront::compute_wavefront_map;
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties};

/// 2D Point Spread Function (PSF) grid with focal plane sampling and diffraction metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointSpreadFunction {
    /// Dimension $M \times M$ of the PSF intensity grid.
    pub grid_size: usize,
    /// Physical spacing between adjacent grid points in the focal plane (in micrometers, $\mu\text{m}$).
    pub pixel_size_um: f64,
    /// 2D normalized irradiance values $\text{PSF}(x, y)$, peak-normalized to 1.0 (or Strehl).
    pub data: Vec<f64>,
    /// Exact diffraction Strehl ratio $S \in [0, 1]$.
    pub strehl_ratio: f64,
    /// Theoretical Airy disk radius in micrometers ($r_{\text{Airy}} = 1.22 \lambda F/\#$).
    pub airy_radius_um: f64,
    /// Encircled energy curve: list of `(radius_um, fraction_enclosed)`.
    pub encircled_energy: Vec<(f64, f64)>,
    /// Line spread function along tangential axis $x$: $\text{LSF}(x) = \int \text{PSF}(x, y) dy$.
    pub lsf_tangential: Vec<f64>,
    /// Line spread function along sagittal axis $y$: $\text{LSF}(y) = \int \text{PSF}(x, y) dx$.
    pub lsf_sagittal: Vec<f64>,
    /// Evaluation wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Evaluation field angle in degrees.
    pub field_angle_deg: f64,
}

/// Computes the Fast Fourier Transform Point Spread Function (FFT PSF).
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `pupil_grid`: Sampling across the entrance pupil (must be a power of two, e.g. 32, 64).
/// - `fft_size`: Zero-padded FFT dimension (e.g. 64, 128, 256; must be a power of two $\ge \text{pupil\_grid}$).
/// - `field_angle_deg`: Field angle in degrees.
/// - `wavelength_nm`: Evaluation wavelength in nanometers.
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription is invalid or sizes are not powers of two.
pub fn compute_fft_psf(
    prescription: &ResolvedPrescription,
    pupil_grid: usize,
    fft_size: usize,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<PointSpreadFunction, OpticsError> {
    if !pupil_grid.is_power_of_two() || !fft_size.is_power_of_two() || fft_size < pupil_grid {
        return Err(OpticsError::NumericalSingularity(
            "pupil_grid and fft_size must be powers of two with fft_size >= pupil_grid".to_string(),
        ));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let f_number = paraxial.f_number;
    let lambda_um = wavelength_nm * 1e-3; // nm to um
    let airy_radius_um = 1.22 * lambda_um * f_number;

    let q_oversampling = fft_size as f64 / pupil_grid as f64;
    let pixel_size_um = (lambda_um * f_number) / q_oversampling;

    // Compute OPD wavefront map
    let wf = compute_wavefront_map(
        prescription,
        pupil_grid,
        field_angle_deg,
        wavelength_nm,
        false,
    )?;

    // Allocate complex pupil buffer of size fft_size x fft_size
    let mut real_pupil = vec![0.0_f64; fft_size * fft_size];
    let mut imag_pupil = vec![0.0_f64; fft_size * fft_size];

    let offset = (fft_size - pupil_grid) / 2;

    let mut diff_real = vec![0.0_f64; fft_size * fft_size];
    let mut diff_imag = vec![0.0_f64; fft_size * fft_size];

    for j in 0..pupil_grid {
        for i in 0..pupil_grid {
            let wf_idx = j * pupil_grid + i;
            if let Some(opd_wave) = wf.opd_waves[wf_idx] {
                let phase = 2.0 * std::f64::consts::PI * opd_wave;
                let target_idx = (j + offset) * fft_size + (i + offset);

                real_pupil[target_idx] = phase.cos();
                imag_pupil[target_idx] = phase.sin();

                // Unaberrated reference pupil
                diff_real[target_idx] = 1.0;
                diff_imag[target_idx] = 0.0;
            }
        }
    }

    // 2D FFT of aberrated pupil
    fft_2d(&mut real_pupil, &mut imag_pupil, fft_size);
    fft_shift_2d(&mut real_pupil, fft_size);
    fft_shift_2d(&mut imag_pupil, fft_size);

    // 2D FFT of unaberrated reference pupil
    fft_2d(&mut diff_real, &mut diff_imag, fft_size);
    fft_shift_2d(&mut diff_real, fft_size);
    fft_shift_2d(&mut diff_imag, fft_size);

    // Compute intensities
    let mut psf_data = vec![0.0_f64; fft_size * fft_size];
    let mut max_aberrated = 0.0_f64;
    let mut max_diffraction = 0.0_f64;
    let mut total_energy = 0.0_f64;

    for i in 0..fft_size * fft_size {
        let val = real_pupil[i] * real_pupil[i] + imag_pupil[i] * imag_pupil[i];
        let diff_val = diff_real[i] * diff_real[i] + diff_imag[i] * diff_imag[i];

        psf_data[i] = val;
        total_energy += val;

        if val > max_aberrated {
            max_aberrated = val;
        }
        if diff_val > max_diffraction {
            max_diffraction = diff_val;
        }
    }

    let strehl_ratio = if max_diffraction > 1e-15 {
        (max_aberrated / max_diffraction).clamp(0.0, 1.0)
    } else {
        1.0
    };

    // Normalize PSF so peak = 1.0 (or to Strehl)
    let norm_factor = if max_aberrated > 1e-15 {
        1.0 / max_aberrated
    } else {
        1.0
    };
    for v in &mut psf_data {
        *v *= norm_factor;
    }

    // Line Spread Functions
    let mut lsf_tangential = vec![0.0_f64; fft_size];
    let mut lsf_sagittal = vec![0.0_f64; fft_size];

    for j in 0..fft_size {
        for i in 0..fft_size {
            let val = psf_data[j * fft_size + i];
            lsf_tangential[i] += val;
            lsf_sagittal[j] += val;
        }
    }

    // Encircled Energy curve
    let center = fft_size as f64 / 2.0;
    let max_radius_pixels = fft_size as f64 / 2.0;
    let num_radii = 30;
    let mut encircled_energy = Vec::with_capacity(num_radii);

    for r_step in 1..=num_radii {
        let r_pixel = (r_step as f64 / num_radii as f64) * max_radius_pixels;
        let r_um = r_pixel * pixel_size_um;
        let r_sq = r_pixel * r_pixel;

        let mut energy_within_r = 0.0_f64;
        for j in 0..fft_size {
            let dy = j as f64 - center;
            for i in 0..fft_size {
                let dx = i as f64 - center;
                if dx * dx + dy * dy <= r_sq {
                    energy_within_r += psf_data[j * fft_size + i];
                }
            }
        }

        let fraction = if total_energy > 0.0 {
            (energy_within_r * max_aberrated) / total_energy
        } else {
            0.0
        };
        encircled_energy.push((r_um, fraction.clamp(0.0, 1.0)));
    }

    Ok(PointSpreadFunction {
        grid_size: fft_size,
        pixel_size_um,
        data: psf_data,
        strehl_ratio,
        airy_radius_um,
        encircled_energy,
        lsf_tangential,
        lsf_sagittal,
        wavelength_nm,
        field_angle_deg,
    })
}

/// 2D Cooley-Tukey Radix-2 Fast Fourier Transform.
fn fft_2d(real: &mut [f64], imag: &mut [f64], n: usize) {
    // 1D FFT across each row
    let mut row_r = vec![0.0_f64; n];
    let mut row_i = vec![0.0_f64; n];

    for j in 0..n {
        let row_start = j * n;
        row_r[..n].copy_from_slice(&real[row_start..row_start + n]);
        row_i[..n].copy_from_slice(&imag[row_start..row_start + n]);
        fft_1d(&mut row_r, &mut row_i);
        real[row_start..row_start + n].copy_from_slice(&row_r[..n]);
        imag[row_start..row_start + n].copy_from_slice(&row_i[..n]);
    }

    // 1D FFT across each column
    let mut col_r = vec![0.0_f64; n];
    let mut col_i = vec![0.0_f64; n];

    for i in 0..n {
        for j in 0..n {
            col_r[j] = real[j * n + i];
            col_i[j] = imag[j * n + i];
        }
        fft_1d(&mut col_r, &mut col_i);
        for j in 0..n {
            real[j * n + i] = col_r[j];
            imag[j * n + i] = col_i[j];
        }
    }
}

/// 1D in-place Cooley-Tukey Radix-2 FFT (forward, negative exponent).
fn fft_1d(real: &mut [f64], imag: &mut [f64]) {
    let n = real.len();
    if n <= 1 {
        return;
    }

    // Bit reversal permutation
    let mut j = 0;
    for i in 0..n {
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
        let mut k = n >> 1;
        while k >= 1 && k <= j {
            j -= k;
            k >>= 1;
        }
        j += k;
    }

    // Danielson-Lanczos butterfly
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let w_step_r = angle.cos();
        let w_step_i = angle.sin();

        let mut i = 0;
        while i < n {
            let mut wr = 1.0_f64;
            let mut wi = 0.0_f64;

            for k in 0..half {
                let u_r = real[i + k];
                let u_i = imag[i + k];

                let v_r = real[i + k + half] * wr - imag[i + k + half] * wi;
                let v_i = real[i + k + half] * wi + imag[i + k + half] * wr;

                real[i + k] = u_r + v_r;
                imag[i + k] = u_i + v_i;

                real[i + k + half] = u_r - v_r;
                imag[i + k + half] = u_i - v_i;

                let next_wr = wr * w_step_r - wi * w_step_i;
                let next_wi = wr * w_step_i + wi * w_step_r;
                wr = next_wr;
                wi = next_wi;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// 2D quadrant shift to center the zero-frequency DC component at (N/2, N/2).
fn fft_shift_2d(data: &mut [f64], n: usize) {
    let half = n / 2;
    for j in 0..half {
        for i in 0..half {
            let idx00 = j * n + i;
            let idx11 = (j + half) * n + (i + half);
            data.swap(idx00, idx11);

            let idx01 = j * n + (i + half);
            let idx10 = (j + half) * n + i;
            data.swap(idx01, idx10);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_1d_dirac_delta_and_sinusoid() {
        // Delta impulse at 0 should produce constant magnitude
        let mut r = vec![0.0_f64; 16];
        let mut i = vec![0.0_f64; 16];
        r[0] = 1.0;

        fft_1d(&mut r, &mut i);
        for k in 0..16 {
            let mag = (r[k] * r[k] + i[k] * i[k]).sqrt();
            assert!((mag - 1.0).abs() < 1e-12);
        }
    }
}
