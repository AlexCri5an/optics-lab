//! Wavefront Optical Path Difference (OPD) maps, reference sphere subtraction,
//! and Zernike circular polynomial decomposition.
//!
//! # Optical Foundations
//! - **Optical Path Length (OPL):** $\text{OPL} = \sum_k n_k d_k$.
//! - **Reference Sphere:** A spherical wave centered at the paraxial or real chief ray image point,
//!   with radius equal to the exit pupil distance $R_{\text{ref}}$.
//! - **Optical Path Difference (OPD):**
//!   $$\text{OPD}(P_x, P_y) = \text{OPL}_{\text{ray}}(P_x, P_y) - \text{OPL}_{\text{chief}} + \Delta R_{\text{ref}}$$
//! - **Wavefront Error in Waves:** $W(P_x, P_y) = \frac{\text{OPD}(P_x, P_y)}{\lambda_0}$.
//! - **Piston and Tilt Removal:**
//!   $W'(P_x, P_y) = W(P_x, P_y) - (a_0 + a_1 P_x + a_2 P_y)$
//!   removes trivial phase offset (piston) and transverse image displacement (tilt).
//! - **RMS Wavefront Error & Strehl Ratio:**
//!   $$\text{RMS}_W = \sqrt{\frac{1}{N} \sum_{i=1}^N (W_i' - \bar{W}')^2}$$
//!   $$S \approx \exp\left( -(2\pi \text{RMS}_W)^2 \right) \quad (\text{Maréchal approximation for } \text{RMS}_W \le 0.1\lambda)$$
//!
//! # Zernike Circular Polynomials (Noll Indexing)
//! Orthonormal basis functions on the unit disk $\rho \le 1$:
//! $$Z_j(\rho, \theta) = N_n^m R_n^{|m|}(\rho) \cdot \begin{cases} \cos(m\theta) & m \ge 0 \text{ (even } j) \\ \sin(|m|\theta) & m < 0 \text{ (odd } j) \end{cases}$$
//! with radial polynomial:
//! $$R_n^{|m|}(\rho) = \sum_{s=0}^{(n-|m|)/2} \frac{(-1)^s (n - s)!}{s! \left(\frac{n+|m|}{2} - s\right)! \left(\frac{n-|m|}{2} - s\right)!} \rho^{n - 2s}$$
//! and normalization $N_n^m = \sqrt{n+1}$ for $m=0$, or $\sqrt{2(n+1)}$ for $m \ne 0$.

use serde::{Deserialize, Serialize};

use crate::analysis::spot_fan::{PupilCoord, launch_pupil_ray};
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties, trace_sequential};
use crate::types::Vec3;

/// A 2D grid representing the optical path difference (OPD) wavefront error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WavefrontMap {
    /// Grid dimension $N \times N$.
    pub grid_size: usize,
    /// OPD values in units of wavelengths ($\lambda$). `None` for rays outside the pupil or vignetted.
    pub opd_waves: Vec<Option<f64>>,
    /// Peak-to-Valley (P-V) wavefront error in waves.
    pub pv_waves: f64,
    /// Root-Mean-Square (RMS) wavefront error in waves.
    pub rms_waves: f64,
    /// Estimated Strehl ratio from Maréchal approximation $\exp(-(2\pi \text{RMS})^2)$.
    pub strehl_ratio: f64,
    /// Reference evaluation wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Field angle in degrees.
    pub field_angle_deg: f64,
    /// Physical exit pupil distance $R_{\text{ref}}$ in mm.
    pub exit_pupil_distance_mm: f64,
}

