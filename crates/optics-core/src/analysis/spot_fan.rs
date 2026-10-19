//! Optical diagnostics, geometric spot diagrams, and transverse ray aberration fans.
//!
//! # Optical Foundations
//! - **Normalized Pupil Coordinates:** $(P_x, P_y)$ with $P_x^2 + P_y^2 \le 1.0$.
//!   Physical pupil coordinates at the entrance pupil plane $z_{\text{ENP}}$ are
//!   $x_p = P_x \cdot r_{\text{ENP}}$ and $y_p = P_y \cdot r_{\text{ENP}}$, where $r_{\text{ENP}} = \text{EPD} / 2$.
//! - **Spot Diagram:** Traces a grid of real rays (hexapolar or rectangular) across the circular pupil
//!   and records their $(x_i, y_i)$ intersections on the image sensor.
//! - **Spot Centroid & Blur Radius:**
//!   - Centroid: $\bar{x} = \frac{1}{N} \sum x_i, \quad \bar{y} = \frac{1}{N} \sum y_i$.
//!   - RMS Spot Radius: $r_{\text{RMS}} = \sqrt{\frac{1}{N} \sum \left[(x_i - \bar{x})^2 + (y_i - \bar{y})^2\right]}$.
//!   - Geometric Radius: $r_{\text{GEO}} = \max_i \sqrt{(x_i - \bar{x})^2 + (y_i - \bar{y})^2}$.
//!   - Airy Disk Radius: $r_{\text{Airy}} = 1.22 \cdot \lambda \cdot F/\#$.
//!   - Diffraction Limited: $r_{\text{RMS}} \le r_{\text{Airy}}$.
//! - **Transverse Ray Aberration Fans:**
//!   - Tangential (meridional) fan: transverse displacement $\epsilon_y$ vs $P_y$ with $P_x = 0$.
//!   - Sagittal fan: transverse displacement $\epsilon_x$ vs $P_x$ with $P_y = 0$.
//!   - All transverse errors are evaluated relative to the reference chief ray ($P_x = 0, P_y = 0$).

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties, trace_sequential};
use crate::types::{Ray, Vec3};

/// Normalized entrance pupil sampling pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PupilPattern {
    /// Concentric hexapolar rings with $6 \times r$ rays per ring.
    ///
    /// Uniform area weighting across the circular entrance pupil.
    Hexapolar {
        /// Number of concentric rings around the central axial ray ($P = 0$).
        num_rings: usize,
    },
    /// Uniform rectangular Cartesian grid within $P_x^2 + P_y^2 \le 1.0$.
    Rectangular {
        /// Number of grid samples along each diameter axis.
        grid_size: usize,
    },
    /// Cross pattern tracing along tangential ($P_x = 0$) and sagittal ($P_y = 0$) meridians.
    Cross {
        /// Number of points per axis.
        num_points: usize,
    },
}

impl Default for PupilPattern {
    fn default() -> Self {
        Self::Hexapolar { num_rings: 6 }
    }
}

/// A 2D normalized coordinate $(P_x, P_y)$ on the circular entrance pupil ($P_x^2 + P_y^2 \le 1.0$).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PupilCoord {
    /// Normalized sagittal pupil coordinate in $[-1.0, 1.0]$.
    pub px: f64,
    /// Normalized tangential pupil coordinate in $[-1.0, 1.0]$.
    pub py: f64,
}

impl PupilCoord {
    /// Creates a new normalized pupil coordinate.
    #[must_use]
    pub const fn new(px: f64, py: f64) -> Self {
        Self { px, py }
    }

    /// Normalized radial pupil distance $\rho = \sqrt{P_x^2 + P_y^2}$.
    #[must_use]
    pub fn rho(&self) -> f64 {
        (self.px * self.px + self.py * self.py).sqrt()
    }
}

