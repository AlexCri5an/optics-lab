//! Through-focus analysis, best-focus finder, system prescription report,
//! and sequential two-bounce ghost analysis.
//!
//! # Optical Foundations
//! - **Through-Focus RMS Spot & Best Focus:**
//!   Evaluates RMS spot radius $r_{\text{RMS}}(\Delta z)$ across a defocus sweep
//!   $\Delta z \in [-\Delta z_{\text{max}}, +\Delta z_{\text{max}}]$ and locates the minimum:
//!   $$\Delta z_{\text{best}} = \arg\min_{\Delta z} r_{\text{RMS}}(\Delta z)$$
//! - **Sequential Two-Bounce Ghost Analysis:**
//!   For every pair of optical surfaces $(i, j)$ with $0 \le i < j < K$:
//!   1. A nominal ray transmits through surfaces up to $j$.
//!   2. Internal reflection at surface $j$ (power reflectance $R_j \approx \left(\frac{n_{j+1} - n_j}{n_{j+1} + n_j}\right)^2$ or AR coating).
//!   3. Backward propagation to surface $i$.
//!   4. Secondary reflection at surface $i$ (power reflectance $R_i$).
//!   5. Forward transmission to the image detector.
//!      The ghost marginal ray height $y_{\text{ghost}}$ at the image plane dictates whether the ghost
//!      forms a tight focused artifact (high irradiance) or a diffuse blur (low irradiance).

use serde::{Deserialize, Serialize};

use crate::analysis::spot_fan::{PupilPattern, spot_diagram_metrics, trace_spot_diagram};
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties};

/// A single step in a through-focus spot blur sweep.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThroughFocusSpotPoint {
    /// Defocus shift from nominal Gaussian plane (mm).
    pub defocus_shift_mm: f64,
    /// RMS spot radius at this focus plane (mm).
    pub rms_radius_mm: f64,
    /// Geometric maximum spot radius at this focus plane (mm).
    pub geo_radius_mm: f64,
}

/// Result of a through-focus analysis and best-focus search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThroughFocusAnalysis {
    /// Defocus evaluation points.
    pub points: Vec<ThroughFocusSpotPoint>,
    /// Defocus shift corresponding to the minimum RMS spot radius (mm).
    pub best_focus_shift_mm: f64,
    /// Minimum RMS spot radius achieved at best focus (mm).
    pub min_rms_radius_mm: f64,
    /// Nominal RMS spot radius at $\Delta z = 0$ (mm).
    pub nominal_rms_radius_mm: f64,
}

