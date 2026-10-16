//! 2D Prism dispersion simulation, ray generation, and analytic validation.
//!
//! Evaluates refractive geometry, Sellmeier dispersion, and traces spectral fans
//! across visible wavelengths (380 nm to 780 nm) for real-time visualization.

use egui::{Pos2, Rect};
use optics_core::{
    Air, BounceEvent, PathRecord, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle, Vec3,
    spectral_fan,
};
use optics_glass::{GlassCatalogDb, GlassMaterial};
use optics_spectral::display_color_for_wavelength;

/// A 2D reflected branch originating at an interface bounce.
#[derive(Clone, Debug, PartialEq)]
pub struct ReflectedBranch2D {
    /// Start position of the reflection in mm.
    pub start: Pos2,
    /// End position of the reflection in mm.
    pub end: Pos2,
    /// Alpha opacity mapped from Fresnel reflectance R_total.
    pub alpha: f32,
    /// Whether this reflection was total internal reflection.
    pub is_tir: bool,
}

/// Classification of optical edge states for the simulation scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeState {
    /// Normal operation: at least some rays exit the prism through refraction.
    Normal,
    /// All spectral rays undergo Total Internal Reflection and cannot exit via standard refraction.
    AllTotalInternalReflection,
    /// Zero transmitted rays exit the prism.
    NoRaysTransmitted,
    /// The incoming ray missed the prism entirely.
    RayMissedPrism,
}

/// A traced ray mapped to 2D screen coordinates.
#[derive(Clone, Debug)]
pub struct TracedRay2D {
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Refractive index of prism glass at this wavelength.
    pub n: f64,
    /// Display sRGB color [R, G, B].
    pub color_rgb: [u8; 3],
    /// Path points in 2D world coordinates (mm), starting at entry point on prism.
    pub points_world: Vec<Pos2>,
    /// Reflected branches from surface bounces.
    pub reflected_branches: Vec<ReflectedBranch2D>,
    /// Whether total internal reflection occurred at any bounce.
    pub has_tir: bool,
    /// Total angular deviation from the incoming direction in degrees (if exited).
    pub deviation_deg: Option<f64>,
    /// Full 3D path record from optics-core.
    pub path_record: PathRecord,
}

/// Simulation state managing 2D prism scene geometry and traced rays.
#[derive(Clone, Debug)]
pub struct PrismSimulation {
    // --- Inputs ---
    /// Selected glass preset name (e.g. "N-BK7", "SF11", "F2", "Fused Silica").
    pub glass_name: String,
    /// Prism apex angle in degrees (20° to 90°).
    pub apex_deg: f64,
    /// Angle of incidence on the entry face in degrees (0° to 80°).
    pub incidence_deg: f64,
    /// Number of sampled spectral wavelengths across 380 nm to 780 nm (8 to 64).
    pub sample_count: usize,

    // --- Computed outputs ---
    /// Traced rays across the spectrum.
    pub rays: Vec<TracedRay2D>,
    /// Refractive index at helium yellow d line (587.56 nm).
    pub nd: f64,
    /// Abbe dispersion number Vd.
    pub vd: f64,
    /// Total angular spread between 380 nm and 780 nm in degrees (if both exit).
    pub total_spread_deg: Option<f64>,
    /// Bounding rectangle containing prism vertices and all ray paths in mm.
    pub bounds_world: Rect,
    /// Left vertex of the prism in mm.
    pub v_left: Pos2,
    /// Apex vertex of the prism in mm.
    pub v_apex: Pos2,
    /// Right vertex of the prism in mm.
    pub v_right: Pos2,
    /// Incoming ray source marker origin in mm.
    pub incoming_origin: Pos2,
    /// Incoming ray entry hit point on prism face in mm.
    pub incoming_hit: Pos2,
    /// Time taken to trace the fan in milliseconds.
    pub trace_time_ms: f64,
}

/// Pure helper mapping Fresnel reflectance R_total in [0.0, 1.0] to a visible alpha opacity.
#[must_use]
pub fn map_reflectance_to_alpha(r_total: f64) -> f32 {
    let r = r_total.clamp(0.0, 1.0) as f32;
    let min_alpha = 0.22_f32;
    let max_alpha = 0.85_f32;
    min_alpha + (max_alpha - min_alpha) * r
}

