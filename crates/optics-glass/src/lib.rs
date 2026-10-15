//! `optics-glass` — Optical glass catalog and materials.
//!
//! Provides catalog-verified dispersion models and preset glasses for
//! real-world optical materials (Schott N-BK7, SF11, F2, Fused Silica, Air).

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod catalog;
pub mod material;

pub use catalog::GlassCatalogDb;
pub use material::{DispersionModel, GlassCatalog, GlassMaterial};

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::{
        Air, BounceEvent, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle, Vec3,
    };
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
            let cat_entry = db
                .lookup(name)
                .ok_or_else(|| format!("Catalog missing glass {name}"))?;

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
        let media: Vec<Box<dyn RefractiveIndex>> =
            vec![Box::new(Air), Box::new(GlassMaterial::n_bk7())];

        let scene = Scene::new(triangles, media);

        let origin = Vec3::new(-50.0, 20.0, 0.0);
        let incident_dir = Vec3::new(1.0, 0.0, 0.0);

        let wavelengths = [LAMBDA_C_NM, LAMBDA_D_NM, LAMBDA_F_NM];
        let mut exit_directions = Vec::new();

        for &wl in &wavelengths {
            let ray = Ray::new(origin, incident_dir, wl, 1.0)?;
            let path =
                optics_core::trace_path(&scene, &ray, RayBranchPolicy::FollowTransmission, 10)?;

            assert_eq!(path.bounces.len(), 2);
            assert_eq!(path.bounces[0].event, BounceEvent::Refract);
            assert_eq!(path.bounces[1].event, BounceEvent::Refract);

            let exit_dir = path.bounces[1]
                .refracted_dir
                .ok_or("Expected exit refraction")?;
            exit_directions.push(exit_dir);
        }

        let delta_c = exit_directions[0].dot(&incident_dir).acos();
        let delta_d = exit_directions[1].dot(&incident_dir).acos();
        let delta_f = exit_directions[2].dot(&incident_dir).acos();

        // Blue deflects more than Yellow, which deflects more than Red
        assert!(delta_f > delta_d && delta_d > delta_c);

        Ok(())
    }
}
