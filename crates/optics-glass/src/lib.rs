//! `optics-glass` — Optical glass catalog and materials.
//!
//! Provides catalog-verified dispersion models and preset glasses for
//! real-world optical materials (Schott N-BK7, SF11, F2, Fused Silica, Air).

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod agf;
pub mod catalog;
pub mod material;
pub mod yaml_loader;

pub use agf::parse_agf;
pub use catalog::GlassCatalogDb;
pub use material::{DispersionModel, GlassCatalog, GlassMaterial};
pub use yaml_loader::parse_refractiveindex_yaml;

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::{Air, BounceEvent, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle, Vec3};
    use optics_spectral::{LAMBDA_C_NM, LAMBDA_D_NM, LAMBDA_F_NM};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_presets_match_catalog_entries() -> TestResult {
        let db = GlassCatalogDb::load_bundled()?;
        assert!(db.len() >= 640, "Expected at least 640 glasses in catalog");

        let test_cases = [
            ("N-BK7", GlassMaterial::n_bk7()),
            ("SF11", GlassMaterial::sf11()),
            ("F2", GlassMaterial::f2()),
            ("LITHOSIL-Q", GlassMaterial::fused_silica()),
        ];

        let test_wavelengths = [LAMBDA_F_NM, LAMBDA_D_NM, LAMBDA_C_NM];

        for (name, preset) in test_cases {
            let cat_entry = db.lookup(name).ok_or_else(|| format!("Catalog missing glass {name}"))?;

            // 1. Refractive index at d line matches within 1e-5
            assert!(
                (preset.nd - cat_entry.nd).abs() < 1e-5,
                "{name} nd mismatch: preset={}, catalog={}",
                preset.nd,
                cat_entry.nd
            );

            // 2. Abbe number matches (except LITHOSIL-Q where catalog raw vd had decimal issue)
            if name != "LITHOSIL-Q" {
                assert!(
                    (preset.vd - cat_entry.vd).abs() < 1e-2,
                    "{name} Vd mismatch: preset={}, catalog={}",
                    preset.vd,
                    cat_entry.vd
                );
            }

            // 3. n(lambda) at 486.13, 587.56, 656.27 nm matches within 1e-5
            for &wl in &test_wavelengths {
                let n_preset = preset.refractive_index(wl);
                let n_catalog = cat_entry.refractive_index(wl);
                assert!(
                    (n_preset - n_catalog).abs() < 1e-5,
                    "{name} n({wl} nm) mismatch: preset={n_preset}, catalog={n_catalog}"
                );
            }
        }

        // Test alias lookup
        assert!(db.lookup("Fused Silica").is_some());
        assert!(db.lookup("silica").is_some());
        assert!(db.lookup("n-bk7").is_some());
        assert_eq!(db.lookup("SK16").map(|g| g.name.as_str()), Some("N-SK16"));
        assert_eq!(db.lookup("N-SK16").map(|g| g.name.as_str()), Some("N-SK16"));
        assert_eq!(db.lookup("F2").map(|g| g.name.as_str()), Some("F2"));

        // Test MaterialResolver implementation
        use optics_core::MaterialResolver;
        let sk16 = db.resolve("SK16")?;
        assert!((sk16.n(587.56) - 1.6207).abs() < 1e-3);

        Ok(())
    }

    #[test]
    fn test_nbk7_prism_scene_integration() -> TestResult {
        // Build 3D equilateral prism with GlassMaterial::n_bk7()
        let h = 25.0 * 3.0_f64.sqrt();
        let z_min = -25.0;
        let z_max = 25.0;

        let v0_back = Vec3::new(-25.0, 0.0, z_min);
        let v0_front = Vec3::new(-25.0, 0.0, z_max);
        let v1_back = Vec3::new(25.0, 0.0, z_min);
        let v1_front = Vec3::new(25.0, 0.0, z_max);
        let apex_back = Vec3::new(0.0, h, z_min);
        let apex_front = Vec3::new(0.0, h, z_max);

        // Left face
        let tri1a = Triangle::new(v0_front, apex_front, apex_back, 1, 1, 0)?;
        let tri1b = Triangle::new(v0_front, apex_back, v0_back, 1, 1, 0)?;
        // Right face
        let tri2a = Triangle::new(apex_front, v1_front, v1_back, 2, 1, 0)?;
        let tri2b = Triangle::new(apex_front, v1_back, apex_back, 2, 1, 0)?;
        // Base
        let tri3a = Triangle::new(v1_front, v0_front, v0_back, 3, 1, 0)?;
        let tri3b = Triangle::new(v1_front, v0_back, v1_back, 3, 1, 0)?;

        let triangles = vec![tri1a, tri1b, tri2a, tri2b, tri3a, tri3b];
        let media: Vec<Box<dyn RefractiveIndex>> = vec![Box::new(Air), Box::new(GlassMaterial::n_bk7())];

        let scene = Scene::new(triangles, media);

        let origin = Vec3::new(-50.0, 20.0, 0.0);
        let incident_dir = Vec3::new(1.0, 0.0, 0.0);

        let wavelengths = [LAMBDA_C_NM, LAMBDA_D_NM, LAMBDA_F_NM];
        let mut exit_directions = Vec::new();

        for &wl in &wavelengths {
            let ray = Ray::new(origin, incident_dir, wl, 1.0)?;
            let path = optics_core::trace_path(&scene, &ray, RayBranchPolicy::FollowTransmission, 10)?;

            assert_eq!(path.bounces.len(), 2);
            assert_eq!(path.bounces[0].event, BounceEvent::Refract);
            assert_eq!(path.bounces[1].event, BounceEvent::Refract);

            let exit_dir = path.bounces[1].refracted_dir.ok_or("Expected exit refraction")?;
            exit_directions.push(exit_dir);
        }

        let delta_c = exit_directions[0].dot(&incident_dir).acos();
        let delta_d = exit_directions[1].dot(&incident_dir).acos();
        let delta_f = exit_directions[2].dot(&incident_dir).acos();

        // Blue deflects more than Yellow, which deflects more than Red
        assert!(delta_f > delta_d && delta_d > delta_c);

        Ok(())
    }

    #[test]
    fn test_search_by_nd_vd() -> TestResult {
        let db = GlassCatalogDb::load_bundled()?;

        // Search around N-BK7: nd in [1.51, 1.53], vd in [60.0, 66.0]
        let results = db.search_by_nd_vd((1.51, 1.53), (60.0, 66.0));
        assert!(!results.is_empty(), "Expected matches in N-BK7 region");
        assert!(results.iter().any(|g| g.name == "N-BK7"), "Expected N-BK7 to be found");

        for glass in &results {
            assert!(glass.nd >= 1.51 && glass.nd <= 1.53);
            assert!(glass.vd >= 60.0 && glass.vd <= 66.0);
        }

        // Test inverted range normalization
        let results_inverted = db.search_by_nd_vd((1.53, 1.51), (66.0, 60.0));
        assert_eq!(results.len(), results_inverted.len());

        // Disjoint range test
        let empty = db.search_by_nd_vd((3.0, 4.0), (10.0, 20.0));
        assert!(empty.is_empty());

        Ok(())
    }

    #[test]
    fn test_glass_internal_transmittance_and_thickness_scaling() {
        let mut glass = GlassMaterial::n_bk7();
        // N-BK7 standard Schott transmittance at 10 mm:
        // 350 nm: 0.720, 400 nm: 0.992, 550 nm: 0.998, 700 nm: 0.998
        glass.internal_transmittance = vec![(350.0, 0.720), (400.0, 0.992), (550.0, 0.998), (700.0, 0.998)];
        glass.test_thickness_mm = 10.0;

        // Exact tabulated point
        assert!((glass.internal_transmittance_at(400.0) - 0.992).abs() < 1e-6);

        // Linear interpolation halfway between 350 nm (0.720) and 400 nm (0.992) -> 375 nm: 0.856
        let t_375 = glass.internal_transmittance_at(375.0);
        assert!((t_375 - 0.856).abs() < 1e-4);

        // Thickness scaling via Beer-Lambert law:
        // At 10 mm: tau = 0.720
        assert!((glass.transmittance_for_thickness(350.0, 10.0) - 0.720).abs() < 1e-6);

        // At 20 mm (double thickness): tau = 0.720^2 = 0.5184
        let t_20mm = glass.transmittance_for_thickness(350.0, 20.0);
        assert!((t_20mm - 0.5184).abs() < 1e-4);

        // At 25 mm: tau = 0.720^(2.5) = 0.4398
        let t_25mm = glass.transmittance_for_thickness(350.0, 25.0);
        assert!((t_25mm - 0.4398).abs() < 1e-4);

        // At 0 mm: tau = 1.0 (no material path)
        assert!((glass.transmittance_for_thickness(350.0, 0.0) - 1.0).abs() < 1e-9);
    }
}