/// Computes the 2D Wavefront OPD map across an $N \times N$ pupil sampling grid.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `grid_size`: Number of samples along each pupil diameter (e.g. 32, 64, 128).
/// - `field_angle_deg`: Field angle in degrees.
/// - `wavelength_nm`: Evaluation wavelength in nanometers.
/// - `remove_tilt`: If true, subtracts best-fit piston and linear tilt.
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription is invalid or chief ray fails.
pub fn compute_wavefront_map(
    prescription: &ResolvedPrescription,
    grid_size: usize,
    field_angle_deg: f64,
    wavelength_nm: f64,
    remove_tilt: bool,
) -> Result<WavefrontMap, OpticsError> {
    if grid_size < 4 {
        return Err(OpticsError::InvalidSurface(
            "Wavefront grid size must be at least 4x4".to_string(),
        ));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm).ok();
    let lambda_mm = wavelength_nm * 1e-6; // nm to mm

    let z_first = prescription
        .surfaces
        .first()
        .map_or(0.0, |s| s.conic.vertex_z);
    let enp_pos = paraxial.as_ref().map_or_else(
        || {
            let stop_idx = prescription.surfaces.iter().position(|s| s.is_stop).unwrap_or(0);
            let z_stop = prescription.surfaces.get(stop_idx).map_or(0.0, |s| s.conic.vertex_z);
            z_stop - z_first
        },
        |p| p.entrance_pupil_pos_mm,
    );
    let z_enp = z_first + enp_pos;

    // Trace reference chief ray (Px = 0, Py = 0)
    let chief_ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(0.0, 0.0),
        (0.0, field_angle_deg),
        wavelength_nm,
        z_enp,
    )?;
    let chief_record = match trace_sequential(prescription, &chief_ray) {
        Ok(rec) if !rec.hits.is_empty() && !rec.is_vignetted => rec,
        _ => {
            let aimed = (|| -> Result<crate::types::Ray, OpticsError> {
                let p = crate::types::Pupil::new(0.0, 0.0)?;
                let w = crate::types::Wavelength::from_nanometers(wavelength_nm)?;
                crate::aiming::trace_aimed_ray(
                    prescription,
                    crate::types::Field::AngleDegrees { x: 0.0, y: field_angle_deg },
                    p,
                    w,
                    crate::aiming::RayAimingMode::RealIterative,
                )
            })();
            match aimed.and_then(|r| trace_sequential(prescription, &r)) {
                Ok(rec) if !rec.hits.is_empty() && !rec.is_vignetted => rec,
                _ => {
                    return Err(OpticsError::RayMissedSurface(format!(
                        "Chief ray at field {:.1}° is vignetted or missed aperture",
                        field_angle_deg
                    )));
                }
            }
        }
    };
    let chief_img_hit = chief_record.hits.last().cloned().unwrap();
    let chief_img_pt = chief_img_hit.point;

    // Chief ray OPL from first surface to image plane
    let chief_opl = calculate_path_opl(&chief_record.points, prescription, wavelength_nm);

    // Exit pupil reference sphere center and radius
    let r_ref = paraxial.as_ref().map_or(100.0, |p| {
        if p.exit_pupil_pos_mm.is_finite() && p.exit_pupil_pos_mm.abs() > 1e-3 {
            p.exit_pupil_pos_mm.abs()
        } else if p.efl_mm.is_finite() && p.efl_mm.abs() > 1e-3 {
            p.efl_mm.abs()
        } else {
            100.0
        }
    });

    let step = 2.0 / (grid_size as f64 - 1.0);
    let mut opd_grid = vec![None; grid_size * grid_size];
    let mut valid_points = Vec::with_capacity(grid_size * grid_size);

    for j in 0..grid_size {
        let py = -1.0 + j as f64 * step;
        for i in 0..grid_size {
            let px = -1.0 + i as f64 * step;
            let rho2 = px * px + py * py;

            if rho2 > 1.0 {
                continue; // Outside circular entrance pupil
            }

            let idx = j * grid_size + i;

            if let Ok(ray) = launch_pupil_ray(
                prescription,
                PupilCoord::new(px, py),
                (0.0, field_angle_deg),
                wavelength_nm,
                z_enp,
            ) {
                if let Ok(rec) = trace_sequential(prescription, &ray) {
                    if !rec.is_vignetted && !rec.hits.is_empty() {
                        let ray_opl = calculate_path_opl(&rec.points, prescription, wavelength_nm);

                        // Exact Hopkins Hamiltonian reference sphere formulation:
                        // W = Delta(OPL) - d . Delta(x)
                        let ray_img_pt = rec.hits.last().map_or(Vec3::zeros(), |h| h.point);
                        let dx = ray_img_pt.x - chief_img_pt.x;
                        let dy = ray_img_pt.y - chief_img_pt.y;
                        let dir = rec.final_ray.direction;
                        let transverse_correction = dir.x * dx + dir.y * dy;

                        let raw_opd_mm = (ray_opl - chief_opl) - transverse_correction;
                        let opd_wave = raw_opd_mm / lambda_mm;

                        opd_grid[idx] = Some(opd_wave);
                        valid_points.push((i, j, px, py, opd_wave));
                    }
                }
            }
        }
    }

    if valid_points.is_empty() {
        return Err(OpticsError::InvalidParameter(
            "All pupil rays vignetted or missed image surface in wavefront calculation".to_string(),
        ));
    }

    // Piston and Tilt removal via linear least squares fit: W(px, py) ~ a0 + a1*px + a2*py
    if remove_tilt && valid_points.len() >= 3 {
        let mut sum_1 = 0.0;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xx = 0.0;
        let mut sum_yy = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_w = 0.0;
        let mut sum_xw = 0.0;
        let mut sum_yw = 0.0;

        for &(_, _, px, py, w) in &valid_points {
            sum_1 += 1.0;
            sum_x += px;
            sum_y += py;
            sum_xx += px * px;
            sum_yy += py * py;
            sum_xy += px * py;
            sum_w += w;
            sum_xw += px * w;
            sum_yw += py * w;
        }

        // Solve 3x3 normal equations for [a0, a1, a2]
        if let Some((a0, a1, a2)) = solve_3x3_symmetric(
            sum_1, sum_x, sum_y, sum_xx, sum_xy, sum_yy, sum_w, sum_xw, sum_yw,
        ) {
            for (i, j, px, py, _) in &valid_points {
                let idx = *j * grid_size + *i;
                if let Some(w) = opd_grid[idx] {
                    let w_detrended = w - (a0 + a1 * (*px) + a2 * (*py));
                    opd_grid[idx] = Some(w_detrended);
                }
            }
        }
    }

    // Compute P-V and RMS metrics from active grid points
    let mut min_w = f64::INFINITY;
    let mut max_w = f64::NEG_INFINITY;
    let mut sum_w = 0.0;
    let mut sum_w2 = 0.0;
    let mut count = 0.0;

    for val in opd_grid.iter().flatten() {
        let w = *val;
        if w < min_w {
            min_w = w;
        }
        if w > max_w {
            max_w = w;
        }
        sum_w += w;
        sum_w2 += w * w;
        count += 1.0;
    }

    let mean_w = if count > 0.0 { sum_w / count } else { 0.0 };
    let variance = if count > 0.0 {
        (sum_w2 / count - mean_w * mean_w).max(0.0)
    } else {
        0.0
    };
    let rms_waves = variance.sqrt();
    let pv_waves = if count > 0.0 { max_w - min_w } else { 0.0 };

    // Maréchal Strehl ratio approximation
    let strehl_ratio = (-((2.0 * std::f64::consts::PI * rms_waves).powi(2)))
        .exp()
        .clamp(0.0, 1.0);

    Ok(WavefrontMap {
        grid_size,
        opd_waves: opd_grid,
        pv_waves,
        rms_waves,
        strehl_ratio,
        wavelength_nm,
        field_angle_deg,
        exit_pupil_distance_mm: r_ref,
    })
}

