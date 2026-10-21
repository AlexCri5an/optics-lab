//! Surface-by-surface ray trace tables, beam footprint diagrams, and OPD fans.
//!
//! # Optical Foundations
//! - **Surface-by-Surface Ray Table:**
//!   Tabulates exact physical intersection coordinates $(x, y, z)$, direction cosines $(L, M, N)$,
//!   angle of incidence $\theta_i$, angle of refraction $\theta_t$, cumulative OPL, and Fresnel power
//!   coefficients at each optical surface.
//! - **Footprint Diagram:**
//!   Computes the 2D spatial distribution $(x_k, y_k)$ of real rays passing through a specific
//!   surface $k$, establishing the clear aperture requirement and ray bundle clearance.
//! - **Optical Path Difference Fans (OPD Fans):**
//!   - Tangential (meridional) OPD fan: $\text{OPD}(P_y)$ with $P_x = 0$.
//!   - Sagittal OPD fan: $\text{OPD}(P_x)$ with $P_y = 0$.

use serde::{Deserialize, Serialize};

use crate::analysis::spot_fan::{PupilCoord, PupilPattern, generate_pupil_distribution, launch_pupil_ray};
use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, calculate_paraxial_properties, trace_sequential};
use crate::types::{Ray, Vec3};

/// A single surface row in a surface-by-surface ray table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceRayRow {
    /// 0-indexed surface number.
    pub surface_index: usize,
    /// Intersection coordinate $x$ (mm).
    pub x_mm: f64,
    /// Intersection coordinate $y$ (mm).
    pub y_mm: f64,
    /// Intersection coordinate $z$ (mm).
    pub z_mm: f64,
    /// Direction cosine $L = d_x$.
    pub dir_x: f64,
    /// Direction cosine $M = d_y$.
    pub dir_y: f64,
    /// Direction cosine $N = d_z$.
    pub dir_z: f64,
    /// Surface normal vector $(n_x, n_y, n_z)$.
    pub normal: [f64; 3],
    /// Angle of incidence $\theta_i$ in degrees.
    pub angle_of_incidence_deg: f64,
    /// Angle of refraction $\theta_t$ in degrees.
    pub angle_of_refraction_deg: f64,
    /// Cumulative optical path length up to this surface (mm).
    pub cumulative_opl_mm: f64,
    /// Refractive index after this surface at the ray wavelength.
    pub refractive_index: f64,
    /// Surface transmission efficiency $\tau \in [0, 1]$.
    pub transmittance: f64,
}

/// Complete surface-by-surface ray trace table for a single ray.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RayTable {
    /// Initial launch ray origin $(x_0, y_0, z_0)$.
    pub launch_ray: Ray,
    /// Surface interaction rows.
    pub rows: Vec<SurfaceRayRow>,
    /// Whether the ray was vignetted before reaching the image plane.
    pub is_vignetted: bool,
    /// Total optical path length from launch to image plane (mm).
    pub total_opl_mm: f64,
}

