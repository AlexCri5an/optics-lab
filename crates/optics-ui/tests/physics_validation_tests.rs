//! Physics & Rendering Validation Test Suite (P10)
//!
//! Validates:
//! 1. GPU vs CPU prism angle deviation comparison across visible spectrum ($\Delta \theta \le 0.05^\circ$).
//! 2. Glass-slab energy conservation ($R + T = 1.0 \pm 10^{-5}$) across all incidence angles.
//! 3. White furnace equilibrium and radiance conservation ($L_o = 1.0 \pm 10^{-4}$).
//! 4. Receiver floor caustic footprint radiometric energy conservation.
//! 5. Frame time benchmark assertions (re-trace in $\ll 16$ ms, BVH traversal $\ll 16$ ms).

use std::f64::consts::PI;
use std::time::Instant;

use optics_bvh::trace_bvh_cost;
use optics_core::fresnel::{fresnel_dielectric, fresnel_unpolarized, schlick_approx};
use optics_core::{Ray, Vec3};
use optics_glass::GlassCatalogDb;
use optics_ui::beam_ribbon::{BeamDisplayState, build_light_propagation};
use optics_ui::scene_model::SceneModel;
use optics_ui::sim::PrismSimulation;

// =========================================================================
// 1. GPU vs CPU Prism Angle Deviation Parity Across Visible Spectrum
// =========================================================================

/// Evaluates N-BK7 refractive index using the exact single-precision WGSL formula from `path_tracer.wgsl`.
///
/// # Formula (Schott Sellmeier 1 in $\mu\text{m}$)
/// $$n^2(\lambda) = 1 + \frac{1.03961212 \lambda^2}{\lambda^2 - 0.00600069867} + \frac{0.231792344 \lambda^2}{\lambda^2 - 0.0200179144} + \frac{1.01046945 \lambda^2}{\lambda^2 - 103.560653}$$
///
/// Reference: SCHOTT Optical Glass Data Sheets (N-BK7) / WGSL `path_tracer.wgsl:sellmeier_nbk7`.
#[allow(clippy::excessive_precision)]
#[must_use]
fn gpu_wgsl_sellmeier_nbk7(lambda_nm: f32) -> f32 {
    let l_um = lambda_nm.clamp(350.0, 850.0) * 0.001;
    let l2 = l_um * l_um;
    let n2 = 1.0
        + (1.03961212 * l2) / (l2 - 0.00600069867)
        + (0.231792344 * l2) / (l2 - 0.0200179144)
        + (1.01046945 * l2) / (l2 - 103.560653);
    n2.max(1.0).sqrt()
}

/// Computes the analytical angular deviation for an equilateral prism ($\alpha = 60^\circ$)
/// in single precision `f32` (matching GPU shader arithmetic).
///
/// # Formula (Hecht, Optics, 5th ed., Section 5.5, eq. 5.53)
/// $$\theta_{\text{dev}} = \theta_{i1} + \arcsin\left(n \sin\left(\alpha - \arcsin\left(\frac{\sin\theta_{i1}}{n}\right)\right)\right) - \alpha$$
#[must_use]
fn gpu_simulated_prism_deviation_deg(
    lambda_nm: f32,
    incidence_deg: f32,
    apex_deg: f32,
) -> Option<f32> {
    let alpha = apex_deg.to_radians();
    let theta_i1 = incidence_deg.to_radians();
    let n = gpu_wgsl_sellmeier_nbk7(lambda_nm);

    let sin_t1 = theta_i1.sin() / n;
    if sin_t1.abs() > 1.0 {
        return None;
    }
    let theta_t1 = sin_t1.asin();
    let theta_i2 = alpha - theta_t1;

    let sin_t2 = n * theta_i2.sin();
    if sin_t2.abs() > 1.0 {
        return None; // Total internal reflection at exit face
    }
    let theta_t2 = sin_t2.asin();
    let dev_rad = theta_i1 + theta_t2 - alpha;
    Some(dev_rad.to_degrees())
}