/// Generates normalized pupil coordinates $(P_x, P_y)$ according to the specified distribution.
#[must_use]
pub fn generate_pupil_distribution(pattern: PupilPattern) -> Vec<PupilCoord> {
    match pattern {
        PupilPattern::Hexapolar { num_rings } => {
            if num_rings == 0 {
                return vec![PupilCoord::new(0.0, 0.0)];
            }

            // Estimate total rays: 1 + 3 * R * (R + 1)
            let mut coords = Vec::with_capacity(1 + 3 * num_rings * (num_rings + 1));
            coords.push(PupilCoord::new(0.0, 0.0)); // Center ray

            for ring in 1..=num_rings {
                let rho = ring as f64 / num_rings as f64;
                let num_points = 6 * ring;
                let d_theta = std::f64::consts::TAU / num_points as f64;

                for i in 0..num_points {
                    let theta = i as f64 * d_theta;
                    let px = rho * theta.cos();
                    let py = rho * theta.sin();
                    coords.push(PupilCoord::new(px, py));
                }
            }

            coords
        }
        PupilPattern::Rectangular { grid_size } => {
            let n = grid_size.max(2);
            let mut coords = Vec::with_capacity(n * n);

            for i in 0..n {
                let py = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
                for j in 0..n {
                    let px = -1.0 + 2.0 * j as f64 / (n - 1) as f64;
                    if px * px + py * py <= 1.0 + 1e-12 {
                        coords.push(PupilCoord::new(px, py));
                    }
                }
            }

            coords
        }
        PupilPattern::Cross { num_points } => {
            let n = if num_points % 2 == 0 {
                num_points + 1
            } else {
                num_points.max(3)
            };
            let mut coords = Vec::with_capacity(2 * n);

            // Tangential axis: Px = 0, Py in [-1, 1]
            for i in 0..n {
                let py = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
                coords.push(PupilCoord::new(0.0, py));
            }

            // Sagittal axis: Py = 0, Px in [-1, 1]
            for i in 0..n {
                let px = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
                if px.abs() > 1e-12 {
                    // Avoid duplicating center (0, 0)
                    coords.push(PupilCoord::new(px, 0.0));
                }
            }

            coords
        }
    }
}

/// Constructs a 3D [`Ray`] originating from a normalized pupil coordinate and field angle.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `pupil`: Normalized pupil coordinate $(P_x, P_y)$.
/// - `field_angle_deg`: Object field angle $(\theta_x, \theta_y)$ in degrees.
/// - `wavelength_nm`: Ray wavelength in nanometers.
/// - `z_enp_mm`: Axial position of entrance pupil in millimeters.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if ray direction vector cannot be normalized.
pub fn launch_pupil_ray(
    prescription: &ResolvedPrescription,
    pupil: PupilCoord,
    field_angle_deg: (f64, f64),
    wavelength_nm: f64,
    z_enp_mm: f64,
) -> Result<Ray, OpticsError> {
    let r_enp = prescription.entrance_pupil_diameter_mm * 0.5;
    let px_mm = pupil.px * r_enp;
    let py_mm = pupil.py * r_enp;

    // Collimated ray direction from field angle using direction cosines (robust up to 89.9 deg)
    let th_x = field_angle_deg.0.to_radians();
    let th_y = field_angle_deg.1.to_radians();
    let kx = th_x.sin() * th_y.cos();
    let ky = th_y.sin();
    let kz = (1.0 - kx * kx - ky * ky).max(1e-12).sqrt();
    let norm_dir = Vec3::new(kx, ky, kz);

    // Launch plane placed in front of the first optical surface vertex, scaled by pupil size
    let z_first = prescription
        .surfaces
        .first()
        .map_or(0.0, |s| s.conic.vertex_z);
    let margin = (prescription.entrance_pupil_diameter_mm * 1.5).clamp(0.5, 10.0);
    let z_launch = z_first - margin;

    // Intersect the pupil plane (x = px_mm, y = py_mm, z = z_enp_mm) backwards to z_launch
    if norm_dir.z.abs() < 1e-12 {
        return Err(OpticsError::NumericalSingularity(
            "Ray direction has near-zero longitudinal component".to_string(),
        ));
    }

    let delta_z = z_launch - z_enp_mm;
    let t = delta_z / norm_dir.z;
    let origin = Vec3::new(px_mm + t * norm_dir.x, py_mm + t * norm_dir.y, z_launch);

    Ray::new(origin, norm_dir, wavelength_nm, 1.0)
}