/// Computes the pure bounding rectangle in world space enclosing all geometry and ray endpoints.
#[must_use]
pub fn compute_scene_bounds(
    prism_vertices: [Pos2; 3],
    incoming_origin: Pos2,
    rays: &[TracedRay2D],
) -> Rect {
    let mut min_x = prism_vertices[0]
        .x
        .min(prism_vertices[1].x)
        .min(prism_vertices[2].x)
        .min(incoming_origin.x);
    let mut max_x = prism_vertices[0]
        .x
        .max(prism_vertices[1].x)
        .max(prism_vertices[2].x)
        .max(incoming_origin.x);
    let mut min_y = prism_vertices[0]
        .y
        .min(prism_vertices[1].y)
        .min(prism_vertices[2].y)
        .min(incoming_origin.y);
    let mut max_y = prism_vertices[0]
        .y
        .max(prism_vertices[1].y)
        .max(prism_vertices[2].y)
        .max(incoming_origin.y);

    for ray in rays {
        for p in &ray.points_world {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        for branch in &ray.reflected_branches {
            min_x = min_x.min(branch.end.x);
            max_x = max_x.max(branch.end.x);
            min_y = min_y.min(branch.end.y);
            max_y = max_y.max(branch.end.y);
        }
    }

    Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
}

impl Default for PrismSimulation {
    fn default() -> Self {
        let mut sim = Self {
            glass_name: "N-BK7".to_string(),
            apex_deg: 60.0,
            incidence_deg: 45.0,
            sample_count: 32,
            rays: Vec::new(),
            nd: 1.5168,
            vd: 64.17,
            total_spread_deg: None,
            bounds_world: Rect::ZERO,
            v_left: Pos2::ZERO,
            v_apex: Pos2::ZERO,
            v_right: Pos2::ZERO,
            incoming_origin: Pos2::ZERO,
            incoming_hit: Pos2::ZERO,
            trace_time_ms: 0.0,
        };
        sim.retrace();
        sim
    }
}

impl PrismSimulation {
    /// Re-evaluates the prism geometry, traces the spectral ray fan, and updates outputs.
    pub fn retrace(&mut self) {
        let start = std::time::Instant::now();

        // 1. Resolve glass material (check fast presets first before falling back to full JSON catalog)
        let glass_mat: GlassMaterial = match self.glass_name.to_lowercase().as_str() {
            "n-bk7" | "nbk7" => GlassMaterial::n_bk7(),
            "sf11" => GlassMaterial::sf11(),
            "f2" => GlassMaterial::f2(),
            "fused silica" | "fused_silica" => GlassMaterial::fused_silica(),
            _ => {
                let catalog = GlassCatalogDb::load_bundled().ok();
                match catalog.as_ref().and_then(|db| db.lookup(&self.glass_name)) {
                    Some(m) => m.clone(),
                    None => GlassMaterial::n_bk7(),
                }
            }
        };

        self.nd = glass_mat.nd;
        self.vd = glass_mat.vd;

        // 2. Geometry setup (side length L = 50 mm)
        let l_side = 50.0_f64;
        let alpha = self.apex_deg.to_radians();
        let beta = alpha / 2.0;

        let h = l_side * beta.cos();
        let w = l_side * beta.sin();

        self.v_apex = Pos2::new(0.0, h as f32);
        self.v_left = Pos2::new(-w as f32, 0.0);
        self.v_right = Pos2::new(w as f32, 0.0);

        // 3D extrusion along Z [-50, +50] mm
        let z_min = -50.0;
        let z_max = 50.0;

        let v_apex_front = Vec3::new(0.0, h, z_max);
        let v_apex_back = Vec3::new(0.0, h, z_min);
        let v_left_front = Vec3::new(-w, 0.0, z_max);
        let v_left_back = Vec3::new(-w, 0.0, z_min);
        let v_right_front = Vec3::new(w, 0.0, z_max);
        let v_right_back = Vec3::new(w, 0.0, z_min);

        let triangles = match (|| -> Result<Vec<Triangle>, optics_core::OpticsError> {
            Ok(vec![
                Triangle::new(v_left_front, v_apex_front, v_apex_back, 1, 1, 0)?,
                Triangle::new(v_left_front, v_apex_back, v_left_back, 1, 1, 0)?,
                Triangle::new(v_apex_front, v_right_front, v_right_back, 2, 1, 0)?,
                Triangle::new(v_apex_front, v_right_back, v_apex_back, 2, 1, 0)?,
                Triangle::new(v_right_front, v_left_front, v_left_back, 3, 1, 0)?,
                Triangle::new(v_right_front, v_left_back, v_right_back, 3, 1, 0)?,
            ])
        })() {
            Ok(t) => t,
            Err(_) => return,
        };

        let media: Vec<Box<dyn RefractiveIndex>> = vec![Box::new(Air), Box::new(glass_mat.clone())];
        let scene = Scene::new(triangles, media);

        // 3. Incoming ray setup
        let theta_inc = self.incidence_deg.to_radians();
        let ray_angle = theta_inc - beta;
        let incident_dir = Vec3::new(ray_angle.cos(), ray_angle.sin(), 0.0);

        let p_mid = Vec3::new(-w * 0.5, h * 0.5, 0.0);
        let origin = p_mid - incident_dir * 45.0;
        self.incoming_origin = Pos2::new(origin.x as f32, origin.y as f32);
        self.incoming_hit = Pos2::new(p_mid.x as f32, p_mid.y as f32);

        let ray_template = match Ray::new(origin, incident_dir, 550.0, 1.0) {
            Ok(r) => r,
            Err(_) => return,
        };

        // 4. Sample wavelengths across 380 nm to 780 nm
        let n_samples = self.sample_count.clamp(8, 64);
        let step = (780.0 - 380.0) / (n_samples - 1) as f64;
        let mut wavelengths = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            wavelengths.push(380.0 + i as f64 * step);
        }

        let fan = spectral_fan(
            &scene,
            &ray_template,
            &wavelengths,
            RayBranchPolicy::FollowTransmission,
            10,
        );

        // 5. Build 2D ray paths with finite ray lengths and Fresnel branches
        let exit_extension = 45.0_f64;
        let branch_length = 24.0_f64;
        let mut traced_rays = Vec::with_capacity(fan.len());

        for (wl, path) in fan {
            let n = glass_mat.n(wl);
            let color_rgb = display_color_for_wavelength(wl);

            let mut pts = Vec::new();
            pts.push(self.incoming_hit);

            let mut has_tir = false;
            let mut dev_deg = None;
            let mut reflected_branches = Vec::new();

            for bounce in &path.bounces {
                let b_pos = Pos2::new(bounce.point.x as f32, bounce.point.y as f32);
                pts.push(b_pos);

                if bounce.event == BounceEvent::TotalInternalReflection {
                    has_tir = true;
                }

                // Spawn reflected branch at each interface
                let branch_end_3d = bounce.point + bounce.reflected_dir * branch_length;
                let branch_end = Pos2::new(branch_end_3d.x as f32, branch_end_3d.y as f32);
                let alpha = map_reflectance_to_alpha(bounce.r_total);

                reflected_branches.push(ReflectedBranch2D {
                    start: b_pos,
                    end: branch_end,
                    alpha,
                    is_tir: bounce.event == BounceEvent::TotalInternalReflection,
                });
            }

            // If the ray exited through refraction on its last bounce, extend into air
            if let Some(last_bounce) = path.bounces.last() {
                if let Some(refr_dir) = last_bounce.refracted_dir {
                    let dev_rad = refr_dir.dot(&incident_dir).clamp(-1.0, 1.0).acos();
                    dev_deg = Some(dev_rad.to_degrees());

                    let end_pos = last_bounce.point + refr_dir * exit_extension;
                    pts.push(Pos2::new(end_pos.x as f32, end_pos.y as f32));
                } else if has_tir && path.bounces.len() >= 2 {
                    // For TIR rays, extend the final reflection branch out slightly
                    let end_pos = last_bounce.point + last_bounce.reflected_dir * exit_extension;
                    pts.push(Pos2::new(end_pos.x as f32, end_pos.y as f32));
                }
            }

            traced_rays.push(TracedRay2D {
                wavelength_nm: wl,
                n,
                color_rgb,
                points_world: pts,
                reflected_branches,
                has_tir,
                deviation_deg: dev_deg,
                path_record: path,
            });
        }

        // 6. Calculate total angular spread between 380 nm and 780 nm
        if traced_rays.len() >= 2 {
            let first_dev = traced_rays.first().and_then(|r| r.deviation_deg);
            let last_dev = traced_rays.last().and_then(|r| r.deviation_deg);
            self.total_spread_deg = match (first_dev, last_dev) {
                (Some(d1), Some(d2)) => Some((d1 - d2).abs()),
                _ => None,
            };
        }

        // 7. Compute pure scene bounds enclosing prism and all rays
        self.bounds_world = compute_scene_bounds(
            [self.v_apex, self.v_left, self.v_right],
            self.incoming_origin,
            &traced_rays,
        );

        self.rays = traced_rays;

        self.trace_time_ms = start.elapsed().as_secs_f64() * 1000.0;
    }

    /// Detects if the current simulation is in an edge state.
    #[must_use]
    pub fn detect_edge_state(&self) -> EdgeState {
        if self.rays.is_empty() {
            return EdgeState::RayMissedPrism;
        }
        let total_rays = self.rays.len();
        let tir_count = self.rays.iter().filter(|r| r.has_tir).count();
        let escaped_count = self
            .rays
            .iter()
            .filter(|r| r.deviation_deg.is_some())
            .count();

        if tir_count == total_rays {
            EdgeState::AllTotalInternalReflection
        } else if escaped_count == 0 {
            EdgeState::NoRaysTransmitted
        } else {
            EdgeState::Normal
        }
    }

    /// Evaluates the deviation angle for a specific wavelength in radians.
    #[must_use]
    pub fn compute_deviation_rad(&self, wavelength_nm: f64) -> Option<f64> {
        let glass_mat: GlassMaterial = match self.glass_name.to_lowercase().as_str() {
            "n-bk7" | "nbk7" => GlassMaterial::n_bk7(),
            "sf11" => GlassMaterial::sf11(),
            "f2" => GlassMaterial::f2(),
            "fused silica" | "fused_silica" => GlassMaterial::fused_silica(),
            _ => {
                let catalog = GlassCatalogDb::load_bundled().ok();
                match catalog.as_ref().and_then(|db| db.lookup(&self.glass_name)) {
                    Some(m) => m.clone(),
                    None => GlassMaterial::n_bk7(),
                }
            }
        };

        let n = glass_mat.n(wavelength_nm);
        let alpha = self.apex_deg.to_radians();
        let theta1 = self.incidence_deg.to_radians();

        let sin_theta2 = theta1.sin() / n;
        if sin_theta2.abs() > 1.0 {
            return None; // Cannot enter
        }
        let theta2 = sin_theta2.asin();

        let theta3 = alpha - theta2;
        let sin_theta4 = n * theta3.sin();
        if sin_theta4.abs() > 1.0 {
            return None; // TIR
        }
        let theta4 = sin_theta4.asin();
        Some(theta1 + theta4 - alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_deviation_equals_analytic_snell_and_cli_value() {
        let sim = PrismSimulation::default(); // N-BK7, apex 60, incidence 45

        // Helium yellow d line = 587.56 nm
        let Some(dev_rad) = sim.compute_deviation_rad(587.56) else {
            unreachable!("d line must exit without TIR");
        };

        // Independent 2-surface analytic Snell evaluation
        let n = GlassMaterial::n_bk7().n(587.56);
        let alpha = (60.0_f64).to_radians();
        let theta1 = (45.0_f64).to_radians();
        let theta2 = (theta1.sin() / n).asin();
        let theta3 = alpha - theta2;
        let theta4 = (n * theta3.sin()).asin();
        let expected_dev_rad = theta1 + theta4 - alpha;

        // Verify equality within 1e-9
        assert!(
            (dev_rad - expected_dev_rad).abs() < 1e-9,
            "UI deviation mismatch: got {dev_rad}, expected {expected_dev_rad}"
        );

        // Verify matches CLI table value (38.9567 deg ~ 38.96 deg)
        let dev_deg = dev_rad.to_degrees();
        assert!(
            (dev_deg - 38.9567).abs() < 0.01,
            "CLI table value match failed: got {dev_deg}"
        );
    }

    #[test]
    fn test_retrace_performance_64_samples() {
        let mut sim = PrismSimulation {
            sample_count: 64,
            ..Default::default()
        };

        // Warmup
        sim.retrace();

        let iterations = 100;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            sim.retrace();
        }
        let elapsed = start.elapsed();
        let ms_per_retrace = elapsed.as_secs_f64() * 1000.0 / (iterations as f64);
        eprintln!("Measured re-trace time at 64 samples: {ms_per_retrace:.4} ms");
        assert!(ms_per_retrace < 5.0);
    }

    #[test]
    fn test_map_reflectance_to_alpha() {
        // Minimum visible alpha on zero reflectance
        let alpha_zero = map_reflectance_to_alpha(0.0);
        assert!((alpha_zero - 0.22).abs() < 1e-5);

        // High reflectance at TIR
        let alpha_one = map_reflectance_to_alpha(1.0);
        assert!((alpha_one - 0.85).abs() < 1e-5);

        // Standard glass 4% reflectance
        let alpha_glass = map_reflectance_to_alpha(0.04);
        assert!(alpha_glass > 0.22 && alpha_glass < 0.30);
    }

    #[test]
    fn test_scene_bounds_containment_and_extremes() {
        // Test normal, extreme low apex, extreme high apex, grazing incidence
        let test_cases = [
            ("N-BK7", 60.0, 45.0),
            ("SF11", 20.0, 0.0),
            ("F2", 90.0, 80.0),
            ("Fused Silica", 20.0, 80.0),
            ("SF11", 90.0, 0.0),
        ];

        for (glass, apex, inc) in test_cases {
            let mut sim = PrismSimulation {
                glass_name: glass.to_string(),
                apex_deg: apex,
                incidence_deg: inc,
                sample_count: 16,
                ..Default::default()
            };
            sim.retrace();

            let bounds = sim.bounds_world;
            assert!(bounds.width() > 0.0);
            assert!(bounds.height() > 0.0);

            // Bounds must contain all prism vertices
            assert!(bounds.contains(sim.v_apex));
            assert!(bounds.contains(sim.v_left));
            assert!(bounds.contains(sim.v_right));
            // Bounds must contain incoming ray source
            assert!(bounds.contains(sim.incoming_origin));

            // Bounds must contain all ray points and reflection ends
            for ray in &sim.rays {
                for p in &ray.points_world {
                    assert!(
                        bounds.contains(*p),
                        "Point {p:?} outside bounds {bounds:?} for {glass} apex={apex} inc={inc}"
                    );
                }
                for branch in &ray.reflected_branches {
                    assert!(
                        bounds.contains(branch.end),
                        "Branch end {:?} outside bounds {bounds:?} for {glass} apex={apex} inc={inc}",
                        branch.end
                    );
                }
            }
        }
    }

    #[test]
    fn test_detect_edge_state_normal() {
        let sim = PrismSimulation {
            glass_name: "N-BK7".to_string(),
            apex_deg: 60.0,
            incidence_deg: 45.0,
            sample_count: 16,
            ..Default::default()
        };
        assert_eq!(sim.detect_edge_state(), EdgeState::Normal);
    }

    #[test]
    fn test_detect_edge_state_tir() {
        let mut sim = PrismSimulation {
            glass_name: "SF11".to_string(),
            apex_deg: 90.0,
            incidence_deg: 80.0,
            sample_count: 16,
            ..Default::default()
        };
        sim.retrace();
        assert_eq!(
            sim.detect_edge_state(),
            EdgeState::AllTotalInternalReflection
        );
    }
}