/// Generates a complete surface-by-surface ray table for a given pupil coordinate $(P_x, P_y)$.
pub fn compute_ray_table(
    prescription: &ResolvedPrescription,
    px: f64,
    py: f64,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<RayTable, OpticsError> {
    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + paraxial.entrance_pupil_pos_mm;

    let ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(px, py),
        (0.0, field_angle_deg),
        wavelength_nm,
        z_enp,
    )?;
    let path = trace_sequential(prescription, &ray)?;

    let mut rows = Vec::with_capacity(path.hits.len());
    let mut cumulative_opl = 0.0_f64;
    let mut current_n = 1.0_f64;

    for (k, hit) in path.hits.iter().enumerate() {
        let prev_pt = path.points[k];
        let hit_pt = hit.point;
        let segment_len = (hit_pt - prev_pt).norm();
        cumulative_opl += current_n * segment_len;

        let next_n = if hit.surface_index < prescription.surfaces.len() {
            prescription.surfaces[hit.surface_index].material.n(wavelength_nm)
        } else {
            current_n
        };

        let diff = hit_pt - prev_pt;
        let diff_norm = diff.norm();
        let inc_dir = if diff_norm > 1e-12 {
            diff / diff_norm
        } else {
            path.final_ray.direction
        };

        let cos_i = (-inc_dir.dot(&hit.normal)).clamp(0.0, 1.0);
        let theta_i_deg = cos_i.acos().to_degrees();

        let sin_t = (current_n / next_n) * (1.0 - cos_i * cos_i).max(0.0).sqrt();
        let theta_t_deg = sin_t.clamp(0.0, 1.0).asin().to_degrees();

        rows.push(SurfaceRayRow {
            surface_index: hit.surface_index,
            x_mm: hit.point.x,
            y_mm: hit.point.y,
            z_mm: hit.point.z,
            dir_x: inc_dir.x,
            dir_y: inc_dir.y,
            dir_z: inc_dir.z,
            normal: [hit.normal.x, hit.normal.y, hit.normal.z],
            angle_of_incidence_deg: theta_i_deg,
            angle_of_refraction_deg: theta_t_deg,
            cumulative_opl_mm: cumulative_opl,
            refractive_index: next_n,
            transmittance: hit.bounce.t_total,
        });

        current_n = next_n;
    }

    Ok(RayTable {
        launch_ray: path.initial_ray,
        rows,
        is_vignetted: path.is_vignetted,
        total_opl_mm: cumulative_opl,
    })
}

/// A 2D intersection coordinate on an optical surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FootprintPoint {
    /// Spatial $x$ coordinate on the surface (mm).
    pub x_mm: f64,
    /// Spatial $y$ coordinate on the surface (mm).
    pub y_mm: f64,
    /// Radial height $r = \sqrt{x^2 + y^2}$ (mm).
    pub r_mm: f64,
    /// Whether the ray was vignetted at or before this surface.
    pub is_vignetted: bool,
}

/// Beam footprint diagram recording ray intersections on a specific surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootprintDiagram {
    /// Target surface index.
    pub target_surface_index: usize,
    /// Surface physical semi-diameter (mm).
    pub semi_diameter_mm: f64,
    /// Ray footprint points.
    pub points: Vec<FootprintPoint>,
    /// Minimum bounding radius containing all unvignetted rays (mm).
    pub max_beam_radius_mm: f64,
    /// Field angle in degrees.
    pub field_angle_deg: f64,
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
}

/// Computes the beam footprint on a designated optical surface across a pupil grid.
pub fn compute_footprint_diagram(
    prescription: &ResolvedPrescription,
    target_surface_index: usize,
    pupil_pattern: PupilPattern,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<FootprintDiagram, OpticsError> {
    if target_surface_index >= prescription.surfaces.len() {
        return Err(OpticsError::InvalidSurface(
            "target_surface_index out of prescription bounds".to_string(),
        ));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + paraxial.entrance_pupil_pos_mm;

    let semi_diameter = prescription.surfaces[target_surface_index].conic.semi_diam_mm;

    let coords = generate_pupil_distribution(pupil_pattern);
    let mut points = Vec::with_capacity(coords.len());
    let mut max_beam_radius = 0.0_f64;

    for coord in coords {
        if let Ok(ray) = launch_pupil_ray(prescription, coord, (0.0, field_angle_deg), wavelength_nm, z_enp) {
            if let Ok(path) = trace_sequential(prescription, &ray) {
                if let Some(hit) = path.hits.iter().find(|h| h.surface_index == target_surface_index) {
                    let r = (hit.point.x * hit.point.x + hit.point.y * hit.point.y).sqrt();
                    if !path.is_vignetted && r > max_beam_radius {
                        max_beam_radius = r;
                    }
                    points.push(FootprintPoint {
                        x_mm: hit.point.x,
                        y_mm: hit.point.y,
                        r_mm: r,
                        is_vignetted: path.is_vignetted,
                    });
                }
            }
        }
    }

    Ok(FootprintDiagram {
        target_surface_index,
        semi_diameter_mm: semi_diameter,
        points,
        max_beam_radius_mm: max_beam_radius,
        field_angle_deg,
        wavelength_nm,
    })
}

/// A point along an Optical Path Difference (OPD) aberration fan.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpdFanPoint {
    /// Normalized pupil coordinate (either $P_y$ for tangential or $P_x$ for sagittal).
    pub pupil_coord: f64,
    /// OPD in units of waves ($\lambda$).
    pub opd_waves: f64,
}

/// Complete OPD fan dataset (tangential and sagittal).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpdFanData {
    /// Tangential (meridional) fan: $\text{OPD}(P_y)$ with $P_x = 0$.
    pub tangential: Vec<OpdFanPoint>,
    /// Sagittal fan: $\text{OPD}(P_x)$ with $P_y = 0$.
    pub sagittal: Vec<OpdFanPoint>,
    /// Evaluation wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Field angle in degrees.
    pub field_angle_deg: f64,
}