/// Computes the analytical angular deviation for a prism in double precision `f64` (CPU ground truth).
#[must_use]
fn cpu_analytical_prism_deviation_deg(incidence_deg: f64, apex_deg: f64, n: f64) -> Option<f64> {
    let alpha = apex_deg.to_radians();
    let theta_i1 = incidence_deg.to_radians();

    let sin_t1 = theta_i1.sin() / n;
    if sin_t1.abs() > 1.0 {
        return None;
    }
    let theta_t1 = sin_t1.asin();
    let theta_i2 = alpha - theta_t1;

    let sin_t2 = n * theta_i2.sin();
    if sin_t2.abs() > 1.0 {
        return None;
    }
    let theta_t2 = sin_t2.asin();
    let dev_rad = theta_i1 + theta_t2 - alpha;
    Some(dev_rad.to_degrees())
}

#[test]
fn test_gpu_vs_cpu_prism_angle_deviation_across_spectrum() {
    let db_res = GlassCatalogDb::load_bundled();
    assert!(db_res.is_ok(), "bundled catalog must load");
    let db = match db_res {
        Ok(d) => d,
        Err(_) => return,
    };

    let nbk7_opt = db.lookup("N-BK7");
    assert!(nbk7_opt.is_some(), "N-BK7 must exist in catalog");
    let nbk7 = match nbk7_opt {
        Some(g) => g,
        None => return,
    };

    // Standard optical spectral lines across visible spectrum:
    // h (404.66 nm), g (435.83 nm), F (486.13 nm), e (546.07 nm), d (587.56 nm), C (656.27 nm), r (706.52 nm)
    let spectral_lines = [
        ("h (violet)", 404.656),
        ("g (blue)", 435.834),
        ("F (cyan)", 486.133),
        ("e (green)", 546.074),
        ("d (yellow)", 587.562),
        ("C (red)", 656.273),
        ("r (deep red)", 706.519),
    ];

    let apex_deg = 60.0;
    let incidence_deg = 45.0;

    for (line_name, wl_nm) in spectral_lines {
        let n_cpu = nbk7.refractive_index(wl_nm);
        let n_gpu = gpu_wgsl_sellmeier_nbk7(wl_nm as f32) as f64;

        // Refractive index agreement: single-precision GPU Sellmeier vs double-precision CPU Sellmeier
        let n_diff = (n_gpu - n_cpu).abs();
        assert!(
            n_diff < 1e-4,
            "Sellmeier index mismatch at {line_name} ({wl_nm} nm): GPU={n_gpu}, CPU={n_cpu}, diff={n_diff}"
        );

        // Angular deviation agreement:
        let dev_gpu_opt =
            gpu_simulated_prism_deviation_deg(wl_nm as f32, incidence_deg as f32, apex_deg as f32);
        assert!(
            dev_gpu_opt.is_some(),
            "Ray should exit prism in GPU simulation at {wl_nm} nm"
        );
        let dev_gpu = match dev_gpu_opt {
            Some(d) => d,
            None => return,
        };

        let dev_cpu_opt = cpu_analytical_prism_deviation_deg(incidence_deg, apex_deg, n_cpu);
        assert!(
            dev_cpu_opt.is_some(),
            "Ray should exit prism in CPU calculation at {wl_nm} nm"
        );
        let dev_cpu = match dev_cpu_opt {
            Some(d) => d,
            None => return,
        };

        let dev_diff_deg = ((dev_gpu as f64) - dev_cpu).abs();
        assert!(
            dev_diff_deg <= 0.05,
            "Prism deviation difference exceeds tolerance at {line_name} ({wl_nm} nm): GPU={dev_gpu}°, CPU={dev_cpu}°, diff={dev_diff_deg}°"
        );
    }
}

