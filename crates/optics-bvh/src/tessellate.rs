//! Tessellation conversion routines for sequential prescriptions to BVH triangle primitives.

use optics_core::OpticalPrescription;

use crate::triangle::Triangle;

/// Tessellates a sequential optical prescription directly into a collection of BVH triangles
/// with assigned material IDs.
#[must_use]
pub fn prescription_to_trimesh(
    prescription: &OpticalPrescription,
    slices: u32,
    rings: u32,
) -> Vec<Triangle> {
    let (core_tris, _) =
        optics_core::prescription_to_trimesh_with_materials(prescription, slices, rings);
    core_tris.into_iter().map(Triangle::from).collect()
}

/// Tessellates a sequential optical prescription into BVH triangles, also returning the
/// list of unique glass material names indexed by `material_id`.
#[must_use]
pub fn prescription_to_trimesh_with_materials(
    prescription: &OpticalPrescription,
    slices: u32,
    rings: u32,
) -> (Vec<Triangle>, Vec<String>) {
    let (core_tris, materials) =
        optics_core::prescription_to_trimesh_with_materials(prescription, slices, rings);
    let bvh_tris = core_tris.into_iter().map(Triangle::from).collect();
    (bvh_tris, materials)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BlasInstance, BvhScene};
    use optics_core::OpticalSurface;
    use optics_core::{Ray, Vec3};
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_bvh_prescription_tessellation_and_traversal() -> TestResult {
        let prescription = OpticalPrescription {
            name: "Plano-Convex Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 10.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 90.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                },
            ],
        };

        let (triangles, materials) = prescription_to_trimesh_with_materials(&prescription, 16, 8);
        assert_eq!(materials, vec!["N-BK7"]);
        assert!(!triangles.is_empty());
        assert_eq!(triangles[0].material_id, 0); // N-BK7

        let aabb = crate::Aabb::from_points(
            &triangles
                .iter()
                .flat_map(|t| [t.v0, t.v1, t.v2])
                .collect::<Vec<_>>(),
        );
        let instance = BlasInstance::new(0, &aabb, nalgebra::Matrix4::identity());
        let scene = BvhScene::build(vec![triangles.clone()], vec![instance]);

        // Fire a ray through the lens aperture at x=1.0, y=1.0 along +z
        let ray = Ray::new(
            Vec3::new(1.0, 1.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let direct_hit = triangles.iter().find_map(|t| t.intersect(&ray, 0));
        assert!(
            direct_hit.is_some(),
            "Direct triangle hit should succeed, total triangles = {}",
            triangles.len()
        );

        let hit = scene.intersect(&ray);
        assert!(hit.is_some(), "Ray should hit the tessellated lens");
        let hit_rec = hit.ok_or("expected hit record")?;
        // First hit should match the analytical spherical sag at r = sqrt(2): z ≈ 0.02 mm
        assert!((hit_rec.point.z - 0.02).abs() < 5e-3);

        Ok(())
    }
}