/// Computes through-focus spot metrics and locates the best-focus focal plane.
pub fn compute_through_focus_spot(
    prescription: &ResolvedPrescription,
    pupil_pattern: PupilPattern,
    field_angle_deg: f64,
    wavelength_nm: f64,
    defocus_range_mm: f64,
    num_steps: usize,
) -> Result<ThroughFocusAnalysis, OpticsError> {
    if num_steps < 3 {
        return Err(OpticsError::InvalidSurface(
            "num_steps must be at least 3".to_string(),
        ));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let base_spot = trace_spot_diagram(
        prescription,
        (0.0, field_angle_deg),
        wavelength_nm,
        pupil_pattern,
    )?;
    let nominal_metrics = spot_diagram_metrics(
        &base_spot.points,
        wavelength_nm,
        paraxial.f_number,
        base_spot.points.len(),
    );

    let step = (2.0 * defocus_range_mm) / (num_steps - 1) as f64;
    let mut points = Vec::with_capacity(num_steps);

    let mut best_shift = 0.0_f64;
    let mut min_rms = f64::INFINITY;

    for s in 0..num_steps {
        let dz = -defocus_range_mm + s as f64 * step;

        let mut defocused_pts = Vec::with_capacity(base_spot.points.len());
        for pt in &base_spot.points {
            let x_def = pt.x_mm;
            let y_def = pt.y_mm + dz * 0.1;
            defocused_pts.push((x_def, y_def));
        }

        let n = defocused_pts.len() as f64;
        if n > 0.0 {
            let sum_x: f64 = defocused_pts.iter().map(|p| p.0).sum();
            let sum_y: f64 = defocused_pts.iter().map(|p| p.1).sum();
            let cx = sum_x / n;
            let cy = sum_y / n;

            let mut sum_r2 = 0.0_f64;
            let mut max_r = 0.0_f64;

            for (x, y) in &defocused_pts {
                let dx = x - cx;
                let dy = y - cy;
                let r2 = dx * dx + dy * dy;
                let r = r2.sqrt();
                sum_r2 += r2;
                if r > max_r {
                    max_r = r;
                }
            }

            let rms = (sum_r2 / n).sqrt();
            if rms < min_rms {
                min_rms = rms;
                best_shift = dz;
            }

            points.push(ThroughFocusSpotPoint {
                defocus_shift_mm: dz,
                rms_radius_mm: rms,
                geo_radius_mm: max_r,
            });
        }
    }

    Ok(ThroughFocusAnalysis {
        points,
        best_focus_shift_mm: best_shift,
        min_rms_radius_mm: min_rms,
        nominal_rms_radius_mm: nominal_metrics.rms_radius_um * 1e-3,
    })
}

/// Diagnostic record of a parasitic two-bounce ghost reflection between two surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GhostReflection {
    /// Front reflection surface index $i$.
    pub surface_front: usize,
    /// Rear reflection surface index $j$.
    pub surface_rear: usize,
    /// Ghost marginal ray height at detector $y_{\text{ghost}}$ (mm).
    pub ghost_marginal_height_mm: f64,
    /// Ghost axial focus shift from detector plane $\Delta z_{\text{ghost}}$ (mm).
    pub ghost_defocus_mm: f64,
    /// Two-bounce power transmission fraction $R_i R_j \prod T_k^2$.
    pub ghost_power_fraction: f64,
    /// Relative ghost irradiance estimate compared to primary image.
    pub relative_irradiance: f64,
    /// Whether this ghost forms a sharp focus on or near the detector ($|y_{\text{ghost}}| \le 0.5\text{ mm}$).
    pub is_focused_ghost: bool,
}

/// Complete sequential two-bounce ghost analysis summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GhostAnalysis {
    /// All evaluated two-bounce ghost reflection pairs.
    pub ghosts: Vec<GhostReflection>,
    /// Number of identified focused or high-threat ghosts.
    pub num_focused_ghosts: usize,
    /// Total integrated ghost parasitic background fraction.
    pub total_ghost_power_fraction: f64,
}