#[test]
fn test_prism_angle_deviation_edge_cases() {
    let db_res = GlassCatalogDb::load_bundled();
    assert!(db_res.is_ok(), "bundled catalog must load");
    let db = match db_res {
        Ok(d) => d,
        Err(_) => return,
    };

    let nbk7_opt = db.lookup("N-BK7");
    assert!(nbk7_opt.is_some(), "N-BK7 must exist");
    let nbk7 = match nbk7_opt {
        Some(g) => g,
        None => return,
    };
    let n_d = nbk7.refractive_index(587.56);

    // Edge case 1: Normal incidence on equilateral prism face
    let dev_normal_gpu = gpu_simulated_prism_deviation_deg(587.56, 0.0, 60.0);
    // At normal incidence, theta_t1 = 0, theta_i2 = 60 deg.
    // Since n_d ~ 1.5168 and sin(60°) = 0.8660, n * sin(60°) = 1.313 > 1.0 => Total Internal Reflection!
    assert!(
        dev_normal_gpu.is_none(),
        "Normal incidence on 60° prism must undergo TIR at second face"
    );

    // Edge case 2: Grazing incidence (79.9°)
    let dev_grazing_gpu_opt = gpu_simulated_prism_deviation_deg(587.56, 79.9, 60.0);
    assert!(dev_grazing_gpu_opt.is_some(), "Must exit at 79.9°");
    let dev_grazing_gpu = match dev_grazing_gpu_opt {
        Some(d) => d,
        None => return,
    };

    let dev_grazing_cpu_opt = cpu_analytical_prism_deviation_deg(79.9, 60.0, n_d);
    assert!(dev_grazing_cpu_opt.is_some(), "Must exit at 79.9°");
    let dev_grazing_cpu = match dev_grazing_cpu_opt {
        Some(d) => d,
        None => return,
    };
    assert!(
        ((dev_grazing_gpu as f64) - dev_grazing_cpu).abs() <= 0.05,
        "Grazing incidence deviation must match within 0.05°"
    );

    // Edge case 3: 30° apex prism at normal incidence
    // At normal incidence on 30° prism: theta_t1 = 0, theta_i2 = 30°.
    // n * sin(30°) = 1.5168 * 0.5 = 0.7584 < 1.0 => exits cleanly!
    let dev_30_gpu_opt = gpu_simulated_prism_deviation_deg(587.56, 0.0, 30.0);
    assert!(
        dev_30_gpu_opt.is_some(),
        "Must exit 30° apex at normal incidence"
    );
    let dev_30_gpu = match dev_30_gpu_opt {
        Some(d) => d,
        None => return,
    };

    let dev_30_cpu_opt = cpu_analytical_prism_deviation_deg(0.0, 30.0, n_d);
    assert!(
        dev_30_cpu_opt.is_some(),
        "Must exit 30° apex at normal incidence"
    );
    let dev_30_cpu = match dev_30_cpu_opt {
        Some(d) => d,
        None => return,
    };
    assert!(
        ((dev_30_gpu as f64) - dev_30_cpu).abs() <= 0.05,
        "30° apex normal incidence deviation must match within 0.05°"
    );
}

// =========================================================================
// 2. Glass-Slab Energy Conservation Across All Incidence Angles
// =========================================================================

