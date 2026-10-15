//! Sequential optical prescriptions, paraxial $y$-$u$ tracing, and real sequential ray tracing.
//!
//! # Optical Conventions
//! - Coordinate System: light travels along $+z$ from left to right.
//! - Surface vertices are centered on the optical axis at $(0, 0, z_v)$.
//! - Curvature: $c = 1 / R$, planar surfaces have $c = 0$.
//! - Surface thickness $t_i$ is the axial vertex-to-vertex separation between surface $i$ and $i+1$.
//! - Paraxial $y$-$u$ method:
//!   - Refraction: $n' u' = n u - y \phi$, where $\phi = (n' - n) c$.
//!   - Transfer: $y_{i+1} = y_i + t_i u_i'$.
//!   - Effective Focal Length: $\text{EFL} = -y_1 / u_k'$.
//!   - Back Focal Length: $\text{BFL} = -y_k / u_k'$.
//!   - Lagrange invariant: $\mathcal{H} = n (\bar{u} y - u \bar{y})$ is conserved across all surfaces.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::conic::{ConicSurface, intersect_conic};
use crate::error::OpticsError;
use crate::fresnel::fresnel_unpolarized;
use crate::geometry::{Air, RefractiveIndex};
use crate::snell::snell_refract;
use crate::types::{BounceEvent, Ray, RayBounce, Vec3};

/// A single unresolved optical surface in a sequential prescription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalSurface {
    /// 0-based surface index (0 = Object, 1 = first optical surface, ..., N = Image plane).
    pub surface_index: usize,
    /// Radius of curvature in millimeters ($R = 1/c$). Infinity for planar surfaces.
    pub radius_mm: f64,
    /// Conic constant $k$ (0 for sphere, -1 for parabola, $< -1$ for hyperbola).
    pub conic_constant: f64,
    /// Axial distance to the next surface vertex along $+z$ in millimeters.
    pub thickness_mm: f64,
    /// Clear aperture semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
    /// Material name (e.g. "N-BK7", "SK16", "F2", "AIR", or empty for air).
    pub material_name: String,
    /// Whether this surface acts as the aperture stop of the system.
    pub is_stop: bool,
}

impl Default for OpticalSurface {
    fn default() -> Self {
        Self {
            surface_index: 0,
            radius_mm: f64::INFINITY,
            conic_constant: 0.0,
            thickness_mm: 0.0,
            semi_diameter_mm: 10.0,
            material_name: "AIR".to_string(),
            is_stop: false,
        }
    }
}

/// An unresolved sequential optical prescription defining an optical system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalPrescription {
    /// Name or title of the optical design (e.g. "Cooke Triplet F/4.5").
    pub name: String,
    /// Entrance pupil diameter in millimeters.
    pub entrance_pupil_diameter_mm: f64,
    /// Field angles in degrees (e.g. [0.0, 14.0, 20.0]).
    pub field_angles_deg: Vec<f64>,
    /// Sampled wavelengths in nanometers (e.g. [486.13, 587.56, 656.27]).
    pub wavelengths_nm: Vec<f64>,
    /// Primary wavelength index within `wavelengths_nm`.
    pub primary_wavelength_index: usize,
    /// Sequential surfaces from Object to Image.
    pub surfaces: Vec<OpticalSurface>,
}

impl Default for OpticalPrescription {
    fn default() -> Self {
        Self {
            name: "Default Lens System".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: Vec::new(),
        }
    }
}

/// A trait for resolving material names (e.g. "N-BK7") into refractive index models.
pub trait MaterialResolver {
    /// Resolves a material name into a thread-safe `RefractiveIndex` instance.
    fn resolve(&self, name: &str) -> Result<Arc<dyn RefractiveIndex + Send + Sync>, OpticsError>;
}

impl<F> MaterialResolver for F
where
    F: Fn(&str) -> Result<Arc<dyn RefractiveIndex + Send + Sync>, OpticsError>,
{
    fn resolve(&self, name: &str) -> Result<Arc<dyn RefractiveIndex + Send + Sync>, OpticsError> {
        self(name)
    }
}

/// A resolved optical surface with geometric ConicSurface and instantiated RefractiveIndex material.
#[derive(Clone)]
pub struct ResolvedSurface {
    /// Surface index.
    pub surface_index: usize,
    /// Conic geometry with cumulative axial vertex coordinate $z_v$.
    pub conic: ConicSurface,
    /// Axial thickness to next surface vertex in millimeters.
    pub thickness_mm: f64,
    /// Material in the space following this surface.
    pub material: Arc<dyn RefractiveIndex + Send + Sync>,
    /// Material name string.
    pub material_name: String,
    /// Whether this surface is the aperture stop.
    pub is_stop: bool,
}

