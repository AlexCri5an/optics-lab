//! 3D scene model and procedural geometry generation for non-sequential ray tracing.
//!
//! Provides parametric scene objects (prisms, spheres, slabs, tessellated lenses, floor planes),
//! Euler angle transforms, physical optical materials, and incremental BVH acceleration rebuilding.

use nalgebra::{Matrix4, Vector3};
use optics_bvh::{Aabb, BlasInstance, BvhScene, Triangle};
use optics_core::Vec3;

/// Choice of optical dispersion model for scene materials.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DispersionModelChoice {
    /// Sellmeier 6-coefficient empirical dispersion:
    /// $$n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{B_i \lambda^2}{\lambda^2 - C_i}$$
    Sellmeier,
    /// Cauchy 2-term approximation:
    /// $$n(\lambda) = A + \frac{B}{\lambda^2}$$
    Cauchy,
    /// Constant, non-dispersive refractive index $n$.
    Constant(f64),
}

/// Optical material parameters for physical path tracing.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialData {
    /// Human-readable material name (e.g. "Schott N-BK7").
    pub name: String,
    /// Glass catalog identifier.
    pub glass_name: String,
    /// Chosen dispersion formulation.
    pub dispersion: DispersionModelChoice,
    /// Surface microfacet roughness $\alpha \in [0.0, 1.0]$ for GGX ground/frosted glass.
    pub roughness: f32,
    /// Refractive index at helium yellow d line (587.56 nm).
    pub nd: f64,
    /// Abbe dispersion number $V_d = (n_d - 1) / (n_F - n_C)$.
    pub vd: f64,
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: "N-BK7".into(),
            glass_name: "N-BK7".into(),
            dispersion: DispersionModelChoice::Sellmeier,
            roughness: 0.0,
            nd: 1.5168,
            vd: 64.17,
        }
    }
}

/// 3D affine transformation composed of translation, Euler rotation, and scaling.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transform {
    /// Translation position in millimeters.
    pub position: Vec3,
    /// Rotation angles in degrees (pitch around X, yaw around Y, roll around Z).
    pub rotation_deg: Vec3,
    /// Non-uniform scale factors.
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation_deg: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Transform {
    /// Creates a transform with specific position, default rotation, and unit scale.
    #[must_use]
    pub const fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation_deg: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }

    /// Converts this transform into a 4x4 affine transformation matrix.
    #[must_use]
    pub fn to_matrix(&self) -> Matrix4<f64> {
        let pitch_rad = self.rotation_deg.x.to_radians();
        let yaw_rad = self.rotation_deg.y.to_radians();
        let roll_rad = self.rotation_deg.z.to_radians();

        let rot = Matrix4::from_euler_angles(pitch_rad, yaw_rad, roll_rad);
        let trans = Matrix4::new_translation(&Vector3::new(
            self.position.x,
            self.position.y,
            self.position.z,
        ));
        let scaling = Matrix4::new_nonuniform_scaling(&Vector3::new(
            self.scale.x,
            self.scale.y,
            self.scale.z,
        ));

        trans * rot * scaling
    }
}

/// Parametric geometry definitions for scene objects.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ObjectGeometry {
    /// Triangular equilateral or scalene prism.
    Prism {
        /// Apex angle $\alpha$ in degrees.
        apex_deg: f64,
        /// Base width in millimeters.
        base_width: f64,
        /// Extrusion depth in millimeters along the Z axis.
        depth: f64,
    },
    /// UV tessellated glass sphere.
    Sphere {
        /// Sphere radius in millimeters.
        radius: f64,
        /// Latitudinal stack subdivisions.
        stacks: u32,
        /// Longitudinal sector subdivisions.
        sectors: u32,
    },
    /// Rectangular optical glass flat / window slab.
    Slab {
        /// Slab width (X axis) in mm.
        width: f64,
        /// Slab height (Y axis) in mm.
        height: f64,
        /// Slab thickness (Z axis) in mm.
        thickness: f64,
    },
    /// Tessellated thick biconvex/plano-convex singlet lens.
    LensMesh {
        /// Front surface radius of curvature $R_1$ in mm (positive for convex, 0.0 for flat).
        r1: f64,
        /// Rear surface radius of curvature $R_2$ in mm (negative for convex, 0.0 for flat).
        r2: f64,
        /// Clear aperture semi-diameter in mm.
        semi_diameter: f64,
        /// Center thickness in mm.
        center_thickness: f64,
        /// Radial subdivision count.
        radial_segments: u32,
        /// Azimuthal angular subdivision count.
        azimuthal_segments: u32,
    },
    /// Horizontal infinite receiver / ground plane.
    FloorPlane {
        /// Half-extent in millimeters.
        size: f64,
    },
}

