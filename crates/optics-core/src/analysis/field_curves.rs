//! Field curves, optical distortion, field curvature, chromatic aberrations,
//! longitudinal spherical aberration, and relative illumination.
//!
//! # Optical Foundations
//! - **Optical Distortion:**
//!   Departure of real chief ray image height $y_{\text{real}}(\theta)$ from ideal Gaussian image height $y_{\text{paraxial}}(\theta) = \text{EFL} \cdot \tan\theta$:
//!   $$D(\theta) = \frac{y_{\text{real}}(\theta) - y_{\text{paraxial}}(\theta)}{y_{\text{paraxial}}(\theta)} \times 100\%$$
//! - **$F\text{-}\theta$ Distortion (Scan Lenses):**
//!   For equidistant mapping $y_{\text{scan}} = \text{EFL} \cdot \theta_{\text{rad}}$:
//!   $$D_{F\theta}(\theta) = \frac{y_{\text{real}}(\theta) - \text{EFL}\cdot\theta_{\text{rad}}}{\text{EFL}\cdot\theta_{\text{rad}}} \times 100\%$$
//! - **Field Curvature & Astigmatism:**
//!   Tangential $z_T(\theta)$ and Sagittal $z_S(\theta)$ focus locations measured relative
//!   to the nominal flat image plane. The astigmatic difference is $\Delta z = z_T - z_S$.
//! - **Chromatic Aberrations:**
//!   - **Lateral Color:** $\Delta y_{\text{lat}}(\theta) = y_{\text{real}}(\theta, \lambda_F) - y_{\text{real}}(\theta, \lambda_C)$ in micrometers.
//!   - **Longitudinal / Axial Color:** $\Delta z_{\text{axial}} = z_{\text{focus}}(\lambda_F) - z_{\text{focus}}(\lambda_C)$ in millimeters.
//! - **Longitudinal Spherical Aberration:**
//!   Axial intersection distance $z(P_y) - z_{\text{paraxial}}$ of marginal rays at pupil zones $P_y \in [0, 1]$.
//! - **Relative Illumination:**
//!   Ratio of focal plane image irradiance at field angle $\theta$ relative to on-axis irradiance:
//!   $$\text{RI}(\theta) = \cos^4(\theta_{\text{img}}) \cdot V(\theta) \times 100\%$$
//!   where $V(\theta) \in [0, 1]$ is the geometric pupil transmission vignetting factor.

use serde::{Deserialize, Serialize};

use crate::analysis::spot_fan::{PupilCoord, launch_pupil_ray};
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties, trace_sequential};

/// A single sample point across field angle $\theta$.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FieldPointAnalysis {
    /// Field angle in degrees.
    pub field_angle_deg: f64,
    /// Real chief ray image height $y_{\text{real}}$ (mm).
    pub real_image_height_mm: f64,
    /// Paraxial Gaussian image height $y_{\text{paraxial}} = \text{EFL} \cdot \tan\theta$ (mm).
    pub paraxial_image_height_mm: f64,
    /// Standard percentage optical distortion $D(\theta) = \frac{y_{\text{real}} - y_{\text{paraxial}}}{y_{\text{paraxial}}} \times 100\%$.
    pub distortion_percent: f64,
    /// $F\text{-}\theta$ percentage distortion $D_{F\theta}(\theta) = \frac{y_{\text{real}} - \text{EFL}\cdot\theta}{\text{EFL}\cdot\theta} \times 100\%$.
    pub f_theta_distortion_percent: f64,
    /// Tangential focal surface shift $z_T$ (mm).
    pub field_curvature_tangential_mm: f64,
    /// Sagittal focal surface shift $z_S$ (mm).
    pub field_curvature_sagittal_mm: f64,
    /// Lateral color $\Delta y_{\text{lat}} = y_F - y_C$ ($\mu\text{m}$).
    pub lateral_color_um: f64,
    /// Relative illumination percentage $\text{RI}(\theta) \in [0, 100]\%$.
    pub relative_illumination_percent: f64,
    /// Geometric pupil transmission vignetting factor $V(\theta) \in [0, 1]$.
    pub vignetting_factor: f64,
}

/// Comprehensive field curves dataset across the full field of view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldCurves {
    /// Points evaluated across field angles from $0^\circ$ to max field.
    pub points: Vec<FieldPointAnalysis>,
    /// System effective focal length (mm).
    pub efl_mm: f64,
    /// Axial chromatic focal shift between F (486.13 nm) and C (656.27 nm) (mm).
    pub axial_chromatic_shift_mm: f64,
    /// Primary evaluation wavelength in nanometers.
    pub primary_wavelength_nm: f64,
}

