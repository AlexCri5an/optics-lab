//! Non-sequential ray propagation engine with medium stack, Fresnel branching, and energy accounting.

use crate::error::NonSeqError;
use crate::geometry::NonSeqHit;
use crate::material::SurfaceFinish;
use crate::scene::NonSeqScene;
use crate::source::NonSeqRay;
use optics_core::{Ray, Vec3};
use optics_glass::catalog::GlassCatalogDb;
use serde::{Deserialize, Serialize};

/// Configuration parameters for non-sequential ray tracing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqTraceConfig {
    /// Maximum bounce / interaction limit per ray.
    pub max_bounces: usize,
    /// Minimum power threshold below which rays are terminated (Watts).
    pub min_power_threshold: f64,
    /// Enable splitting at partial Fresnel reflection/refraction boundaries.
    pub enable_branching: bool,
    /// Record segment paths for stray-light and ghost reflection analysis.
    pub record_paths: bool,
}

impl Default for NonSeqTraceConfig {
    fn default() -> Self {
        Self {
            max_bounces: 30,
            min_power_threshold: 1e-6,
            enable_branching: true,
            record_paths: true,
        }
    }
}

/// A recorded ray trajectory segment between interactions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RayPathSegment {
    /// Segment start point.
    pub start: Vec3,
    /// Segment end point.
    pub end: Vec3,
    /// Power carried along segment in Watts.
    pub power_watts: f64,
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Intersected object ID (None if escaped to infinity).
    pub hit_object_id: Option<usize>,
    /// Hit detector ID (None if did not hit a detector).
    pub hit_detector_id: Option<usize>,
    /// Interaction type tag.
    pub interaction: String,
    /// Cumulative reflection count before this interaction.
    pub reflection_count: usize,
    /// Cumulative total internal reflection count before this interaction.
    pub tir_count: usize,
}

/// Global energy conservation and diagnostic report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqTraceReport {
    /// Total radiant power launched into scene (Watts).
    pub launched_flux_watts: f64,
    /// Total power collected on all detectors (Watts).
    pub detected_flux_watts: f64,
    /// Total power absorbed in bulk media and surfaces (Watts).
    pub absorbed_flux_watts: f64,
    /// Total power escaping scene to ambient infinity (Watts).
    pub escaped_flux_watts: f64,
    /// Total rays generated and traced.
    pub total_rays_traced: usize,
    /// Total ray segment interactions.
    pub total_interactions: usize,
    /// Recorded ray path segments (if enabled).
    pub paths: Vec<RayPathSegment>,
}

impl NonSeqTraceReport {
    /// Energy balance ratio $(\Phi_{\text{detected}} + \Phi_{\text{absorbed}} + \Phi_{\text{escaped}}) / \Phi_{\text{launched}}$.
    pub fn energy_conservation_ratio(&self) -> f64 {
        if self.launched_flux_watts <= 0.0 {
            1.0
        } else {
            (self.detected_flux_watts + self.absorbed_flux_watts + self.escaped_flux_watts)
                / self.launched_flux_watts
        }
    }
}

/// Internal active ray state during traversal.
struct ActiveRay {
    ray: Ray,
    ray_id: usize,
    bounce_count: usize,
    reflection_count: usize,
    tir_count: usize,
    medium_stack: Vec<usize>, // Object IDs currently enclosing the ray
}

/// Non-sequential ray tracing engine.
pub struct NonSeqTracer {
    config: NonSeqTraceConfig,
}

impl NonSeqTracer {
    /// Create a new tracer with specified configuration.
    pub fn new(config: NonSeqTraceConfig) -> Self {
        Self { config }
    }

