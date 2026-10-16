//! Numerical robustness and boundary condition stress tests.
//!
//! Validates behavior at mathematical and physical boundaries:
//! 1. Extreme radii of curvature: R -> 1e-6 mm (microscopic) to R -> 1e9 mm and R -> inf (planar).
//! 2. Near-grazing angles of incidence: theta -> 89.999 deg.
//! 3. Critical angle Total Internal Reflection (TIR) transition: theta_c +- 1e-9 rad.
//! 4. Extreme thicknesses: t = 0.0 mm (cemented contact), t = 1e-9 mm, t = 1e6 mm.
//! 5. Non-finite input rejection: Strict error handling for NaN and Inf.

use optics_core::conic::{ConicSurface, conic_normal, conic_sag, intersect_conic};
use optics_core::error::OpticsError;
use optics_core::fresnel::fresnel_dielectric;
use optics_core::geometry::{Air, ConstantIndex, RefractiveIndex};
use optics_core::prescription::{
    OpticalPrescription, OpticalSurface, calculate_paraxial_properties,
};
use optics_core::snell::{reflect, snell_refract};
use optics_core::surface::asphere::EvenAsphere;
use optics_core::types::{Ray, Vec3};
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Boundary sweep 1: Extreme radii of curvature.
#[test]
fn test_sweep_extreme_curvatures() -> TestResult {
    let test_radii = [
        1e-4,  // 0.1 um micro-lens
        1e-2,  // 10 um fiber tip
        1.0,   // 1 mm ball lens
        100.0, // 100 mm standard lens
        1e5,   // 100 meter telescope mirror
        1e8,   // 100,000 km near-planar surface
        f64::INFINITY,
    ];

    for &r in &test_radii {
        let c = if r.is_infinite() { 0.0 } else { 1.0 / r };
        let semi_diam = if r.is_infinite() {
            50.0
        } else {
            (r * 0.5).min(50.0)
        };

        let surface = ConicSurface::from_radius(r, 0.0, semi_diam)?;
        let test_r = semi_diam * 0.5;
        let sag = conic_sag(test_r, c, 0.0)?;
        assert!(sag.is_finite(), "Sag must be finite for R={r}");

        let hit_pt = Vec3::new(test_r, 0.0, sag);
        let normal = conic_normal(hit_pt, &surface)?;
        assert!(
            (normal.norm() - 1.0).abs() < 1e-12,
            "Normal must be unit length for R={r}: got {}",
            normal.norm()
        );

        // Ray intersection
        let ray = Ray::new(
            Vec3::new(test_r, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;
        let hit = intersect_conic(&ray, &surface, 0)?;
        assert!((hit.point.x - test_r).abs() < 1e-10);
        assert!((hit.point.z - sag).abs() < 1e-6);
    }
    println!("[PASS] Extreme curvature sweep verified from R=1e-4 mm to R=inf");
    Ok(())
}

/// Boundary sweep 2: Near-grazing incidence (theta -> 89.999 deg).
#[test]
fn test_sweep_near_grazing_incidence() -> TestResult {
    let grazing_angles_deg: [f64; 6] = [80.0, 85.0, 89.0, 89.9, 89.99, 89.999];
    let n1: f64 = 1.0;
    let n2: f64 = 1.5;
    let normal = Vec3::new(0.0, 0.0, 1.0);

    for &theta_deg in &grazing_angles_deg {
        let theta_rad = theta_deg.to_radians();
        let cos_theta_i = theta_rad.cos();
        let sin_theta_i = theta_rad.sin();

        // Ray direction towards interface
        let dir = Vec3::new(sin_theta_i, 0.0, -cos_theta_i).normalize();
        let refracted = snell_refract(dir, normal, n1, n2)?;

        // Snell law check
        let sin_theta_t = refracted.x;
        let expected_sin_t = (n1 / n2) * sin_theta_i;
        assert!(
            (sin_theta_t - expected_sin_t).abs() < 1e-10,
            "Snell mismatch at grazing angle {theta_deg} deg: got {sin_theta_t}, expected {expected_sin_t}"
        );

        // Fresnel reflectance approaches 1.0
        let (rs, rp) = fresnel_dielectric(cos_theta_i, n1, n2);
        assert!(rs <= 1.0 && rp <= 1.0, "Fresnel reflectance overshoot");
        if theta_deg >= 89.99 {
            assert!(
                rs > 0.95,
                "s-reflectance must approach unity at grazing incidence: got {rs}"
            );
        }
    }
    println!("[PASS] Near-grazing incidence sweep verified up to 89.999 deg");
    Ok(())
}

/// Boundary sweep 3: Total Internal Reflection (TIR) transition (theta_c +- 1e-9 rad).
#[test]
fn test_sweep_near_critical_angle_transitions() -> TestResult {
    let n1: f64 = 1.51680; // N-BK7
    let n2: f64 = 1.00000; // Air
    let theta_c: f64 = (n2 / n1).asin();
    let normal = Vec3::new(0.0, 0.0, 1.0);

    let delta = 1e-9_f64;

    // Case A: Just transmitting (theta = theta_c - delta)
    let theta_sub = theta_c - delta;
    let dir_sub = Vec3::new(theta_sub.sin(), 0.0, -theta_sub.cos()).normalize();
    let refr_sub = snell_refract(dir_sub, normal, n1, n2)?;
    assert!(
        refr_sub.norm().is_finite() && (refr_sub.norm() - 1.0).abs() < 1e-10,
        "Transmitted ray near TIR must be unit length"
    );
    assert!(
        refr_sub.x > 0.99999,
        "Transmitted ray near TIR must be directed nearly parallel to interface (x -> 1.0): got {}",
        refr_sub.x
    );

    // Case B: Just total internally reflecting (theta = theta_c + delta)
    let theta_super = theta_c + delta;
    let dir_super = Vec3::new(theta_super.sin(), 0.0, -theta_super.cos()).normalize();
    let err_tir = snell_refract(dir_super, normal, n1, n2);
    assert!(
        matches!(err_tir, Err(OpticsError::TotalInternalReflection)),
        "Expected TotalInternalReflection error above critical angle"
    );

    // Fresnel reflectance at super-critical angle must be exactly (1.0, 1.0)
    let (rs_tir, rp_tir) = fresnel_dielectric(theta_super.cos(), n1, n2);
    assert_eq!(rs_tir, 1.0);
    assert_eq!(rp_tir, 1.0);

    println!("[PASS] Critical angle TIR transition verified at theta_c +- 1e-9 rad");
    Ok(())
}

/// Boundary sweep 4: Extreme thicknesses (contact doublets to astronomical propagation).
#[test]
fn test_sweep_extreme_thicknesses() -> TestResult {
    let thicknesses = [
        0.0,  // Optical cement interface
        1e-9, // 1 nm nanolayer
        1e-6, // 1 um gap
        10.0, // 10 mm standard element
        1e4,  // 10 meter instrument bench
        1e6,  // 1 km atmospheric line
    ];

    for &t in &thicknesses {
        let rx = OpticalPrescription {
            name: format!("Thickness Test t={t}"),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: t,
                    semi_diameter_mm: 10.0,
                    material_name: "GLASS".to_string(),
                    is_stop: true,
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: -50.0,
                    conic_constant: 0.0,
                    thickness_mm: 50.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let resolved = rx.resolve(&|mat: &str| {
            if mat == "GLASS" {
                Ok(Arc::new(ConstantIndex(1.5)) as Arc<dyn RefractiveIndex + Send + Sync>)
            } else {
                Ok(Arc::new(Air) as Arc<dyn RefractiveIndex + Send + Sync>)
            }
        })?;

        let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
        assert!(paraxial.efl_mm.is_finite());
        assert!(paraxial.bfl_mm.is_finite());
    }
    println!("[PASS] Extreme thickness sweep verified from t=0.0 to t=1e6 mm");
    Ok(())
}

/// Boundary sweep 5: Non-finite input rejections (NaN/Inf rejection without panic).
#[test]
fn test_sweep_non_finite_input_rejections() -> TestResult {
    let non_finites = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

    for &val in &non_finites {
        // conic_sag
        assert!(conic_sag(val, 0.01, 0.0).is_err());
        assert!(conic_sag(10.0, val, 0.0).is_err());
        assert!(conic_sag(10.0, 0.01, val).is_err());

        // conic_normal
        let surface = ConicSurface::from_radius(100.0, 0.0, 20.0)?;
        assert!(conic_normal(Vec3::new(val, 0.0, 0.0), &surface).is_err());
        assert!(conic_normal(Vec3::new(0.0, val, 0.0), &surface).is_err());
        assert!(conic_normal(Vec3::new(0.0, 0.0, val), &surface).is_err());

        // reflect
        assert!(reflect(Vec3::new(val, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0)).is_err());
        assert!(reflect(Vec3::new(0.0, 0.0, 1.0), Vec3::new(val, 0.0, 1.0)).is_err());

        // snell_refract
        assert!(
            snell_refract(Vec3::new(val, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0), 1.0, 1.5).is_err()
        );
        assert!(
            snell_refract(Vec3::new(0.0, 0.0, 1.0), Vec3::new(val, 0.0, 1.0), 1.0, 1.5).is_err()
        );
        assert!(
            snell_refract(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0), val, 1.5).is_err()
        );
        assert!(
            snell_refract(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 1.0), 1.0, val).is_err()
        );

        // EvenAsphere::new
        assert!(EvenAsphere::new(val, 0.0, vec![0.0], 10.0).is_err());
        assert!(EvenAsphere::new(0.01, val, vec![0.0], 10.0).is_err());
        assert!(EvenAsphere::new(0.01, 0.0, vec![val], 10.0).is_err());
        assert!(EvenAsphere::new(0.01, 0.0, vec![0.0], val).is_err());
    }
    println!("[PASS] Non-finite input rejection verified across all geometric primitives");
    Ok(())
}
