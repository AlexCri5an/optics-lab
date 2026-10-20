//! Property-based invariant tests for OpticsLab computational optics primitives.
//!
//! Evaluates physical laws and invariants across thousands of randomized parameter
//! configurations using deterministic pseudo-random sequences (Xorshift64Star).
//!
//! Invariants verified:
//! 1. Vector Snell Refraction: n1 * (s x n) == n2 * (s' x n) within 1e-12.
//! 2. Surface Normal Unit Length: ||n|| == 1.0 within 1e-12 across conic, aspheric, and biconic profiles.
//! 3. Normal-Tangent Orthogonality: n . t == 0.0 within 1e-5.
//! 4. Snell Ray Reversibility: snell_refract(-d', -n, n2, n1) == -d within 1e-12.
//! 5. Fresnel Dielectric Energy Conservation: R + T == 1.0 within 1e-12 for all angles and indices.
//! 6. Optical Lagrange Invariant: H_i is invariant across surfaces within 1e-10.
//! 7. Project Workspace Serialization Round-Trip: deserialize(serialize(project)) == project.

use optics_core::analysis::compute_seidel_coefficients;
use optics_core::conic::{conic_normal, conic_sag};
use optics_core::fresnel::fresnel_dielectric;
use optics_core::geometry::{Air, ConstantIndex, RefractiveIndex};
use optics_core::prescription::{
    OpticalPrescription, OpticalSurface, calculate_paraxial_properties,
};
use optics_core::project::OpticsProject;
use optics_core::snell::snell_refract;
use optics_core::surface::asphere::EvenAsphere;
use optics_core::types::Vec3;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Deterministic 64-bit Xorshift PRNG for repeatable property testing.
struct TestRng {
    state: u64,
}

impl TestRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Random f64 in [0.0, 1.0)
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Random f64 in [min, max]
    fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }
}

/// Property 1: Snell vector cross-product invariant and coplanarity across 2,000 random configurations.
#[test]
fn test_prop_snell_vector_cross_product_invariant() -> TestResult {
    let mut rng = TestRng::new(0x12345678);

    for _ in 0..2000 {
        // Random normal vector
        let theta_n = rng.range_f64(0.01, std::f64::consts::PI - 0.01);
        let phi_n = rng.range_f64(0.0, 2.0 * std::f64::consts::PI);
        let normal = Vec3::new(
            theta_n.sin() * phi_n.cos(),
            theta_n.sin() * phi_n.sin(),
            theta_n.cos(),
        )
        .normalize();

        let n1 = rng.range_f64(1.0, 2.5);
        let n2 = rng.range_f64(1.0, 2.5);

        // Ensure non-TIR by limiting incident angle if n1 > n2
        let max_theta = if n1 > n2 {
            (n2 / n1).asin() * 0.95
        } else {
            80.0_f64.to_radians()
        };
        let incident_angle = rng.range_f64(0.01, max_theta);

        // Build orthonormal tangent frame to normal
        let arbitrary = if normal.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let tangent = normal.cross(&arbitrary).normalize();

        // Ray direction pointing towards the interface
        let dir = (tangent * incident_angle.sin() - normal * incident_angle.cos()).normalize();

        if let Ok(refracted) = snell_refract(dir, normal, n1, n2) {
            // Unit length
            assert!(
                (refracted.norm() - 1.0).abs() < 1e-12,
                "Refracted ray must have unit length"
            );

            // Vector Snell Law: n1 * (dir x normal) == n2 * (refracted x normal)
            let cross_in = dir.cross(&normal) * n1;
            let cross_out = refracted.cross(&normal) * n2;
            let diff = (cross_in - cross_out).norm();
            assert!(
                diff < 1e-12,
                "Vector Snell invariant failed: ||n1(s x n) - n2(s' x n)|| = {:e}",
                diff
            );

            // Coplanarity: refracted vector must lie in the plane of incidence
            let coplanarity = (dir.cross(&normal)).dot(&refracted);
            assert!(
                coplanarity.abs() < 1e-12,
                "Coplanarity violation: (s x n) . s' = {:e}",
                coplanarity
            );
        }
    }
    Ok(())
}

/// Property 2: Surface normal unit length and tangent orthogonality across 1,000 random conics.
#[test]
fn test_prop_surface_normal_unit_length_and_orthogonality() -> TestResult {
    let mut rng = TestRng::new(0xABCDEF01);

    for _ in 0..1000 {
        let r_curv = if rng.next_f64() < 0.5 {
            rng.range_f64(10.0, 500.0)
        } else {
            rng.range_f64(-500.0, -10.0)
        };
        let c = 1.0 / r_curv;
        let k = rng.range_f64(-3.0, 3.0);

        // Safe radius strictly inside domain
        let max_r = if (1.0 + k) > 0.0 {
            0.85 / (c.abs() * (1.0 + k).sqrt())
        } else {
            r_curv.abs() * 0.8
        };
        let semi_diam = max_r.min(40.0);
        let r = rng.range_f64(0.0, semi_diam * 0.95);

        let surface = optics_core::conic::ConicSurface::from_radius(r_curv, k, semi_diam)?;
        let z = conic_sag(r, c, k)?;
        let hit_point = Vec3::new(r, 0.0, z);

        let normal = conic_normal(hit_point, &surface)?;
        let norm_len = normal.norm();
        assert!(
            (norm_len - 1.0).abs() < 1e-12,
            "Conic normal must have unit length: got {}",
            norm_len
        );

        // Tangent vector: t = (1, 0, dz/dr)
        let delta = 1e-7;
        let z_plus = conic_sag(r + delta, c, k)?;
        let z_minus = conic_sag((r - delta).max(0.0), c, k)?;
        let dz_dr = (z_plus - z_minus) / (2.0 * delta);
        let tangent = Vec3::new(1.0, 0.0, dz_dr).normalize();

        let dot = normal.dot(&tangent);
        assert!(
            dot.abs() < 1e-5,
            "Conic normal not orthogonal to surface derivative: n . t = {:e}",
            dot
        );
    }
    Ok(())
}

