//! `optics-core` — Ray primitives, 3D math, Snell/Fresnel vector algebra,
//! and sequential paraxial ray tracing.
//!
//! # Architecture & Accuracy Policy
//! All CPU calculations in `optics-core` utilize double-precision floating point (`f64`)
//! to ensure optical calculations (e.g. cumulative ray aberrations, interferometric path lengths,
//! and paraxial marginal/chief traces) remain numerically stable over multi-surface systems.
//!
//! # Coordinate & Normal Conventions
//! - Space coordinate system: right-handed 3D Cartesian coordinates in millimeters.
//! - Surface Normal Convention: Interface normals $\mathbf{n}$ are canonicalized to point
//!   **toward the incident medium** ($\mathbf{i} \cdot \mathbf{n} \le 0$). All refraction
//!   and reflection routines automatically detect and invert flipped normals if passed.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod aiming;
pub mod analysis;
pub mod conic;
pub mod error;
pub mod fresnel;
pub mod geometry;
pub mod multiconfig;
pub mod path;
pub mod polarization;
pub mod prescription;
pub mod project;
pub mod snell;
pub mod surface;
pub mod tessellate;
pub mod thin_film;
pub mod types;

// Re-export primary types and functions at the crate root
pub use aiming::{
    Conjugate, PolarizedRayTraceResult, RayAimingMode, SurfacePolarizationRecord,
    VignettingFactors, WavelengthSet, trace_aimed_ray, trace_polarized_ray,
};
pub use analysis::{
    FieldCurves, FieldPointAnalysis, FootprintDiagram, FootprintPoint, GhostAnalysis,
    GhostReflection, LongitudinalAberrationPoint, MtfCurve, MtfPoint, OpdFanData, OpdFanPoint,
    PointSpreadFunction, PrescriptionSummary, PupilCoord, PupilPattern, RayFanData, RayFanPoint,
    RayTable, SeidelCoefficients, SpotDiagram, SpotMetrics, SpotPoint, SurfaceRayRow,
    SurfaceSeidel, ThroughFocusAnalysis, ThroughFocusMtfPoint, ThroughFocusSpotPoint, WavefrontMap,
    ZernikeFit, compute_diffraction_mtf, compute_fft_psf, compute_field_curves,
    compute_footprint_diagram, compute_geometric_mtf, compute_longitudinal_spherical_aberration,
    compute_opd_fan, compute_ray_table, compute_seidel_coefficients,
    compute_through_focus_geometric_mtf, compute_through_focus_spot, compute_two_bounce_ghosts,
    diffraction_limit_mtf, fit_zernike_noll, generate_prescription_summary,
    generate_pupil_distribution, launch_pupil_ray, noll_index_to_nm, spot_diagram_metrics,
    trace_ray_fan, trace_spot_diagram, zernike_noll_eval,
};
pub use conic::{ConicSurface, conic_normal, conic_sag, intersect_conic};
pub use error::OpticsError;
pub use fresnel::{fresnel_dielectric, fresnel_unpolarized, schlick_approx};
pub use geometry::{Air, ConstantIndex, RefractiveIndex, Triangle};
pub use multiconfig::{
    ConfigOverride, Configuration, MultiConfigSystem, Pickup, solve_focal_length,
};
pub use path::{PathRecord, RayBranchPolicy, Scene, spectral_fan, trace_path};
pub use polarization::{
    JonesMatrix, JonesVector, MuellerMatrix, StokesVector, surface_coating_jones,
    surface_fresnel_jones,
};
pub use prescription::{
    MaterialResolver, OpticalPrescription, OpticalSurface, ParaxialProperties, ParaxialRay,
    ResolvedPrescription, ResolvedSurface, SequentialPathRecord, SequentialRayHit,
    calculate_paraxial_properties, trace_paraxial_chief, trace_paraxial_marginal, trace_sequential,
};
pub use project::{CameraConfig, OpticsProject, PROJECT_SCHEMA_VERSION, RenderConfig, SceneConfig};
pub use snell::{reflect, snell_refract};
pub use surface::{
    Aperture, ApertureShape, BiconicSurface, CoordinateBreak, DiffractivePhaseSurface, EvenAsphere,
    ForbesQbfsSurface, ForbesQconSurface, GrinProfile, OddAsphere, SurfaceProfile, SurfaceTransform,
    ToroidalSurface, XyPolynomialSurface, XyTerm, ZernikeSagSurface, trace_grin_rk4,
};
pub use tessellate::{
    prescription_to_trimesh, prescription_to_trimesh_with_materials, tessellate_conic_surface,
    tessellate_lens_rim,
};
pub use thin_film::{Coating, CoatingResponse, Complex, SurfaceCoating, ThinFilmLayer};
pub use types::{
    BounceEvent, Field, HitRecord, Pupil, Ray, RayBounce, SurfaceId, Vec3, Wavelength,
};

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Helper to construct a normalized 2D direction in the XY plane.
    /// theta=0 points along -Y (normal incidence against surface at Y=0 with normal +Y).
    fn dir_from_angle(theta_rad: f64) -> Vec3 {
        Vec3::new(theta_rad.sin(), -theta_rad.cos(), 0.0)
    }

    #[test]
    fn test_snell_air_to_nbk7_refraction_angles() -> TestResult {
        // Analytically derived:
        // n1 = 1.0 (air), n2 = 1.5168 (Schott N-BK7 at sodium D line 589.3 nm)
        // Snell's Law: n1 * sin(theta_i) = n2 * sin(theta_t)
        // => theta_t = asin(sin(theta_i) / n2)
        let n1 = 1.0;
        let n2 = 1.5168;
        let normal = Vec3::new(0.0, 1.0, 0.0);

        for deg in [30.0_f64, 45.0_f64] {
            let theta_i = deg.to_radians();
            let incident = dir_from_angle(theta_i);
            let refracted = snell_refract(incident, normal, n1, n2)?;

            // Expected refraction angle relative to -normal:
            let expected_theta_t = (theta_i.sin() / n2).asin();

            // Computed refraction angle from refracted vector:
            // refracted = (sin(theta_t), -cos(theta_t), 0)
            let computed_theta_t = (-refracted.dot(&normal)).clamp(0.0, 1.0).acos();

            assert!(
                (computed_theta_t - expected_theta_t).abs() < 1e-9,
                "Refraction angle mismatch at {deg}°: got {computed_theta_t}, expected {expected_theta_t}"
            );

            // Refracted x-component should match sin(theta_t) directly
            assert!(
                (refracted.x - expected_theta_t.sin()).abs() < 1e-9,
                "Refracted x component mismatch at {deg}°"
            );
        }

        Ok(())
    }

    #[test]
    fn test_snell_reversibility() -> TestResult {
        // Principle of Optical Reversibility:
        // Light traveling from Medium 1 -> Medium 2 along ray i creates transmitted ray t.
        // A ray starting in Medium 2 along -t traversing into Medium 1 must exit along -i.
        let n1 = 1.0;
        let n2 = 1.5168;
        let normal = Vec3::new(0.0, 1.0, 0.0);

        let theta_i = 35.0_f64.to_radians();
        let incident = dir_from_angle(theta_i);

        let refracted = snell_refract(incident, normal, n1, n2)?;

        // Reverse path: ray starts in glass propagating in direction refracted,
        // striking interface from glass into air.
        // Interface normal pointing toward glass (incident medium) is -normal.
        let reverse_incident = refracted;
        let reverse_normal = -normal;
        let reverse_refracted = snell_refract(reverse_incident, reverse_normal, n2, n1)?;

        let diff = (reverse_refracted - incident).norm();
        assert!(
            diff < 1e-9,
            "Reversibility violated: difference norm = {diff}"
        );

        Ok(())
    }

    #[test]
    fn test_snell_geometry_properties() -> TestResult {
        let n1 = 1.0;
        let n2 = 1.5168;
        let normal = Vec3::new(0.0, 1.0, 0.0);

        // 1. Normal incidence: incident along (0, -1, 0) passes straight through
        let normal_incident = Vec3::new(0.0, -1.0, 0.0);
        let normal_refracted = snell_refract(normal_incident, normal, n1, n2)?;
        assert!(
            (normal_refracted - normal_incident).norm() < 1e-12,
            "Normal incidence should not deviate"
        );

        // 2. Unit length & Coplanarity for arbitrary 3D incident ray
        let incident = Vec3::new(0.3, -0.8, 0.5196152422706632).normalize();
        let arbitrary_normal = Vec3::new(0.1, 0.98, -0.17).normalize();
        let refracted = snell_refract(incident, arbitrary_normal, n1, n2)?;

        // Unit length check
        assert!(
            (refracted.norm() - 1.0).abs() < 1e-12,
            "Refracted ray must have unit length"
        );

        // Plane of incidence is spanned by incident and normal.
        // Plane normal = incident x normal.
        // Refracted ray must lie in plane: refracted . (incident x normal) == 0.
        let plane_normal = incident.cross(&arbitrary_normal);
        let out_of_plane = refracted.dot(&plane_normal).abs();
        assert!(
            out_of_plane < 1e-12,
            "Refracted ray must lie in plane of incidence: out of plane = {out_of_plane}"
        );

        Ok(())
    }

    #[test]
    fn test_snell_total_internal_reflection() -> TestResult {
        // Critical angle theta_c = asin(n2 / n1).
        // Glass (n1 = 1.5168) -> Air (n2 = 1.0).
        // theta_c = asin(1.0 / 1.5168) = 0.720819129... rad (approx 41.3005 degrees).
        let n1: f64 = 1.5168;
        let n2: f64 = 1.0;
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let theta_c: f64 = (n2 / n1).asin();

        let delta = 1e-6;

        // Just below critical angle: refraction must succeed
        let sub_critical_angle = theta_c - delta;
        let sub_critical_ray = dir_from_angle(sub_critical_angle);
        let sub_res = snell_refract(sub_critical_ray, normal, n1, n2);
        assert!(
            sub_res.is_ok(),
            "Ray just below critical angle should refract"
        );

        // Just above critical angle: must return TotalInternalReflection
        let super_critical_angle = theta_c + delta;
        let super_critical_ray = dir_from_angle(super_critical_angle);
        let super_res = snell_refract(super_critical_ray, normal, n1, n2);
        assert!(matches!(
            super_res,
            Err(OpticsError::TotalInternalReflection)
        ));

        Ok(())
    }

    #[test]
    fn test_reflect_properties() -> TestResult {
        let normal = Vec3::new(0.0, 1.0, 0.0);

        // Angle of incidence = angle of reflection
        for deg in [15.0_f64, 45.0, 75.0] {
            let theta = deg.to_radians();
            let incident = dir_from_angle(theta);
            let reflected = reflect(incident, normal)?;

            // Reflected should be (sin(theta), cos(theta), 0)
            let expected = Vec3::new(theta.sin(), theta.cos(), 0.0);
            assert!(
                (reflected - expected).norm() < 1e-12,
                "Reflection direction mismatch at {deg}°"
            );

            // Double reflection: reflecting again gives original incoming vector inverted
            let reflected_again = reflect(reflected, normal)?;
            assert!(
                (reflected_again - incident).norm() < 1e-12,
                "Double reflection should restore original direction"
            );
        }

        Ok(())
    }

    #[test]
    fn test_flipped_normal_robustness() -> TestResult {
        let n1 = 1.0;
        let n2 = 1.5;
        let incident = dir_from_angle(30.0_f64.to_radians());
        let canonical_normal = Vec3::new(0.0, 1.0, 0.0); // dot(i, n) < 0
        let flipped_normal = Vec3::new(0.0, -1.0, 0.0); // dot(i, n) > 0

        let r_canonical = reflect(incident, canonical_normal)?;
        let r_flipped = reflect(incident, flipped_normal)?;
        assert!(
            (r_canonical - r_flipped).norm() < 1e-12,
            "reflect should produce identical results regardless of normal orientation"
        );

        let t_canonical = snell_refract(incident, canonical_normal, n1, n2)?;
        let t_flipped = snell_refract(incident, flipped_normal, n1, n2)?;
        assert!(
            (t_canonical - t_flipped).norm() < 1e-12,
            "snell_refract should produce identical results regardless of normal orientation"
        );

        Ok(())
    }

    #[test]
    fn test_fresnel_dielectric_exact_values() -> TestResult {
        let n1 = 1.0;
        let n2 = 1.5168;

        // 1. Normal incidence (theta_i = 0, cos_i = 1.0):
        // Rs = Rp = ((n1 - n2) / (n1 + n2))^2
        // R = ((1.0 - 1.5168) / (1.0 + 1.5168))^2 = (-0.5168 / 2.5168)^2 ≈ 0.0421685...
        let (rs_norm, rp_norm) = fresnel_dielectric(1.0, n1, n2);
        let expected_r0 = ((n1 - n2) / (n1 + n2)).powi(2);
        assert!(
            (rs_norm - expected_r0).abs() < 1e-12,
            "Rs at normal incidence mismatch"
        );
        assert!(
            (rp_norm - expected_r0).abs() < 1e-12,
            "Rp at normal incidence mismatch"
        );

        // 2. Brewster's Angle: theta_B = atan(n2 / n1)
        // At theta_B, Rp must be identically 0.
        let theta_b = (n2 / n1).atan();
        let cos_b = theta_b.cos();
        let (rs_b, rp_b) = fresnel_dielectric(cos_b, n1, n2);
        assert!(rp_b < 1e-12, "Rp at Brewster's angle must be 0, got {rp_b}");
        assert!(
            rs_b > 0.05,
            "Rs at Brewster's angle should be non-zero, got {rs_b}"
        );

        // 3. Grazing incidence (theta_i -> 90 deg, cos_i -> 0):
        // Both Rs and Rp approach 1.0.
        let (rs_graze, rp_graze) = fresnel_dielectric(0.0, n1, n2);
        assert!((rs_graze - 1.0).abs() < 1e-12);
        assert!((rp_graze - 1.0).abs() < 1e-12);

        // 4. Energy Conservation: R_total + T_total == 1.0
        for cos_i in [0.1, 0.3, 0.6, 0.9, 1.0] {
            let (r_tot, t_tot) = fresnel_unpolarized(cos_i, n1, n2);
            assert!(
                (r_tot + t_tot - 1.0).abs() < 1e-12,
                "Energy conservation violated: R+T = {}",
                r_tot + t_tot
            );
        }

        // 5. Total Internal Reflection returns (1.0, 1.0)
        // Glass -> Air (n1 = 1.5168, n2 = 1.0) beyond critical angle
        let theta_c = (1.0 / 1.5168_f64).asin();
        let cos_super = (theta_c + 0.1).cos();
        let (rs_tir, rp_tir) = fresnel_dielectric(cos_super, n2, n1); // Note: n1=glass, n2=air
        assert_eq!(rs_tir, 1.0);
        assert_eq!(rp_tir, 1.0);

        // 6. Symmetry (Stokes relation): unpolarized power reflectance entering vs exiting
        // at conjugate Snell angles theta_i and theta_t must match exactly.
        let theta_i = 40.0_f64.to_radians();
        let cos_i = theta_i.cos();
        let theta_t = (theta_i.sin() * (n1 / n2)).asin();
        let cos_t = theta_t.cos();

        let (r_in, _) = fresnel_unpolarized(cos_i, n1, n2);
        let (r_out, _) = fresnel_unpolarized(cos_t, n2, n1);
        assert!(
            (r_in - r_out).abs() < 1e-12,
            "Fresnel symmetry between conjugate angles violated: in={r_in}, out={r_out}"
        );

        Ok(())
    }

    #[test]
    fn test_schlick_approximation_comparison() -> TestResult {
        let n1 = 1.0;
        let n2 = 1.5168;

        // Normal incidence: Schlick matches exact R0
        let schlick_0 = schlick_approx(1.0, n1, n2);
        let (exact_0, _) = fresnel_unpolarized(1.0, n1, n2);
        assert!(
            (schlick_0 - exact_0).abs() < 1e-12,
            "Schlick must match exact Fresnel at normal incidence"
        );

        // Grazing incidence: Schlick matches 1.0
        let schlick_graze = schlick_approx(0.0, n1, n2);
        assert!(
            (schlick_graze - 1.0).abs() < 1e-12,
            "Schlick must approach 1.0 at grazing incidence"
        );

        // Intermediate angles: Schlick approximates within ~0.03 deviation
        // (the largest deviation occurs around Brewster's angle where Rp vanishes)
        for deg in [15.0_f64, 30.0, 45.0, 60.0, 75.0] {
            let cos_i = deg.to_radians().cos();
            let schlick_val = schlick_approx(cos_i, n1, n2);
            let (exact_val, _) = fresnel_unpolarized(cos_i, n1, n2);
            let err = (schlick_val - exact_val).abs();
            assert!(
                err < 0.035,
                "Schlick deviation at {deg}° too large: err={err}, schlick={schlick_val}, exact={exact_val}"
            );
        }

        Ok(())
    }

    #[test]
    fn test_ray_and_hitrecord_constructors() -> TestResult {
        let origin = Vec3::new(0.0, 10.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 2.0); // unnormalized
        let ray = Ray::new(origin, dir, 589.3, 1.0)?;

        assert_eq!(ray.origin, origin);
        assert!((ray.direction - Vec3::new(0.0, 0.0, 1.0)).norm() < 1e-12);
        assert_eq!(ray.at(10.0), Vec3::new(0.0, 10.0, 5.0));

        // Non-finite direction should return NumericalSingularity
        let bad_ray = Ray::new(origin, Vec3::new(f64::NAN, 0.0, 1.0), 589.3, 1.0);
        assert!(matches!(bad_ray, Err(OpticsError::NumericalSingularity(_))));

        Ok(())
    }

    /// Dispersive glass model for N-BK7 to test non-sequential ray path tracing without catalog crate dependency.
    struct MockNbk7;
    impl RefractiveIndex for MockNbk7 {
        fn n(&self, wavelength_nm: f64) -> f64 {
            let lam_um = wavelength_nm / 1000.0;
            let l2 = lam_um * lam_um;
            let b = [1.03961212, 0.231792344, 1.01046945];
            let c = [0.00600069867, 0.0200179144, 103.560653];
            let n2 = 1.0
                + (b[0] * l2) / (l2 - c[0])
                + (b[1] * l2) / (l2 - c[1])
                + (b[2] * l2) / (l2 - c[2]);
            n2.sqrt()
        }
    }

    #[test]
    fn test_equilateral_prism_spectral_dispersion() -> TestResult {
        // Construct an equilateral prism in 3D:
        // Triangle cross-section in XY plane, extruded from Z = -50 to +50 mm.
        // Base vertices:
        // V0 = (-25.0, 0.0), V1 = (+25.0, 0.0), Apex V2 = (0.0, 25.0 * sqrt(3.0))
        let h = 25.0 * 3.0_f64.sqrt(); // ~43.30127 mm
        let z_min = -50.0;
        let z_max = 50.0;

        let v0_back = Vec3::new(-25.0, 0.0, z_min);
        let v0_front = Vec3::new(-25.0, 0.0, z_max);
        let v1_back = Vec3::new(25.0, 0.0, z_min);
        let v1_front = Vec3::new(25.0, 0.0, z_max);
        let apex_back = Vec3::new(0.0, h, z_min);
        let apex_front = Vec3::new(0.0, h, z_max);

        // Face 1 (Left face, between v0 and apex):
        // Normal points outwards (up-left)
        let tri1a = Triangle::new(v0_front, apex_front, apex_back, 1, 1, 0)?;
        let tri1b = Triangle::new(v0_front, apex_back, v0_back, 1, 1, 0)?;

        // Face 2 (Right face, between apex and v1):
        // Normal points outwards (up-right)
        let tri2a = Triangle::new(apex_front, v1_front, v1_back, 2, 1, 0)?;
        let tri2b = Triangle::new(apex_front, v1_back, apex_back, 2, 1, 0)?;

        // Face 3 (Base, between v1 and v0):
        let tri3a = Triangle::new(v1_front, v0_front, v0_back, 3, 1, 0)?;
        let tri3b = Triangle::new(v1_front, v0_back, v1_back, 3, 1, 0)?;

        let triangles = vec![tri1a, tri1b, tri2a, tri2b, tri3a, tri3b];
        let media: Vec<Box<dyn RefractiveIndex>> = vec![Box::new(Air), Box::new(MockNbk7)];

        let scene = Scene::new(triangles, media);

        // Fire horizontal ray toward the left face at height y = 20 mm
        let origin = Vec3::new(-50.0, 20.0, 0.0);
        let incident_dir = Vec3::new(1.0, 0.0, 0.0);

        // Fraunhofer lines: Red C (656.27 nm), Yellow d (587.56 nm), Blue F (486.13 nm)
        let wavelengths = [656.27, 587.56, 486.13];
        let mut exit_directions = Vec::new();

        for &wl in &wavelengths {
            let ray = Ray::new(origin, incident_dir, wl, 1.0)?;
            let path = trace_path(&scene, &ray, RayBranchPolicy::FollowTransmission, 10)?;

            // Ray must undergo exactly 2 transmissions: into prism, out of prism
            assert_eq!(
                path.bounces.len(),
                2,
                "Expected 2 bounces through prism for lambda={wl}"
            );
            assert_eq!(path.bounces[0].event, BounceEvent::Refract);
            assert_eq!(path.bounces[1].event, BounceEvent::Refract);

            let exit_dir = path.bounces[1]
                .refracted_dir
                .ok_or("Expected exit refraction")?;
            exit_directions.push(exit_dir);
        }

        // Calculate total deviation angle delta = acos(incident_dir . exit_dir)
        let delta_red = exit_directions[0].dot(&incident_dir).acos();
        let delta_yellow = exit_directions[1].dot(&incident_dir).acos();
        let delta_blue = exit_directions[2].dot(&incident_dir).acos();

        // Physical dispersion law: Blue light refracts/deflects more than yellow, which deflects more than red
        assert!(
            delta_blue > delta_yellow,
            "Blue deflection ({delta_blue} rad) must exceed yellow deflection ({delta_yellow} rad)"
        );
        assert!(
            delta_yellow > delta_red,
            "Yellow deflection ({delta_yellow} rad) must exceed red deflection ({delta_red} rad)"
        );

        // Angular dispersion between red C and blue F:
        let dispersion_spread = delta_blue - delta_red;
        assert!(
            dispersion_spread > 0.005,
            "Dispersion spread {dispersion_spread} rad is too small"
        );

        // Independent 2-surface analytic Snell evaluation using exact Sellmeier n
        let n1_face = Vec3::new(-3.0_f64.sqrt() / 2.0, 0.5, 0.0);
        let n2_face = Vec3::new(3.0_f64.sqrt() / 2.0, 0.5, 0.0);
        let analytic_snell_dev = |wl: f64| -> Result<f64, OpticsError> {
            let n_val = MockNbk7.n(wl);
            let t1 = snell_refract(incident_dir, n1_face, 1.0, n_val)?;
            let t2 = snell_refract(t1, -n2_face, n_val, 1.0)?;
            Ok(t2.dot(&incident_dir).clamp(-1.0, 1.0).acos())
        };

        let analytic_c = analytic_snell_dev(656.27)?;
        let analytic_d = analytic_snell_dev(587.56)?;
        let analytic_f = analytic_snell_dev(486.13)?;

        assert!(
            (delta_red - analytic_c).abs() < 1e-9,
            "C line deviation mismatch: got {delta_red}, analytic {analytic_c}"
        );
        assert!(
            (delta_yellow - analytic_d).abs() < 1e-9,
            "d line deviation mismatch: got {delta_yellow}, analytic {analytic_d}"
        );
        assert!(
            (delta_blue - analytic_f).abs() < 1e-9,
            "F line deviation mismatch: got {delta_blue}, analytic {analytic_f}"
        );

        Ok(())
    }

    #[test]
    fn test_spectral_fan_monotonic_deviation_and_analytic_snell() -> TestResult {
        // Build 60-degree equilateral prism
        let h = 25.0 * 3.0_f64.sqrt();
        let z_min = -50.0;
        let z_max = 50.0;
        let v0_back = Vec3::new(-25.0, 0.0, z_min);
        let v0_front = Vec3::new(-25.0, 0.0, z_max);
        let v1_back = Vec3::new(25.0, 0.0, z_min);
        let v1_front = Vec3::new(25.0, 0.0, z_max);
        let apex_back = Vec3::new(0.0, h, z_min);
        let apex_front = Vec3::new(0.0, h, z_max);

        let tri1a = Triangle::new(v0_front, apex_front, apex_back, 1, 1, 0)?;
        let tri1b = Triangle::new(v0_front, apex_back, v0_back, 1, 1, 0)?;
        let tri2a = Triangle::new(apex_front, v1_front, v1_back, 2, 1, 0)?;
        let tri2b = Triangle::new(apex_front, v1_back, apex_back, 2, 1, 0)?;
        let tri3a = Triangle::new(v1_front, v0_front, v0_back, 3, 1, 0)?;
        let tri3b = Triangle::new(v1_front, v0_back, v1_back, 3, 1, 0)?;

        let triangles = vec![tri1a, tri1b, tri2a, tri2b, tri3a, tri3b];
        let media: Vec<Box<dyn RefractiveIndex>> = vec![Box::new(Air), Box::new(MockNbk7)];
        let scene = Scene::new(triangles, media);

        // Incidence angle theta_i = 45 degrees relative to left face normal (-sqrt(3)/2, 1/2, 0).
        // Left face normal is at 150 deg; ray direction at 15 deg gives exactly 45 deg incidence:
        // dot([cos 15, sin 15], [-cos 30, sin 30]) = -cos(45).
        let rad15 = (15.0_f64).to_radians();
        let incident_dir = Vec3::new(rad15.cos(), rad15.sin(), 0.0);
        let origin = Vec3::new(-60.0, 8.0, 0.0);
        let ray_template = Ray::new(origin, incident_dir, 550.0, 1.0)?;

        // Sample spectrum across 380 nm to 780 nm
        let mut sample_wls = Vec::new();
        let mut wl = 380.0;
        while wl <= 780.0 {
            sample_wls.push(wl);
            wl += 20.0;
        }

        let fan = spectral_fan(
            &scene,
            &ray_template,
            &sample_wls,
            RayBranchPolicy::FollowTransmission,
            10,
        );

        assert_eq!(fan.len(), sample_wls.len());

        let mut prev_deviation: Option<f64> = None;
        for (w, path) in &fan {
            assert_eq!(path.bounces.len(), 2, "Expected 2 bounces at {w} nm");
            let exit_dir = path.bounces[1]
                .refracted_dir
                .ok_or("Expected exit refraction")?;
            let deviation = exit_dir.dot(&incident_dir).clamp(-1.0, 1.0).acos();

            if let Some(prev_dev) = prev_deviation {
                // In normal dispersion, refractive index decreases with wavelength -> deviation decreases monotonically
                assert!(
                    deviation < prev_dev,
                    "Deviation should decrease monotonically with wavelength: at {w} nm dev={deviation} >= prev={prev_dev}"
                );
            }
            prev_deviation = Some(deviation);
        }

        // Check Fraunhofer F (486.13 nm), d (587.56 nm), C (656.27 nm)
        let fraunhofer_wls = [486.13, 587.56, 656.27];
        let fraunhofer_fan = spectral_fan(
            &scene,
            &ray_template,
            &fraunhofer_wls,
            RayBranchPolicy::FollowTransmission,
            10,
        );
        assert_eq!(fraunhofer_fan.len(), 3);

        let n1_face = Vec3::new(-3.0_f64.sqrt() / 2.0, 0.5, 0.0);
        let n2_face = Vec3::new(3.0_f64.sqrt() / 2.0, 0.5, 0.0);

        for (w, path) in &fraunhofer_fan {
            let n_glass = MockNbk7.n(*w);
            let t1 = snell_refract(incident_dir, n1_face, 1.0, n_glass)?;
            let t2 = snell_refract(t1, -n2_face, n_glass, 1.0)?;
            let analytic_dev = t2.dot(&incident_dir).clamp(-1.0, 1.0).acos();

            let exit_dir = path.bounces[1]
                .refracted_dir
                .ok_or("Expected exit refraction")?;
            let traced_dev = exit_dir.dot(&incident_dir).clamp(-1.0, 1.0).acos();

            assert!(
                (traced_dev - analytic_dev).abs() < 1e-9,
                "Deviation mismatch for lambda={w}: traced={traced_dev}, analytic={analytic_dev}"
            );
        }

        Ok(())
    }
}
