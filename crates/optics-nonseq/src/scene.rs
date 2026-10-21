//! Non-sequential 3D optical scene graph, objects, and hierarchy.

use crate::detector::Detector2D;
use crate::error::NonSeqError;
use crate::geometry::{NonSeqHit, PrimitiveGeometry, Transform3D};
use crate::material::{NonSeqMaterial, OpticalMedium};
use crate::source::{RaySource, SourceSpectrum};
use optics_core::prescription::OpticalPrescription;
use optics_core::tessellate::{tessellate_conic_surface, tessellate_lens_rim};
use optics_core::{Ray, Vec3};
use serde::{Deserialize, Serialize};

/// A physical object in a non-sequential scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqObject {
    /// Unique object identifier.
    pub id: usize,
    /// Human-readable object name.
    pub name: String,
    /// Geometric surface shape.
    pub geometry: PrimitiveGeometry,
    /// 3D world transform.
    pub transform: Transform3D,
    /// Optical bulk and surface finish material.
    pub material: NonSeqMaterial,
    /// Active toggle (if false, ray ignores this object).
    pub is_active: bool,
}

impl NonSeqObject {
    /// Intersect a world-space ray against this object.
    pub fn intersect(&self, world_ray: &Ray) -> Option<NonSeqHit> {
        if !self.is_active {
            return None;
        }

        let local_ray = self.transform.inverse_transform_ray(world_ray).ok()?;
        let (t_local, local_normal, uv) = self.geometry.intersect_local(&local_ray)?;

        let local_hit_pt = local_ray.at(t_local);
        let world_hit_pt = self.transform.transform_point(&local_hit_pt);

        // Distance along world ray direction
        let diff = world_hit_pt - world_ray.origin;
        let t_world = diff.dot(&world_ray.direction);
        if t_world <= 1e-8 {
            return None;
        }

        let world_geometric_normal = self.transform.transform_normal(&local_normal).ok()?;
        let cos_theta = world_ray.direction.dot(&world_geometric_normal);
        let entering = cos_theta < 0.0;

        let surface_normal = if entering {
            world_geometric_normal
        } else {
            -world_geometric_normal
        };

        Some(NonSeqHit {
            t: t_world,
            point: world_hit_pt,
            normal: surface_normal,
            geometric_normal: world_geometric_normal,
            object_id: self.id,
            uv,
            entering,
        })
    }
}

/// A complete non-sequential optical scene containing objects, sources, and detectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqScene {
    /// Physical optical objects in the scene.
    pub objects: Vec<NonSeqObject>,
    /// Radiometric detectors collecting energy.
    pub detectors: Vec<Detector2D>,
    /// Ray emission sources.
    pub sources: Vec<RaySource>,
    /// Background ambient medium (default Air).
    pub ambient_medium: OpticalMedium,
}

impl Default for NonSeqScene {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            detectors: Vec::new(),
            sources: Vec::new(),
            ambient_medium: OpticalMedium::Air,
        }
    }
}

impl NonSeqScene {
    /// Create a new empty non-sequential scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an object to the scene.
    pub fn add_object(&mut self, mut object: NonSeqObject) -> usize {
        let id = self.objects.len();
        object.id = id;
        self.objects.push(object);
        id
    }

    /// Add a detector to the scene.
    pub fn add_detector(&mut self, detector: Detector2D) -> usize {
        let id = self.detectors.len();
        self.detectors.push(detector);
        id
    }

    /// Add a ray source to the scene.
    pub fn add_source(&mut self, source: RaySource) -> usize {
        let id = self.sources.len();
        self.sources.push(source);
        id
    }

    /// Reset all detectors in the scene.
    pub fn clear_detectors(&mut self) {
        for det in &mut self.detectors {
            det.clear();
        }
    }

    /// Find the closest object intersection along a ray.
    pub fn intersect_closest_object(&self, ray: &Ray) -> Option<NonSeqHit> {
        let mut closest_hit: Option<NonSeqHit> = None;
        let mut min_t = f64::INFINITY;

        for obj in &self.objects {
            if let Some(hit) = obj.intersect(ray) {
                if hit.t < min_t && hit.t > 1e-8 {
                    min_t = hit.t;
                    closest_hit = Some(hit);
                }
            }
        }

        closest_hit
    }