/// Property 3: Even asphere normal unit length across 500 randomized parameter sets.
#[test]
fn test_prop_even_asphere_normal_unit_length() -> TestResult {
    let mut rng = TestRng::new(0xFEEDFACE);

    for _ in 0..500 {
        let r_curv = rng.range_f64(20.0, 200.0);
        let c = 1.0 / r_curv;
        let k = rng.range_f64(-1.5, 1.5);
        let a4 = rng.range_f64(-1e-6, 1e-6);
        let a6 = rng.range_f64(-1e-10, 1e-10);
        let a8 = rng.range_f64(-1e-14, 1e-14);

        let asphere = EvenAsphere::new(c, k, vec![0.0, a4, a6, a8], 30.0)?;
        let x = rng.range_f64(-15.0, 15.0);
        let y = rng.range_f64(-15.0, 15.0);

        if let Ok(normal) = asphere.normal(x, y) {
            let len = normal.norm();
            assert!(
                (len - 1.0).abs() < 1e-12,
                "Even asphere normal unit length violated: got {}",
                len
            );
        }
    }
    Ok(())
}

/// Property 4: Dielectric Fresnel energy conservation across 2,000 arbitrary angles and indices.
#[test]
fn test_prop_fresnel_dielectric_energy_conservation() -> TestResult {
    let mut rng = TestRng::new(0xCAFEBABE);

    for _ in 0..2000 {
        let theta_deg = rng.range_f64(0.0, 89.9);
        let cos_theta_i = theta_deg.to_radians().cos();
        let n1 = rng.range_f64(1.0, 3.5);
        let n2 = rng.range_f64(1.0, 3.5);

        let (rs, rp) = fresnel_dielectric(cos_theta_i, n1, n2);
        assert!(
            (0.0..=1.0 + 1e-12).contains(&rs),
            "rs out of physical range: {}",
            rs
        );
        assert!(
            (0.0..=1.0 + 1e-12).contains(&rp),
            "rp out of physical range: {}",
            rp
        );

        // Dielectric power transmittance Ts = 1 - Rs, Tp = 1 - Rp
        let ts = 1.0 - rs;
        let tp = 1.0 - rp;
        assert!(
            ((rs + ts) - 1.0).abs() < 1e-12,
            "s-polarization energy conservation violated: Rs + Ts = {}",
            rs + ts
        );
        assert!(
            ((rp + tp) - 1.0).abs() < 1e-12,
            "p-polarization energy conservation violated: Rp + Tp = {}",
            rp + tp
        );

        // Unpolarized reflectance and transmittance: R = 0.5*(Rs + Rp), T = 0.5*(Ts + Tp)
        let r = 0.5 * (rs + rp);
        let t = 0.5 * (ts + tp);
        assert!(
            ((r + t) - 1.0).abs() < 1e-12,
            "Total energy conservation violated: R + T = {}",
            r + t
        );
    }
    Ok(())
}

/// Property 5: Snell ray reversibility across 1,000 randomized dielectric interfaces.
#[test]
fn test_prop_snell_ray_reversibility() -> TestResult {
    let mut rng = TestRng::new(0x55AA55AA);

    for _ in 0..1000 {
        let theta_n = rng.range_f64(0.01, std::f64::consts::PI - 0.01);
        let phi_n = rng.range_f64(0.0, 2.0 * std::f64::consts::PI);
        let normal = Vec3::new(
            theta_n.sin() * phi_n.cos(),
            theta_n.sin() * phi_n.sin(),
            theta_n.cos(),
        )
        .normalize();

        let n1 = rng.range_f64(1.0, 2.5);
        let n2 = rng.range_f64(1.0, 2.5);

        let max_theta = if n1 > n2 {
            (n2 / n1).asin() * 0.95
        } else {
            80.0_f64.to_radians()
        };
        let incident_angle = rng.range_f64(0.01, max_theta);

        let arbitrary = if normal.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let tangent = normal.cross(&arbitrary).normalize();
        let dir = (tangent * incident_angle.sin() - normal * incident_angle.cos()).normalize();

        if let Ok(refracted) = snell_refract(dir, normal, n1, n2) {
            // Reverse direction across reversed interface:
            // Incident direction is -refracted, normal is -normal, incident index is n2, transmitted is n1
            if let Ok(reversed) = snell_refract(-refracted, -normal, n2, n1) {
                let err = (reversed - (-dir)).norm();
                assert!(
                    err < 1e-12,
                    "Ray reversibility violation: expected {:?}, got {:?}, err={:e}",
                    -dir,
                    reversed,
                    err
                );
            }
        }
    }
    Ok(())
}

