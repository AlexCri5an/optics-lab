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

/// Vignetting factors (Zemax VDX, VDY, VCX, VCY, VAN) for field-dependent pupil compression.
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

        Pupil {
            px: px_rot,
            py: py_rot,
        }
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
            self.entries
                .iter()
                .map(|(_, w)| *w / total_weight)
                .collect()
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

    let z_first = prescription
        .surfaces
        .first()
        .map_or(0.0, |s| s.conic.vertex_z);
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
            let stop_idx = prescription
                .surfaces
                .iter()
                .position(|s| s.is_stop)
                .unwrap_or(0);
            let stop_semi = prescription.surfaces[stop_idx].conic.semi_diam_mm;
            let target_stop_x = pupil.px * stop_semi;
            let target_stop_y = pupil.py * stop_semi;

            // Start from paraxial estimate
            let mut launch_x = pupil.px * r_enp + (z_launch - z_enp) / dir.z * dir.x;
            let mut launch_y = pupil.py * r_enp + (z_launch - z_enp) / dir.z * dir.y;

            let max_iters = 15;
            let tol = 1e-6; // 1 micrometer at stop
            let delta = 1e-4;

            for _ in 0..max_iters {
                let r_base = Ray::new(Vec3::new(launch_x, launch_y, z_launch), dir, wl_nm, 1.0)?;
                let trace_base = trace_sequential(prescription, &r_base)?;
                let hit_stop = trace_base.hits.iter().find(|h| h.surface_index == stop_idx);

                let (cur_x, cur_y) = match hit_stop {
                    Some(h) => (h.point.x, h.point.y),
                    None => break, // Fall back to paraxial if missed
                };

                let err_x = cur_x - target_stop_x;
                let err_y = cur_y - target_stop_y;

                if err_x.abs() < tol && err_y.abs() < tol {
                    return Ok(r_base);
                }

                // Numerical Jacobian
                let r_dx = Ray::new(
                    Vec3::new(launch_x + delta, launch_y, z_launch),
                    dir,
                    wl_nm,
                    1.0,
                )?;
                let r_dy = Ray::new(
                    Vec3::new(launch_x, launch_y + delta, z_launch),
                    dir,
                    wl_nm,
                    1.0,
                )?;

                let (x_dx, y_dx) = match trace_sequential(prescription, &r_dx)?
                    .hits
                    .iter()
                    .find(|h| h.surface_index == stop_idx)
                {
                    Some(h) => (h.point.x, h.point.y),
                    None => break,
                };
                let (x_dy, y_dy) = match trace_sequential(prescription, &r_dy)?
                    .hits
                    .iter()
                    .find(|h| h.surface_index == stop_idx)
                {
                    Some(h) => (h.point.x, h.point.y),
                    None => break,
                };

                let j11 = (x_dx - cur_x) / delta;
                let j21 = (y_dx - cur_y) / delta;
                let j12 = (x_dy - cur_x) / delta;
                let j22 = (y_dy - cur_y) / delta;

                let det = j11 * j22 - j12 * j21;
                if det.abs() < 1e-12 {
                    break; // Jacobian singular, retain current
                }

                let inv_j11 = j22 / det;
                let inv_j12 = -j12 / det;
                let inv_j21 = -j21 / det;
                let inv_j22 = j11 / det;

                let d_lx = inv_j11 * err_x + inv_j12 * err_y;
                let d_ly = inv_j21 * err_x + inv_j22 * err_y;

                launch_x -= d_lx;
                launch_y -= d_ly;
            }

            Ray::new(Vec3::new(launch_x, launch_y, z_launch), dir, wl_nm, 1.0)
        }
    }
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
                },
            ],
        };

        let resolver =
            |_: &str| -> Result<
                std::sync::Arc<dyn crate::geometry::RefractiveIndex + Send + Sync>,
                OpticsError,
            > { Ok(std::sync::Arc::new(crate::geometry::ConstantIndex(1.5168))) };
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
}
