//! Non-sequential ray path tracer with multi-bounce tracking and branching policies.

use crate::error::OpticsError;
use crate::fresnel::fresnel_dielectric;
use crate::geometry::{RefractiveIndex, Triangle};
use crate::snell::{reflect, snell_refract};
use crate::types::{BounceEvent, HitRecord, Ray, RayBounce};

/// Policy governing ray propagation branching at dielectric boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RayBranchPolicy {
    /// Follow the transmitted (refracted) ray path. If TIR occurs, follow reflection.
    FollowTransmission,
    /// Follow the specularly reflected ray path.
    FollowReflection,
    /// Branch deterministically or stochastically along the path with highest Fresnel throughput.
    Stochastic,
}

/// Full record of a ray's non-sequential path trajectory through an optical scene.
#[derive(Debug, Clone, PartialEq)]
pub struct PathRecord {
    /// Ordered sequence of bounce interaction records.
    pub bounces: Vec<RayBounce>,
    /// Whether the path terminated within `max_bounces` (due to escape, absorption, or exhaustion).
    pub terminated: bool,
}

/// An optical scene consisting of boundary triangles and refractive media tables.
pub struct Scene {
    /// Collection of non-sequential boundary triangles.
    pub triangles: Vec<Triangle>,
    /// Refractive index models indexed by medium ID.
    pub media: Vec<Box<dyn RefractiveIndex>>,
}

impl Scene {
    /// Creates a new scene with specified triangles and media table.
    #[must_use]
    pub fn new(triangles: Vec<Triangle>, media: Vec<Box<dyn RefractiveIndex>>) -> Self {
        Self { triangles, media }
    }

    /// Finds the closest triangle intersection for a given ray in $[t_{\min}, t_{\max}]$.
    #[must_use]
    pub fn intersect(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<(HitRecord, &Triangle)> {
        let mut closest_hit: Option<(HitRecord, &Triangle)> = None;
        let mut closest_t = t_max;

        for tri in &self.triangles {
            if let Some(hit) = tri.intersect(ray, t_min, closest_t) {
                closest_t = hit.t;
                closest_hit = Some((hit, tri));
            }
        }

        closest_hit
    }
}

/// Traces a single ray non-sequentially through a triangle scene up to `max_bounces`.
///
/// # Arguments
/// * `scene` — The optical scene with geometry and media definitions.
/// * `ray` — Initial incoming ray.
/// * `policy` — Branching decision policy at interfaces.
/// * `max_bounces` — Maximum number of interaction bounces allowed.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if numerical degradation occurs.
pub fn trace_path(
    scene: &Scene,
    ray: &Ray,
    policy: RayBranchPolicy,
    max_bounces: usize,
) -> Result<PathRecord, OpticsError> {
    let mut bounces = Vec::with_capacity(max_bounces);
    let mut current_ray = *ray;
    let mut terminated = false;

    for _ in 0..max_bounces {
        // Find closest surface intersection (offset t_min=1e-5 to avoid self-intersection)
        let Some((hit, tri)) = scene.intersect(&current_ray, 1e-5, 1e9) else {
            // Ray escaped scene into free space
            terminated = true;
            break;
        };

        // Determine refractive indices on both sides of the boundary
        let (medium_from, medium_to) = if hit.is_front_face {
            (tri.medium_outside, tri.medium_inside)
        } else {
            (tri.medium_inside, tri.medium_outside)
        };

        let n1 = scene
            .media
            .get(medium_from)
            .map_or(1.0, |m| m.n(current_ray.wavelength_nm));
        let n2 = scene
            .media
            .get(medium_to)
            .map_or(1.0, |m| m.n(current_ray.wavelength_nm));

        let cos_i = (-current_ray.direction.dot(&hit.normal)).clamp(0.0, 1.0);
        let incident_angle_rad = cos_i.acos();

        // Exact specular reflection
        let reflected_dir = reflect(current_ray.direction, hit.normal)?;

        // Fresnel coefficients
        let (rs, rp) = fresnel_dielectric(cos_i, n1, n2);
        let r_total = (rs + rp) * 0.5;
        let t_total = (1.0 - r_total).clamp(0.0, 1.0);

        // Snell's Law refraction
        let snell_res = snell_refract(current_ray.direction, hit.normal, n1, n2);

        let (event, next_dir, next_energy, refracted_dir, refracted_angle_rad) = match snell_res {
            Err(OpticsError::TotalInternalReflection) => {
                // Must reflect on TIR
                (
                    BounceEvent::TotalInternalReflection,
                    reflected_dir,
                    current_ray.energy,
                    None,
                    None,
                )
            }
            Ok(refracted) => {
                let cos_t = (-refracted.dot(&hit.normal)).clamp(0.0, 1.0);
                let refr_angle = cos_t.acos();

                match policy {
                    RayBranchPolicy::FollowTransmission => (
                        BounceEvent::Refract,
                        refracted,
                        current_ray.energy * t_total,
                        Some(refracted),
                        Some(refr_angle),
                    ),
                    RayBranchPolicy::FollowReflection => (
                        BounceEvent::Reflect,
                        reflected_dir,
                        current_ray.energy * r_total,
                        Some(refracted),
                        Some(refr_angle),
                    ),
                    RayBranchPolicy::Stochastic => {
                        // Route along the channel with higher energy throughput
                        if t_total >= r_total {
                            (
                                BounceEvent::Refract,
                                refracted,
                                current_ray.energy * t_total,
                                Some(refracted),
                                Some(refr_angle),
                            )
                        } else {
                            (
                                BounceEvent::Reflect,
                                reflected_dir,
                                current_ray.energy * r_total,
                                Some(refracted),
                                Some(refr_angle),
                            )
                        }
                    }
                }
            }
            Err(e) => return Err(e),
        };

        bounces.push(RayBounce {
            surface_index: hit.surface_index,
            point: hit.point,
            normal: hit.normal,
            incident_dir: current_ray.direction,
            reflected_dir,
            refracted_dir,
            n1,
            n2,
            incident_angle_rad,
            refracted_angle_rad,
            rs,
            rp,
            r_total,
            t_total,
            event,
            wavelength_nm: current_ray.wavelength_nm,
        });

        // Advance ray with epsilon standoff in next propagation direction
        current_ray = Ray::new(
            hit.point + next_dir * 1e-5,
            next_dir,
            current_ray.wavelength_nm,
            next_energy,
        )?;
    }

    Ok(PathRecord {
        bounces,
        terminated,
    })
}

/// Traces a fan of rays across a spectrum of wavelengths through an optical scene.
///
/// For each wavelength $\lambda$ in `wavelengths_nm`, an initial ray is constructed
/// sharing the origin, direction, and energy of `ray_template`, and traced through `scene`
/// using [`trace_path`].
#[must_use]
pub fn spectral_fan(
    scene: &Scene,
    ray_template: &Ray,
    wavelengths_nm: &[f64],
    policy: RayBranchPolicy,
    max_bounces: usize,
) -> Vec<(f64, PathRecord)> {
    wavelengths_nm
        .iter()
        .copied()
        .filter_map(|wl| {
            let ray = Ray::new(
                ray_template.origin,
                ray_template.direction,
                wl,
                ray_template.energy,
            )
            .ok()?;
            let path = trace_path(scene, &ray, policy, max_bounces).ok()?;
            Some((wl, path))
        })
        .collect()
}