/// Single ray spot intersection on the final image plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpotPoint {
    /// Normalized entrance pupil coordinate that generated this spot.
    pub pupil: PupilCoord,
    /// Transverse $x$ hit coordinate on image plane in millimeters.
    pub x_mm: f64,
    /// Transverse $y$ hit coordinate on image plane in millimeters.
    pub y_mm: f64,
    /// Residual optical power transmittance through system.
    pub intensity: f64,
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
}

/// Statistical blur metrics computed from a geometric spot diagram.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpotMetrics {
    /// Geometric spot centroid $x$ coordinate in millimeters.
    pub centroid_x_mm: f64,
    /// Geometric spot centroid $y$ coordinate in millimeters.
    pub centroid_y_mm: f64,
    /// Root-mean-square (RMS) blur radius from centroid in micrometers ($\mu\text{m}$).
    pub rms_radius_um: f64,
    /// Maximum geometric (GEO) blur radius from centroid in micrometers ($\mu\text{m}$).
    pub geo_radius_um: f64,
    /// Theoretical diffraction Airy disk radius in micrometers ($r_{\text{Airy}} = 1.22 \lambda F/\#$).
    pub airy_radius_um: f64,
    /// Whether the spot is diffraction-limited ($r_{\text{RMS}} \le r_{\text{Airy}}$).
    pub is_diffraction_limited: bool,
    /// Total number of pupil rays attempted.
    pub total_rays_traced: usize,
    /// Number of pupil rays reaching image plane without vignetting.
    pub passed_rays_count: usize,
    /// Ratio of passed rays to total rays traced ($1.0 = \text{no vignetting}$).
    pub vignetting_factor: f64,
}

/// Evaluates statistical blur metrics for a collection of image plane spot hits.
///
/// # Arguments
/// - `points`: Traced spot hit coordinates.
/// - `wavelength_nm`: Primary wavelength in nanometers.
/// - `f_number`: Image-space system F-number ($F/\#$).
/// - `total_traced`: Total pupil rays launched.
#[must_use]
pub fn spot_diagram_metrics(
    points: &[SpotPoint],
    wavelength_nm: f64,
    f_number: f64,
    total_traced: usize,
) -> SpotMetrics {
    let passed = points.len();
    if passed == 0 {
        return SpotMetrics {
            centroid_x_mm: 0.0,
            centroid_y_mm: 0.0,
            rms_radius_um: f64::INFINITY,
            geo_radius_um: f64::INFINITY,
            airy_radius_um: 1.22 * (wavelength_nm * 1e-3) * f_number,
            is_diffraction_limited: false,
            total_rays_traced: total_traced,
            passed_rays_count: 0,
            vignetting_factor: 0.0,
        };
    }

    let n = passed as f64;
    let sum_x: f64 = points.iter().map(|p| p.x_mm).sum();
    let sum_y: f64 = points.iter().map(|p| p.y_mm).sum();
    let cx = sum_x / n;
    let cy = sum_y / n;

    let mut sum_r2 = 0.0_f64;
    let mut max_r2 = 0.0_f64;

    for p in points {
        let dx = p.x_mm - cx;
        let dy = p.y_mm - cy;
        let r2 = dx * dx + dy * dy;
        sum_r2 += r2;
        if r2 > max_r2 {
            max_r2 = r2;
        }
    }

    let rms_mm = (sum_r2 / n).sqrt();
    let geo_mm = max_r2.sqrt();

    // Convert mm to micrometers (um)
    let rms_radius_um = rms_mm * 1000.0;
    let geo_radius_um = geo_mm * 1000.0;

    // Airy disk radius: r_Airy = 1.22 * lambda * F/# (lambda in um)
    let lambda_um = wavelength_nm * 1e-3;
    let airy_radius_um = 1.22 * lambda_um * f_number;
    let is_diffraction_limited = rms_radius_um <= airy_radius_um;

    SpotMetrics {
        centroid_x_mm: cx,
        centroid_y_mm: cy,
        rms_radius_um,
        geo_radius_um,
        airy_radius_um,
        is_diffraction_limited,
        total_rays_traced: total_traced,
        passed_rays_count: passed,
        vignetting_factor: passed as f64 / total_traced.max(1) as f64,
    }
}