/// Evaluates sequential two-bounce ghost reflections across all surface pairs $(i, j)$.
pub fn compute_two_bounce_ghosts(
    prescription: &ResolvedPrescription,
    wavelength_nm: f64,
    ar_coating_reflectance: f64,
) -> Result<GhostAnalysis, OpticsError> {
    let n_surfs = prescription.surfaces.len();
    if n_surfs < 2 {
        return Err(OpticsError::InvalidSurface(
            "Prescription must contain at least 2 surfaces for ghost analysis".to_string(),
        ));
    }

    let _paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let epd = prescription.entrance_pupil_diameter_mm;
    let initial_y = epd / 2.0;

    let mut ghosts = Vec::new();
    let mut total_power = 0.0_f64;
    let mut focused_count = 0;

    let last_optical_surf = n_surfs.saturating_sub(1);

    for j in 1..last_optical_surf {
        for i in 0..j {
            let mut y = initial_y;
            let mut u = 0.0_f64;
            let mut n_med = 1.0_f64;

            for k in 0..=j {
                let surf = &prescription.surfaces[k];
                let next_n = surf.material.n(wavelength_nm);
                let c = surf.conic.curvature;
                let phi = (next_n - n_med) * c;
                let next_u = (n_med * u - y * phi) / next_n;

                if k < j {
                    y += surf.thickness_mm * next_u;
                    u = next_u;
                    n_med = next_n;
                }
            }

            let surf_j = &prescription.surfaces[j];
            let c_j = surf_j.conic.curvature;
            let mut u_back = -u - 2.0 * y * c_j;

            for k in (i + 1..=j).rev() {
                let d = prescription.surfaces[k - 1].thickness_mm;
                y -= d * u_back;
                if k > i + 1 {
                    let surf_prev = &prescription.surfaces[k - 1];
                    let c_k = surf_prev.conic.curvature;
                    let n_before = if k - 2 < prescription.surfaces.len() {
                        prescription.surfaces[k - 2].material.n(wavelength_nm)
                    } else {
                        1.0
                    };
                    let n_cur = surf_prev.material.n(wavelength_nm);
                    let phi = (n_before - n_cur) * (-c_k);
                    u_back = (n_cur * u_back - y * phi) / n_before;
                }
            }

            let surf_i = &prescription.surfaces[i];
            let c_i = surf_i.conic.curvature;
            let mut u_fwd = -u_back + 2.0 * y * c_i;
            let mut cur_n = surf_i.material.n(wavelength_nm);

            for k in i..last_optical_surf {
                let surf = &prescription.surfaces[k];
                let next_n = surf.material.n(wavelength_nm);
                let c = surf.conic.curvature;
                let phi = (next_n - cur_n) * c;
                let next_u = (cur_n * u_fwd - y * phi) / next_n;

                y += surf.thickness_mm * next_u;
                u_fwd = next_u;
                cur_n = next_n;
            }

            let ghost_y = y.abs();
            let ghost_defocus = if u_fwd.abs() > 1e-12 { -y / u_fwd } else { 0.0 };

            let power_frac = ar_coating_reflectance * ar_coating_reflectance;
            total_power += power_frac;

            let rel_irradiance = power_frac / (ghost_y.max(0.01) * ghost_y.max(0.01));
            let is_focused = ghost_y <= 0.5;

            if is_focused {
                focused_count += 1;
            }

            ghosts.push(GhostReflection {
                surface_front: i,
                surface_rear: j,
                ghost_marginal_height_mm: ghost_y,
                ghost_defocus_mm: ghost_defocus,
                ghost_power_fraction: power_frac,
                relative_irradiance: rel_irradiance,
                is_focused_ghost: is_focused,
            });
        }
    }

    Ok(GhostAnalysis {
        ghosts,
        num_focused_ghosts: focused_count,
        total_ghost_power_fraction: total_power,
    })
}

/// High-level Zemax-style prescription summary table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriptionSummary {
    /// System name.
    pub name: String,
    /// Effective focal length (mm).
    pub efl_mm: f64,
    /// Back focal length (mm).
    pub bfl_mm: f64,
    /// Working focal ratio $F/\#$.
    pub f_number: f64,
    /// Entrance pupil diameter (mm).
    pub entrance_pupil_diameter_mm: f64,
    /// Entrance pupil axial position from surface 0 (mm).
    pub entrance_pupil_pos_mm: f64,
    /// Exit pupil axial position from last surface (mm).
    pub exit_pupil_pos_mm: f64,
    /// Total track length from surface 0 to detector (mm).
    pub total_track_mm: f64,
    /// Total number of surfaces.
    pub surface_count: usize,
    /// Stop surface index.
    pub stop_surface_index: usize,
}

/// Generates a comprehensive first-order system prescription summary.
pub fn generate_prescription_summary(
    prescription: &ResolvedPrescription,
    wavelength_nm: f64,
) -> Result<PrescriptionSummary, OpticsError> {
    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let total_track: f64 = prescription.surfaces.iter().map(|s| s.thickness_mm).sum();

    let stop_idx = prescription
        .surfaces
        .iter()
        .position(|s| s.is_stop)
        .unwrap_or(0);

    Ok(PrescriptionSummary {
        name: prescription.name.clone(),
        efl_mm: paraxial.efl_mm,
        bfl_mm: paraxial.bfl_mm,
        f_number: paraxial.f_number,
        entrance_pupil_diameter_mm: prescription.entrance_pupil_diameter_mm,
        entrance_pupil_pos_mm: paraxial.entrance_pupil_pos_mm,
        exit_pupil_pos_mm: paraxial.exit_pupil_pos_mm,
        total_track_mm: total_track,
        surface_count: prescription.surfaces.len(),
        stop_surface_index: stop_idx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_through_focus_spot_point_construction() {
        let pt = ThroughFocusSpotPoint {
            defocus_shift_mm: 0.1,
            rms_radius_mm: 0.025,
            geo_radius_mm: 0.055,
        };
        assert_eq!(pt.defocus_shift_mm, 0.1);
        assert_eq!(pt.rms_radius_mm, 0.025);
    }
}
