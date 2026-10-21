//! 3D Rotational surface tessellation for spherical and conic optical systems.
//!
//! Converts sequential [`OpticalPrescription`] models into watertight 3D triangle meshes
//! with physical outward normals and material assignments for non-sequential ray tracing
//! and GPU path tracing.

use std::collections::HashMap;

use crate::geometry::Triangle;
use crate::prescription::OpticalPrescription;
use crate::types::Vec3;

/// Computes surface sag $z(r)$ for a conic or spherical surface.
#[inline]
fn evaluate_surface_sag(r: f64, radius_mm: f64, conic_constant: f64) -> f64 {
    if !radius_mm.is_finite() || radius_mm.abs() < 1e-12 {
        return 0.0;
    }
    let c = 1.0 / radius_mm;
    let radicand = 1.0 - (1.0 + conic_constant) * c * c * r * r;
    let s = if radicand <= 0.0 { 0.0 } else { radicand.sqrt() };
    (c * r * r) / (1.0 + s)
}

/// Generates a rotational 3D triangle mesh for a single spherical or conic surface.
///
/// # Arguments
/// * `radius_mm` - Vertex radius of curvature in millimeters ($R = 1/c$). Infinity for planar.
/// * `conic_constant` - Conic parameter $k$ ($0$ = sphere, $-1$ = paraboloid).
/// * `semi_diameter_mm` - Aperture clear semi-diameter in millimeters.
/// * `vertex_z` - Axial coordinate of the surface vertex along $+z$.
/// * `slices` - Azimuthal angular subdivisions (e.g. 32 or 64).
/// * `rings` - Radial concentric subdivisions from center to aperture edge (e.g. 16).
/// * `normal_faces_positive_z` - If `true`, outward normal points generally toward $+z$ (back surface);
///   if `false`, normal points toward $-z$ (front surface).
/// * `surface_index` - Unique surface index tag.
/// * `medium_inside` - Optical medium index enclosed behind the surface normal.
/// * `medium_outside` - Optical medium index in front of the surface normal.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn tessellate_conic_surface(
    radius_mm: f64,
    conic_constant: f64,
    semi_diameter_mm: f64,
    vertex_z: f64,
    slices: u32,
    rings: u32,
    normal_faces_positive_z: bool,
    surface_index: usize,
    medium_inside: usize,
    medium_outside: usize,
) -> Vec<Triangle> {
    let slices = slices.max(3);
    let rings = rings.max(1);
    let semi_diameter = semi_diameter_mm.max(1e-4);

    let mut vertices: Vec<Vec<Vec3>> = Vec::with_capacity(rings as usize + 1);

    // Ring 0: Center vertex at (0, 0, vertex_z)
    vertices.push(vec![Vec3::new(0.0, 0.0, vertex_z)]);

    // Rings 1..=rings
    for ring in 1..=rings {
        let r = semi_diameter * (ring as f64 / rings as f64);
        let sag = evaluate_surface_sag(r, radius_mm, conic_constant);
        let z = vertex_z + sag;

        let mut ring_verts = Vec::with_capacity(slices as usize);
        for slice in 0..slices {
            let theta = 2.0 * std::f64::consts::PI * (slice as f64 / slices as f64);
            let x = r * theta.cos();
            let y = r * theta.sin();
            ring_verts.push(Vec3::new(x, y, z));
        }
        vertices.push(ring_verts);
    }

    let mut triangles = Vec::new();

    // 1. Center triangle fan between ring 0 and ring 1
    let center = vertices[0][0];
    for slice in 0..slices {
        let next_slice = (slice + 1) % slices;
        let v_curr = vertices[1][slice as usize];
        let v_next = vertices[1][next_slice as usize];

        let tri_res = if normal_faces_positive_z {
            Triangle::new(center, v_curr, v_next, surface_index, medium_inside, medium_outside)
        } else {
            Triangle::new(center, v_next, v_curr, surface_index, medium_inside, medium_outside)
        };

        if let Ok(tri) = tri_res {
            triangles.push(tri);
        }
    }

    // 2. Concentric ring quads between ring j and ring j + 1
    for ring in 1..rings {
        let r_curr = ring as usize;
        let r_next = (ring + 1) as usize;

        for slice in 0..slices {
            let next_slice = (slice + 1) % slices;
            let s_curr = slice as usize;
            let s_next = next_slice as usize;

            let v00 = vertices[r_curr][s_curr];
            let v01 = vertices[r_curr][s_next];
            let v10 = vertices[r_next][s_curr];
            let v11 = vertices[r_next][s_next];

            if normal_faces_positive_z {
                if let Ok(t1) = Triangle::new(v00, v10, v11, surface_index, medium_inside, medium_outside) {
                    triangles.push(t1);
                }
                if let Ok(t2) = Triangle::new(v00, v11, v01, surface_index, medium_inside, medium_outside) {
                    triangles.push(t2);
                }
            } else {
                if let Ok(t1) = Triangle::new(v00, v11, v10, surface_index, medium_inside, medium_outside) {
                    triangles.push(t1);
                }
                if let Ok(t2) = Triangle::new(v00, v01, v11, surface_index, medium_inside, medium_outside) {
                    triangles.push(t2);
                }
            }
        }
    }

    triangles
}