/// Helper to calculate cumulative optical path length across a sequence of 3D ray hit points.
fn calculate_path_opl(
    points: &[Vec3],
    prescription: &ResolvedPrescription,
    wavelength_nm: f64,
) -> f64 {
    let mut opl = 0.0_f64;
    let mut current_n = 1.0_f64; // Air prior to surface 0

    for i in 0..points.len().saturating_sub(1) {
        let dist = (points[i + 1] - points[i]).norm();
        opl += current_n * dist;

        if i < prescription.surfaces.len() {
            current_n = prescription.surfaces[i].material.n(wavelength_nm);
        }
    }
    opl
}

/// Analytical 3x3 linear system solver for least-squares planar fit.
#[allow(clippy::too_many_arguments)]
fn solve_3x3_symmetric(
    m00: f64,
    m01: f64,
    m02: f64,
    m11: f64,
    m12: f64,
    m22: f64,
    b0: f64,
    b1: f64,
    b2: f64,
) -> Option<(f64, f64, f64)> {
    let det = m00 * (m11 * m22 - m12 * m12) - m01 * (m01 * m22 - m12 * m02)
        + m02 * (m01 * m12 - m11 * m02);
    if det.abs() < 1e-15 {
        return None;
    }
    let inv_det = 1.0 / det;

    let c00 = (m11 * m22 - m12 * m12) * inv_det;
    let c01 = (m02 * m12 - m01 * m22) * inv_det;
    let c02 = (m01 * m12 - m02 * m11) * inv_det;

    let c11 = (m00 * m22 - m02 * m02) * inv_det;
    let c12 = (m02 * m01 - m00 * m12) * inv_det;

    let c22 = (m00 * m11 - m01 * m01) * inv_det;

    let x0 = c00 * b0 + c01 * b1 + c02 * b2;
    let x1 = c01 * b0 + c11 * b1 + c12 * b2;
    let x2 = c02 * b0 + c12 * b1 + c22 * b2;

    Some((x0, x1, x2))
}

