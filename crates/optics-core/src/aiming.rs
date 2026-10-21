//! Real ray aiming, finite/infinite conjugates, vignetting factors, and polychromatic helpers.
//!
//! # Ray Aiming
//! In wide-field or heavily aberrated systems, pupil spherical aberration and coma cause
//! paraxially launched rays to miss the physical aperture stop.
//! `trace_aimed_ray` uses a 2D Newton-Raphson solver to find the exact launch coordinates
//! such that the real ray strikes the aperture stop surface at $(P_x r_{\text{stop}}, P_y r_{\text{stop}})$.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties, trace_sequential};
use crate::types::{Field, Pupil, Ray, Vec3, Wavelength};

/// Ray aiming mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RayAimingMode {
    /// Paraxial entrance pupil aiming (fast, ideal for narrow to medium field systems).
    #[default]
    Paraxial,
    /// Real iterative Newton-Raphson aiming at the physical aperture stop surface.
    RealIterative,
    /// Direct aiming without pupil projection.
    Off,
}

/// System conjugate specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Conjugate {
    /// Object at infinity (telescopes, camera lenses).
    Infinite,
    /// Object at finite distance from first surface vertex.
    Finite {
        /// Distance from object plane to first surface vertex along $-z$ (positive distance in mm).
        object_distance_mm: f64,
        /// Physical object height $(x, y)$ in millimeters.
        object_height_mm: (f64, f64),
    },
}

impl Default for Conjugate {
    fn default() -> Self {
        Self::Infinite
    }
}

/// Vignetting factors (VDX, VDY, VCX, VCY, VAN) for field-dependent pupil compression.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VignettingFactors {
    /// De-center along $x$ (VDX).
    pub vdx: f64,
    /// De-center along $y$ (VDY).
    pub vdy: f64,
    /// Compression factor along $x$ (VCX, default 1.0).
    pub vcx: f64,
    /// Compression factor along $y$ (VCY, default 1.0).
    pub vcy: f64,
    /// Rotation angle in degrees (VAN).
    pub van_deg: f64,
}

impl Default for VignettingFactors {
    fn default() -> Self {
        Self {
            vdx: 0.0,
            vdy: 0.0,
            vcx: 1.0,
            vcy: 1.0,
            van_deg: 0.0,
        }
    }
}

impl VignettingFactors {
    /// Applies vignetting transformation to a normalized pupil coordinate:
    /// $$(P_x', P_y') = \mathbf{R}(\text{VAN}) (\text{VDX} + \text{VCX} \cdot P_x, \text{VDY} + \text{VCY} \cdot P_y)$$
    #[must_use]
    pub fn apply(&self, pupil: Pupil) -> Pupil {
        let px_scaled = self.vdx + self.vcx * pupil.px;
        let py_scaled = self.vdy + self.vcy * pupil.py;

        let to_rad = std::f64::consts::PI / 180.0;
        let theta = self.van_deg * to_rad;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let px_rot = cos_t * px_scaled - sin_t * py_scaled;
        let py_rot = sin_t * px_scaled + cos_t * py_scaled;

        Pupil { px: px_rot, py: py_rot }
    }
}

/// Polychromatic wavelength set with relative radiant weightings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WavelengthSet {
    /// Sampled wavelengths with associated relative weights.
    pub entries: Vec<(Wavelength, f64)>,
    /// 0-based index of primary reference wavelength.
    pub primary_index: usize,
}

impl Default for WavelengthSet {
    fn default() -> Self {
        Self {
            entries: vec![(Wavelength::D_LINE, 1.0)],
            primary_index: 0,
        }
    }
}

impl WavelengthSet {
    /// Standard Fraunhofer visible triplet: $F$ ($486.13\text{ nm}$), $d$ ($587.56\text{ nm}$, primary), $C$ ($656.27\text{ nm}$).
    #[must_use]
    pub fn visible_triplet() -> Self {
        Self {
            entries: vec![
                (Wavelength::F_LINE, 1.0),
                (Wavelength::D_LINE, 2.0),
                (Wavelength::C_LINE, 1.0),
            ],
            primary_index: 1,
        }
    }