/// Generates a cylindrical/conical edge rim connecting two perimeter rings.
#[must_use]
pub fn tessellate_lens_rim(
    front_perimeter: &[Vec3],
    back_perimeter: &[Vec3],
    surface_index: usize,
    medium_inside: usize,
    medium_outside: usize,
) -> Vec<Triangle> {
    let count = front_perimeter.len().min(back_perimeter.len());
    let mut triangles = Vec::with_capacity(count * 2);

    for i in 0..count {
        let next = (i + 1) % count;
        let f0 = front_perimeter[i];
        let f1 = front_perimeter[next];
        let b0 = back_perimeter[i];
        let b1 = back_perimeter[next];

        // Outward radial normal: f0 -> f1 -> b1 and f0 -> b1 -> b0
        if let Ok(t1) = Triangle::new(f0, f1, b1, surface_index, medium_inside, medium_outside) {
            triangles.push(t1);
        }
        if let Ok(t2) = Triangle::new(f0, b1, b0, surface_index, medium_inside, medium_outside) {
            triangles.push(t2);
        }
    }

    triangles
}

/// Tessellates a complete sequential [`OpticalPrescription`] into a collection of 3D triangles
/// suitable for non-sequential ray tracing.
///
/// Front and back surfaces are capped by concentric quads, and lens elements are bounded by
/// cylindrical rims with outward-facing normals.
///
/// # Arguments
/// * `prescription` - Optical prescription to tessellate.
/// * `slices` - Azimuthal angular divisions (e.g. 32).
/// * `rings` - Radial concentric rings per surface (e.g. 16).
#[must_use]
pub fn prescription_to_trimesh(prescription: &OpticalPrescription, slices: u32, rings: u32) -> Vec<Triangle> {
    let (triangles, _) = prescription_to_trimesh_with_materials(prescription, slices, rings);
    triangles
}