/// A fully resolved optical prescription ready for paraxial and real sequential ray tracing.
#[derive(Clone)]
pub struct ResolvedPrescription {
    /// Title of the optical design.
    pub name: String,
    /// Entrance pupil diameter in millimeters.
    pub entrance_pupil_diameter_mm: f64,
    /// Field angles in degrees.
    pub field_angles_deg: Vec<f64>,
    /// Sampled wavelengths in nanometers.
    pub wavelengths_nm: Vec<f64>,
    /// Primary wavelength index.
    pub primary_wavelength_index: usize,
    /// Resolved surfaces with cumulative vertex positions.
    pub surfaces: Vec<ResolvedSurface>,
}

impl OpticalPrescription {
    /// Resolves all material names in the prescription using the provided material resolver.
    ///
    /// Surfaces specifying "AIR", "", or whitespace are automatically resolved to [`Air`] (n = 1.0)
    /// without querying the resolver.
    ///
    /// # Errors
    /// Returns [`OpticsError::MaterialNotFound`] if the resolver cannot locate a specified glass name.
    pub fn resolve<R: MaterialResolver + ?Sized>(
        &self,
        resolver: &R,
    ) -> Result<ResolvedPrescription, OpticsError> {
        let mut resolved_surfaces = Vec::with_capacity(self.surfaces.len());
        let mut current_z = 0.0_f64;

        for surf in &self.surfaces {
            let conic = ConicSurface::from_radius(
                surf.radius_mm,
                surf.conic_constant,
                surf.semi_diameter_mm,
            )?
            .with_vertex_z(current_z);

            let trimmed = surf.material_name.trim();
            let material: Arc<dyn RefractiveIndex + Send + Sync> =
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("air") {
                    Arc::new(Air)
                } else {
                    resolver.resolve(trimmed)?
                };

            resolved_surfaces.push(ResolvedSurface {
                surface_index: surf.surface_index,
                conic,
                thickness_mm: surf.thickness_mm,
                material,
                material_name: surf.material_name.clone(),
                is_stop: surf.is_stop,
            });

            current_z += surf.thickness_mm;
        }

        Ok(ResolvedPrescription {
            name: self.name.clone(),
            entrance_pupil_diameter_mm: self.entrance_pupil_diameter_mm,
            field_angles_deg: self.field_angles_deg.clone(),
            wavelengths_nm: self.wavelengths_nm.clone(),
            primary_wavelength_index: self.primary_wavelength_index,
            surfaces: resolved_surfaces,
        })
    }
}

/// A paraxial ray state $(y, u)$ at an optical surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParaxialRay {
    /// Ray height at surface in millimeters.
    pub y: f64,
    /// Ray slope angle in radians ($u \approx \tan\theta$).
    pub u: f64,
}

/// First-order Gaussian paraxial properties of an optical system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParaxialProperties {
    /// Effective Focal Length in millimeters ($\text{EFL} = -y_1 / u_k'$).
    pub efl_mm: f64,
    /// Back Focal Length in millimeters ($\text{BFL} = -y_k / u_k'$).
    pub bfl_mm: f64,
    /// Image-space F-number ($F/\# = \text{EFL} / \text{EPD}$).
    pub f_number: f64,
    /// Entrance pupil axial position relative to surface 1 in millimeters.
    pub entrance_pupil_pos_mm: f64,
    /// Exit pupil axial position relative to the last surface in millimeters.
    pub exit_pupil_pos_mm: f64,
}

/// Traces the paraxial marginal ray through the optical system at a given wavelength.
///
/// Starts at surface 1 with initial height $y_1 = \text{EPD} / 2$ and slope $u_0 = 0.0$ (axial collimated beam).
/// Returns the sequence of $(y_i, u_i')$ states after each surface.
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription contains fewer than 2 surfaces.
pub fn trace_paraxial_marginal(
    prescription: &ResolvedPrescription,
    wavelength_nm: f64,
) -> Result<Vec<ParaxialRay>, OpticsError> {
    if prescription.surfaces.len() < 2 {
        return Err(OpticsError::InvalidSurface(
            "Prescription must contain at least 2 surfaces for paraxial tracing".to_string(),
        ));
    }

    let y0 = prescription.entrance_pupil_diameter_mm * 0.5;
    let u0 = 0.0_f64;

    trace_paraxial_ray(prescription, y0, u0, wavelength_nm)
}