impl ObjectGeometry {
    /// Tessellates this geometry into watertight 3D triangles with calculated outward normals.
    #[must_use]
    pub fn tessellate(&self, material_id: u32) -> Vec<Triangle> {
        match self {
            Self::Prism {
                apex_deg,
                base_width,
                depth,
            } => {
                let half_apex_rad = (apex_deg * 0.5).to_radians();
                let height = (base_width * 0.5) / half_apex_rad.tan().max(1e-4);
                let depth_half = depth * 0.5;

                let v_apex_front = Vec3::new(0.0, height, depth_half);
                let v_left_front = Vec3::new(-base_width * 0.5, 0.0, depth_half);
                let v_right_front = Vec3::new(base_width * 0.5, 0.0, depth_half);

                let v_apex_back = Vec3::new(0.0, height, -depth_half);
                let v_left_back = Vec3::new(-base_width * 0.5, 0.0, -depth_half);
                let v_right_back = Vec3::new(base_width * 0.5, 0.0, -depth_half);

                vec![
                    // Front face (outward normal +Z)
                    Triangle::new(v_left_front, v_right_front, v_apex_front, material_id),
                    // Back face (outward normal -Z)
                    Triangle::new(v_right_back, v_left_back, v_apex_back, material_id),
                    // Left entry face
                    Triangle::new(v_left_back, v_left_front, v_apex_front, material_id),
                    Triangle::new(v_left_back, v_apex_front, v_apex_back, material_id),
                    // Right exit face
                    Triangle::new(v_right_front, v_right_back, v_apex_front, material_id),
                    Triangle::new(v_right_back, v_apex_back, v_apex_front, material_id),
                    // Bottom base face
                    Triangle::new(v_left_front, v_left_back, v_right_front, material_id),
                    Triangle::new(v_right_front, v_left_back, v_right_back, material_id),
                ]
            }

            Self::Sphere {
                radius,
                stacks,
                sectors,
            } => {
                let stacks = (*stacks).max(4);
                let sectors = (*sectors).max(6);
                let r = *radius;

                let mut vertices =
                    Vec::with_capacity((stacks + 1) as usize * (sectors + 1) as usize);
                for i in 0..=stacks {
                    let v = i as f64 / stacks as f64;
                    let phi = std::f64::consts::PI * (v - 0.5); // -PI/2 to PI/2
                    let y = r * phi.sin();
                    let cos_phi = phi.cos();

                    for j in 0..=sectors {
                        let u = j as f64 / sectors as f64;
                        let theta = 2.0 * std::f64::consts::PI * u;
                        let x = r * cos_phi * theta.sin();
                        let z = r * cos_phi * theta.cos();
                        vertices.push(Vec3::new(x, y, z));
                    }
                }

                let mut triangles = Vec::with_capacity((stacks * sectors * 2) as usize);
                let stride = (sectors + 1) as usize;

                for i in 0..stacks as usize {
                    for j in 0..sectors as usize {
                        let p00 = vertices[i * stride + j];
                        let p01 = vertices[i * stride + j + 1];
                        let p10 = vertices[(i + 1) * stride + j];
                        let p11 = vertices[(i + 1) * stride + j + 1];

                        if i != 0 {
                            triangles.push(Triangle::new(p00, p01, p10, material_id));
                        }
                        if i != (stacks - 1) as usize {
                            triangles.push(Triangle::new(p01, p11, p10, material_id));
                        }
                    }
                }
                triangles
            }

            Self::Slab {
                width,
                height,
                thickness,
            } => {
                let hw = width * 0.5;
                let hh = height * 0.5;
                let ht = thickness * 0.5;

                // 8 corners
                let p0 = Vec3::new(-hw, -hh, ht);
                let p1 = Vec3::new(hw, -hh, ht);
                let p2 = Vec3::new(hw, hh, ht);
                let p3 = Vec3::new(-hw, hh, ht);
                let p4 = Vec3::new(-hw, -hh, -ht);
                let p5 = Vec3::new(hw, -hh, -ht);
                let p6 = Vec3::new(hw, hh, -ht);
                let p7 = Vec3::new(-hw, hh, -ht);

                vec![
                    // Front (+Z)
                    Triangle::new(p0, p1, p2, material_id),
                    Triangle::new(p0, p2, p3, material_id),
                    // Back (-Z)
                    Triangle::new(p5, p4, p7, material_id),
                    Triangle::new(p5, p7, p6, material_id),
                    // Left (-X)
                    Triangle::new(p4, p0, p3, material_id),
                    Triangle::new(p4, p3, p7, material_id),
                    // Right (+X)
                    Triangle::new(p1, p5, p6, material_id),
                    Triangle::new(p1, p6, p2, material_id),
                    // Top (+Y)
                    Triangle::new(p3, p2, p6, material_id),
                    Triangle::new(p3, p6, p7, material_id),
                    // Bottom (-Y)
                    Triangle::new(p4, p5, p1, material_id),
                    Triangle::new(p4, p1, p0, material_id),
                ]
            }

            Self::LensMesh {
                r1,
                r2,
                semi_diameter,
                center_thickness,
                radial_segments,
                azimuthal_segments,
            } => {
                let nr = (*radial_segments).max(3);
                let nphi = (*azimuthal_segments).max(6);
                let sd = *semi_diameter;
                let ct = *center_thickness;

                // Sagitta formula for front surface: z1(r) = ct/2 - (r1 - sqrt(r1^2 - r^2)) if r1 > 0
                let sag1 = |r: f64| -> f64 {
                    if r1.abs() < 1e-6 {
                        ct * 0.5
                    } else if *r1 > 0.0 {
                        let rad_sq = r1 * r1;
                        let under = (rad_sq - r * r).max(0.0);
                        ct * 0.5 - (*r1 - under.sqrt())
                    } else {
                        let rad_sq = r1 * r1;
                        let under = (rad_sq - r * r).max(0.0);
                        ct * 0.5 - (*r1 + under.sqrt())
                    }
                };

                // Sagitta formula for rear surface: z2(r) = -ct/2 + (r2 - sqrt(r2^2 - r^2))
                let sag2 = |r: f64| -> f64 {
                    if r2.abs() < 1e-6 {
                        -ct * 0.5
                    } else if *r2 < 0.0 {
                        let rad_sq = r2 * r2;
                        let under = (rad_sq - r * r).max(0.0);
                        -ct * 0.5 - (*r2 + under.sqrt())
                    } else {
                        let rad_sq = r2 * r2;
                        let under = (rad_sq - r * r).max(0.0);
                        -ct * 0.5 - (*r2 - under.sqrt())
                    }
                };

                let mut triangles = Vec::new();

                // 1. Front and Rear concentric ring surfaces
                for i in 0..nr {
                    let r_inner = sd * (i as f64 / nr as f64);
                    let r_outer = sd * ((i + 1) as f64 / nr as f64);

                    let z1_in = sag1(r_inner);
                    let z1_out = sag1(r_outer);
                    let z2_in = sag2(r_inner);
                    let z2_out = sag2(r_outer);

                    for j in 0..nphi {
                        let phi0 = 2.0 * std::f64::consts::PI * (j as f64 / nphi as f64);
                        let phi1 = 2.0 * std::f64::consts::PI * ((j + 1) as f64 / nphi as f64);

                        let c0 = phi0.cos();
                        let s0 = phi0.sin();
                        let c1 = phi1.cos();
                        let s1 = phi1.sin();

                        // Front surface vertices
                        let f_in0 = Vec3::new(r_inner * c0, r_inner * s0, z1_in);
                        let f_in1 = Vec3::new(r_inner * c1, r_inner * s1, z1_in);
                        let f_out0 = Vec3::new(r_outer * c0, r_outer * s0, z1_out);
                        let f_out1 = Vec3::new(r_outer * c1, r_outer * s1, z1_out);

                        if i == 0 {
                            triangles.push(Triangle::new(f_in0, f_out0, f_out1, material_id));
                        } else {
                            triangles.push(Triangle::new(f_in0, f_out0, f_out1, material_id));
                            triangles.push(Triangle::new(f_in0, f_out1, f_in1, material_id));
                        }

                        // Rear surface vertices (wound in reverse for outward normal)
                        let b_in0 = Vec3::new(r_inner * c0, r_inner * s0, z2_in);
                        let b_in1 = Vec3::new(r_inner * c1, r_inner * s1, z2_in);
                        let b_out0 = Vec3::new(r_outer * c0, r_outer * s0, z2_out);
                        let b_out1 = Vec3::new(r_outer * c1, r_outer * s1, z2_out);

                        if i == 0 {
                            triangles.push(Triangle::new(b_in0, b_out1, b_out0, material_id));
                        } else {
                            triangles.push(Triangle::new(b_in0, b_out1, b_out0, material_id));
                            triangles.push(Triangle::new(b_in0, b_in1, b_out1, material_id));
                        }
                    }
                }

                // 2. Outer cylindrical rim connecting front and rear perimeters
                let z1_edge = sag1(sd);
                let z2_edge = sag2(sd);

                for j in 0..nphi {
                    let phi0 = 2.0 * std::f64::consts::PI * (j as f64 / nphi as f64);
                    let phi1 = 2.0 * std::f64::consts::PI * ((j + 1) as f64 / nphi as f64);

                    let c0 = phi0.cos();
                    let s0 = phi0.sin();
                    let c1 = phi1.cos();
                    let s1 = phi1.sin();

                    let rim_f0 = Vec3::new(sd * c0, sd * s0, z1_edge);
                    let rim_f1 = Vec3::new(sd * c1, sd * s1, z1_edge);
                    let rim_b0 = Vec3::new(sd * c0, sd * s0, z2_edge);
                    let rim_b1 = Vec3::new(sd * c1, sd * s1, z2_edge);

                    triangles.push(Triangle::new(rim_f0, rim_b0, rim_b1, material_id));
                    triangles.push(Triangle::new(rim_f0, rim_b1, rim_f1, material_id));
                }

                triangles
            }

            Self::FloorPlane { size } => {
                let s = *size;
                let f0 = Vec3::new(-s, -0.01, -s);
                let f1 = Vec3::new(s, -0.01, -s);
                let f2 = Vec3::new(s, -0.01, s);
                let f3 = Vec3::new(-s, -0.01, s);

                vec![
                    Triangle::new(f0, f1, f2, material_id),
                    Triangle::new(f0, f2, f3, material_id),
                ]
            }
        }
    }
}