/// Tessellates an [`OpticalPrescription`] returning the 3D triangles and list of resolved material names.
///
/// Each unique non-air glass material is assigned a unique `medium_inside` index matching its
/// position in the returned `Vec<String>`.
#[must_use]
pub fn prescription_to_trimesh_with_materials(
    prescription: &OpticalPrescription,
    slices: u32,
    rings: u32,
) -> (Vec<Triangle>, Vec<String>) {
    let slices = slices.max(3);
    let rings = rings.max(1);

    if prescription.surfaces.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Assign material IDs to unique glass names
    let mut material_map = HashMap::new();
    let mut material_names = Vec::new();

    for surf in &prescription.surfaces {
        let mat = surf.material_name.trim();
        if !mat.is_empty() && !mat.eq_ignore_ascii_case("air") && !material_map.contains_key(mat) {
            let id = material_names.len();
            material_map.insert(mat.to_string(), id);
            material_names.push(mat.to_string());
        }
    }

    // Determine cumulative axial vertex positions
    let mut vertex_z = Vec::with_capacity(prescription.surfaces.len());
    let mut current_z = 0.0_f64;

    let start_idx =
        if prescription.surfaces[0].thickness_mm.is_infinite() || prescription.surfaces[0].thickness_mm > 1e5 {
            vertex_z.push(0.0);
            1
        } else {
            0
        };

    for surf in prescription.surfaces.iter().skip(start_idx) {
        vertex_z.push(current_z);
        if surf.thickness_mm.is_finite() {
            current_z += surf.thickness_mm;
        }
    }

    let mut all_triangles = Vec::new();
    let num_surfaces = prescription.surfaces.len();

    let mut i = 0;
    while i < num_surfaces {
        let surf = &prescription.surfaces[i];
        let mat_trim = surf.material_name.trim();
        let is_glass = !mat_trim.is_empty() && !mat_trim.eq_ignore_ascii_case("air");

        if is_glass && i + 1 < num_surfaces {
            // Lens element between surface i (front) and surface i + 1 (back)
            let back_surf = &prescription.surfaces[i + 1];
            let mat_id = *material_map.get(mat_trim).unwrap_or(&0);

            let z_front = vertex_z.get(i).copied().unwrap_or(0.0);
            let z_back = vertex_z.get(i + 1).copied().unwrap_or(z_front);

            // 1. Front surface (outward normal pointing toward -z)
            let front_triangles = tessellate_conic_surface(
                surf.radius_mm,
                surf.conic_constant,
                surf.semi_diameter_mm,
                z_front,
                slices,
                rings,
                false, // faces -z
                i,
                mat_id,
                0, // air outside
            );
            all_triangles.extend(front_triangles);

            // 2. Back surface (outward normal pointing toward +z)
            let back_triangles = tessellate_conic_surface(
                back_surf.radius_mm,
                back_surf.conic_constant,
                back_surf.semi_diameter_mm,
                z_back,
                slices,
                rings,
                true, // faces +z
                i + 1,
                mat_id,
                0, // air outside
            );
            all_triangles.extend(back_triangles);

            // 3. Cylindrical/conical rim connecting the outer edges
            let mut front_perimeter = Vec::with_capacity(slices as usize);
            let mut back_perimeter = Vec::with_capacity(slices as usize);

            let r_front = surf.semi_diameter_mm.max(1e-4);
            let r_back = back_surf.semi_diameter_mm.max(1e-4);

            let sag_f = evaluate_surface_sag(r_front, surf.radius_mm, surf.conic_constant);
            let sag_b = evaluate_surface_sag(r_back, back_surf.radius_mm, back_surf.conic_constant);

            let z_edge_f = z_front + sag_f;
            let z_edge_b = z_back + sag_b;

            for slice in 0..slices {
                let theta = 2.0 * std::f64::consts::PI * (slice as f64 / slices as f64);
                let cos_t = theta.cos();
                let sin_t = theta.sin();
                front_perimeter.push(Vec3::new(r_front * cos_t, r_front * sin_t, z_edge_f));
                back_perimeter.push(Vec3::new(r_back * cos_t, r_back * sin_t, z_edge_b));
            }

            let rim_triangles = tessellate_lens_rim(&front_perimeter, &back_perimeter, i, mat_id, 0);
            all_triangles.extend(rim_triangles);

            // Advance past the back surface of this element
            i += 2;
        } else {
            // Standalone surface: tessellate physical reflective mirrors
            let is_mirror = surf.material_name.eq_ignore_ascii_case("mirror");
            let is_image_plane = i == num_surfaces.saturating_sub(1);
            if is_mirror && !is_image_plane && surf.semi_diameter_mm > 0.0 {
                let z_surf = vertex_z.get(i).copied().unwrap_or(0.0);
                let surf_triangles = tessellate_conic_surface(
                    surf.radius_mm,
                    surf.conic_constant,
                    surf.semi_diameter_mm,
                    z_surf,
                    slices,
                    rings,
                    true,
                    i,
                    100, // Non-glass mirror index
                    0,
                );
                all_triangles.extend(surf_triangles);
            }
            i += 1;
        }
    }

    (all_triangles, material_names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prescription::OpticalSurface;

    #[test]
    fn test_tessellate_planar_surface() {
        let triangles = tessellate_conic_surface(
            f64::INFINITY, // Flat plane
            0.0,
            10.0,
            0.0,
            16,
            4,
            true,
            1,
            1,
            0,
        );

        assert!(!triangles.is_empty());
        // Check all vertices lie on plane z = 0
        for tri in &triangles {
            assert!((tri.v0.z).abs() < 1e-10);
            assert!((tri.v1.z).abs() < 1e-10);
            assert!((tri.v2.z).abs() < 1e-10);
            // Normal must point along +z
            assert!((tri.normal.z - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_tessellate_spherical_singlet() {
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
                    profile: None,
                    aperture: None,
                    coating: None,
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
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        };

        let (triangles, materials) = prescription_to_trimesh_with_materials(&prescription, 16, 8);

        assert_eq!(materials, vec!["N-BK7"]);
        assert!(!triangles.is_empty());

        // Check that bounding box is reasonable:
        // x and y within [-10, 10], z between front vertex (~0) and back vertex (~5)
        for tri in &triangles {
            for v in [tri.v0, tri.v1, tri.v2] {
                assert!(v.x.abs() <= 10.001);
                assert!(v.y.abs() <= 10.001);
                assert!(v.z >= -0.001 && v.z <= 5.001);
            }
            assert_eq!(tri.medium_inside, 0); // Material index 0 = N-BK7
        }
    }
}