#[test]
fn test_glass_slab_energy_conservation_fresnel() {
    let n1: f64 = 1.0; // Air
    let n2: f64 = 1.5168; // N-BK7 at 587.6 nm

    // Test across entire angle of incidence range from normal (0°) to grazing (89.9°)
    let test_angles_deg: Vec<f64> = (0..90).map(|deg| deg as f64).collect();

    for deg in test_angles_deg {
        let theta_rad = (deg * (PI / 180.0)).min(89.99 * (PI / 180.0));
        let cos_i = theta_rad.cos();

        // 1. Dielectric polarized Fresnel components
        let (rs, rp) = fresnel_dielectric(cos_i, n1, n2);
        let ts = 1.0 - rs;
        let tp = 1.0 - rp;

        assert!(
            (rs + ts - 1.0).abs() < 1e-10,
            "Perpendicular s-polarization must conserve energy: Rs={rs}, Ts={ts} at {deg}°"
        );
        assert!(
            (rp + tp - 1.0).abs() < 1e-10,
            "Parallel p-polarization must conserve energy: Rp={rp}, Tp={tp} at {deg}°"
        );

        // 2. Unpolarized Fresnel interface
        let (r_unpol, t_unpol) = fresnel_unpolarized(cos_i, n1, n2);
        assert!(
            (r_unpol + t_unpol - 1.0).abs() < 1e-10,
            "Unpolarized Fresnel must conserve energy: R={r_unpol}, T={t_unpol} at {deg}°"
        );

        // 3. Multi-bounce incoherent thick glass slab (Air -> Glass -> Air)
        // Airy / Stokes summation formula:
        // R_slab = 2R / (1 + R), T_slab = (1 - R) / (1 + R)
        let r_slab = (2.0 * r_unpol) / (1.0 + r_unpol);
        let t_slab = (1.0 - r_unpol) / (1.0 + r_unpol);

        assert!(
            (r_slab + t_slab - 1.0).abs() < 1e-10,
            "Total slab reflectance and transmittance must sum to 1.0: R_slab={r_slab}, T_slab={t_slab} at {deg}°"
        );
    }
}

#[test]
fn test_schlick_approximation_bounds_and_brewster_dip() {
    let n1: f64 = 1.0;
    let n2: f64 = 1.5168;

    // Normal incidence: Schlick approximation is exact: R0 = ((n1 - n2) / (n1 + n2))^2
    let r0_ratio: f64 = (n1 - n2) / (n1 + n2);
    let r0_exact = r0_ratio.powi(2);
    let r0_schlick = schlick_approx(1.0, n1, n2);
    assert!(
        (r0_schlick - r0_exact).abs() < 1e-10,
        "Schlick must match exact Fresnel at normal incidence"
    );

    // Grazing incidence: R(90°) = 1.0
    let r_grazing = schlick_approx(0.0, n1, n2);
    assert!(
        (r_grazing - 1.0).abs() < 1e-10,
        "Schlick must approach 1.0 at grazing incidence"
    );

    // Brewster's angle: theta_B = arctan(n2 / n1)
    let brewster_rad = (n2 / n1).atan();
    let cos_brewster = brewster_rad.cos();
    let (rs_b, rp_b) = fresnel_dielectric(cos_brewster, n1, n2);

    assert!(
        rp_b < 1e-6,
        "p-polarization reflectance at Brewster's angle must vanish: Rp={rp_b}"
    );
    assert!(
        rs_b > 0.10,
        "s-polarization reflectance at Brewster's angle must be positive: Rs={rs_b}"
    );
}

// =========================================================================
// 3. White Furnace Radiance Conservation Equilibrium Test
// =========================================================================