/// Complete geometric spot diagram evaluation for a specified field and wavelength.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotDiagram {
    /// Object field angle $(\theta_x, \theta_y)$ in degrees.
    pub field_angle_deg: (f64, f64),
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Traced ray intersection coordinates on the image sensor.
    pub points: Vec<SpotPoint>,
    /// Summary blur and diffraction metrics.
    pub metrics: SpotMetrics,
    /// Reference chief ray intersection coordinates $(x, y)$ in millimeters.
    pub chief_ray_hit_mm: (f64, f64),
}

/// Traces a geometric spot diagram across the circular entrance pupil for a given field and wavelength.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `field_angle_deg`: Field angle $(\theta_x, \theta_y)$ in degrees.
/// - `wavelength_nm`: Evaluation wavelength in nanometers.
/// - `pattern`: Pupil sampling distribution (e.g. [`PupilPattern::Hexapolar`]).
///
/// # Errors
/// Returns [`OpticsError`] if paraxial cardinal properties cannot be computed.
pub fn trace_spot_diagram(
    prescription: &ResolvedPrescription,
    field_angle_deg: (f64, f64),
    wavelength_nm: f64,
    pattern: PupilPattern,
) -> Result<SpotDiagram, OpticsError> {
    let parax = calculate_paraxial_properties(prescription, wavelength_nm).ok();
    let enp_pos = parax.as_ref().map_or_else(
        || {
            let stop_idx = prescription.surfaces.iter().position(|s| s.is_stop).unwrap_or(0);
            let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
            let z_stop = prescription.surfaces.get(stop_idx).map_or(0.0, |s| s.conic.vertex_z);
            z_stop - z_first
        },
        |p| p.entrance_pupil_pos_mm,
    );
    let z_first = prescription
        .surfaces
        .first()
        .map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + enp_pos;

    // Trace reference chief ray (Px = 0, Py = 0)
    let chief_ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(0.0, 0.0),
        field_angle_deg,
        wavelength_nm,
        z_enp,
    )?;

    let chief_hit_mm = match trace_sequential(prescription, &chief_ray) {
        Ok(path) => {
            let last_surf = prescription.surfaces.last();
            let is_plane = last_surf.is_some_and(|s| s.thickness_mm == 0.0);
            if is_plane {
                path.hits
                    .last()
                    .map_or((0.0, 0.0), |h| (h.point.x, h.point.y))
            } else {
                let d = last_surf.map_or(0.0, |s| s.thickness_mm);
                let z_target = last_surf.map_or(0.0, |s| s.conic.vertex_z) + d;
                let t = (z_target - path.final_ray.origin.z) / path.final_ray.direction.z;
                (
                    path.final_ray.origin.x + t * path.final_ray.direction.x,
                    path.final_ray.origin.y + t * path.final_ray.direction.y,
                )
            }
        }
        Err(_) => (0.0, 0.0),
    };

    let pupil_coords = generate_pupil_distribution(pattern);
    let total_traced = pupil_coords.len();
    let mut points = Vec::with_capacity(total_traced);

    for pupil in pupil_coords {
        let Ok(ray) = launch_pupil_ray(prescription, pupil, field_angle_deg, wavelength_nm, z_enp)
        else {
            continue;
        };

        if let Ok(path) = trace_sequential(prescription, &ray) {
            let last_surf = prescription.surfaces.last();
            let is_plane = last_surf.is_some_and(|s| s.thickness_mm == 0.0);
            let (x_mm, y_mm) = if is_plane {
                path.hits
                    .last()
                    .map_or((0.0, 0.0), |h| (h.point.x, h.point.y))
            } else {
                let d = last_surf.map_or(0.0, |s| s.thickness_mm);
                let z_target = last_surf.map_or(0.0, |s| s.conic.vertex_z) + d;
                let t = (z_target - path.final_ray.origin.z) / path.final_ray.direction.z;
                (
                    path.final_ray.origin.x + t * path.final_ray.direction.x,
                    path.final_ray.origin.y + t * path.final_ray.direction.y,
                )
            };

            points.push(SpotPoint {
                pupil,
                x_mm,
                y_mm,
                intensity: path.cumulative_throughput,
                wavelength_nm,
            });
        }
    }

    let f_number = parax.as_ref().map_or(f64::INFINITY, |p| p.f_number);
    let metrics = spot_diagram_metrics(&points, wavelength_nm, f_number, total_traced);

    Ok(SpotDiagram {
        field_angle_deg,
        wavelength_nm,
        points,
        metrics,
        chief_ray_hit_mm: chief_hit_mm,
    })
}