/// An identifiable, editable object within the 3D optical scene.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SceneObject {
    /// Unique stable identifier.
    pub id: u64,
    /// Human-readable object name in the scene tree.
    pub name: String,
    /// Visibility toggle (whether included in BVH and path tracer).
    pub visible: bool,
    /// Spatial transformation.
    pub transform: Transform,
    /// Parametric geometry definition.
    pub geometry: ObjectGeometry,
    /// Assigned optical material.
    pub material: MaterialData,
}

impl SceneObject {
    /// Creates a new scene object with assigned parameters.
    #[must_use]
    pub fn new(
        id: u64,
        name: impl Into<String>,
        transform: Transform,
        geometry: ObjectGeometry,
        material: MaterialData,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            transform,
            geometry,
            material,
        }
    }

    /// Tessellates this object's geometry and produces its BLAS instance.
    #[must_use]
    pub fn tessellate(&self, material_id: u32) -> Vec<Triangle> {
        self.geometry.tessellate(material_id)
    }
}

/// Complete editable scene model holding all objects and BVH compilation logic.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SceneModel {
    /// Ordered list of objects in the scene.
    pub objects: Vec<SceneObject>,
    /// Identifier of the currently selected object, if any.
    pub selected_id: Option<u64>,
    /// Counter for allocating monotonic IDs.
    next_id: u64,
    /// Measured execution time of the most recent BVH build in milliseconds.
    pub last_bvh_build_time_ms: f64,
}