/// Computes tangential and sagittal OPD aberration fans across the entrance pupil.
pub fn compute_opd_fan(
    prescription: &ResolvedPrescription,
    num_points: usize,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<OpdFanData, OpticsError> {
    if num_points < 3 {
        return Err(OpticsError::InvalidSurface("num_points must be at least 3".to_string()));
    }

    let paraxial = calculate_paraxial_properties(prescription, wavelength_nm)?;
    let z_first = prescription.surfaces.first().map_or(0.0, |s| s.conic.vertex_z);
    let z_enp = z_first + paraxial.entrance_pupil_pos_mm;

    let lambda_mm = wavelength_nm * 1e-6;

    // Chief ray reference OPL
    let chief_ray = launch_pupil_ray(
        prescription,
        PupilCoord::new(0.0, 0.0),
        (0.0, field_angle_deg),
        wavelength_nm,
        z_enp,
    )?;
    let chief_path = trace_sequential(prescription, &chief_ray)?;
    let chief_opl = calculate_opl(&chief_path.points, prescription, wavelength_nm);

    let step = 2.0 / (num_points - 1) as f64;

    // Tangential fan (Px = 0, Py from -1 to +1)
    let mut tangential = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let py = -1.0 + i as f64 * step;
        let opd_w = if let Ok(ray) = launch_pupil_ray(
            prescription,
            PupilCoord::new(0.0, py),
            (0.0, field_angle_deg),
            wavelength_nm,
            z_enp,
        ) {
            if let Ok(path) = trace_sequential(prescription, &ray) {
                if !path.is_vignetted {
                    let ray_opl = calculate_opl(&path.points, prescription, wavelength_nm);
                    (ray_opl - chief_opl) / lambda_mm
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        tangential.push(OpdFanPoint {
            pupil_coord: py,
            opd_waves: opd_w,
        });
    }

    // Sagittal fan (Px from -1 to +1, Py = 0)
    let mut sagittal = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let px = -1.0 + i as f64 * step;
        let opd_w = if let Ok(ray) = launch_pupil_ray(
            prescription,
            PupilCoord::new(px, 0.0),
            (0.0, field_angle_deg),
            wavelength_nm,
            z_enp,
        ) {
            if let Ok(path) = trace_sequential(prescription, &ray) {
                if !path.is_vignetted {
                    let ray_opl = calculate_opl(&path.points, prescription, wavelength_nm);
                    (ray_opl - chief_opl) / lambda_mm
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        sagittal.push(OpdFanPoint {
            pupil_coord: px,
            opd_waves: opd_w,
        });
    }

    Ok(OpdFanData {
        tangential,
        sagittal,
        wavelength_nm,
        field_angle_deg,
    })
}

fn calculate_opl(points: &[Vec3], prescription: &ResolvedPrescription, wavelength_nm: f64) -> f64 {
    let mut opl = 0.0_f64;
    let mut current_n = 1.0_f64;

    for i in 0..points.len().saturating_sub(1) {
        let dist = (points[i + 1] - points[i]).norm();
        opl += current_n * dist;

        if i < prescription.surfaces.len() {
            current_n = prescription.surfaces[i].material.n(wavelength_nm);
        }
    }
    opl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opd_fan_point_construction() {
        let pt = OpdFanPoint {
            pupil_coord: 0.5,
            opd_waves: 0.12,
        };
        assert_eq!(pt.pupil_coord, 0.5);
        assert_eq!(pt.opd_waves, 0.12);
    }
}