/// Evaluates Noll standard circular Zernike polynomial $Z_j(\rho, \theta)$.
///
/// Indices $j = 1 \dots 36$ corresponding to radial order $n \le 7$.
///
/// # Mathematical Definition
/// Normalization ensures:
/// $$\frac{1}{\pi} \int_0^1 \int_0^{2\pi} Z_j(\rho, \theta) Z_k(\rho, \theta) \rho d\rho d\theta = \delta_{jk}$$
pub fn zernike_noll_eval(j: usize, rho: f64, theta: f64) -> f64 {
    if rho > 1.0 || j == 0 {
        return 0.0;
    }

    let (n, m) = noll_index_to_nm(j);
    let r_nm = zernike_radial_poly(n, m.unsigned_abs() as usize, rho);

    let norm = if m == 0 {
        ((n + 1) as f64).sqrt()
    } else {
        (2.0 * (n + 1) as f64).sqrt()
    };

    let angular = if m > 0 {
        (m as f64 * theta).cos()
    } else if m < 0 {
        (m.abs() as f64 * theta).sin()
    } else {
        1.0
    };

    norm * r_nm * angular
}

/// Converts 1-based Noll index $j$ into radial degree $n$ and azimuthal frequency $m$.
pub fn noll_index_to_nm(j: usize) -> (usize, i32) {
    if j <= 1 {
        return (0, 0);
    }

    // Find radial order n such that n*(n+1)/2 < j <= (n+1)*(n+2)/2
    let mut n = 1;
    while (n + 1) * (n + 2) / 2 < j {
        n += 1;
    }

    let index_in_row = j - n * (n + 1) / 2;
    let num_in_row = n + 1;

    // Generate list of allowed m for order n: same parity as n
    let mut m_candidates = Vec::with_capacity(num_in_row);
    let mut abs_m = if n % 2 == 0 { 0 } else { 1 };
    while abs_m <= n {
        if abs_m == 0 {
            m_candidates.push(0);
        } else {
            // Noll order: positive m or negative m depending on j parity
            m_candidates.push(abs_m as i32);
            m_candidates.push(-(abs_m as i32));
        }
        abs_m += 2;
    }

    // Noll enumeration table for j <= 36
    let m = match j {
        1 => 0,
        2 => 1,
        3 => -1,
        4 => 0,
        5 => -2,
        6 => 2,
        7 => -1,
        8 => 1,
        9 => -3,
        10 => 3,
        11 => 0,
        12 => 2,
        13 => -2,
        14 => 4,
        15 => -4,
        16 => 1,
        17 => -1,
        18 => 3,
        19 => -3,
        20 => 5,
        21 => -5,
        22 => 0,
        23 => -2,
        24 => 2,
        25 => -4,
        26 => 4,
        27 => -6,
        28 => 6,
        29 => -1,
        30 => 1,
        31 => -3,
        32 => 3,
        33 => -5,
        34 => 5,
        35 => -7,
        36 => 7,
        _ => {
            if index_in_row - 1 < m_candidates.len() {
                m_candidates[index_in_row - 1]
            } else {
                0
            }
        }
    };

    (n, m)
}