impl Default for SceneModel {
    fn default() -> Self {
        Self::default_prism_scene()
    }
}

impl SceneModel {
    /// Creates an empty scene model.
    #[must_use]
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            selected_id: None,
            next_id: 1,
            last_bvh_build_time_ms: 0.0,
        }
    }

    /// Creates the standard default scene: a dispersive glass prism and dark glossy floor.
    #[must_use]
    pub fn default_prism_scene() -> Self {
        let mut model = Self::new();

        let prism_id = model.add_object(SceneObject::new(
            0,
            "Dispersive Prism",
            Transform::from_position(Vec3::new(0.0, 0.0, 0.0)),
            ObjectGeometry::Prism {
                apex_deg: 60.0,
                base_width: 40.0,
                depth: 50.0,
            },
            MaterialData {
                name: "Schott N-BK7".into(),
                glass_name: "N-BK7".into(),
                dispersion: DispersionModelChoice::Sellmeier,
                roughness: 0.0,
                nd: 1.5168,
                vd: 64.17,
            },
        ));

        model.add_object(SceneObject::new(
            0,
            "Ground Plane",
            Transform::from_position(Vec3::new(0.0, 0.0, 0.0)),
            ObjectGeometry::FloorPlane { size: 500.0 },
            MaterialData {
                name: "Glossy Dark Floor".into(),
                glass_name: "Floor".into(),
                dispersion: DispersionModelChoice::Constant(1.5),
                roughness: 0.05,
                nd: 1.5,
                vd: 50.0,
            },
        ));

        model.selected_id = Some(prism_id);
        model
    }

    /// Creates a 3D scene model populated with tessellated lens elements from an optical prescription.
    #[must_use]
    pub fn from_optical_prescription(
        prescription: &optics_core::prescription::OpticalPrescription,
    ) -> Self {
        let mut model = Self::new();
        let surfaces = &prescription.surfaces;
        let n = surfaces.len();
        if n >= 2 {
            let mut z_vertices = Vec::with_capacity(n);
            let mut current_z = 0.0;
            for s in surfaces {
                z_vertices.push(current_z);
                current_z += s.thickness_mm;
            }

            let mut elem_idx = 0;
            for i in 0..n - 1 {
                let front = &surfaces[i];
                let rear = &surfaces[i + 1];
                let is_glass = !front.material_name.trim().eq_ignore_ascii_case("AIR")
                    && !front.material_name.trim().is_empty();

                if is_glass {
                    let z_front = z_vertices[i];
                    let ct = front.thickness_mm;
                    let z_center = z_front + ct * 0.5;
                    let sd = front.semi_diameter_mm.max(rear.semi_diameter_mm).max(1.0);

                    model.add_object(SceneObject::new(
                        0,
                        format!("Element {} ({})", elem_idx + 1, front.material_name.trim()),
                        Transform::from_position(Vec3::new(0.0, 0.0, z_center)),
                        ObjectGeometry::LensMesh {
                            r1: front.radius_mm,
                            r2: rear.radius_mm,
                            semi_diameter: sd,
                            center_thickness: ct,
                            radial_segments: 16,
                            azimuthal_segments: 32,
                        },
                        MaterialData {
                            name: front.material_name.clone(),
                            glass_name: front.material_name.clone(),
                            dispersion: DispersionModelChoice::Sellmeier,
                            roughness: 0.0,
                            nd: 1.5,
                            vd: 50.0,
                        },
                    ));
                    elem_idx += 1;
                }
            }
        }

        // Add receiver / ground plane
        model.add_object(SceneObject::new(
            0,
            "Ground Plane",
            Transform::from_position(Vec3::new(0.0, -35.0, 25.0)),
            ObjectGeometry::FloorPlane { size: 300.0 },
            MaterialData {
                name: "Glossy Dark Floor".into(),
                glass_name: "Floor".into(),
                dispersion: DispersionModelChoice::Constant(1.5),
                roughness: 0.05,
                nd: 1.5,
                vd: 50.0,
            },
        ));

        if let Some(first) = model.objects.first() {
            model.selected_id = Some(first.id);
        }
        model
    }

    /// Adds a new object to the scene and assigns it a unique ID.
    pub fn add_object(&mut self, mut object: SceneObject) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        object.id = id;
        self.objects.push(object);
        id
    }

    /// Duplicates an existing object by ID with a new name and offset position.
    pub fn duplicate_object(&mut self, id: u64) -> Option<u64> {
        let original = self.get_object(id)?.clone();
        let mut clone = original;
        clone.name = format!("{} Copy", clone.name);
        clone.transform.position.x += 15.0; // slight offset so user can see it
        let new_id = self.add_object(clone);
        self.selected_id = Some(new_id);
        Some(new_id)
    }

    /// Removes an object by ID from the scene.
    pub fn delete_object(&mut self, id: u64) -> bool {
        if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
            self.objects.remove(pos);
            if self.selected_id == Some(id) {
                self.selected_id = self.objects.first().map(|o| o.id);
            }
            true
        } else {
            false
        }
    }

    /// Renames an existing object.
    pub fn rename_object(&mut self, id: u64, new_name: impl Into<String>) -> bool {
        if let Some(obj) = self.get_object_mut(id) {
            obj.name = new_name.into();
            true
        } else {
            false
        }
    }

    /// Toggles or sets the visibility of an object.
    pub fn set_visibility(&mut self, id: u64, visible: bool) -> bool {
        if let Some(obj) = self.get_object_mut(id) {
            obj.visible = visible;
            true
        } else {
            false
        }
    }

    /// Returns a reference to an object by ID.
    #[must_use]
    pub fn get_object(&self, id: u64) -> Option<&SceneObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// Returns a mutable reference to an object by ID.
    #[must_use]
    pub fn get_object_mut(&mut self, id: u64) -> Option<&mut SceneObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// Compiles all visible scene objects into a 2-level BVH acceleration structure
    /// and records the elapsed build time in milliseconds.
    pub fn build_bvh_scene(&mut self) -> (BvhScene, f64) {
        let start = std::time::Instant::now();

        let visible_objects: Vec<&SceneObject> =
            self.objects.iter().filter(|o| o.visible).collect();

        let mut mesh_geometries = Vec::with_capacity(visible_objects.len());
        let mut instances = Vec::with_capacity(visible_objects.len());

        for (i, obj) in visible_objects.iter().enumerate() {
            let material_id = if obj.name.contains("Floor")
                || matches!(obj.geometry, ObjectGeometry::FloorPlane { .. })
            {
                1
            } else {
                0
            };

            let triangles = obj.tessellate(material_id);
            let local_aabb = triangles
                .iter()
                .fold(Aabb::empty(), |a, t| a.union(&t.aabb()));

            mesh_geometries.push(triangles);
            instances.push(BlasInstance::new(i, &local_aabb, obj.transform.to_matrix()));
        }

        let scene = BvhScene::build(mesh_geometries, instances);
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.last_bvh_build_time_ms = elapsed_ms;

        (scene, elapsed_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prism_tessellation_watertight() {
        let prism = ObjectGeometry::Prism {
            apex_deg: 60.0,
            base_width: 40.0,
            depth: 50.0,
        };
        let triangles = prism.tessellate(0);
        assert_eq!(triangles.len(), 8, "Prism must have 8 triangles");

        let aabb = triangles
            .iter()
            .fold(Aabb::empty(), |a, t| a.union(&t.aabb()));
        assert!((aabb.min.x - -20.0).abs() < 1e-4);
        assert!((aabb.max.x - 20.0).abs() < 1e-4);
        assert!((aabb.min.y - 0.0).abs() < 1e-4);
        assert!(aabb.max.y > 10.0, "Prism height must be positive");
    }

    #[test]
    fn test_sphere_tessellation() {
        let sphere = ObjectGeometry::Sphere {
            radius: 25.0,
            stacks: 8,
            sectors: 12,
        };
        let triangles = sphere.tessellate(0);
        assert!(!triangles.is_empty());

        let aabb = triangles
            .iter()
            .fold(Aabb::empty(), |a, t| a.union(&t.aabb()));
        assert!((aabb.min.x + 25.0).abs() < 1e-2);
        assert!((aabb.max.x - 25.0).abs() < 1e-2);
        assert!((aabb.min.y + 25.0).abs() < 1e-2);
        assert!((aabb.max.y - 25.0).abs() < 1e-2);
    }

    #[test]
    fn test_slab_tessellation() {
        let slab = ObjectGeometry::Slab {
            width: 30.0,
            height: 20.0,
            thickness: 10.0,
        };
        let triangles = slab.tessellate(0);
        assert_eq!(triangles.len(), 12, "Slab cuboid must have 12 triangles");
    }

    #[test]
    fn test_lens_mesh_tessellation() {
        let lens = ObjectGeometry::LensMesh {
            r1: 50.0,
            r2: -50.0,
            semi_diameter: 20.0,
            center_thickness: 8.0,
            radial_segments: 6,
            azimuthal_segments: 12,
        };
        let triangles = lens.tessellate(0);
        assert!(!triangles.is_empty(), "Lens mesh should generate triangles");
    }

    #[test]
    fn test_scene_crud_operations() {
        let mut model = SceneModel::new();
        assert!(model.objects.is_empty());

        let id1 = model.add_object(SceneObject::new(
            0,
            "Sphere 1",
            Transform::default(),
            ObjectGeometry::Sphere {
                radius: 10.0,
                stacks: 6,
                sectors: 8,
            },
            MaterialData::default(),
        ));
        assert_eq!(model.objects.len(), 1);
        assert_eq!(
            model.get_object(id1).map(|o| o.name.as_str()),
            Some("Sphere 1")
        );

        // Rename
        assert!(model.rename_object(id1, "Central Sphere"));
        assert_eq!(
            model.get_object(id1).map(|o| o.name.as_str()),
            Some("Central Sphere")
        );

        // Duplicate
        let id2 = model.duplicate_object(id1).unwrap_or(0);
        assert_ne!(id2, 0);
        assert_eq!(model.objects.len(), 2);
        assert_eq!(
            model.get_object(id2).map(|o| o.name.as_str()),
            Some("Central Sphere Copy")
        );
        assert_ne!(id1, id2);

        // Visibility
        assert!(model.set_visibility(id1, false));
        assert_eq!(model.get_object(id1).map(|o| o.visible), Some(false));

        // Delete
        assert!(model.delete_object(id1));
        assert_eq!(model.objects.len(), 1);
        assert!(model.get_object(id1).is_none());
    }

    #[test]
    fn test_bvh_build_timing() {
        let mut model = SceneModel::default_prism_scene();
        let (scene, elapsed_ms) = model.build_bvh_scene();
        assert_eq!(scene.instances.len(), 2);
        assert!(elapsed_ms >= 0.0);
        assert_eq!(model.last_bvh_build_time_ms, elapsed_ms);
    }

    #[test]
    fn test_optical_prescription_scene_tessellation() {
        use optics_core::{OpticalPrescription, OpticalSurface};
        let prescription = OpticalPrescription {
            name: "Test Doublet".into(),
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 12.5,
                    material_name: "N-BK7".into(),
                    is_stop: false,
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: -50.0,
                    conic_constant: 0.0,
                    thickness_mm: 10.0,
                    semi_diameter_mm: 12.5,
                    material_name: "".into(),
                    is_stop: false,
                },
            ],
            entrance_pupil_diameter_mm: 25.0,
            wavelengths_nm: vec![587.6],
            primary_wavelength_index: 0,
            field_angles_deg: vec![0.0],
        };
        let mut model = SceneModel::from_optical_prescription(&prescription);
        assert_eq!(model.objects.len(), 2);
        assert_eq!(model.objects[0].name, "Element 1 (N-BK7)");
        let (bvh, _) = model.build_bvh_scene();
        assert_eq!(bvh.instances.len(), 2);
    }
}
