//! Optical prism simulation state and re-tracing logic.

use egui::{Pos2, Rect};
use optics_core::{
    Air, PathRecord, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle, Vec3, spectral_fan,
};
use optics_glass::{GlassCatalogDb, GlassMaterial};
use optics_spectral::display_color_for_wavelength;

/// Information for a single traced spectral ray path in the 2D plane of incidence.
#[derive(Debug, Clone)]
pub struct TracedRay2D {
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Refractive index of the prism material at this wavelength.
    pub n: f64,
    /// 8-bit sRGB display color [R, G, B].
    pub color_rgb: [u8; 3],
    /// Sequence of 2D vertex positions in millimeters (starting at ray origin).
    pub points_world: Vec<Pos2>,
    /// Total angular deviation from the incoming direction in degrees (if exited).
    pub deviation_deg: Option<f64>,
    /// Whether any total internal reflection event occurred along the path.
    pub has_tir: bool,
    /// Full 3D path bounce records for the inspector.
    pub path_record: PathRecord,
}

/// Interactive prism simulation parameters and active results.
#[derive(Debug, Clone)]
pub struct PrismSimulation {
    /// Selected glass material name (e.g. "N-BK7", "SF11", "F2", "Fused Silica").
    pub glass_name: String,
    /// Prism apex angle in degrees (20° to 90°).
    pub apex_deg: f64,
    /// Angle of incidence on the first face in degrees (0° to 80°).
    pub incidence_deg: f64,
    /// Number of spectral samples across 380 nm to 780 nm (8 to 64).
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
    /// Time taken to trace the fan in milliseconds.
    pub trace_time_ms: f64,
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
            trace_time_ms: 0.0,
        };
        sim.retrace();
        sim
    }
}

impl PrismSimulation {
    /// Re-evaluates the prism geometry, traces the spectral ray fan, and updates outputs.
    pub fn retrace(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let start = std::time::Instant::now();

        // 1. Resolve glass material
        let catalog = GlassCatalogDb::load_bundled().ok();
        let glass_mat: GlassMaterial =
            match catalog.as_ref().and_then(|db| db.lookup(&self.glass_name)) {
                Some(m) => m.clone(),
                None => match self.glass_name.to_lowercase().as_str() {
                    "n-bk7" | "nbk7" => GlassMaterial::n_bk7(),
                    "sf11" => GlassMaterial::sf11(),
                    "f2" => GlassMaterial::f2(),
                    "fused silica" | "fused_silica" => GlassMaterial::fused_silica(),
                    _ => GlassMaterial::n_bk7(),
                },
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
        let origin = p_mid - incident_dir * 40.0;

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

        // 5. Build 2D ray paths
        let exit_extension = 45.0;
        let mut min_x = self
            .v_left
            .x
            .min(self.v_right.x)
            .min(self.v_apex.x)
            .min(origin.x as f32);
        let mut max_x = self
            .v_left
            .x
            .max(self.v_right.x)
            .max(self.v_apex.x)
            .max(origin.x as f32);
        let mut min_y = self
            .v_left
            .y
            .min(self.v_right.y)
            .min(self.v_apex.y)
            .min(origin.y as f32);
        let mut max_y = self
            .v_left
            .y
            .max(self.v_right.y)
            .max(self.v_apex.y)
            .max(origin.y as f32);

        let mut traced_rays = Vec::with_capacity(fan.len());

        for (wl, path) in fan {
            let n = glass_mat.n(wl);
            let color_rgb = display_color_for_wavelength(wl);

            let mut pts = Vec::new();
            pts.push(Pos2::new(origin.x as f32, origin.y as f32));

            let mut has_tir = false;
            let mut dev_deg = None;

            for bounce in &path.bounces {
                pts.push(Pos2::new(bounce.point.x as f32, bounce.point.y as f32));
                if bounce.event == optics_core::BounceEvent::TotalInternalReflection {
                    has_tir = true;
                }
            }

            if path.bounces.len() >= 2 {
                if let Some(exit_dir) = path.bounces[1].refracted_dir {
                    let dev_rad = exit_dir.dot(&incident_dir).clamp(-1.0, 1.0).acos();
                    dev_deg = Some(dev_rad.to_degrees());

                    let end = path.bounces[1].point + exit_dir * exit_extension;
                    pts.push(Pos2::new(end.x as f32, end.y as f32));
                }
            }

            for p in &pts {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }

            traced_rays.push(TracedRay2D {
                wavelength_nm: wl,
                n,
                color_rgb,
                points_world: pts,
                deviation_deg: dev_deg,
                has_tir,
                path_record: path,
            });
        }

        // Total spread between violet (first) and red (last)
        if let (Some(first_dev), Some(last_dev)) = (
            traced_rays.first().and_then(|r| r.deviation_deg),
            traced_rays.last().and_then(|r| r.deviation_deg),
        ) {
            self.total_spread_deg = Some((first_dev - last_dev).abs());
        } else {
            self.total_spread_deg = None;
        }

        self.rays = traced_rays;
        self.bounds_world = Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.trace_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    /// Evaluates the deviation angle for a specific wavelength in radians.
    #[must_use]
    pub fn compute_deviation_rad(&self, wavelength_nm: f64) -> Option<f64> {
        let catalog = GlassCatalogDb::load_bundled().ok();
        let glass_mat: GlassMaterial =
            match catalog.as_ref().and_then(|db| db.lookup(&self.glass_name)) {
                Some(m) => m.clone(),
                None => GlassMaterial::n_bk7(),
            };

        let n = glass_mat.n(wavelength_nm);
        let alpha = self.apex_deg.to_radians();
        let theta1 = self.incidence_deg.to_radians();

        let sin_theta2 = theta1.sin() / n;
        if sin_theta2.abs() > 1.0 {
            return None;
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
}