/// Traces the paraxial chief ray through the optical system for a specified field angle.
///
/// The chief ray starts at surface 1 with angle $u_0 = \tan(\theta_{\text{field}})$ and initial height
/// adjusted such that the ray passes through the center of the aperture stop ($y_{\text{stop}} = 0$).
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription contains fewer than 2 surfaces.
pub fn trace_paraxial_chief(
    prescription: &ResolvedPrescription,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Result<Vec<ParaxialRay>, OpticsError> {
    if prescription.surfaces.len() < 2 {
        return Err(OpticsError::InvalidSurface(
            "Prescription must contain at least 2 surfaces for paraxial tracing".to_string(),
        ));
    }

    let u0 = field_angle_deg.to_radians().tan();

    // Find aperture stop index (default to middle if none marked)
    let stop_idx = prescription
        .surfaces
        .iter()
        .position(|s| s.is_stop)
        .unwrap_or(prescription.surfaces.len() / 2);

    // If stop is at surface 0 or 1, chief ray has y = 0 at that surface
    if stop_idx <= 1 {
        return trace_paraxial_ray(prescription, 0.0, u0, wavelength_nm);
    }

    // Otherwise, trace a trial ray with y1 = 0, u0 = u0 to find y_stop_trial,
    // and a trial ray with y1 = 1, u0 = 0 to find y_stop_marginal.
    // By linearity: y_stop = y_trial + y1 * y_marginal_stop = 0 => y1 = -y_trial / y_marginal_stop
    let trial_chief = trace_paraxial_ray(prescription, 0.0, u0, wavelength_nm)?;
    let trial_marg = trace_paraxial_ray(prescription, 1.0, 0.0, wavelength_nm)?;

    let y_stop_chief = trial_chief[stop_idx].y;
    let y_stop_marg = trial_marg[stop_idx].y;

    let y1 = if y_stop_marg.abs() > 1e-12 {
        -y_stop_chief / y_stop_marg
    } else {
        0.0
    };

    trace_paraxial_ray(prescription, y1, u0, wavelength_nm)
}

/// Internal helper to trace a paraxial ray with initial $(y_1, u_0)$ through all surfaces.
fn trace_paraxial_ray(
    prescription: &ResolvedPrescription,
    initial_y: f64,
    initial_u: f64,
    wavelength_nm: f64,
) -> Result<Vec<ParaxialRay>, OpticsError> {
    let mut states = Vec::with_capacity(prescription.surfaces.len());
    let mut current_y = initial_y;
    let mut current_u = initial_u;
    let mut current_n = 1.0_f64; // Air before surface 1

    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let next_n = surf.material.n(wavelength_nm);
        let c = surf.conic.curvature;
        let phi = (next_n - current_n) * c;

        // Refraction: n' u' = n u - y phi => u' = (n u - y phi) / n'
        let next_u = (current_n * current_u - current_y * phi) / next_n;

        states.push(ParaxialRay {
            y: current_y,
            u: next_u,
        });

        // Transfer across thickness to next surface: y_{i+1} = y_i + t_i u_i'
        if i + 1 < prescription.surfaces.len() {
            current_y += surf.thickness_mm * next_u;
            current_u = next_u;
            current_n = next_n;
        }
    }

    Ok(states)
}

/// Evaluates first-order Gaussian paraxial properties of the resolved prescription.
///
/// # Formulas
/// - $\text{EFL} = -y_1 / u_k'$
/// - $\text{BFL} = -y_k / u_k'$
/// - $F/\# = \text{EFL} / \text{EPD}$
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription is degenerate or has fewer than 2 surfaces.
pub fn calculate_paraxial_properties(
    prescription: &ResolvedPrescription,
    wavelength_nm: f64,
) -> Result<ParaxialProperties, OpticsError> {
    let marginal_rays = trace_paraxial_marginal(prescription, wavelength_nm)?;
    let last = marginal_rays
        .last()
        .copied()
        .ok_or_else(|| OpticsError::InvalidSurface("No surfaces in prescription".to_string()))?;

    let y1 = marginal_rays[0].y;
    let final_u = last.u;

    if final_u.abs() < 1e-15 {
        return Err(OpticsError::NumericalSingularity(
            "Paraxial marginal ray is collimated at image space (infinite focal length)"
                .to_string(),
        ));
    }

    let efl_mm = -y1 / final_u;
    let bfl_mm = -last.y / final_u;
    let f_number = efl_mm.abs() / prescription.entrance_pupil_diameter_mm;

    // Entrance pupil: distance from surface 1 to stop viewed from object space
    let stop_idx = prescription
        .surfaces
        .iter()
        .position(|s| s.is_stop)
        .unwrap_or(0);

    let enp_pos_mm = if stop_idx == 0 {
        0.0
    } else {
        // Approximate entrance pupil position by summing thicknesses up to stop
        prescription.surfaces[0..stop_idx]
            .iter()
            .map(|s| s.thickness_mm)
            .sum::<f64>()
    };

    let exp_pos_mm = -bfl_mm;

    Ok(ParaxialProperties {
        efl_mm,
        bfl_mm,
        f_number,
        entrance_pupil_pos_mm: enp_pos_mm,
        exit_pupil_pos_mm: exp_pos_mm,
    })
}