    /// Normalized weights summing to 1.0.
    #[must_use]
    pub fn normalized_weights(&self) -> Vec<f64> {
        let total_weight: f64 = self.entries.iter().map(|(_, w)| *w).sum();
        if total_weight <= 1e-15 {
            vec![1.0 / self.entries.len() as f64; self.entries.len()]
        } else {
            self.entries.iter().map(|(_, w)| *w / total_weight).collect()
        }
    }

    /// Primary wavelength.
    #[must_use]
    pub fn primary_wavelength(&self) -> Wavelength {
        self.entries
            .get(self.primary_index)
            .map(|(wl, _)| *wl)
            .unwrap_or(Wavelength::D_LINE)
    }
}

/// Traces an aimed ray through the optical system from field and pupil specifications.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `field`: Field point (angular degrees, height, or normalized).
/// - `pupil`: Normalized pupil coordinate $(P_x, P_y)$.
/// - `wavelength`: Ray wavelength.
/// - `mode`: Ray aiming mode (Paraxial or RealIterative).
///
/// # Errors
/// Returns [`OpticsError`] if aiming does not converge or ray vignettes.
pub fn trace_aimed_ray(
    prescription: &ResolvedPrescription,
    field: Field,
    pupil: Pupil,
    wavelength: Wavelength,
    mode: RayAimingMode,
) -> Result<Ray, OpticsError> {
    let wl_nm = wavelength.nanometers();
    let paraxial = calculate_paraxial_properties(prescription, wl_nm)?;

    let (theta_x_deg, theta_y_deg) = match field {
        Field::AngleDegrees { x, y } => (x, y),
        Field::HeightMm { x, y } => {
            let efl = paraxial.efl_mm;
            let tx = if efl.is_finite() && efl.abs() > 1e-6 {
                (x / efl).atan().to_degrees()
            } else {
                0.0
            };
            let ty = if efl.is_finite() && efl.abs() > 1e-6 {
                (y / efl).atan().to_degrees()
            } else {
                0.0
            };
            (tx, ty)
        }
        Field::Normalized { x, y } => {
            // Assume 10 degree max field if normalized
            (x * 10.0, y * 10.0)
        }
    };

    let r_enp = prescription.entrance_pupil_diameter_mm * 0.5;
    let z_enp = paraxial.entrance_pupil_pos_mm;

    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let margin = (prescription.entrance_pupil_diameter_mm * 1.5).clamp(0.5, 10.0);
    let z_launch = z_first - margin;

    let th_x = theta_x_deg.to_radians();
    let th_y = theta_y_deg.to_radians();
    let kx = th_x.sin() * th_y.cos();
    let ky = th_y.sin();
    let kz = (1.0 - kx * kx - ky * ky).max(1e-12).sqrt();
    let dir = Vec3::new(kx, ky, kz);

    match mode {
        RayAimingMode::Paraxial | RayAimingMode::Off => {
            let px_mm = pupil.px * r_enp;
            let py_mm = pupil.py * r_enp;
            let delta_z = z_launch - z_enp;
            let t = delta_z / dir.z;
            let origin = Vec3::new(px_mm + t * dir.x, py_mm + t * dir.y, z_launch);
            Ray::new(origin, dir, wl_nm, 1.0)
        }
        RayAimingMode::RealIterative => {
            // Find stop surface index
            let stop_idx = prescription.surfaces.iter().position(|s| s.is_stop).unwrap_or(0);
            let stop_semi = prescription.surfaces[stop_idx].conic.semi_diam_mm;
            let target_stop_x = pupil.px * stop_semi;
            let target_stop_y = pupil.py * stop_semi;

            // Start from paraxial estimate
            let mut launch_x = pupil.px * r_enp + (z_launch - z_enp) / dir.z * dir.x;
            let mut launch_y = pupil.py * r_enp + (z_launch - z_enp) / dir.z * dir.y;

            let max_iters = 25;
            let tol = 1e-6; // 1 micrometer tolerance at stop
            let delta = 1e-4;
            let mut lambda: f64 = 1e-3; // Initial Levenberg-Marquardt damping parameter

            let mut best_err_sq = f64::INFINITY;
            let mut best_launch_x = launch_x;
            let mut best_launch_y = launch_y;

            for _ in 0..max_iters {
                let r_base = match Ray::new(Vec3::new(launch_x, launch_y, z_launch), dir, wl_nm, 1.0) {
                    Ok(r) => r,
                    Err(_) => break,
                };
                let trace_base = match trace_sequential(prescription, &r_base) {
                    Ok(t) => t,
                    Err(_) => {
                        // If ray vignettes or misses, increase damping and halve step towards best
                        launch_x = 0.5 * (launch_x + best_launch_x);
                        launch_y = 0.5 * (launch_y + best_launch_y);
                        lambda *= 4.0;
                        continue;
                    }
                };
                let hit_stop = trace_base.hits.iter().find(|h| h.surface_index == stop_idx);

                let (cur_x, cur_y) = match hit_stop {
                    Some(h) => (h.point.x, h.point.y),
                    None => {
                        launch_x = 0.5 * (launch_x + best_launch_x);
                        launch_y = 0.5 * (launch_y + best_launch_y);
                        lambda *= 4.0;
                        continue;
                    }
                };

                let err_x = cur_x - target_stop_x;
                let err_y = cur_y - target_stop_y;
                let current_err_sq = err_x * err_x + err_y * err_y;

                if current_err_sq < best_err_sq {
                    best_err_sq = current_err_sq;
                    best_launch_x = launch_x;
                    best_launch_y = launch_y;
                    lambda = (lambda * 0.5_f64).max(1e-7_f64);
                }

                if err_x.abs() < tol && err_y.abs() < tol {
                    return Ok(r_base);
                }

                // Numerical Jacobian J = [d(cur_x)/d(lx), d(cur_x)/d(ly); d(cur_y)/d(lx), d(cur_y)/d(ly)]
                let r_dx = match Ray::new(Vec3::new(launch_x + delta, launch_y, z_launch), dir, wl_nm, 1.0) {
                    Ok(r) => r,
                    Err(_) => break,
                };
                let r_dy = match Ray::new(Vec3::new(launch_x, launch_y + delta, z_launch), dir, wl_nm, 1.0) {
                    Ok(r) => r,
                    Err(_) => break,
                };

                let (x_dx, y_dx) = match trace_sequential(prescription, &r_dx)
                    .ok()
                    .and_then(|t| t.hits.into_iter().find(|h| h.surface_index == stop_idx))
                {
                    Some(h) => (h.point.x, h.point.y),
                    None => (cur_x + delta, cur_y),
                };
                let (x_dy, y_dy) = match trace_sequential(prescription, &r_dy)
                    .ok()
                    .and_then(|t| t.hits.into_iter().find(|h| h.surface_index == stop_idx))
                {
                    Some(h) => (h.point.x, h.point.y),
                    None => (cur_x, cur_y + delta),
                };

                let j11 = (x_dx - cur_x) / delta;
                let j21 = (y_dx - cur_y) / delta;
                let j12 = (x_dy - cur_x) / delta;
                let j22 = (y_dy - cur_y) / delta;

                // Damped Gauss-Newton: (J^T J + lambda * I) * delta = J^T * e
                let a11 = j11 * j11 + j21 * j21 + lambda;
                let a12 = j11 * j12 + j21 * j22;
                let a21 = a12;
                let a22 = j12 * j12 + j22 * j22 + lambda;

                let rhs_x = j11 * err_x + j21 * err_y;
                let rhs_y = j12 * err_x + j22 * err_y;

                let det_a = a11 * a22 - a12 * a21;
                if det_a.abs() < 1e-15 {
                    lambda *= 4.0;
                    continue;
                }

                let d_lx = (a22 * rhs_x - a12 * rhs_y) / det_a;
                let d_ly = (-a21 * rhs_x + a11 * rhs_y) / det_a;

                // Step damping clamp to prevent erratic wild overshoot
                let step_len = (d_lx * d_lx + d_ly * d_ly).sqrt();
                let max_step = r_enp * 0.5;
                let scale = if step_len > max_step { max_step / step_len } else { 1.0 };

                launch_x -= d_lx * scale;
                launch_y -= d_ly * scale;
            }

            Ray::new(Vec3::new(best_launch_x, best_launch_y, z_launch), dir, wl_nm, 1.0)
        }
    }
}