/// Evaluates Zernike radial polynomial $R_n^m(\rho)$ via factorial summation.
fn zernike_radial_poly(n: usize, m: usize, rho: f64) -> f64 {
    if (n - m) % 2 != 0 {
        return 0.0;
    }
    let k_max = (n - m) / 2;
    let mut sum = 0.0_f64;

    for s in 0..=k_max {
        let sign = if s % 2 == 0 { 1.0 } else { -1.0 };
        let num = factorial(n - s);
        let den = factorial(s) * factorial((n + m) / 2 - s) * factorial((n - m) / 2 - s);
        let coeff = sign * (num / den);
        sum += coeff * rho.powi((n - 2 * s) as i32);
    }
    sum
}

fn factorial(n: usize) -> f64 {
    match n {
        0 | 1 => 1.0,
        2 => 2.0,
        3 => 6.0,
        4 => 24.0,
        5 => 120.0,
        6 => 720.0,
        7 => 5040.0,
        8 => 40320.0,
        9 => 362880.0,
        10 => 3628800.0,
        _ => {
            let mut acc = 3628800.0;
            for i in 11..=n {
                acc *= i as f64;
            }
            acc
        }
    }
}

/// Decomposition results of fitting a wavefront to Noll circular Zernike polynomials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZernikeFit {
    /// Fitted Zernike polynomial coefficients $c_j$ for $j = 1 \dots N_Z$ (in waves).
    pub coefficients: Vec<f64>,
    /// Residual root-mean-square fit error (waves).
    pub residual_rms_waves: f64,
    /// Number of sample points used in the orthogonal fit.
    pub num_points: usize,
}