    /// Convert a sequential prescription into a non-sequential lens element mesh and add to the scene.
    #[allow(clippy::too_many_arguments)]
    pub fn add_prescription_lens(
        &mut self,
        name: &str,
        prescription: &OpticalPrescription,
        surf_front_idx: usize,
        surf_back_idx: usize,
        material: NonSeqMaterial,
        slices: u32,
        rings: u32,
    ) -> Result<usize, NonSeqError> {
        let surf_front = prescription
            .surfaces
            .get(surf_front_idx)
            .ok_or_else(|| NonSeqError::InvalidGeometry(format!("Surface index {surf_front_idx} out of range")))?;
        let surf_back = prescription
            .surfaces
            .get(surf_back_idx)
            .ok_or_else(|| NonSeqError::InvalidGeometry(format!("Surface index {surf_back_idx} out of range")))?;

        let mut z_front = 0.0;
        for s in &prescription.surfaces[..surf_front_idx] {
            z_front += s.thickness_mm;
        }
        let z_back = z_front + surf_front.thickness_mm;

        let semi_diam = surf_front.semi_diameter_mm.max(surf_back.semi_diameter_mm);

        let mut triangles = Vec::new();

        // 1. Front optical surface: outward normal faces -z (into incident air)
        let front_tris = tessellate_conic_surface(
            surf_front.radius_mm,
            surf_front.conic_constant,
            semi_diam,
            z_front,
            slices,
            rings,
            false,
            surf_front_idx,
            0,
            0,
        );
        triangles.extend(front_tris);

        // 2. Rear optical surface: outward normal faces +z (into exit air)
        let back_tris = tessellate_conic_surface(
            surf_back.radius_mm,
            surf_back.conic_constant,
            semi_diam,
            z_back,
            slices,
            rings,
            true,
            surf_back_idx,
            0,
            0,
        );
        triangles.extend(back_tris);

        // 3. Watertight cylindrical rim
        let mut front_perimeter = Vec::with_capacity(slices as usize);
        let mut back_perimeter = Vec::with_capacity(slices as usize);

        let c_front = if surf_front.radius_mm.abs() > 1e-12 {
            1.0 / surf_front.radius_mm
        } else {
            0.0
        };
        let c_back = if surf_back.radius_mm.abs() > 1e-12 {
            1.0 / surf_back.radius_mm
        } else {
            0.0
        };

        let sag_f = optics_core::conic_sag(semi_diam, c_front, surf_front.conic_constant).unwrap_or(0.0);
        let sag_b = optics_core::conic_sag(semi_diam, c_back, surf_back.conic_constant).unwrap_or(0.0);

        let z_edge_f = z_front + sag_f;
        let z_edge_b = z_back + sag_b;

        for slice in 0..slices {
            let theta = 2.0 * std::f64::consts::PI * (slice as f64 / slices as f64);
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            front_perimeter.push(Vec3::new(semi_diam * cos_t, semi_diam * sin_t, z_edge_f));
            back_perimeter.push(Vec3::new(semi_diam * cos_t, semi_diam * sin_t, z_edge_b));
        }

        let rim_tris = tessellate_lens_rim(&front_perimeter, &back_perimeter, surf_front_idx, 0, 0);
        triangles.extend(rim_tris);

        let obj = NonSeqObject {
            id: 0,
            name: name.to_string(),
            geometry: PrimitiveGeometry::Mesh { triangles },
            transform: Transform3D::default(),
            material,
            is_active: true,
        };

        Ok(self.add_object(obj))
    }

    /// Automatically synthesizes a complete non-sequential scene from an optical prescription,
    /// converting sequential surfaces into solid 3D watertight elements, a collimated entrance pupil
    /// source, and a pixelated radiometric detector at the image surface.
    pub fn build_from_prescription(
        prescription: &OpticalPrescription,
        detector_res: usize,
    ) -> Result<Self, NonSeqError> {
        let mut scene = Self::new();
        let num_surfaces = prescription.surfaces.len();
        if num_surfaces < 2 {
            return Err(NonSeqError::InvalidGeometry(
                "Prescription must contain at least 2 surfaces to build a scene".to_string(),
            ));
        }

        let mut element_idx = 1;
        let mut i = 0;
        while i + 1 < num_surfaces {
            let surf = &prescription.surfaces[i];
            let mat = surf.material_name.trim();
            let is_glass = !mat.is_empty()
                && !mat.eq_ignore_ascii_case("AIR")
                && !mat.eq_ignore_ascii_case("VACUUM")
                && !mat.eq_ignore_ascii_case("NONE");

            if is_glass {
                let name = format!("Element_{}_{}", element_idx, mat);
                let medium = OpticalMedium::GlassCatalog { name: mat.to_string() };
                let material = NonSeqMaterial::dielectric(medium);
                scene.add_prescription_lens(&name, prescription, i, i + 1, material, 32, 16)?;
                element_idx += 1;
            }
            i += 1;
        }

        // Calculate total track distance to image plane
        let mut z_image = 0.0;
        for s in &prescription.surfaces[..num_surfaces - 1] {
            z_image += s.thickness_mm;
        }

        let img_semi_diam = prescription.surfaces.last().map(|s| s.semi_diameter_mm).unwrap_or(10.0);
        let det_size = (img_semi_diam * 2.0).max(4.0);
        let res = detector_res.clamp(10, 256);

        // Add 2D pixelated detector at image plane
        let detector = Detector2D::new(
            0,
            "ImageDetector",
            Vec3::new(0.0, 0.0, z_image),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            det_size,
            det_size,
            res,
            res,
        )?;
        scene.add_detector(detector);

        let primary_wl = prescription
            .wavelengths_nm
            .get(prescription.primary_wavelength_index)
            .copied()
            .unwrap_or(587.56);

        // Add collimated source on entrance pupil
        let epd = prescription.entrance_pupil_diameter_mm.max(1.0);
        let source = RaySource::CollimatedBeam {
            center: Vec3::new(0.0, 0.0, -10.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            radius: epd * 0.5,
            total_flux_watts: 1.0,
            spectrum: SourceSpectrum::Monochromatic {
                wavelength_nm: primary_wl,
            },
        };
        scene.add_source(source);

        Ok(scene)
    }
}