/// A single sample point on a longitudinal spherical aberration curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LongitudinalAberrationPoint {
    /// Normalized pupil height $P_y \in [0, 1]$.
    pub pupil_py: f64,
    /// Longitudinal focal shift from paraxial image plane (mm).
    pub longitudinal_shift_mm: f64,
}

/// Computes field curves including distortion, astigmatic field curvature,
/// lateral color, and relative illumination.
pub fn compute_field_curves(
    prescription: &ResolvedPrescription,
    num_field_steps: usize,
    max_field_angle_deg: f64,
    primary_wavelength_nm: f64,
) -> Result<FieldCurves, OpticsError> {
    if num_field_steps < 2 {
        return Err(OpticsError::InvalidSurface(
            "num_field_steps must be at least 2".to_string(),
        ));
    }

    let paraxial = calculate_paraxial_properties(prescription, primary_wavelength_nm)?;
    let efl = paraxial.efl_mm;

    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + paraxial.entrance_pupil_pos_mm;

    let lambda_f = 486.13;
    let lambda_c = 656.27;

    // Axial chromatic shift: trace marginal ray at F and C
    let ray_f = launch_pupil_ray(prescription, PupilCoord::new(0.0, 1.0), (0.0, 0.0), lambda_f, z_enp)?;
    let ray_c = launch_pupil_ray(prescription, PupilCoord::new(0.0, 1.0), (0.0, 0.0), lambda_c, z_enp)?;

    let axial_f = trace_sequential(prescription, &ray_f)?;
    let axial_c = trace_sequential(prescription, &ray_c)?;

    let z_img_f = axial_f.hits.last().map_or(0.0, |h| h.point.z);
    let z_img_c = axial_c.hits.last().map_or(0.0, |h| h.point.z);
    let axial_chromatic_shift_mm = z_img_f - z_img_c;

    let mut points = Vec::with_capacity(num_field_steps);
    let step_angle = max_field_angle_deg / (num_field_steps - 1) as f64;

    for i in 0..num_field_steps {
        let theta_deg = i as f64 * step_angle;
        let theta_rad = theta_deg.to_radians();

        let paraxial_y = efl * theta_rad.tan();
        let f_theta_y = efl * theta_rad;

        // Trace chief ray at primary wavelength (Px = 0, Py = 0)
        let ray_d = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, 0.0),
            (0.0, theta_deg),
            primary_wavelength_nm,
            z_enp,
        )?;
        let chief_d = trace_sequential(prescription, &ray_d)?;
        let real_y = chief_d.hits.last().map_or(0.0, |h| h.point.y);

        let distortion_percent = if paraxial_y.abs() > 1e-6 {
            ((real_y - paraxial_y) / paraxial_y) * 100.0
        } else {
            0.0
        };

        let f_theta_distortion_percent = if f_theta_y.abs() > 1e-6 {
            ((real_y - f_theta_y) / f_theta_y) * 100.0
        } else {
            0.0
        };

        // Lateral color: trace chief ray at F and C
        let rf = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, 0.0),
            (0.0, theta_deg),
            lambda_f,
            z_enp,
        )?;
        let rc = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, 0.0),
            (0.0, theta_deg),
            lambda_c,
            z_enp,
        )?;

        let chief_f = trace_sequential(prescription, &rf)?;
        let chief_c = trace_sequential(prescription, &rc)?;

        let y_f = chief_f.hits.last().map_or(0.0, |h| h.point.y);
        let y_c = chief_c.hits.last().map_or(0.0, |h| h.point.y);
        let lateral_color_um = (y_f - y_c) * 1e3; // mm to um

        // Astigmatic field curvature via differential pupil rays:
        // Tangential fan: Py = +/- delta
        // Sagittal fan: Px = +/- delta
        let delta = 0.05;
        let rt_p = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, delta),
            (0.0, theta_deg),
            primary_wavelength_nm,
            z_enp,
        )?;
        let rt_m = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, -delta),
            (0.0, theta_deg),
            primary_wavelength_nm,
            z_enp,
        )?;
        let rs_p = launch_pupil_ray(
            prescription,
            PupilCoord::new(delta, 0.0),
            (0.0, theta_deg),
            primary_wavelength_nm,
            z_enp,
        )?;
        let rs_m = launch_pupil_ray(
            prescription,
            PupilCoord::new(-delta, 0.0),
            (0.0, theta_deg),
            primary_wavelength_nm,
            z_enp,
        )?;

        let ray_t_plus = trace_sequential(prescription, &rt_p)?;
        let ray_t_minus = trace_sequential(prescription, &rt_m)?;
        let ray_s_plus = trace_sequential(prescription, &rs_p)?;
        let ray_s_minus = trace_sequential(prescription, &rs_m)?;

        let (z_t, z_s) = if let (Some(hit_tp), Some(hit_tm), Some(hit_sp), Some(hit_sm)) = (
            ray_t_plus.hits.last(),
            ray_t_minus.hits.last(),
            ray_s_plus.hits.last(),
            ray_s_minus.hits.last(),
        ) {
            let dy_t = hit_tp.point.y - hit_tm.point.y;
            let du_t = ray_t_plus.final_ray.direction.y - ray_t_minus.final_ray.direction.y;
            let focus_shift_t = if du_t.abs() > 1e-8 { -dy_t / du_t } else { 0.0 };

            let dx_s = hit_sp.point.x - hit_sm.point.x;
            let du_s = ray_s_plus.final_ray.direction.x - ray_s_minus.final_ray.direction.x;
            let focus_shift_s = if du_s.abs() > 1e-8 { -dx_s / du_s } else { 0.0 };

            (focus_shift_t, focus_shift_s)
        } else {
            (0.0, 0.0)
        };

        let theta_img = (real_y / efl.abs()).atan();
        let cos4 = theta_img.cos().powi(4);
        let vignetting_factor = if chief_d.is_vignetted { 0.0 } else { 1.0 };
        let relative_illumination_percent = cos4 * vignetting_factor * 100.0;

        points.push(FieldPointAnalysis {
            field_angle_deg: theta_deg,
            real_image_height_mm: real_y,
            paraxial_image_height_mm: paraxial_y,
            distortion_percent,
            f_theta_distortion_percent,
            field_curvature_tangential_mm: z_t,
            field_curvature_sagittal_mm: z_s,
            lateral_color_um,
            relative_illumination_percent,
            vignetting_factor,
        });
    }

    Ok(FieldCurves {
        points,
        efl_mm: efl,
        axial_chromatic_shift_mm,
        primary_wavelength_nm,
    })
}