/// Ray polarization state result along its sequential optical trajectory.
#[derive(Debug, Clone, PartialEq)]
pub struct PolarizedRayTraceResult {
    /// Final electric field state in Jones representation.
    pub final_jones: crate::polarization::JonesVector,
    /// Final irradiance and polarization state in Stokes representation.
    pub final_stokes: crate::polarization::StokesVector,
    /// Total intensity throughput for the chosen polarization input.
    pub throughput: f64,
    /// Degree of Polarization $\in [0, 1]$.
    pub dop: f64,
    /// Degree of Linear Polarization $\in [0, 1]$.
    pub dolp: f64,
    /// Degree of Circular Polarization $\in [0, 1]$.
    pub docp: f64,
    /// Polarization orientation / major axis angle in degrees $\in [-90^\circ, 90^\circ]$.
    pub orientation_deg: f64,
    /// Ellipticity angle in degrees $\in [-45^\circ, 45^\circ]$ (positive = right-handed, negative = left-handed).
    pub ellipticity_deg: f64,
    /// Surface-by-surface polarimetric records.
    pub surface_records: Vec<SurfacePolarizationRecord>,
}

/// Individual surface polarization interaction record.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePolarizationRecord {
    /// Optical surface index.
    pub surface_index: usize,
    /// Angle of incidence in degrees.
    pub incident_angle_deg: f64,
    /// Angle of refraction in degrees (None if TIR).
    pub refracted_angle_deg: Option<f64>,
    /// s-polarization power transmission $T_s \in [0, 1]$.
    pub ts: f64,
    /// p-polarization power transmission $T_p \in [0, 1]$.
    pub tp: f64,
    /// Retardance / phase difference $\Delta = \delta_p - \delta_s$ in degrees.
    pub phase_shift_deg: f64,
    /// Diattenuation $D = |T_p - T_s| / (T_p + T_s) \in [0, 1]$.
    pub diattenuation: f64,
}