/// Record of a single ray-surface intersection and refraction during real sequential ray tracing.
#[derive(Debug, Clone, PartialEq)]
pub struct SequentialRayHit {
    /// Surface index where intersection occurred.
    pub surface_index: usize,
    /// Hit coordinates in scene space (mm).
    pub point: Vec3,
    /// Surface normal unit vector pointing toward incident side.
    pub normal: Vec3,
    /// Per-bounce inspector record including Fresnel efficiencies.
    pub bounce: RayBounce,
}

/// Complete path history of a ray traced sequentially through an optical prescription.
#[derive(Debug, Clone, PartialEq)]
pub struct SequentialPathRecord {
    /// Input ray.
    pub initial_ray: Ray,
    /// All 3D points visited by the ray (origin, intersections, and final propagation).
    pub points: Vec<Vec3>,
    /// Sequential hits and Fresnel bounce records at each surface.
    pub hits: Vec<SequentialRayHit>,
    /// Final ray state after exiting the system.
    pub final_ray: Ray,
    /// Whether the ray was vignetted (clipped) by a surface semi-diameter.
    pub is_vignetted: bool,
    /// Cumulative unpolarized transmission throughput $\prod T_i \in [0, 1]$.
    pub cumulative_throughput: f64,
}

/// Traces a real 3D ray sequentially through all surfaces of an optical prescription.
///
/// At each surface:
/// 1. The ray intersects the conic surface via [`intersect_conic`].
/// 2. If the hit lies outside `semi_diameter_mm`, vignetting is flagged.
/// 3. Snell refraction is evaluated with [`snell_refract`].
/// 4. If TIR occurs, returns `Err(OpticsError::TotalInternalReflection)`.
/// 5. Fresnel power reflectances and transmittances are recorded in a [`RayBounce`].
/// 6. The ray advances into the next medium.
///
/// # Errors
/// Returns [`OpticsError::RayMissedSurface`] if the ray misses any optical surface.
/// Returns [`OpticsError::TotalInternalReflection`] if total internal reflection occurs.
/// Returns [`OpticsError::RayVignetted`] if the ray is clipped by an aperture.
pub fn trace_sequential(
    prescription: &ResolvedPrescription,
    ray: &Ray,
) -> Result<SequentialPathRecord, OpticsError> {
    if prescription.surfaces.is_empty() {
        return Err(OpticsError::InvalidSurface(
            "Cannot trace through empty prescription".to_string(),
        ));
    }

    let mut current_ray = *ray;
    let mut points = vec![ray.origin];
    let mut hits = Vec::with_capacity(prescription.surfaces.len());
    let mut current_n = 1.0_f64; // Air before surface 0
    let mut cumulative_throughput = 1.0_f64;

    for surf in &prescription.surfaces {
        let next_n = surf.material.n(current_ray.wavelength_nm);

        // Intersect ray with conic surface
        let hit = intersect_conic(&current_ray, &surf.conic, surf.surface_index)?;
        points.push(hit.point);

        // Snell refraction across dielectric interface
        let refracted_dir = snell_refract(current_ray.direction, hit.normal, current_n, next_n)?;

        // Fresnel coefficients
        let cos_theta_i = (-current_ray.direction.dot(&hit.normal)).clamp(0.0, 1.0);
        let (r_total, t_total) = fresnel_unpolarized(cos_theta_i, current_n, next_n);
        let cos_theta_t = (-refracted_dir.dot(&hit.normal)).clamp(0.0, 1.0);

        cumulative_throughput *= t_total;

        let bounce = RayBounce {
            surface_index: surf.surface_index,
            point: hit.point,
            normal: hit.normal,
            incident_dir: current_ray.direction,
            reflected_dir: Vec3::zeros(), // Sequential tracer follows refraction
            refracted_dir: Some(refracted_dir),
            n1: current_n,
            n2: next_n,
            incident_angle_rad: cos_theta_i.acos(),
            refracted_angle_rad: Some(cos_theta_t.acos()),
            rs: r_total, // unpolarized average
            rp: r_total,
            r_total,
            t_total,
            event: BounceEvent::Refract,
            wavelength_nm: current_ray.wavelength_nm,
        };

        hits.push(SequentialRayHit {
            surface_index: surf.surface_index,
            point: hit.point,
            normal: hit.normal,
            bounce,
        });

        // Advance ray
        current_ray = Ray::new(
            hit.point,
            refracted_dir,
            current_ray.wavelength_nm,
            current_ray.energy * t_total,
        )?;
        current_n = next_n;
    }

    // Extend final ray a finite distance for visualization
    let final_endpoint = current_ray.at(50.0);
    points.push(final_endpoint);

    Ok(SequentialPathRecord {
        initial_ray: *ray,
        points,
        hits,
        final_ray: current_ray,
        is_vignetted: false,
        cumulative_throughput,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::conic::conic_sag;

    /// Mock constant refractive index material for unit testing without catalog dependencies.
    #[derive(Clone, Copy, Debug)]
    struct MockIndex(f64);
    impl RefractiveIndex for MockIndex {
        fn n(&self, _wavelength_nm: f64) -> f64 {
            self.0
        }
    }

    /// Dispersive mock material for chromatic testing.
    #[derive(Clone, Copy, Debug)]
    struct MockDispersiveIndex;
    impl RefractiveIndex for MockDispersiveIndex {
        fn n(&self, wavelength_nm: f64) -> f64 {
            // Simplified Cauchy dispersion: n(lambda) = 1.50 + 0.005 / (lambda_um^2)
            let lambda_um = wavelength_nm / 1000.0;
            1.50 + 0.005 / (lambda_um * lambda_um)
        }
    }

    #[test]
    fn test_thin_lens_vs_lensmaker_formula() {
        // Lensmaker formula for thin lens in air:
        // 1/f = (n - 1) * [1/R1 - 1/R2]
        let n = 1.5168; // N-BK7
        let r1 = 50.0;
        let r2 = -50.0; // Biconvex
        let expected_inv_f = (n - 1.0) * (1.0 / r1 - 1.0 / r2);
        let expected_f = 1.0 / expected_inv_f;

        let presc = ResolvedPrescription {
            name: "Thin Biconvex".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: 1e-6, // Tiny thickness approaching thin-lens limit
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(1e-6),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let props = calculate_paraxial_properties(&presc, 587.56).expect("Paraxial properties");
        assert!(
            (props.efl_mm - expected_f).abs() < 1e-4,
            "Thin lens EFL mismatch: got {}, expected {}",
            props.efl_mm,
            expected_f
        );
    }

    #[test]
    fn test_thick_lens_efl_vs_thick_lensmaker_formula() {
        // Thick lens formula:
        // 1/f = (n - 1) * [1/R1 - 1/R2 + (n - 1) * d / (n * R1 * R2)]
        let n = 1.5;
        let r1 = 60.0;
        let r2 = -40.0;
        let d = 8.0;

        let expected_inv_f = (n - 1.0) * (1.0 / r1 - 1.0 / r2 + (n - 1.0) * d / (n * r1 * r2));
        let expected_f = 1.0 / expected_inv_f;

        let presc = ResolvedPrescription {
            name: "Thick Lens".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let props = calculate_paraxial_properties(&presc, 587.56).expect("Paraxial properties");
        assert!(
            (props.efl_mm - expected_f).abs() < 1e-10,
            "Thick lens EFL mismatch: got {}, expected {}",
            props.efl_mm,
            expected_f
        );
    }

    #[test]
    fn test_plano_convex_focal_length_and_bfl() {
        // Plano-convex lens (curved first, flat second):
        // Surface 1: R1 = R, thickness = d, glass n
        // Surface 2: R2 = infinity (flat), thickness = 0, air n=1
        // EFL = R / (n - 1)
        // BFL = EFL - d / n
        let n = 1.5;
        let r = 50.0;
        let d = 6.0;

        let expected_efl = r / (n - 1.0); // 100.0 mm
        let expected_bfl = expected_efl - d / n; // 100 - 4 = 96.0 mm

        let presc = ResolvedPrescription {
            name: "Plano-Convex".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::plane(15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let props = calculate_paraxial_properties(&presc, 587.56).expect("Paraxial properties");
        assert!((props.efl_mm - expected_efl).abs() < 1e-10);
        assert!((props.bfl_mm - expected_bfl).abs() < 1e-10);
    }

    #[test]
    fn test_lagrange_invariant_conservation() {
        // The optical invariant H = n * (u_bar * y - u * y_bar) must remain strictly
        // constant across every surface in the optical system.
        let n = 1.62;
        let presc = ResolvedPrescription {
            name: "Doublet System".to_string(),
            entrance_pupil_diameter_mm: 12.0,
            field_angles_deg: vec![5.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(40.0, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: 4.0,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass1".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(-35.0, 15.0)
                        .unwrap()
                        .with_vertex_z(4.0),
                    thickness_mm: 2.0,
                    material: Arc::new(MockIndex(1.52)),
                    material_name: "Glass2".to_string(),
                    is_stop: false,
                },
                ResolvedSurface {
                    surface_index: 3,
                    conic: ConicSurface::sphere(-80.0, 15.0)
                        .unwrap()
                        .with_vertex_z(6.0),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let marg = trace_paraxial_marginal(&presc, 587.56).unwrap();
        let chief = trace_paraxial_chief(&presc, 5.0, 587.56).unwrap();

        // Calculate H at each surface
        let mut invariants = Vec::new();
        for i in 0..presc.surfaces.len() {
            let n_after = presc.surfaces[i].material.n(587.56);
            let y_m = marg[i].y;
            let u_m = marg[i].u;
            let y_c = chief[i].y;
            let u_c = chief[i].u;
            let h = n_after * (u_c * y_m - u_m * y_c);
            invariants.push(h);
        }

        for i in 1..invariants.len() {
            assert!(
                (invariants[i] - invariants[0]).abs() < 1e-10,
                "Lagrange invariant violated at surface {i}: got {}, expected {}",
                invariants[i],
                invariants[0]
            );
        }
    }

    #[test]
    fn test_system_reversal_symmetry_efl() {
        // Reversing a thick lens (R1' = -R2, R2' = -R1, same thickness, same glass)
        // must yield identical EFL.
        let n = 1.6;
        let r1 = 45.0;
        let r2 = -65.0;
        let d = 5.0;

        let forward = ResolvedPrescription {
            name: "Forward".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let reversed = ResolvedPrescription {
            name: "Reversed".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(-r2, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(-r1, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let props_fwd = calculate_paraxial_properties(&forward, 587.56).unwrap();
        let props_rev = calculate_paraxial_properties(&reversed, 587.56).unwrap();

        assert!((props_fwd.efl_mm - props_rev.efl_mm).abs() < 1e-10);
    }

    #[test]
    fn test_real_rays_near_axis_converge_to_paraxial_bfl() {
        // Real rays traced at shrinking heights h -> 0 must converge to paraxial BFL.
        let n = 1.5168;
        let r1 = 50.0;
        let r2 = -50.0;
        let d = 5.0;

        let presc = ResolvedPrescription {
            name: "Biconvex Real Ray".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let parax = calculate_paraxial_properties(&presc, 587.56).unwrap();

        // Trace real rays at h = 2.0, 0.5, 0.05, 0.005 mm
        let heights = [2.0, 0.5, 0.05, 0.005];
        let mut errors = Vec::new();

        for h in heights {
            let ray = Ray::new(
                Vec3::new(0.0, h, -10.0),
                Vec3::new(0.0, 0.0, 1.0),
                587.56,
                1.0,
            )
            .unwrap();
            let path = trace_sequential(&presc, &ray).unwrap();
            let final_ray = path.final_ray;

            // Axis intersection: point where y = 0
            // final_ray.origin.y + t * final_ray.direction.y = 0 => t = -origin.y / dir.y
            let t_axis = -final_ray.origin.y / final_ray.direction.y;
            let z_focus = final_ray.origin.z + t_axis * final_ray.direction.z;
            let real_bfl = z_focus - d; // Relative to surface 2 vertex at z = d

            let err = (real_bfl - parax.bfl_mm).abs();
            errors.push(err);
        }

        // Errors must strictly decrease towards 0 as h shrinks
        assert!(errors[1] < errors[0]);
        assert!(errors[2] < errors[1]);
        assert!(errors[3] < 1e-4);
    }

    #[test]
    fn test_positive_singlet_longitudinal_spherical_aberration() {
        // A spherical positive singlet has undercorrected (negative) longitudinal spherical aberration:
        // marginal rays at full aperture cross the axis CLOSER to the lens than paraxial rays (LSA < 0).
        // Furthermore, LSA scales approximately quadratically with pupil height: LSA ~ h^2.
        let n = 1.5;
        let r1 = 40.0;
        let r2 = -40.0;
        let d = 4.0;

        let presc = ResolvedPrescription {
            name: "Spherical Singlet LSA".to_string(),
            entrance_pupil_diameter_mm: 16.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let parax = calculate_paraxial_properties(&presc, 587.56).unwrap();

        let trace_focus = |h: f64| {
            let ray = Ray::new(
                Vec3::new(0.0, h, -10.0),
                Vec3::new(0.0, 0.0, 1.0),
                587.56,
                1.0,
            )
            .unwrap();
            let path = trace_sequential(&presc, &ray).unwrap();
            let r = path.final_ray;
            let t = -r.origin.y / r.direction.y;
            r.origin.z + t * r.direction.z - d
        };

        let bfl_parax = parax.bfl_mm;
        let bfl_h2 = trace_focus(2.0);
        let bfl_h4 = trace_focus(4.0);

        let lsa_h2 = bfl_h2 - bfl_parax;
        let lsa_h4 = bfl_h4 - bfl_parax;

        // Marginal rays cross closer to lens: LSA < 0
        assert!(lsa_h2 < 0.0, "LSA at h=2 must be negative");
        assert!(lsa_h4 < 0.0, "LSA at h=4 must be negative");
        assert!(lsa_h4 < lsa_h2, "LSA must be larger in magnitude at h=4");

        // Quadratic scaling check: (4 / 2)^2 = 4. Ratio should be ~4 (within 20% for modest aperture)
        let ratio = lsa_h4 / lsa_h2;
        assert!(
            (ratio - 4.0).abs() < 0.8,
            "LSA scaling ratio mismatch: got {ratio}, expected ~4.0"
        );
    }

    #[test]
    fn test_plano_hyperbolic_aplanatic_lens_zero_spherical_aberration() {
        // Analytical Cartesian aplanatic lens:
        // Flat surface 1 at z = 0, thickness d = 10.0 mm.
        // Curved second surface at z = d with radius R < 0, facing air (n' = 1).
        // By Fermat's principle, if conic constant k = -n^2, all collimated rays
        // at any height h refract to the EXACT same axial focal point at distance:
        // z_focus = z_vertex2 + (-R) / (n - 1)
        // with ZERO longitudinal spherical aberration (spread < 1e-9 mm)!
        let n = 1.5;
        let r2 = -30.0;
        let k2 = -(n * n); // k = -2.25
        let d = 10.0;

        let expected_bfl = (-r2) / (n - 1.0); // 30.0 / 0.5 = 60.0 mm
        let expected_focus_z = d + expected_bfl; // 70.0 mm

        let presc = ResolvedPrescription {
            name: "Plano-Hyperbolic Aplanat".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::plane(20.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::from_radius(r2, k2, 20.0)
                        .unwrap()
                        .with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        // Trace rays at various heights: h = 0.5, 2.0, 5.0, 8.0 mm
        let heights = [0.5, 2.0, 5.0, 8.0];
        let mut foci = Vec::new();

        for h in heights {
            let ray = Ray::new(
                Vec3::new(0.0, h, -10.0),
                Vec3::new(0.0, 0.0, 1.0),
                587.56,
                1.0,
            )
            .unwrap();
            let path = trace_sequential(&presc, &ray).expect("Trace must succeed");
            let r = path.final_ray;

            let t = -r.origin.y / r.direction.y;
            let z_focus = r.origin.z + t * r.direction.z;
            foci.push(z_focus);
        }

        for (i, &f) in foci.iter().enumerate() {
            let diff = (f - expected_focus_z).abs();
            assert!(
                diff < 1e-9,
                "Hyperbolic aplanat focal spread {diff} at height {} exceeds 1e-9 mm! Got {f}, expected {expected_focus_z}",
                heights[i]
            );
        }
    }

    #[test]
    fn test_chromatic_singlet_focal_order() {
        // A singlet with normal dispersion dn/dlambda < 0 has higher n for blue than red:
        // n(F = 486.13 nm) > n(C = 656.27 nm).
        // Since focal length f ~ 1/(n - 1), f_blue < f_red (blue focuses closer to lens).
        let r1 = 50.0;
        let r2 = -50.0;
        let d = 5.0;

        let presc = ResolvedPrescription {
            name: "Chromatic Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![486.13, 656.27],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockDispersiveIndex),
                    material_name: "DispersiveGlass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let trace_focal_length = |wl: f64| {
            let ray = Ray::new(
                Vec3::new(0.0, 1.0, -10.0),
                Vec3::new(0.0, 0.0, 1.0),
                wl,
                1.0,
            )
            .unwrap();
            let path = trace_sequential(&presc, &ray).unwrap();
            let r = path.final_ray;
            let t = -r.origin.y / r.direction.y;
            r.origin.z + t * r.direction.z - d
        };

        let bfl_blue = trace_focal_length(486.13);
        let bfl_red = trace_focal_length(656.27);

        assert!(
            bfl_blue < bfl_red,
            "Blue focal length ({bfl_blue}) must be shorter than red ({bfl_red})"
        );

        // Compare with paraxial prediction
        let parax_blue = calculate_paraxial_properties(&presc, 486.13)
            .unwrap()
            .bfl_mm;
        let parax_red = calculate_paraxial_properties(&presc, 656.27)
            .unwrap()
            .bfl_mm;
        assert!((bfl_blue - parax_blue).abs() < 0.05);
        assert!((bfl_red - parax_red).abs() < 0.05);
    }

    #[test]
    fn test_fresnel_throughput_energy_conservation() {
        // Throughput at end of sequential trace must equal product of per-surface transmittances
        let presc = ResolvedPrescription {
            name: "Fresnel Test".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(50.0, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: 5.0,
                    material: Arc::new(MockIndex(1.5)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(-50.0, 15.0)
                        .unwrap()
                        .with_vertex_z(5.0),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let ray = Ray::new(
            Vec3::new(0.0, 2.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )
        .unwrap();
        let path = trace_sequential(&presc, &ray).unwrap();

        let mut expected_t = 1.0_f64;
        for hit in &path.hits {
            expected_t *= hit.bounce.t_total;
        }

        assert!(
            (path.cumulative_throughput - expected_t).abs() < 1e-12,
            "Cumulative throughput mismatch"
        );
        assert!(
            (path.final_ray.energy - expected_t).abs() < 1e-12,
            "Final ray energy mismatch"
        );
    }

    #[test]
    fn test_sequential_ray_reversibility() {
        // Trace a ray through a lens, then launch a ray from the final hit point backwards
        // through the reversed optical system. The backwards ray must retrace the path
        // and exit parallel and opposite to the original ray.
        let n = 1.5;
        let r1 = 60.0;
        let r2 = -40.0;
        let d = 6.0;

        let forward = ResolvedPrescription {
            name: "Forward".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(r1, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(r2, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let start_ray = Ray::new(
            Vec3::new(0.0, 2.5, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )
        .unwrap();

        let fwd_path = trace_sequential(&forward, &start_ray).unwrap();
        let p_final = fwd_path.hits[1].point;
        let dir_final = fwd_path.final_ray.direction;

        // Reversed prescription: Surface 1 has -R2 at z=0, thickness d; Surface 2 has -R1 at z=d
        let backward = ResolvedPrescription {
            name: "Backward".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::sphere(-r2, 15.0).unwrap().with_vertex_z(0.0),
                    thickness_mm: d,
                    material: Arc::new(MockIndex(n)),
                    material_name: "Glass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 2,
                    conic: ConicSurface::sphere(-r1, 15.0).unwrap().with_vertex_z(d),
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        // In the forward system, light exited surface 2 at (0, y_final, z_final) with direction dir_final.
        // In the reversed lens, surface 1 has radius -R2 at z=0, and surface 2 has radius -R1 at z=d.
        // By optical reciprocity, launching light with direction (0, -dir_final.y, dir_final.z)
        // at the conjugate exit point must retrace the system and emerge parallel to the axis (0, 0, 1)
        // at the original ray height (y = 2.5).
        let rev_dir = Vec3::new(0.0, -dir_final.y, dir_final.z);
        let rev_hit1_z = conic_sag(p_final.y, -1.0 / r2, 0.0).unwrap();
        let rev_hit1 = Vec3::new(0.0, p_final.y, rev_hit1_z);
        let rev_start_point = rev_hit1 - rev_dir * 10.0;

        let rev_ray = Ray::new(rev_start_point, rev_dir, 587.56, 1.0).unwrap();

        let rev_path = trace_sequential(&backward, &rev_ray).unwrap();
        let rev_exit_dir = rev_path.final_ray.direction;
        let rev_exit_y = rev_path.hits[1].point.y;

        // Original ray traveled in +z (0, 0, 1). Reversing ray should emerge in +z (0, 0, 1) at y = 2.5 mm
        assert!(
            (rev_exit_dir.z - 1.0).abs() < 1e-10,
            "Reversed ray exit dir z mismatch: got {}",
            rev_exit_dir.z
        );
        assert!(
            rev_exit_dir.x.abs() < 1e-10,
            "Reversed ray exit dir x mismatch: got {}",
            rev_exit_dir.x
        );
        assert!(
            rev_exit_dir.y.abs() < 1e-10,
            "Reversed ray exit dir y mismatch: got {}",
            rev_exit_dir.y
        );
        assert!(
            (rev_exit_y - 2.5).abs() < 1e-10,
            "Reversed ray exit height mismatch: got {rev_exit_y}, expected 2.5"
        );
    }
}