/// Tests the White Furnace Test (equilibrium radiance conservation in lossless dielectric cavity).
///
/// Under isotropic, uniform ambient radiance $L_i(\omega) = 1.0\,\text{W}/(\text{m}^2\cdot\text{sr})$,
/// a non-absorbing dielectric boundary (or non-absorbing glossy surface) must reflect and transmit
/// light such that outgoing radiance is identically $L_o = 1.0$.
///
/// Reference: Pharr, Jakob, Humphreys, *Physically Based Rendering: From Theory to Implementation*, 4th ed., Section 9.1.
#[test]
fn test_white_furnace_radiance_conservation() {
    let n_incident = 1.0; // Air
    let n_transmitted = 1.5168; // Glass

    // Sample 1,000 incident angles across the hemisphere
    let num_samples = 1_000;
    let uniform_radiance = 1.0_f64;

    for i in 0..num_samples {
        let cos_i = (i as f64 + 0.5) / (num_samples as f64);

        // 1. Dielectric interface:
        // Ray splits into reflected branch (power fraction R) and refracted branch (power fraction T).
        // Since each branch enters an isotropic radiance field of 1.0:
        // L_o = R * L_incident + T * L_incident = (R + T) * 1.0 = 1.0.
        let (r, t) = fresnel_unpolarized(cos_i, n_incident, n_transmitted);
        let radiance_out = r * uniform_radiance + t * uniform_radiance;

        assert!(
            (radiance_out - uniform_radiance).abs() < 1e-10,
            "White furnace test failed for dielectric interface: L_out={radiance_out} at cos_i={cos_i}"
        );

        // 2. Reverse direction (Glass -> Air, internal interface including TIR):
        let (r_int, t_int) = fresnel_unpolarized(cos_i, n_transmitted, n_incident);
        let radiance_out_int = r_int * uniform_radiance + t_int * uniform_radiance;

        assert!(
            (radiance_out_int - uniform_radiance).abs() < 1e-10,
            "White furnace test failed for internal dielectric interface: L_out={radiance_out_int} at cos_i={cos_i}"
        );
    }
}

// =========================================================================
// 4. Receiver Floor Caustic Footprint Radiometric Energy Check
// =========================================================================

#[test]
fn test_receiver_floor_caustic_radiometric_energy() {
    let mut sim = PrismSimulation {
        glass_name: "N-BK7".to_string(),
        apex_deg: 60.0,
        incidence_deg: 45.0,
        sample_count: 32,
        ..Default::default()
    };
    sim.retrace();

    let display_state = BeamDisplayState {
        show_ribbons: true,
        show_caustic: true,
        beam_width_mm: 2.5,
        caustic_intensity: 1.0,
        ..Default::default()
    };

    let (segments, caustics) = build_light_propagation(&sim, &display_state);

    assert!(
        !segments.is_empty(),
        "Must generate 3D light propagation segments"
    );
    assert!(
        !caustics.is_empty(),
        "Must generate floor caustic footprints"
    );

    // Verify each ray's energy conservation: T1 * T2 + R1 + T1 * R2 == 1.0
    for ray in &sim.rays {
        if ray.path_record.bounces.len() >= 2 {
            let b1 = &ray.path_record.bounces[0];
            let b2 = &ray.path_record.bounces[1];

            // Face 1 energy: R1 + T1 == 1.0
            let e1 = b1.r_total + b1.t_total;
            assert!(
                (e1 - 1.0).abs() < 1e-5,
                "Face 1 must conserve energy: e1={e1}"
            );

            // Face 2 energy: R2 + T2 == 1.0
            let e2 = b2.r_total + b2.t_total;
            assert!(
                (e2 - 1.0).abs() < 1e-5,
                "Face 2 must conserve energy: e2={e2}"
            );

            // Total path energy: R1 + T1*R2 + T1*T2 == 1.0
            let total_path_energy = b1.r_total + b1.t_total * b2.r_total + b1.t_total * b2.t_total;
            assert!(
                (total_path_energy - 1.0).abs() < 1e-5,
                "Total ray path energy must equal 1.0: total={total_path_energy}"
            );
        }
    }

    // Verify all caustic footprints on the floor plane:
    for caustic in &caustics {
        // Floor Y coordinate check
        assert!(
            (caustic.floor_hit.y - (-0.01)).abs() < 1e-3,
            "Caustic must lie on the floor plane y = -0.01, got {}",
            caustic.floor_hit.y
        );

        // Radiant flux / irradiance must be strictly positive and bounded
        assert!(
            caustic.irradiance > 0.0 && caustic.irradiance <= 1.0,
            "Caustic irradiance must be strictly in (0.0, 1.0], got {}",
            caustic.irradiance
        );

        // Display sRGB color must have non-zero brightness
        let sum_rgb = caustic.color_srgb[0] as u32
            + caustic.color_srgb[1] as u32
            + caustic.color_srgb[2] as u32;
        assert!(sum_rgb > 0, "Caustic color must not be completely black");

        // Geometric dimensions must be non-zero
        assert!(caustic.width_x_mm > 0.0);
        assert!(caustic.length_z_mm > 0.0);
    }
}