    /// Trace an ensemble of rays through the scene.
    pub fn trace_rays(
        &self,
        scene: &mut NonSeqScene,
        rays: &[NonSeqRay],
        catalog: Option<&GlassCatalogDb>,
    ) -> Result<NonSeqTraceReport, NonSeqError> {
        let mut launched_flux = 0.0;
        let mut detected_flux = 0.0;
        let mut absorbed_flux = 0.0;
        let mut escaped_flux = 0.0;
        let mut total_interactions = 0;
        let mut recorded_paths = Vec::new();

        let mut queue: Vec<ActiveRay> = Vec::with_capacity(rays.len());

        for r in rays {
            launched_flux += r.ray.energy;
            queue.push(ActiveRay {
                ray: r.ray,
                ray_id: r.ray_id,
                bounce_count: r.bounce_count,
                reflection_count: 0,
                tir_count: 0,
                medium_stack: r.medium_stack.clone(),
            });
        }

        while let Some(active) = queue.pop() {
            if active.ray.energy <= self.config.min_power_threshold {
                absorbed_flux += active.ray.energy;
                continue;
            }

            if active.bounce_count >= self.config.max_bounces {
                absorbed_flux += active.ray.energy;
                continue;
            }

            total_interactions += 1;

            // 1. Check for detector intersections
            let mut closest_det_hit: Option<(usize, f64, f64, f64)> = None; // (det_idx, t, lx, ly)
            let mut min_det_t = f64::INFINITY;

            for (det_idx, det) in scene.detectors.iter().enumerate() {
                if let Some((t, lx, ly)) = det.intersect_ray(&active.ray) {
                    if t < min_det_t && t > 1e-8 {
                        min_det_t = t;
                        closest_det_hit = Some((det_idx, t, lx, ly));
                    }
                }
            }

            // 2. Check for object intersections
            let closest_obj_hit = scene.intersect_closest_object(&active.ray);

            // Determine what the ray encounters first
            let hit_detector = if let Some((_, det_t, _, _)) = closest_det_hit {
                if let Some(ref obj_hit) = closest_obj_hit {
                    det_t < obj_hit.t
                } else {
                    true
                }
            } else {
                false
            };

            if hit_detector {
                let (det_idx, t, lx, ly) = closest_det_hit.ok_or_else(|| {
                    NonSeqError::InvalidGeometry("Detector hit resolution failed".to_string())
                })?;
                let hit_pt = active.ray.at(t);

                // Apply bulk absorption along path if inside medium
                let path_len = t;
                let mut transmitted_power = active.ray.energy;
                if let Some(&inside_obj_id) = active.medium_stack.last() {
                    if let Some(obj) = scene.objects.get(inside_obj_id) {
                        if let Some(ref abs) = obj.material.absorption {
                            let atten = abs.transmission(path_len);
                            absorbed_flux += transmitted_power * (1.0 - atten);
                            transmitted_power *= atten;
                        }
                    }
                }

                if self.config.record_paths {
                    recorded_paths.push(RayPathSegment {
                        start: active.ray.origin,
                        end: hit_pt,
                        power_watts: transmitted_power,
                        wavelength_nm: active.ray.wavelength_nm,
                        hit_object_id: None,
                        hit_detector_id: Some(det_idx),
                        interaction: "DetectorHit".to_string(),
                        reflection_count: active.reflection_count,
                        tir_count: active.tir_count,
                    });
                }

                scene.detectors[det_idx].record_hit_classified(
                    lx,
                    ly,
                    transmitted_power,
                    active.ray.wavelength_nm,
                    active.bounce_count,
                    active.reflection_count,
                    active.tir_count,
                );
                detected_flux += transmitted_power;
                // Detector absorbs the incident ray
                continue;
            }

            // If object hit
            if let Some(hit) = closest_obj_hit {
                let hit_obj = &scene.objects[hit.object_id];
                let path_len = hit.t;
                let mut current_power = active.ray.energy;

                // Bulk absorption along segment
                if let Some(&inside_obj_id) = active.medium_stack.last() {
                    if let Some(obj) = scene.objects.get(inside_obj_id) {
                        if let Some(ref abs) = obj.material.absorption {
                            let atten = abs.transmission(path_len);
                            absorbed_flux += current_power * (1.0 - atten);
                            current_power *= atten;
                        }
                    }
                }

                if current_power <= self.config.min_power_threshold {
                    absorbed_flux += current_power;
                    continue;
                }

                match hit_obj.material.finish {
                    SurfaceFinish::Absorber => {
                        absorbed_flux += current_power;
                        if self.config.record_paths {
                            recorded_paths.push(RayPathSegment {
                                start: active.ray.origin,
                                end: hit.point,
                                power_watts: current_power,
                                wavelength_nm: active.ray.wavelength_nm,
                                hit_object_id: Some(hit.object_id),
                                hit_detector_id: None,
                                interaction: "Absorption".to_string(),
                                reflection_count: active.reflection_count,
                                tir_count: active.tir_count,
                            });
                        }
                    }
                    SurfaceFinish::Mirror { reflectance } => {
                        let r_power = current_power * reflectance.clamp(0.0, 1.0);
                        absorbed_flux += current_power - r_power;

                        let refl_dir = (active.ray.direction
                            - hit.normal * (2.0 * active.ray.direction.dot(&hit.normal)))
                        .normalize();
                        let refl_origin = hit.point + refl_dir * 1e-6;

                        if self.config.record_paths {
                            recorded_paths.push(RayPathSegment {
                                start: active.ray.origin,
                                end: hit.point,
                                power_watts: r_power,
                                wavelength_nm: active.ray.wavelength_nm,
                                hit_object_id: Some(hit.object_id),
                                hit_detector_id: None,
                                interaction: "MirrorReflection".to_string(),
                                reflection_count: active.reflection_count,
                                tir_count: active.tir_count,
                            });
                        }

                        if r_power > self.config.min_power_threshold {
                            let new_ray =
                                Ray::new(refl_origin, refl_dir, active.ray.wavelength_nm, r_power)
                                    .map_err(NonSeqError::Core)?;

                            queue.push(ActiveRay {
                                ray: new_ray,
                                ray_id: active.ray_id,
                                bounce_count: active.bounce_count + 1,
                                reflection_count: active.reflection_count + 1,
                                tir_count: active.tir_count,
                                medium_stack: active.medium_stack.clone(),
                            });
                        }
                    }
                    SurfaceFinish::Lambertian { albedo } => {
                        let diff_power = current_power * albedo.clamp(0.0, 1.0);
                        absorbed_flux += current_power - diff_power;

                        if self.config.record_paths {
                            recorded_paths.push(RayPathSegment {
                                start: active.ray.origin,
                                end: hit.point,
                                power_watts: diff_power,
                                wavelength_nm: active.ray.wavelength_nm,
                                hit_object_id: Some(hit.object_id),
                                hit_detector_id: None,
                                interaction: "LambertianScatter".to_string(),
                                reflection_count: active.reflection_count,
                                tir_count: active.tir_count,
                            });
                        }

                        if diff_power > self.config.min_power_threshold {
                            let mut local_rng = crate::source::Rng64::new(
                                (active.ray_id as u64)
                                    .wrapping_mul(0x9E3779B97F4A7C15)
                                    .wrapping_add(active.bounce_count as u64),
                            );
                            let (u_axis, v_axis) =
                                crate::source::make_orthonormal_basis(&hit.normal);
                            let r_dir = local_rng.next_f64().sqrt();
                            let phi_dir = local_rng.next_f64() * 2.0 * std::f64::consts::PI;
                            let z_dir = (1.0 - r_dir * r_dir).max(0.0).sqrt();

                            let diff_dir = (u_axis * (r_dir * phi_dir.cos())
                                + v_axis * (r_dir * phi_dir.sin())
                                + hit.normal * z_dir)
                                .normalize();

                            let new_ray = Ray::new(
                                hit.point + diff_dir * 1e-6,
                                diff_dir,
                                active.ray.wavelength_nm,
                                diff_power,
                            )
                            .map_err(NonSeqError::Core)?;

                            queue.push(ActiveRay {
                                ray: new_ray,
                                ray_id: active.ray_id,
                                bounce_count: active.bounce_count + 1,
                                reflection_count: active.reflection_count + 1,
                                tir_count: active.tir_count,
                                medium_stack: active.medium_stack.clone(),
                            });
                        }
                    }
                    SurfaceFinish::Specular => {
                        // Determine n1 and n2
                        let n1 = if let Some(&inside_id) = active.medium_stack.last() {
                            scene.objects[inside_id]
                                .material
                                .medium
                                .refractive_index(active.ray.wavelength_nm, catalog)?
                        } else {
                            scene
                                .ambient_medium
                                .refractive_index(active.ray.wavelength_nm, catalog)?
                        };

                        let n2 = if hit.entering {
                            hit_obj
                                .material
                                .medium
                                .refractive_index(active.ray.wavelength_nm, catalog)?
                        } else {
                            // Exiting object: return to parent medium in stack
                            let mut temp_stack = active.medium_stack.clone();
                            if let Some(pos) =
                                temp_stack.iter().rposition(|&id| id == hit.object_id)
                            {
                                temp_stack.remove(pos);
                            }
                            if let Some(&parent_id) = temp_stack.last() {
                                scene.objects[parent_id]
                                    .material
                                    .medium
                                    .refractive_index(active.ray.wavelength_nm, catalog)?
                            } else {
                                scene
                                    .ambient_medium
                                    .refractive_index(active.ray.wavelength_nm, catalog)?
                            }
                        };

                        let (r_power, t_power, refl_ray, refr_ray) =
                            compute_snell_fresnel_interaction(
                                &active.ray,
                                &hit,
                                n1,
                                n2,
                                current_power,
                            )?;

                        if self.config.record_paths {
                            recorded_paths.push(RayPathSegment {
                                start: active.ray.origin,
                                end: hit.point,
                                power_watts: current_power,
                                wavelength_nm: active.ray.wavelength_nm,
                                hit_object_id: Some(hit.object_id),
                                hit_detector_id: None,
                                interaction: if t_power <= 0.0 {
                                    "TotalInternalReflection".to_string()
                                } else {
                                    "SnellFresnelSplit".to_string()
                                },
                                reflection_count: active.reflection_count,
                                tir_count: active.tir_count,
                            });
                        }

                        // Enqueue reflected branch
                        if r_power > self.config.min_power_threshold {
                            if let Some(r_ray) = refl_ray {
                                let is_tir = t_power <= 0.0;
                                queue.push(ActiveRay {
                                    ray: r_ray,
                                    ray_id: active.ray_id,
                                    bounce_count: active.bounce_count + 1,
                                    reflection_count: if is_tir {
                                        active.reflection_count
                                    } else {
                                        active.reflection_count + 1
                                    },
                                    tir_count: if is_tir {
                                        active.tir_count + 1
                                    } else {
                                        active.tir_count
                                    },
                                    medium_stack: active.medium_stack.clone(),
                                });
                            }
                        } else {
                            absorbed_flux += r_power;
                        }

                        // Enqueue refracted branch
                        if t_power > self.config.min_power_threshold {
                            if let Some(t_ray) = refr_ray {
                                let mut next_stack = active.medium_stack.clone();
                                if hit.entering {
                                    next_stack.push(hit.object_id);
                                } else if let Some(pos) =
                                    next_stack.iter().rposition(|&id| id == hit.object_id)
                                {
                                    next_stack.remove(pos);
                                }

                                queue.push(ActiveRay {
                                    ray: t_ray,
                                    ray_id: active.ray_id,
                                    bounce_count: active.bounce_count + 1,
                                    reflection_count: active.reflection_count,
                                    tir_count: active.tir_count,
                                    medium_stack: next_stack,
                                });
                            }
                        } else {
                            absorbed_flux += t_power;
                        }
                    }
                }
            } else {
                // Ray escaped scene without hitting object or detector
                escaped_flux += active.ray.energy;
                if self.config.record_paths {
                    let far_end = active.ray.at(1000.0);
                    recorded_paths.push(RayPathSegment {
                        start: active.ray.origin,
                        end: far_end,
                        power_watts: active.ray.energy,
                        wavelength_nm: active.ray.wavelength_nm,
                        hit_object_id: None,
                        hit_detector_id: None,
                        interaction: "EscapedToInfinity".to_string(),
                        reflection_count: active.reflection_count,
                        tir_count: active.tir_count,
                    });
                }
            }
        }

        Ok(NonSeqTraceReport {
            launched_flux_watts: launched_flux,
            detected_flux_watts: detected_flux,
            absorbed_flux_watts: absorbed_flux,
            escaped_flux_watts: escaped_flux,
            total_rays_traced: rays.len(),
            total_interactions,
            paths: recorded_paths,
        })
    }
}