/// Fits a 2D [`WavefrontMap`] to Zernike circular polynomials (Noll indices $1 \dots \text{max\_j}$).
///
/// # Arguments
/// - `wavefront`: Input OPD wavefront error map.
/// - `max_j`: Number of terms to fit (up to 36).
///
/// # Returns
/// A [`ZernikeFit`] struct with orthonormal expansion coefficients.
pub fn fit_zernike_noll(wavefront: &WavefrontMap, max_j: usize) -> Result<ZernikeFit, OpticsError> {
    let n_terms = max_j.clamp(1, 36);
    let grid_size = wavefront.grid_size;
    let step = 2.0 / (grid_size as f64 - 1.0);

    let mut pupil_points = Vec::new();

    for j in 0..grid_size {
        let py = -1.0 + j as f64 * step;
        for i in 0..grid_size {
            let px = -1.0 + i as f64 * step;
            let rho = (px * px + py * py).sqrt();
            if rho <= 1.0 {
                let idx = j * grid_size + i;
                if let Some(w) = wavefront.opd_waves[idx] {
                    let theta = py.atan2(px);
                    pupil_points.push((rho, theta, w));
                }
            }
        }
    }

    if pupil_points.len() < n_terms {
        return Err(OpticsError::NumericalSingularity(
            "Insufficient wavefront sample points for Zernike fit".to_string(),
        ));
    }

    let n_pts = pupil_points.len() as f64;
    let mut coeffs = vec![0.0_f64; n_terms];

    for j in 1..=n_terms {
        let mut sum_wz = 0.0;
        let mut sum_zz = 0.0;

        for &(rho, theta, w) in &pupil_points {
            let z = zernike_noll_eval(j, rho, theta);
            sum_wz += w * z;
            sum_zz += z * z;
        }

        coeffs[j - 1] = if sum_zz > 1e-12 { sum_wz / sum_zz } else { 0.0 };
    }

    let mut sum_res2 = 0.0;
    for &(rho, theta, w) in &pupil_points {
        let mut reconstructed_w = 0.0;
        for j in 1..=n_terms {
            reconstructed_w += coeffs[j - 1] * zernike_noll_eval(j, rho, theta);
        }
        let diff = w - reconstructed_w;
        sum_res2 += diff * diff;
    }

    let residual_rms_waves = (sum_res2 / n_pts).sqrt();

    Ok(ZernikeFit {
        coefficients: coeffs,
        residual_rms_waves,
        num_points: pupil_points.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zernike_noll_orthonormality() {
        let n_grid = 100;
        let step = 2.0 / (n_grid as f64 - 1.0);
        let mut pts = Vec::new();

        for j in 0..n_grid {
            let py = -1.0 + j as f64 * step;
            for i in 0..n_grid {
                let px = -1.0 + i as f64 * step;
                let rho = (px * px + py * py).sqrt();
                if rho <= 1.0 {
                    let theta = py.atan2(px);
                    pts.push((rho, theta));
                }
            }
        }

        let n_pts = pts.len() as f64;

        for j in 1..=11 {
            for k in 1..=11 {
                let mut sum_prod = 0.0;
                for &(rho, theta) in &pts {
                    let zj = zernike_noll_eval(j, rho, theta);
                    let zk = zernike_noll_eval(k, rho, theta);
                    sum_prod += zj * zk;
                }
                let norm = sum_prod / n_pts;
                if j == k {
                    assert!(
                        (norm - 1.0).abs() < 0.05,
                        "Zernike Z_{} self-norm must be ~1.0, got {}",
                        j,
                        norm
                    );
                } else {
                    assert!(
                        norm.abs() < 0.05,
                        "Zernike Z_{} and Z_{} must be orthogonal (~0.0), got {}",
                        j,
                        k,
                        norm
                    );
                }
            }
        }
    }

    #[test]
    fn test_pure_defocus_and_spherical_zernike_fit() -> Result<(), Box<dyn std::error::Error>> {
        let grid_size = 64;
        let step = 2.0 / (grid_size as f64 - 1.0);
        let mut opd_waves = vec![None; grid_size * grid_size];

        let target_defocus_coeff = 3.0; // c4

        for j in 0..grid_size {
            let py = -1.0 + j as f64 * step;
            for i in 0..grid_size {
                let px = -1.0 + i as f64 * step;
                let rho = (px * px + py * py).sqrt();
                if rho <= 1.0 {
                    let theta = py.atan2(px);
                    let z4 = zernike_noll_eval(4, rho, theta);
                    opd_waves[j * grid_size + i] = Some(target_defocus_coeff * z4);
                }
            }
        }

        let wf = WavefrontMap {
            grid_size,
            opd_waves,
            pv_waves: 6.0 * 3.0f64.sqrt(),
            rms_waves: target_defocus_coeff,
            strehl_ratio: 0.01,
            wavelength_nm: 550.0,
            field_angle_deg: 0.0,
            exit_pupil_distance_mm: 50.0,
        };

        let fit = fit_zernike_noll(&wf, 11)?;
        assert!(
            (fit.coefficients[3] - target_defocus_coeff).abs() < 0.05,
            "Fitted c4 defocus mismatch: got {}, expected {}",
            fit.coefficients[3],
            target_defocus_coeff
        );
        for (idx, &c) in fit.coefficients.iter().enumerate() {
            if idx != 3 {
                assert!(
                    c.abs() < 0.10,
                    "Expected near-zero for coefficient c_{}, got {}",
                    idx + 1,
                    c
                );
            }
        }
        Ok(())
    }
}