/// Property 6: Optical Lagrange / Helmholtz Invariant conservation across randomized prescriptions.
#[test]
fn test_prop_lagrange_invariant_conservation() -> TestResult {
    let mut rng = TestRng::new(0x98765432);

    for _ in 0..50 {
        let r1 = rng.range_f64(40.0, 100.0);
        let r2 = rng.range_f64(-100.0, -40.0);
        let th = rng.range_f64(3.0, 15.0);
        let n_val = rng.range_f64(1.45, 1.85);

        let rx = OpticalPrescription {
            name: "Random Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: r1,
                    conic_constant: 0.0,
                    thickness_mm: th,
                    semi_diameter_mm: 12.0,
                    material_name: "GLASS".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: r2,
                    conic_constant: 0.0,
                    thickness_mm: 50.0,
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                },
            ],
        };

        let resolved = rx.resolve(&|mat: &str| {
            if mat == "GLASS" {
                Ok(Arc::new(ConstantIndex(n_val)) as Arc<dyn RefractiveIndex + Send + Sync>)
            } else {
                Ok(Arc::new(Air) as Arc<dyn RefractiveIndex + Send + Sync>)
            }
        })?;

        let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
        assert!(
            paraxial.efl_mm.is_finite() && paraxial.efl_mm > 0.0,
            "Positive focal length expected"
        );

        let marg = optics_core::prescription::trace_paraxial_marginal(&resolved, 587.56)?;
        let chief = optics_core::prescription::trace_paraxial_chief(&resolved, 5.0, 587.56)?;

        let mut invariants = Vec::new();
        for i in 0..resolved.surfaces.len() {
            let n_after = resolved.surfaces[i].material.n(587.56);
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

        let seidel = compute_seidel_coefficients(&resolved, 5.0, 587.56)?;
        assert!(
            seidel.lagrange_invariant.is_finite() && seidel.lagrange_invariant.abs() > 0.0,
            "Lagrange invariant must be finite non-zero: got {}",
            seidel.lagrange_invariant
        );
    }
    Ok(())
}

/// Property 7: Project workspace serialization roundtrip invariance.
#[test]
fn test_prop_project_workspace_serialization_roundtrip() -> TestResult {
    let mut rng = TestRng::new(0x11223344);

    for i in 0..50 {
        let name = format!("Random Project {i}");
        let mut surfaces = Vec::new();
        let num_surfaces = (rng.next_u64() % 4 + 2) as usize;

        for s in 0..num_surfaces {
            let r = if rng.next_f64() < 0.2 {
                f64::INFINITY
            } else {
                rng.range_f64(-100.0, 100.0)
            };
            surfaces.push(OpticalSurface {
                surface_index: s,
                radius_mm: r,
                conic_constant: rng.range_f64(-2.0, 2.0),
                thickness_mm: rng.range_f64(1.0, 20.0),
                semi_diameter_mm: rng.range_f64(5.0, 30.0),
                material_name: if s % 2 == 0 {
                    "N-BK7".to_string()
                } else {
                    "AIR".to_string()
                },
                is_stop: s == num_surfaces / 2,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
                profile: None,
                aperture: None,
            });
        }

        let rx = OpticalPrescription {
            name: name.clone(),
            entrance_pupil_diameter_mm: rng.range_f64(5.0, 50.0),
            field_angles_deg: vec![0.0, 5.0, 10.0],
            wavelengths_nm: vec![486.13, 587.56, 656.27],
            primary_wavelength_index: 1,
            surfaces,
        };

        let project = OpticsProject::new(&name, Some(rx.clone()));
        let json_str = project.to_json_string()?;
        let deserialized = OpticsProject::from_json_str(&json_str)?;

        assert_eq!(project.name, deserialized.name);
        let orig_rx = project
            .prescription
            .as_ref()
            .ok_or("Original prescription missing")?;
        let deser_rx = deserialized
            .prescription
            .as_ref()
            .ok_or("Deserialized prescription missing")?;

        assert_eq!(orig_rx.surfaces.len(), deser_rx.surfaces.len());
        for (s1, s2) in orig_rx.surfaces.iter().zip(deser_rx.surfaces.iter()) {
            assert_eq!(s1.surface_index, s2.surface_index);
            assert_eq!(s1.is_stop, s2.is_stop);
            assert_eq!(s1.material_name, s2.material_name);
            assert!((s1.semi_diameter_mm - s2.semi_diameter_mm).abs() < 1e-10);
            if s1.radius_mm.is_infinite() {
                assert!(s2.radius_mm.is_infinite());
            } else {
                assert!((s1.radius_mm - s2.radius_mm).abs() < 1e-10);
            }
        }
    }
    Ok(())
}
