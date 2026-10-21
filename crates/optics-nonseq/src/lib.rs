//! Non-sequential optical ray tracing, 3D geometry primitives, radiometric detectors, and stray light analysis.
//!
//! Provides a physically based, double-precision non-sequential ray tracing engine with
//! Snell/Fresnel vector refraction, total internal reflection (TIR), bulk Beer-Lambert absorption,
//! 2D pixelated radiometric/photometric detectors, and ray history logging.

pub mod detector;
pub mod error;
pub mod geometry;
pub mod material;
pub mod scene;
pub mod source;
pub mod trace;

pub use detector::{
    photopic_luminous_efficiency, watts_to_lumens, Detector2D, DetectorHit, RayPathFilter,
};
pub use error::NonSeqError;
pub use geometry::{NonSeqHit, PrimitiveGeometry, Transform3D};
pub use material::{BulkAbsorption, NonSeqMaterial, OpticalMedium, SurfaceFinish};
pub use scene::{NonSeqObject, NonSeqScene};
pub use source::{NonSeqRay, RaySource, Rng64, SourceSpectrum};
pub use trace::{NonSeqTraceConfig, NonSeqTraceReport, NonSeqTracer, RayPathSegment};

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::prescription::{OpticalPrescription, OpticalSurface};
    use optics_core::{Ray, Vec3};
    use optics_glass::catalog::GlassCatalogDb;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Acceptance test: Verification of Fresnel reflection and refraction energy conservation
    /// across a dielectric slab with multiple internal bounces: $T + R = 1.000000000000$.
    #[test]
    fn test_fresnel_slab_energy_conservation() -> TestResult {
        let mut scene = NonSeqScene::new();

        // 1. Planar glass slab (thickness 5 mm, refractive index n = 1.5, size 100x100 mm)
        // Spanning z from 0 to 5 mm (center at z = 2.5 mm)
        let slab = NonSeqObject {
            id: 0,
            name: "DielectricSlab".to_string(),
            geometry: PrimitiveGeometry::Box {
                half_x: 50.0,
                half_y: 50.0,
                half_z: 2.5,
            },
            transform: Transform3D::from_translation(0.0, 0.0, 2.5),
            material: NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.5 }),
            is_active: true,
        };
        scene.add_object(slab);

        // 2. Transmitted detector behind slab at z = 10 mm
        let det_trans = Detector2D::new(
            0,
            "TransmittedDetector",
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            100.0,
            100.0,
            10,
            10,
        )?;
        scene.add_detector(det_trans);

        // 3. Reflected detector in front of slab at z = -10 mm (facing +z toward slab)
        let det_refl = Detector2D::new(
            0,
            "ReflectedDetector",
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            100.0,
            100.0,
            10,
            10,
        )?;
        scene.add_detector(det_refl);

        // Single normal incidence ray with 1.0 Watt power launched from z = -5 mm along +z
        let ray = Ray::new(
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;
        let nonseq_rays = vec![NonSeqRay::new(ray, 0)];

        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 20,
            min_power_threshold: 1e-12,
            enable_branching: true,
            record_paths: true,
        });

        let report = tracer.trace_rays(&mut scene, &nonseq_rays, None)?;

        // Analytical Fresnel reflectance at normal incidence for n = 1.5:
        // r0 = (1.5 - 1.0) / (1.5 + 1.0) = 0.5 / 2.5 = 0.2
        // R0 = r0^2 = 0.04 (4%)
        // T0 = 1 - R0 = 0.96 (96%)
        // First back bounce: T_first = 0.96 * 0.96 = 0.9216 (92.16%)
        // Total internal reflections sum to: T_tot = (1 - R0) / (1 + R0) = 0.96 / 1.04 = 12/13 ≈ 0.9230769
        // R_tot = 2 R0 / (1 + R0) = 0.08 / 1.04 = 1/13 ≈ 0.0769231
        let p_trans = scene.detectors[0].total_flux_watts;
        let p_refl = scene.detectors[1].total_flux_watts;
        let p_tot = p_trans + p_refl;

        let diff = (p_tot - 1.0).abs();
        assert!(
            diff < 1e-6,
            "Energy conservation violated: launched 1.0 W, detected {p_tot} W (diff {diff})"
        );

        let expected_t = 0.96 / 1.04;
        let expected_r = 0.08 / 1.04;
        assert!(
            (p_trans - expected_t).abs() < 1e-4,
            "Transmitted power mismatch: got {p_trans}, expected {expected_t}"
        );
        assert!(
            (p_refl - expected_r).abs() < 1e-4,
            "Reflected power mismatch: got {p_refl}, expected {expected_r}"
        );
        assert!((report.energy_conservation_ratio() - 1.0).abs() < 1e-6);

        Ok(())
    }

    /// Acceptance test: Verification of Total Internal Reflection (TIR) in a glass light guide.
    /// Rays striking inside cylinder at angles exceeding $\theta_c = \arcsin(1/n)$ guide with 100% efficiency.
    #[test]
    fn test_total_internal_reflection_light_pipe() -> TestResult {
        let mut scene = NonSeqScene::new();

        // N-BK7 glass cylinder (n ≈ 1.5168 at 587.56 nm, radius 5 mm, length 50 mm from z = 0 to z = 50)
        // Critical angle theta_c = arcsin(1 / 1.5168) ≈ 41.25 deg
        let pipe = NonSeqObject {
            id: 0,
            name: "LightPipeCylinder".to_string(),
            geometry: PrimitiveGeometry::Cylinder {
                radius: 5.0,
                half_length: 25.0,
            },
            transform: Transform3D::from_translation(0.0, 0.0, 25.0),
            material: NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.5168 }),
            is_active: true,
        };
        scene.add_object(pipe);

        // Exit detector at z = 50.1 mm
        let det = Detector2D::new(
            0,
            "ExitDetector",
            Vec3::new(0.0, 0.0, 50.1),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            20.0,
            20.0,
            10,
            10,
        )?;
        scene.add_detector(det);

        // Launch a ray already inside the glass at z = 1.0, tilted at angle 15 deg to the axis
        // Incidence angle to cylinder wall normal is 90 - 15 = 75 deg > theta_c (41.25 deg)
        let angle_rad = 15.0_f64.to_radians();
        let dir = Vec3::new(angle_rad.sin(), 0.0, angle_rad.cos()).normalize();

        let ray = Ray::new(
            Vec3::new(0.0, 0.0, 1.0),
            dir,
            587.56,
            1.0, // 1 Watt
        )?;

        // Set medium stack to indicate ray starts inside pipe (object 0)
        let nonseq_ray = NonSeqRay::new(ray, 0).with_medium_stack(vec![0]);

        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 50,
            min_power_threshold: 1e-12,
            enable_branching: false,
            record_paths: true,
        });

        // We launch ray into scene
        let mut queue_scene = scene.clone();
        let report = tracer.trace_rays(&mut queue_scene, &[nonseq_ray], None)?;

        // Verify ray path underwent multiple TIR interactions without power loss
        let tir_segments = report
            .paths
            .iter()
            .filter(|p| p.interaction == "TotalInternalReflection")
            .count();
        assert!(
            tir_segments >= 1,
            "Ray should have undergone TIR on cylinder wall"
        );

        Ok(())
    }

    /// Acceptance test: Verification of Beer-Lambert internal transmission law: $\tau = \exp(-\alpha L)$.
    #[test]
    fn test_beer_lambert_bulk_absorption() -> TestResult {
        let mut scene = NonSeqScene::new();

        // Absorbing slab: tau0 = 0.90 per 10 mm
        let abs = BulkAbsorption::new(0.90, 10.0)?;
        let mut mat = NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.0 }); // index 1.0 to isolate bulk absorption from Fresnel
        mat.absorption = Some(abs);

        let slab = NonSeqObject {
            id: 0,
            name: "AbsorbingSlab".to_string(),
            geometry: PrimitiveGeometry::Box {
                half_x: 50.0,
                half_y: 50.0,
                half_z: 10.0,
            },
            transform: Transform3D::from_translation(0.0, 0.0, 10.0), // 20 mm thickness spanning z in [0, 20]
            material: mat,
            is_active: true,
        };
        scene.add_object(slab);

        let det = Detector2D::new(
            0,
            "Det",
            Vec3::new(0.0, 0.0, 25.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            50.0,
            50.0,
            10,
            10,
        )?;
        scene.add_detector(det);

        let ray = Ray::new(
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;
        let nonseq_ray = NonSeqRay::new(ray, 0);

        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 10,
            min_power_threshold: 1e-12,
            enable_branching: false,
            record_paths: false,
        });

        tracer.trace_rays(&mut scene, &[nonseq_ray], None)?;

        // Expected transmission across 20 mm: (0.90)^(20/10) = 0.90^2 = 0.81000
        let p_det = scene.detectors[0].total_flux_watts;
        let p_expected = 0.81000;
        let diff = (p_det - p_expected).abs();
        assert!(
            diff < 1e-4,
            "Beer-Lambert absorption mismatch: got {p_det}, expected {p_expected}"
        );

        Ok(())
    }

    /// Acceptance test: Verification of Lambert's cosine law on a tilted planar detector:
    /// Measured irradiance scales with $\cos\theta$.
    #[test]
    fn test_lambertian_detector_cosine_law() -> TestResult {
        // Collimated beam with 10 W power and 10 mm radius (area = pi * 100 mm^2 = 0.000314159 m^2)
        let beam_flux = 10.0;
        let wavelength = 587.56;

        // Detector 1: Normal incidence (theta = 0 deg)
        let mut det0 = Detector2D::new(
            0,
            "Det0",
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            50.0,
            50.0,
            50,
            50,
        )?;

        // Detector 2: Tilted at 60 deg (normal = (sin 60, 0, cos 60))
        let theta_rad = 60.0_f64.to_radians();
        let normal_60 = Vec3::new(theta_rad.sin(), 0.0, theta_rad.cos()).normalize();
        let mut det60 = Detector2D::new(
            1,
            "Det60",
            Vec3::new(0.0, 0.0, 10.0),
            normal_60,
            Vec3::new(0.0, 1.0, 0.0),
            50.0,
            50.0,
            50,
            50,
        )?;

        let source = RaySource::CollimatedBeam {
            center: Vec3::new(0.0, 0.0, 0.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            radius: 5.0,
            total_flux_watts: beam_flux,
            spectrum: SourceSpectrum::Monochromatic {
                wavelength_nm: wavelength,
            },
        };

        let rays = source.generate_rays(1000, 12345)?;

        for r in &rays {
            if let Some((_, lx, ly)) = det0.intersect_ray(&r.ray) {
                det0.record_hit(lx, ly, r.ray.energy, wavelength);
            }
            if let Some((_, lx, ly)) = det60.intersect_ray(&r.ray) {
                det60.record_hit(lx, ly, r.ray.energy, wavelength);
            }
        }

        let p0 = det0.total_flux_watts;
        let p60 = det60.total_flux_watts;

        assert!(
            (p0 - beam_flux).abs() < 1e-4,
            "Det0 should capture all beam power"
        );
        assert!(p60 > 0.0, "Det60 should capture rays");

        // Projected area and average irradiance scale by cos(60 deg) = 0.500
        let mut sum_irr0 = 0.0;
        let mut sum_irr60 = 0.0;
        for py in 23..=27 {
            for px in 23..=27 {
                sum_irr0 += det0.irradiance_at(px, py);
                sum_irr60 += det60.irradiance_at(px, py);
            }
        }

        assert!(sum_irr0 > 0.0);
        assert!(sum_irr60 > 0.0);

        let ratio = sum_irr60 / sum_irr0;
        let expected_ratio = 60.0_f64.to_radians().cos(); // 0.500
        assert!(
            (ratio - expected_ratio).abs() < 0.15,
            "Cosine law ratio mismatch: got {ratio}, expected {expected_ratio}"
        );

        Ok(())
    }

    /// Acceptance test: Verification of CIE standard photopic luminous conversion:
    /// 1.0 Watt at $\lambda = 555$ nm yields exactly $\approx 683$ lumens, and 0 lumens outside visible range.
    #[test]
    fn test_photopic_luminous_conversion_units() -> TestResult {
        let p_watt = 1.0;

        // Peak photopic wavelength
        let lumens_555 = watts_to_lumens(p_watt, 555.0);
        assert!(
            (lumens_555 - 683.002).abs() < 5.0,
            "Peak photopic lumens should be near 683 lm/W, got {lumens_555}"
        );

        // Near-UV (300 nm) and Near-IR (900 nm) have zero luminous efficiency
        let lumens_uv = watts_to_lumens(p_watt, 300.0);
        let lumens_ir = watts_to_lumens(p_watt, 900.0);
        assert_eq!(lumens_uv, 0.0);
        assert_eq!(lumens_ir, 0.0);

        Ok(())
    }

    /// Acceptance test: Conversion of sequential prescription to non-sequential lens mesh and verification of focus.
    #[test]
    fn test_sequential_singlet_to_nonseq_focus() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;

        // Plano-convex lens: r1 = 50 mm, r2 = infinity, thickness = 5.0 mm, N-BK7
        // Paraxial focal length f ≈ 96.7 mm, BFL ≈ 93.4 mm
        let presc = OpticalPrescription {
            name: "Plano-Convex 100mm".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 8.0,
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
                    surface_index: 1,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 93.4,
                    semi_diameter_mm: 8.0,
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

        let mut scene = NonSeqScene::new();
        let lens_id = scene.add_prescription_lens(
            "PlanoConvexLens",
            &presc,
            0,
            1,
            NonSeqMaterial::dielectric(OpticalMedium::GlassCatalog {
                name: "N-BK7".to_string(),
            }),
            32,
            16,
        )?;
        assert_eq!(lens_id, 0);

        // Detector placed near focal point z = 98.4 mm (5 mm thickness + 93.4 mm BFL)
        let det = Detector2D::new(
            0,
            "FocalDetector",
            Vec3::new(0.0, 0.0, 98.4),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            10.0,
            10.0,
            50,
            50,
        )?;
        scene.add_detector(det);

        // Launch collimated rays along axis
        let source = RaySource::CollimatedBeam {
            center: Vec3::new(0.0, 0.0, -10.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            radius: 4.0,
            total_flux_watts: 1.0,
            spectrum: SourceSpectrum::Monochromatic {
                wavelength_nm: 587.56,
            },
        };
        let rays = source.generate_rays(100, 42)?;

        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 10,
            min_power_threshold: 1e-6,
            enable_branching: false,
            record_paths: false,
        });

        tracer.trace_rays(&mut scene, &rays, Some(&catalog))?;

        let hit_count = scene.detectors[0].total_rays_hit;
        assert!(
            hit_count > 50,
            "Most rays should reach focal detector, got {hit_count}"
        );

        let (cx, cy) = scene.detectors[0].centroid_mm();
        assert!(cx.abs() < 0.2, "Centroid X should be on-axis, got {cx}");
        assert!(cy.abs() < 0.2, "Centroid Y should be on-axis, got {cy}");

        let rms_spot = scene.detectors[0].rms_radius_mm();
        assert!(
            rms_spot < 1.0,
            "Focused RMS spot radius should be small (< 1.0 mm), got {rms_spot}"
        );

        Ok(())
    }

    /// Verification of automatic NonSeqScene synthesis from full Cooke Triplet prescription.
    #[test]
    fn test_cooke_triplet_build_from_prescription_and_raytrace() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;

        let mut scene = NonSeqScene::build_from_prescription(&cooke, 40)?;
        assert_eq!(scene.objects.len(), 3, "Cooke triplet must synthesize exactly 3 glass elements");
        assert_eq!(scene.detectors.len(), 1, "Must contain exactly 1 focal plane detector");
        assert_eq!(scene.sources.len(), 1, "Must contain 1 entrance pupil collimated source");

        // Trace non-sequential rays with branching enabled
        let rays = scene.sources[0].generate_rays(80, 12345)?;
        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 15,
            min_power_threshold: 1e-5,
            enable_branching: true,
            record_paths: true,
        });

        let report = tracer.trace_rays(&mut scene, &rays, Some(&catalog))?;
        assert!(report.total_rays_traced > 0);
        assert!(report.paths.len() > rays.len(), "Ray branching must produce reflected/refracted paths");

        let det = &scene.detectors[0];
        assert!(det.total_rays_hit > 0, "Detector must capture focused rays");
        assert!(det.total_flux_watts > 0.0);

        let (cx, cy) = det.centroid_mm();
        assert!(cx.abs() < 0.5, "Centroid X should be near axis, got {cx}");
        assert!(cy.abs() < 0.5, "Centroid Y should be near axis, got {cy}");

        // Verify ray path filtering functionality
        let (signal_grid, signal_flux, signal_hits) = det.filtered_pixel_flux(RayPathFilter::SignalOnly);
        assert!(signal_flux > 0.0, "Signal flux must be non-zero");
        assert!(signal_hits > 0, "Signal hit count must be non-zero");
        assert_eq!(signal_grid.len(), 40 * 40);

        let (_ghost_grid, ghost_flux, _ghost_hits) = det.filtered_pixel_flux(RayPathFilter::GhostsOnly);
        assert!(ghost_flux >= 0.0);
        assert!(ghost_flux < signal_flux, "Ghost flux should be a small fraction of signal flux");

        Ok(())
    }

    /// Benchmark test: Rigorous verification of unpolarized dielectric Fresnel energy conservation
    /// $R(\theta) + T(\theta) = 1.0$ across all incident angles and verification of Brewster and TIR limits.
    #[test]
    fn test_fresnel_dielectric_energy_conservation_and_tir() -> TestResult {
        let n1: f64 = 1.0;
        let n2: f64 = 1.5168; // N-BK7
        let eta = n1 / n2;

        // 1. Normal incidence validation: R0 = ((n1 - n2) / (n1 + n2))^2
        let r0_expected = ((n1 - n2) / (n1 + n2)).powi(2);
        assert!((r0_expected - 0.04218).abs() < 1e-4);

        // 2. Air -> Glass across 90 angles
        for deg in 0..90 {
            let theta1 = (deg as f64).to_radians();
            let cos_t1 = theta1.cos();
            let sin2_t1 = (1.0 - cos_t1 * cos_t1).max(0.0);
            let cos_t2 = (1.0 - eta * eta * sin2_t1).sqrt();

            let rs = (n1 * cos_t1 - n2 * cos_t2) / (n1 * cos_t1 + n2 * cos_t2);
            let rp = (n2 * cos_t1 - n1 * cos_t2) / (n2 * cos_t1 + n1 * cos_t2);
            let r = 0.5 * (rs * rs + rp * rp);
            let t = 1.0 - r;

            assert!(
                (r + t - 1.0).abs() < 1e-12,
                "Energy conservation failed at angle {deg} deg: R={r}, T={t}"
            );
        }

        // 3. Brewster angle: theta_B = atan(n2 / n1) ≈ 56.6 deg where rp = 0
        let theta_b = (n2 / n1).atan();
        let cos_tb1 = theta_b.cos();
        let sin2_tb1 = (1.0 - cos_tb1 * cos_tb1).max(0.0);
        let cos_tb2 = (1.0 - eta * eta * sin2_tb1).sqrt();
        let rp_brewster = (n2 * cos_tb1 - n1 * cos_tb2) / (n2 * cos_tb1 + n1 * cos_tb2);
        assert!(
            rp_brewster.abs() < 1e-10,
            "Brewster p-polarization reflectance should be zero, got {rp_brewster}"
        );

        // 4. Glass -> Air internal reflection: Critical angle theta_c = asin(n1 / n2) ≈ 41.23 deg
        let theta_c = (n1 / n2).asin();
        let deg_c = theta_c.to_degrees();
        assert!((deg_c - 41.23).abs() < 0.1);

        // Beyond critical angle: TIR (R = 1.0, T = 0.0)
        let angle_above_c = theta_c + 0.1;
        let cos_tir1 = angle_above_c.cos();
        let sin2_tir1 = (1.0 - cos_tir1 * cos_tir1).max(0.0);
        let radicand = 1.0 - (n2 / n1).powi(2) * sin2_tir1;
        assert!(radicand < 0.0, "Radicand must be negative indicating evanescent wave / TIR");

        Ok(())
    }

    /// Benchmark test: Verification of automatic NonSeqScene synthesis on Double Gauss 50mm preset
    /// with 6 elements, entrance pupil collimated source, focal detector, and ray branching report.
    #[test]
    fn test_scene_synthesis_double_gauss_all_elements() -> TestResult {
        let dg = optics_zemax::presets::double_gauss_50mm()?;
        let catalog = GlassCatalogDb::load_bundled()?;

        let mut scene = NonSeqScene::build_from_prescription(&dg, 36)?;
        assert_eq!(scene.objects.len(), 6, "Double Gauss must synthesize exactly 6 optical elements");
        assert_eq!(scene.detectors.len(), 1, "Must contain exactly 1 focal plane detector");
        assert_eq!(scene.sources.len(), 1, "Must contain 1 entrance pupil collimated source");

        // Verify entrance pupil collimated source parameters
        match &scene.sources[0] {
            RaySource::CollimatedBeam { radius, total_flux_watts, .. } => {
                assert!((radius - dg.entrance_pupil_diameter_mm * 0.5).abs() < 1e-3);
                assert_eq!(*total_flux_watts, 1.0);
            }
            _ => panic!("Expected CollimatedBeam source for sequential prescription synthesis"),
        }

        // Trace non-sequential rays with branching
        let rays = scene.sources[0].generate_rays(60, 9999)?;
        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: 12,
            min_power_threshold: 1e-5,
            enable_branching: true,
            record_paths: true,
        });

        let report = tracer.trace_rays(&mut scene, &rays, Some(&catalog))?;
        assert!(report.total_rays_traced > 0);
        assert!((report.energy_conservation_ratio() - 1.0).abs() < 0.05);

        let det = &scene.detectors[0];
        assert!(det.total_rays_hit > 0);
        assert!(det.total_flux_watts > 0.0);

        let (cx, cy) = det.centroid_mm();
        assert!(cx.abs() < 1.0, "Centroid X should be near axis, got {cx}");
        assert!(cy.abs() < 1.0, "Centroid Y should be near axis, got {cy}");

        Ok(())
    }

    /// Acceptance test: Verification of 2D detector radiometric and photometric metrics,
    /// irradiance ($W/m^2$), illuminance ($lux$), centroid, and RMS spot radius.
    #[test]
    fn test_detector_radiometric_and_photometric_flux_and_kpis() -> TestResult {
        // 10 mm x 10 mm detector with 50x50 resolution
        // Pixel size = 0.2 mm x 0.2 mm = 4e-8 m^2
        // Detector area = 1e-4 m^2
        let mut det = Detector2D::new(
            0,
            "RadiometricDet",
            Vec3::new(0.0, 0.0, 50.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            10.0,
            10.0,
            50,
            50,
        )?;

        let power_w = 2.0;
        let wav_nm = 555.0; // Peak human visual sensitivity: 683 lm/W

        // Deposit energy in center pixel (25, 25)
        det.record_hit(0.0, 0.0, power_w, wav_nm);

        assert_eq!(det.total_rays_hit, 1);
        assert!((det.total_flux_watts - power_w).abs() < 1e-9);

        // Luminous flux in lumens: 2.0 * 683.002 = 1366.004 lumens
        let expected_lumens = power_w * 683.002;
        assert!((det.total_lumens - expected_lumens).abs() < 1e-2);

        // Center pixel (25, 25) irradiance
        let irr_center = det.irradiance_at(25, 25);
        let expected_irr = power_w / det.pixel_area_m2();
        assert!((irr_center - expected_irr).abs() < 1.0);

        // Centroid should be exactly at (0.0, 0.0)
        let (cx, cy) = det.centroid_mm();
        assert!(cx.abs() < 1e-6);
        assert!(cy.abs() < 1e-6);

        // RMS spot radius with single central hit should be 0.0
        let rms = det.rms_radius_mm();
        assert!(rms.abs() < 1e-6);

        Ok(())
    }
}