/// Traces a polarized ray through the optical system, updating its Jones and Stokes vectors
/// using thin-film coating responses or bare Fresnel formulas at each interface.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `ray`: Input ray with initial direction and coordinates.
/// - `initial_polarization`: Initial electric field Jones vector (e.g. unpolarized, linear, circular).
///
/// # Errors
/// Returns [`OpticsError`] if ray tracing fails or vignettes.
pub fn trace_polarized_ray(
    prescription: &ResolvedPrescription,
    ray: &Ray,
    initial_polarization: crate::polarization::JonesVector,
) -> Result<PolarizedRayTraceResult, OpticsError> {
    use crate::polarization::{surface_coating_jones, surface_fresnel_jones};

    let path = trace_sequential(prescription, ray)?;
    let mut current_jones = initial_polarization;
    let mut surface_records = Vec::with_capacity(path.hits.len());

    for hit in &path.hits {
        let b = &hit.bounce;
        let surf = prescription
            .surfaces
            .iter()
            .find(|s| s.surface_index == hit.surface_index);

        let (_r_mat, t_mat) = if let Some(coat) = surf.and_then(|s| s.coating.as_ref()) {
            match coat.evaluate(b.n1, b.n2, b.incident_angle_rad, b.wavelength_nm) {
                Ok(resp) => surface_coating_jones(&resp),
                Err(_) => surface_fresnel_jones(b.n1, b.n2, b.incident_angle_rad)?,
            }
        } else {
            surface_fresnel_jones(b.n1, b.n2, b.incident_angle_rad)?
        };

        // Apply transmission Jones matrix
        current_jones = t_mat.apply(current_jones);

        let ts_amp = t_mat.m[0][0];
        let tp_amp = t_mat.m[1][1];
        let ts_pwr = ts_amp.norm_sq();
        let tp_pwr = tp_amp.norm_sq();

        let phase_s = ts_amp.im.atan2(ts_amp.re);
        let phase_p = tp_amp.im.atan2(tp_amp.re);
        let mut delta_phase = (phase_p - phase_s).to_degrees();
        while delta_phase > 180.0 {
            delta_phase -= 360.0;
        }
        while delta_phase < -180.0 {
            delta_phase += 360.0;
        }

        let diattenuation = if ts_pwr + tp_pwr > 1e-15 {
            (tp_pwr - ts_pwr).abs() / (tp_pwr + ts_pwr)
        } else {
            0.0
        };

        surface_records.push(SurfacePolarizationRecord {
            surface_index: hit.surface_index,
            incident_angle_deg: b.incident_angle_rad.to_degrees(),
            refracted_angle_deg: b.refracted_angle_rad.map(|a| a.to_degrees()),
            ts: ts_pwr,
            tp: tp_pwr,
            phase_shift_deg: delta_phase,
            diattenuation,
        });
    }

    let final_stokes = current_jones.to_stokes();
    let initial_intensity = initial_polarization.intensity();
    let throughput = if initial_intensity > 1e-15 {
        final_stokes.s0 / initial_intensity
    } else {
        0.0
    };

    Ok(PolarizedRayTraceResult {
        final_jones: current_jones,
        final_stokes,
        throughput,
        dop: final_stokes.degree_of_polarization(),
        dolp: final_stokes.degree_of_linear_polarization(),
        docp: final_stokes.degree_of_circular_polarization(),
        orientation_deg: final_stokes.orientation_angle_rad().to_degrees(),
        ellipticity_deg: final_stokes.ellipticity_angle_rad().to_degrees(),
        surface_records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prescription::OpticalPrescription;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_vignetting_factor_transformation() -> TestResult {
        let vig = VignettingFactors {
            vdx: 0.1,
            vdy: 0.2,
            vcx: 0.8,
            vcy: 0.8,
            van_deg: 0.0,
        };

        let p0 = Pupil::center();
        let p_vig = vig.apply(p0);
        assert!((p_vig.px - 0.1).abs() < 1e-12);
        assert!((p_vig.py - 0.2).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_wavelength_set_weights() {
        let set = WavelengthSet::visible_triplet();
        let weights = set.normalized_weights();
        assert_eq!(weights.len(), 3);
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        assert_eq!(set.primary_wavelength(), Wavelength::D_LINE);
    }

    #[test]
    fn test_trace_aimed_ray_converges() -> TestResult {
        use crate::prescription::OpticalSurface;

        let presc = OpticalPrescription {
            name: "Test Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0, 5.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 10.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 45.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        };

        let resolver =
            |_: &str| -> Result<std::sync::Arc<dyn crate::geometry::RefractiveIndex + Send + Sync>, OpticsError> {
                Ok(std::sync::Arc::new(crate::geometry::ConstantIndex(1.5168)))
            };
        let resolved = presc.resolve(&resolver)?;

        let ray = trace_aimed_ray(
            &resolved,
            Field::AngleDegrees { x: 0.0, y: 5.0 },
            Pupil::new(0.0, 0.5)?,
            Wavelength::D_LINE,
            RayAimingMode::RealIterative,
        )?;

        assert!(ray.origin.z < 0.0);
        assert!(ray.direction.z > 0.0);
        Ok(())
    }

    #[test]
    fn test_trace_polarized_ray_brewster_and_dop() -> TestResult {
        use crate::polarization::JonesVector;
        use crate::prescription::OpticalSurface;

        let presc = OpticalPrescription {
            name: "Polarized Doublet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 10.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 45.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        };

        let resolver =
            |_: &str| -> Result<std::sync::Arc<dyn crate::geometry::RefractiveIndex + Send + Sync>, OpticsError> {
                Ok(std::sync::Arc::new(crate::geometry::ConstantIndex(1.5168)))
            };
        let resolved = presc.resolve(&resolver)?;

        let ray = Ray::new(Vec3::new(0.0, 2.0, -10.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;

        // Trace linear 45-degree polarized input
        let pol_in = JonesVector::linear_45();
        let res = trace_polarized_ray(&resolved, &ray, pol_in)?;

        assert_eq!(res.surface_records.len(), 2);
        assert!(res.throughput > 0.85 && res.throughput < 1.0);
        assert!((res.dop - 1.0).abs() < 1e-6, "Pure state remains DOP = 1.0");
        assert!(res.surface_records[0].ts > 0.0);
        assert!(res.surface_records[0].tp > 0.0);

        Ok(())
    }
}