/// Compute Snell refraction vector, Fresnel reflection coefficient, and reflected/refracted rays.
fn compute_snell_fresnel_interaction(
    ray: &Ray,
    hit: &NonSeqHit,
    n1: f64,
    n2: f64,
    incident_power: f64,
) -> Result<(f64, f64, Option<Ray>, Option<Ray>), NonSeqError> {
    let normal = hit.normal; // Normal facing incident side: ray.direction . normal <= 0
    let cos_theta1 = (-ray.direction.dot(&normal)).clamp(0.0, 1.0);
    let eta = n1 / n2;

    let sin2_theta1 = (1.0_f64 - cos_theta1 * cos_theta1).max(0.0);
    let radicand = 1.0 - eta * eta * sin2_theta1;

    // Reflected ray
    let refl_dir = (ray.direction + normal * (2.0 * cos_theta1)).normalize();
    let refl_origin = hit.point + refl_dir * 1e-6;

    if radicand <= 0.0 {
        // Total Internal Reflection (TIR)
        let r_ray = Ray::new(refl_origin, refl_dir, ray.wavelength_nm, incident_power)
            .map_err(NonSeqError::Core)?;
        return Ok((incident_power, 0.0, Some(r_ray), None));
    }

    let cos_theta2 = radicand.sqrt();
    let refr_dir = (ray.direction * eta + normal * (eta * cos_theta1 - cos_theta2)).normalize();
    let refr_origin = hit.point + refr_dir * 1e-6;

    // Unpolarized dielectric Fresnel power reflectance
    let rs = (n1 * cos_theta1 - n2 * cos_theta2) / (n1 * cos_theta1 + n2 * cos_theta2);
    let rp = (n2 * cos_theta1 - n1 * cos_theta2) / (n2 * cos_theta1 + n1 * cos_theta2);
    let reflectance = (0.5 * (rs * rs + rp * rp)).clamp(0.0, 1.0);
    let transmittance = 1.0 - reflectance;

    let r_power = incident_power * reflectance;
    let t_power = incident_power * transmittance;

    let r_ray =
        Ray::new(refl_origin, refl_dir, ray.wavelength_nm, r_power).map_err(NonSeqError::Core)?;

    let t_ray =
        Ray::new(refr_origin, refr_dir, ray.wavelength_nm, t_power).map_err(NonSeqError::Core)?;

    Ok((r_power, t_power, Some(r_ray), Some(t_ray)))
}