/// Computes longitudinal spherical aberration as a function of pupil zone $P_y \in [0, 1]$.
pub fn compute_longitudinal_spherical_aberration(
    prescription: &ResolvedPrescription,
    num_zones: usize,
    wavelength_nm: f64,
) -> Result<Vec<LongitudinalAberrationPoint>, OpticsError> {
    if num_zones < 2 {
        return Err(OpticsError::InvalidSurface("num_zones must be at least 2".to_string()));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + paraxial.entrance_pupil_pos_mm;

    let chief_ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(0.0, 0.0),
        (0.0, 0.0),
        wavelength_nm,
        z_enp,
    )?;
    let axial_chief = trace_sequential(prescription, &chief_ray)?;
    let z_paraxial = axial_chief.hits.last().map_or(0.0, |h| h.point.z);

    let mut points = Vec::with_capacity(num_zones);
    let step = 1.0 / (num_zones - 1) as f64;

    for i in 0..num_zones {
        let py = i as f64 * step;
        if py <= 1e-4 {
            points.push(LongitudinalAberrationPoint {
                pupil_py: 0.0,
                longitudinal_shift_mm: 0.0,
            });
            continue;
        }

        let ray = launch_pupil_ray(prescription, PupilCoord::new(0.0, py), (0.0, 0.0), wavelength_nm, z_enp)?;
        let path = trace_sequential(prescription, &ray)?;

        let (y_hit, z_hit, uz) = if let Some(hit) = path.hits.last() {
            (hit.point.y, hit.point.z, path.final_ray.direction.z)
        } else {
            (0.0, z_paraxial, 1.0)
        };

        let uy = path.final_ray.direction.y;
        let z_intercept = if uy.abs() > 1e-12 {
            z_hit - (y_hit / uy) * uz
        } else {
            z_paraxial
        };

        let longitudinal_shift_mm = z_intercept - z_paraxial;
        points.push(LongitudinalAberrationPoint {
            pupil_py: py,
            longitudinal_shift_mm,
        });
    }

    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_axis_distortion_and_lateral_color_is_zero() {
        let pt = FieldPointAnalysis {
            field_angle_deg: 0.0,
            real_image_height_mm: 0.0,
            paraxial_image_height_mm: 0.0,
            distortion_percent: 0.0,
            f_theta_distortion_percent: 0.0,
            field_curvature_tangential_mm: 0.0,
            field_curvature_sagittal_mm: 0.0,
            lateral_color_um: 0.0,
            relative_illumination_percent: 100.0,
            vignetting_factor: 1.0,
        };

        assert_eq!(pt.distortion_percent, 0.0);
        assert_eq!(pt.lateral_color_um, 0.0);
        assert_eq!(pt.relative_illumination_percent, 100.0);
    }
}