/// A single sample point along a transverse ray aberration fan curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RayFanPoint {
    /// Normalized pupil coordinate ($P_y$ for tangential, $P_x$ for sagittal) in $[-1.0, 1.0]$.
    pub pupil_coord: f64,
    /// Transverse ray aberration error relative to reference chief ray in micrometers ($\mu\text{m}$).
    pub aberration_um: f64,
    /// True if the ray traversed all surfaces without vignetting or TIR.
    pub ray_passed: bool,
}

/// Transverse ray aberration fan data for tangential and sagittal meridians.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RayFanData {
    /// Object field angle $(\theta_x, \theta_y)$ in degrees.
    pub field_angle_deg: (f64, f64),
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Tangential (meridional) fan: $\epsilon_y$ vs $P_y$ with $P_x = 0$.
    pub tangential: Vec<RayFanPoint>,
    /// Sagittal fan: $\epsilon_x$ vs $P_x$ with $P_y = 0$.
    pub sagittal: Vec<RayFanPoint>,
    /// Reference chief ray intersection $(x, y)$ on image plane in millimeters.
    pub chief_ray_hit_mm: (f64, f64),
}

/// Traces tangential and sagittal transverse ray aberration fans across the entrance pupil.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription.
/// - `field_angle_deg`: Object field angle $(\theta_x, \theta_y)$ in degrees.
/// - `wavelength_nm`: Evaluation wavelength in nanometers.
/// - `num_points`: Number of evaluation samples along each meridian (forced odd to include $P = 0$).
///
/// # Errors
/// Returns [`OpticsError`] if paraxial cardinal properties cannot be computed.
pub fn trace_ray_fan(
    prescription: &ResolvedPrescription,
    field_angle_deg: (f64, f64),
    wavelength_nm: f64,
    num_points: usize,
) -> Result<RayFanData, OpticsError> {
    let parax = calculate_paraxial_properties(prescription, wavelength_nm).ok();
    let enp_pos = parax.as_ref().map_or_else(
        || {
            let stop_idx = prescription.surfaces.iter().position(|s| s.is_stop).unwrap_or(0);
            let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
            let z_stop = prescription.surfaces.get(stop_idx).map_or(0.0, |s| s.conic.vertex_z);
            z_stop - z_first
        },
        |p| p.entrance_pupil_pos_mm,
    );
    let z_first = prescription
        .surfaces
        .first()
        .map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + enp_pos;

    // Trace reference chief ray (Px = 0, Py = 0)
    let chief_ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(0.0, 0.0),
        field_angle_deg,
        wavelength_nm,
        z_enp,
    )?;

    let chief_hit_mm = match trace_sequential(prescription, &chief_ray) {
        Ok(path) => {
            let last_surf = prescription.surfaces.last();
            let is_plane = last_surf.is_some_and(|s| s.thickness_mm == 0.0);
            if is_plane {
                path.hits
                    .last()
                    .map_or((0.0, 0.0), |h| (h.point.x, h.point.y))
            } else {
                let d = last_surf.map_or(0.0, |s| s.thickness_mm);
                let z_target = last_surf.map_or(0.0, |s| s.conic.vertex_z) + d;
                let t = (z_target - path.final_ray.origin.z) / path.final_ray.direction.z;
                (
                    path.final_ray.origin.x + t * path.final_ray.direction.x,
                    path.final_ray.origin.y + t * path.final_ray.direction.y,
                )
            }
        }
        Err(_) => (0.0, 0.0),
    };

    let n = if num_points % 2 == 0 {
        num_points + 1
    } else {
        num_points.max(3)
    };

    let last_surf = prescription.surfaces.last();
    let is_plane = last_surf.is_some_and(|s| s.thickness_mm == 0.0);
    let d = last_surf.map_or(0.0, |s| s.thickness_mm);
    let z_target = last_surf.map_or(0.0, |s| s.conic.vertex_z) + d;

    // 1. Tangential Fan: Px = 0, Py in [-1.0, 1.0]
    let mut tangential = Vec::with_capacity(n);
    for i in 0..n {
        let py = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
        let Ok(ray) = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, py),
            field_angle_deg,
            wavelength_nm,
            z_enp,
        ) else {
            tangential.push(RayFanPoint {
                pupil_coord: py,
                aberration_um: 0.0,
                ray_passed: false,
            });
            continue;
        };

        match trace_sequential(prescription, &ray) {
            Ok(path) => {
                let y_hit = if is_plane {
                    path.hits.last().map_or(0.0, |h| h.point.y)
                } else {
                    let t = (z_target - path.final_ray.origin.z) / path.final_ray.direction.z;
                    path.final_ray.origin.y + t * path.final_ray.direction.y
                };
                let ey_um = (y_hit - chief_hit_mm.1) * 1000.0;
                tangential.push(RayFanPoint {
                    pupil_coord: py,
                    aberration_um: ey_um,
                    ray_passed: true,
                });
            }
            Err(_) => {
                tangential.push(RayFanPoint {
                    pupil_coord: py,
                    aberration_um: 0.0,
                    ray_passed: false,
                });
            }
        }
    }

    // 2. Sagittal Fan: Py = 0, Px in [-1.0, 1.0]
    let mut sagittal = Vec::with_capacity(n);
    for i in 0..n {
        let px = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
        let Ok(ray) = launch_pupil_ray(
            prescription,
            PupilCoord::new(px, 0.0),
            field_angle_deg,
            wavelength_nm,
            z_enp,
        ) else {
            sagittal.push(RayFanPoint {
                pupil_coord: px,
                aberration_um: 0.0,
                ray_passed: false,
            });
            continue;
        };

        match trace_sequential(prescription, &ray) {
            Ok(path) => {
                let x_hit = if is_plane {
                    path.hits.last().map_or(0.0, |h| h.point.x)
                } else {
                    let t = (z_target - path.final_ray.origin.z) / path.final_ray.direction.z;
                    path.final_ray.origin.x + t * path.final_ray.direction.x
                };
                let ex_um = (x_hit - chief_hit_mm.0) * 1000.0;
                sagittal.push(RayFanPoint {
                    pupil_coord: px,
                    aberration_um: ex_um,
                    ray_passed: true,
                });
            }
            Err(_) => {
                sagittal.push(RayFanPoint {
                    pupil_coord: px,
                    aberration_um: 0.0,
                    ray_passed: false,
                });
            }
        }
    }

    Ok(RayFanData {
        field_angle_deg,
        wavelength_nm,
        tangential,
        sagittal,
        chief_ray_hit_mm: chief_hit_mm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conic::ConicSurface;
    use crate::geometry::{Air, ConstantIndex};
    use crate::prescription::ResolvedSurface;
    use std::sync::Arc;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_hexapolar_distribution_counts_and_symmetry() {
        let coords_r1 = generate_pupil_distribution(PupilPattern::Hexapolar { num_rings: 1 });
        // 1 center + 6 = 7
        assert_eq!(coords_r1.len(), 7);

        let coords_r6 = generate_pupil_distribution(PupilPattern::Hexapolar { num_rings: 6 });
        // 1 + 3 * 6 * 7 = 127
        assert_eq!(coords_r6.len(), 127);

        // All points inside unit disk
        for p in &coords_r6 {
            assert!(p.rho() <= 1.0 + 1e-12);
        }

        // Center ray is at (0, 0)
        assert_eq!(coords_r6[0], PupilCoord::new(0.0, 0.0));
    }

    #[test]
    fn test_cartesian_aplanat_zero_transverse_aberrations_and_spot() -> TestResult {
        // Analytical Cartesian aplanatic lens: flat front, hyperbolic rear (k = -n^2).
        // Zero longitudinal aberration, so ALL rays refract to exact same focus!
        // Consequently, on-axis ray fan and spot diagram must have ZERO aberration (< 1e-6 um)!
        let n = 1.5;
        let r2 = -30.0;
        let k2 = -(n * n); // k = -2.25
        let d = 10.0;
        let bfl = (-r2) / (n - 1.0); // 60.0 mm

        let presc = ResolvedPrescription {
            name: "Plano-Hyperbolic Aplanat".to_string(),
            entrance_pupil_diameter_mm: 16.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::plane(20.0)?.with_vertex_z(0.0),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: d,
                    material: Arc::new(ConstantIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::from_radius(r2, k2, 20.0)?.with_vertex_z(d),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: bfl,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
                ResolvedSurface {
                    surface_index: 3,
                    conic: ConicSurface::plane(20.0)?.with_vertex_z(d + bfl),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        // 1. Ray fan must be zero everywhere
        let fan = trace_ray_fan(&presc, (0.0, 0.0), 587.56, 31)?;
        assert_eq!(fan.tangential.len(), 31);
        for pt in &fan.tangential {
            assert!(
                pt.aberration_um.abs() < 1e-6,
                "Aplanat tangential aberration {} exceeds 1e-6 um at Py={}",
                pt.aberration_um,
                pt.pupil_coord
            );
        }
        for pt in &fan.sagittal {
            assert!(
                pt.aberration_um.abs() < 1e-6,
                "Aplanat sagittal aberration {} exceeds 1e-6 um at Px={}",
                pt.aberration_um,
                pt.pupil_coord
            );
        }

        // 2. Spot diagram must have zero RMS blur (< 1e-6 um) and be strictly diffraction-limited
        let spot = trace_spot_diagram(
            &presc,
            (0.0, 0.0),
            587.56,
            PupilPattern::Hexapolar { num_rings: 6 },
        )?;
        assert!(spot.metrics.rms_radius_um < 1e-6);
        assert!(spot.metrics.geo_radius_um < 1e-6);
        assert!(spot.metrics.is_diffraction_limited);
        assert_eq!(spot.metrics.passed_rays_count, 127);

        Ok(())
    }

    #[test]
    fn test_spherical_singlet_cubic_ray_fan_signature() -> TestResult {
        let n = 1.5168;
        let r1 = 50.0;
        let r2 = -50.0;
        let d = 5.0;

        let presc_nopower = ResolvedPrescription {
            name: "Spherical Singlet".to_string(),
            entrance_pupil_diameter_mm: 12.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0)?.with_vertex_z(0.0),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: d,
                    material: Arc::new(ConstantIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0)?.with_vertex_z(d),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let parax = calculate_paraxial_properties(&presc_nopower, 587.56)?;
        let bfl = parax.bfl_mm;

        // Add image plane surface at paraxial focus
        let presc = ResolvedPrescription {
            name: "Spherical Singlet Focused".to_string(),
            entrance_pupil_diameter_mm: 12.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0)?.with_vertex_z(0.0),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: d,
                    material: Arc::new(ConstantIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0)?.with_vertex_z(d),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: bfl,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
                ResolvedSurface {
                    surface_index: 3,
                    conic: ConicSurface::plane(15.0)?.with_vertex_z(d + bfl),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let fan = trace_ray_fan(&presc, (0.0, 0.0), 587.56, 41)?;

        // Find values at Py = 0.5 and Py = 1.0
        let pt_half = fan
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - 0.5).abs() < 1e-4)
            .ok_or("Py=0.5 must exist")?;
        let pt_full = fan
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - 1.0).abs() < 1e-4)
            .ok_or("Py=1.0 must exist")?;
        let pt_neg_full = fan
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - -1.0).abs() < 1e-4)
            .ok_or("Py=-1.0 must exist")?;

        // Odd symmetry: ey(-1.0) == -ey(1.0) within 1e-6
        assert!(
            (pt_full.aberration_um + pt_neg_full.aberration_um).abs() < 1e-4,
            "Spherical fan must exhibit odd symmetry"
        );

        // Non-zero spherical aberration at marginal pupil
        assert!(pt_full.aberration_um.abs() > 10.0);

        // Cubic scaling ratio: (1.0 / 0.5)^3 = 8.0 (allow small 5th order contribution)
        let ratio = pt_full.aberration_um / pt_half.aberration_um;
        assert!(
            (ratio - 8.0).abs() < 1.0,
            "Spherical fan scaling ratio mismatch: got {ratio}, expected ~8.0"
        );

        Ok(())
    }

    #[test]
    fn test_chromatic_fan_slope_difference() -> TestResult {
        let r1 = 50.0;
        let r2 = -50.0;
        let d = 5.0;

        let presc_d = ResolvedPrescription {
            name: "Chromatic Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0)?.with_vertex_z(0.0),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: d,
                    material: Arc::new(ConstantIndex(1.5168)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0)?.with_vertex_z(d),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let bfl_d = calculate_paraxial_properties(&presc_d, 587.56)?.bfl_mm;

        let build_system = |n_val: f64, wl: f64| -> Result<ResolvedPrescription, OpticsError> {
            Ok(ResolvedPrescription {
                name: "Chromatic Singlet Focused".to_string(),
                entrance_pupil_diameter_mm: 10.0,
                field_angles_deg: vec![0.0],
                wavelengths_nm: vec![wl],
                primary_wavelength_index: 0,
                surfaces: vec![
                    ResolvedSurface {
                        surface_index: 1,
                        conic: ConicSurface::sphere(r1, 15.0)?.with_vertex_z(0.0),
                        asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                        thickness_mm: d,
                        material: Arc::new(ConstantIndex(n_val)),
                        material_name: "Glass".to_string(),
                        is_stop: true,
                    },
                    ResolvedSurface {
                        surface_index: 2,
                        conic: ConicSurface::sphere(r2, 15.0)?.with_vertex_z(d),
                        asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                        thickness_mm: bfl_d,
                        material: Arc::new(Air),
                        material_name: "AIR".to_string(),
                        is_stop: false,
                    },
                    ResolvedSurface {
                        surface_index: 3,
                        conic: ConicSurface::plane(15.0)?.with_vertex_z(d + bfl_d),
                        asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                        thickness_mm: 0.0,
                        material: Arc::new(Air),
                        material_name: "AIR".to_string(),
                        is_stop: false,
                    },
                ],
            })
        };

        // Blue (F): n = 1.5224, Red (C): n = 1.5143
        let sys_f = build_system(1.5224, 486.13)?;
        let sys_c = build_system(1.5143, 656.27)?;

        let fan_f = trace_ray_fan(&sys_f, (0.0, 0.0), 486.13, 21)?;
        let fan_c = trace_ray_fan(&sys_c, (0.0, 0.0), 656.27, 21)?;

        let pt_f = fan_f
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - 0.2).abs() < 1e-4)
            .ok_or("Py=0.2 must exist for F-ray fan")?;
        let pt_c = fan_c
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - 0.2).abs() < 1e-4)
            .ok_or("Py=0.2 must exist for C-ray fan")?;

        assert!(
            pt_f.aberration_um < pt_c.aberration_um,
            "Blue marginal ray must hit lower than red on the paraxial d-plane (shorter focus)"
        );

        Ok(())
    }

    #[test]
    fn test_pupil_distribution_patterns() {
        let rect = generate_pupil_distribution(PupilPattern::Rectangular { grid_size: 10 });
        assert!(!rect.is_empty());
        for p in &rect {
            assert!(p.rho() <= 1.0 + 1e-12);
        }

        let cross = generate_pupil_distribution(PupilPattern::Cross { num_points: 15 });
        assert_eq!(cross.len(), 29);
        for p in &cross {
            assert!(p.rho() <= 1.0 + 1e-12);
            assert!(p.px == 0.0 || p.py == 0.0);
        }
    }

    #[test]
    fn test_empty_spot_metrics() {
        let metrics = spot_diagram_metrics(&[], 587.56, 4.5, 0);
        assert!(metrics.rms_radius_um.is_infinite());
        assert!(!metrics.is_diffraction_limited);
        assert_eq!(metrics.passed_rays_count, 0);
    }
}