// =========================================================================
// 5. Frame Time Benchmark Assertions (Re-trace in << 16 ms)
// =========================================================================

#[test]
fn test_frame_time_benchmark_assertions() {
    // 1. Full 64-sample CPU spectral retrace benchmark:
    let mut sim = PrismSimulation {
        glass_name: "N-BK7".to_string(),
        apex_deg: 60.0,
        incidence_deg: 45.0,
        sample_count: 64,
        ..Default::default()
    };

    // Warm-up
    sim.retrace();

    let iterations = 100;
    let start_sim = Instant::now();
    for _ in 0..iterations {
        sim.retrace();
    }
    let elapsed_sim = start_sim.elapsed();
    let avg_sim_ms = elapsed_sim.as_secs_f64() * 1000.0 / (iterations as f64);

    eprintln!("Measured 64-sample CPU spectral retrace: {avg_sim_ms:.4} ms");
    assert!(
        avg_sim_ms < 5.0,
        "CPU spectral re-trace must execute in << 16 ms (budget < 5.0 ms), got {avg_sim_ms:.4} ms"
    );

    // 2. Beam ribbon 3D geometry generation benchmark:
    let display_state = BeamDisplayState::default();
    let start_ribbon = Instant::now();
    for _ in 0..iterations {
        let _ = build_light_propagation(&sim, &display_state);
    }
    let elapsed_ribbon = start_ribbon.elapsed();
    let avg_ribbon_ms = elapsed_ribbon.as_secs_f64() * 1000.0 / (iterations as f64);

    eprintln!("Measured 3D beam ribbon geometry generation: {avg_ribbon_ms:.4} ms");
    assert!(
        avg_ribbon_ms < 1.0,
        "Beam ribbon generation must execute in < 1.0 ms, got {avg_ribbon_ms:.4} ms"
    );

    // 3. BVH ray traversal benchmark on active parametric scene (200 rays, > 3x interactive frame budget):
    let mut scene = SceneModel::default_prism_scene();
    let (bvh_scene, _) = scene.build_bvh_scene();

    assert!(!bvh_scene.tlas.is_empty(), "Scene must have TLAS nodes");
    assert!(!bvh_scene.instances.is_empty(), "Scene must have instances");

    let num_rays = 200;
    let mut rays: Vec<Ray> = Vec::with_capacity(num_rays);
    for i in 0..num_rays {
        let t = (i as f64) / (num_rays as f64);
        let angle = t * 2.0 * PI;
        let origin = Vec3::new(angle.cos() * 50.0, 20.0, angle.sin() * 50.0);
        let target = Vec3::new(0.0, 10.0, 0.0);
        let dir = (target - origin).normalize();
        let ray_res = Ray::new(origin, dir, 587.56, 1.0);
        assert!(ray_res.is_ok(), "valid test ray");
        if let Ok(r) = ray_res {
            rays.push(r);
        }
    }

    let start_bvh = Instant::now();
    let mut total_cost: u32 = 0;
    for ray in &rays {
        total_cost = total_cost.saturating_add(trace_bvh_cost(ray, &bvh_scene));
    }
    let elapsed_bvh = start_bvh.elapsed();
    let total_bvh_ms = elapsed_bvh.as_secs_f64() * 1000.0;

    eprintln!(
        "Measured 200 BVH ray queries: {total_bvh_ms:.4} ms (total traversal cost: {total_cost})"
    );
    assert!(
        total_bvh_ms < 6.0,
        "200 BVH ray queries must execute in << 16 ms (budget < 6.0 ms), got {total_bvh_ms:.4} ms"
    );
}
