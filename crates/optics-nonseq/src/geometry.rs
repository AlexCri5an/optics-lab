//! Non-sequential 3D geometry primitives, rigid transforms, and intersection solvers.

use crate::error::NonSeqError;
use nalgebra::{Matrix4, Vector3, Vector4};
use optics_bvh::Aabb;
use optics_core::{Ray, Triangle, Vec3};
use serde::{Deserialize, Serialize};

/// 3D rigid affine transformation comprising translation, Euler rotation, and scaling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    /// Translation displacement vector $(\Delta x, \Delta y, \Delta z)$ in millimeters.
    pub translation: Vec3,
    /// Extrinsic Euler rotation angles $(\theta_x, \theta_y, \theta_z)$ in degrees.
    pub rotation_euler_deg: Vec3,
    /// Scaling factors along local axes $(s_x, s_y, s_z)$.
    pub scale: Vec3,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            translation: Vec3::zeros(),
            rotation_euler_deg: Vec3::zeros(),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Transform3D {
    /// Create a pure translation transform.
    pub fn from_translation(x: f64, y: f64, z: f64) -> Self {
        Self {
            translation: Vec3::new(x, y, z),
            ..Default::default()
        }
    }

    /// Compute the 4x4 affine matrix transformation.
    pub fn to_matrix4(&self) -> Matrix4<f64> {
        let tx = self.translation.x;
        let ty = self.translation.y;
        let tz = self.translation.z;

        let rx = self.rotation_euler_deg.x.to_radians();
        let ry = self.rotation_euler_deg.y.to_radians();
        let rz = self.rotation_euler_deg.z.to_radians();

        let sx = self.scale.x;
        let sy = self.scale.y;
        let sz = self.scale.z;

        let cx = rx.cos();
        let sx_sin = rx.sin();
        let cy = ry.cos();
        let sy_sin = ry.sin();
        let cz = rz.cos();
        let sz_sin = rz.sin();

        // R = Rz * Ry * Rx
        let rot_matrix = Matrix4::new(
            cz * cy,
            cz * sy_sin * sx_sin - sz_sin * cx,
            cz * sy_sin * cx + sz_sin * sx_sin,
            0.0,
            sz_sin * cy,
            sz_sin * sy_sin * sx_sin + cz * cx,
            sz_sin * sy_sin * cx - cz * sx_sin,
            0.0,
            -sy_sin,
            cy * sx_sin,
            cy * cx,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );

        let scale_matrix = Matrix4::new(
            sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, 0.0, 0.0, 0.0, 1.0,
        );

        let trans_matrix = Matrix4::new(
            1.0, 0.0, 0.0, tx, 0.0, 1.0, 0.0, ty, 0.0, 0.0, 1.0, tz, 0.0, 0.0, 0.0, 1.0,
        );

        trans_matrix * rot_matrix * scale_matrix
    }

    /// Compute the inverse 4x4 affine matrix transformation.
    pub fn inverse_matrix4(&self) -> Result<Matrix4<f64>, NonSeqError> {
        let m = self.to_matrix4();
        m.try_inverse()
            .ok_or_else(|| NonSeqError::SingularTransform("Matrix cannot be inverted".to_string()))
    }

    /// Transform a world-space Ray into object local space.
    pub fn inverse_transform_ray(&self, ray: &Ray) -> Result<Ray, NonSeqError> {
        let inv_m = self.inverse_matrix4()?;
        let orig4 = Vector4::new(ray.origin.x, ray.origin.y, ray.origin.z, 1.0);
        let dir4 = Vector4::new(ray.direction.x, ray.direction.y, ray.direction.z, 0.0);

        let local_orig = inv_m * orig4;
        let local_dir = inv_m * dir4;

        let local_dir_vec = Vector3::new(local_dir[0], local_dir[1], local_dir[2]);
        let len = local_dir_vec.norm();
        if len <= 1e-15 || !len.is_finite() {
            return Err(NonSeqError::InvalidGeometry(
                "Degenerate ray direction in local space".to_string(),
            ));
        }

        Ray::new(
            Vector3::new(local_orig[0], local_orig[1], local_orig[2]),
            local_dir_vec / len,
            ray.wavelength_nm,
            ray.energy,
        )
        .map_err(NonSeqError::Core)
    }

    /// Transform a local normal vector back to world space using the inverse-transpose.
    pub fn transform_normal(&self, local_normal: &Vec3) -> Result<Vec3, NonSeqError> {
        let inv_m = self.inverse_matrix4()?;
        let inv_t = inv_m.transpose();
        let norm4 = Vector4::new(local_normal.x, local_normal.y, local_normal.z, 0.0);
        let world_n4 = inv_t * norm4;
        let mut world_n = Vector3::new(world_n4[0], world_n4[1], world_n4[2]);
        let norm = world_n.norm();
        if norm <= 1e-15 || !norm.is_finite() {
            return Err(NonSeqError::InvalidGeometry(
                "Degenerate transformed normal vector".to_string(),
            ));
        }
        world_n /= norm;
        Ok(world_n)
    }

    /// Transform a local point to world space.
    pub fn transform_point(&self, local_point: &Vec3) -> Vec3 {
        let m = self.to_matrix4();
        let p4 = Vector4::new(local_point.x, local_point.y, local_point.z, 1.0);
        let w4 = m * p4;
        Vector3::new(w4[0], w4[1], w4[2])
    }
}

/// Geometric shape primitives supported in non-sequential simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveGeometry {
    /// Rectangular planar surface centered at origin on $z = 0$.
    Plane {
        /// Half-width along local X in mm.
        half_width: f64,
        /// Half-height along local Y in mm.
        half_height: f64,
    },
    /// Circular or annular disk on $z = 0$.
    Disk {
        /// Outer radius in mm.
        outer_radius: f64,
        /// Inner hole radius in mm (0.0 for solid disk).
        inner_radius: f64,
    },
    /// Analytical sphere centered at origin.
    Sphere {
        /// Sphere radius in mm.
        radius: f64,
    },
    /// Cylinder aligned with local Z axis from $-L/2$ to $+L/2$.
    Cylinder {
        /// Cylinder outer radius in mm.
        radius: f64,
        /// Half-length along local Z in mm.
        half_length: f64,
    },
    /// Rectangular solid box / optical slab / window centered at local origin.
    Box {
        /// Half-width along local X in mm.
        half_x: f64,
        /// Half-height along local Y in mm.
        half_y: f64,
        /// Half-thickness along local Z in mm.
        half_z: f64,
    },
    /// Polygonal triangular mesh.
    Mesh {
        /// Triangle facets.
        triangles: Vec<Triangle>,
    },
}

/// Record of an intersection between a ray and a non-sequential object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NonSeqHit {
    /// Ray distance parameter $t \ge 0$ in millimeters.
    pub t: f64,
    /// Intersection point in world coordinates.
    pub point: Vec3,
    /// Surface unit normal vector in world coordinates (pointing toward incident side).
    pub normal: Vec3,
    /// Outward unit normal of the geometry (regardless of ray direction).
    pub geometric_normal: Vec3,
    /// Identifier of the intersected object.
    pub object_id: usize,
    /// Surface parametric coordinates $(u, v) \in [0, 1]^2$.
    pub uv: (f64, f64),
    /// True if ray is entering from exterior; false if ray is exiting from interior.
    pub entering: bool,
}

impl PrimitiveGeometry {
    /// Intersect a local-space ray against this primitive geometry.
    pub fn intersect_local(&self, local_ray: &Ray) -> Option<(f64, Vec3, (f64, f64))> {
        match *self {
            PrimitiveGeometry::Plane {
                half_width,
                half_height,
            } => {
                let dz = local_ray.direction.z;
                if dz.abs() < 1e-12 {
                    return None;
                }
                let t = -local_ray.origin.z / dz;
                if t <= 1e-9 {
                    return None;
                }
                let hit_x = local_ray.origin.x + t * local_ray.direction.x;
                let hit_y = local_ray.origin.y + t * local_ray.direction.y;
                if hit_x.abs() <= half_width && hit_y.abs() <= half_height {
                    let u = (hit_x + half_width) / (2.0 * half_width);
                    let v = (hit_y + half_height) / (2.0 * half_height);
                    let normal = Vec3::new(0.0, 0.0, 1.0);
                    Some((t, normal, (u, v)))
                } else {
                    None
                }
            }
            PrimitiveGeometry::Disk {
                outer_radius,
                inner_radius,
            } => {
                let dz = local_ray.direction.z;
                if dz.abs() < 1e-12 {
                    return None;
                }
                let t = -local_ray.origin.z / dz;
                if t <= 1e-9 {
                    return None;
                }
                let hit_x = local_ray.origin.x + t * local_ray.direction.x;
                let hit_y = local_ray.origin.y + t * local_ray.direction.y;
                let r2 = hit_x * hit_x + hit_y * hit_y;
                if r2 >= inner_radius * inner_radius && r2 <= outer_radius * outer_radius {
                    let _r = r2.sqrt();
                    let u = (hit_x / outer_radius + 1.0) * 0.5;
                    let v = (hit_y / outer_radius + 1.0) * 0.5;
                    let normal = Vec3::new(0.0, 0.0, 1.0);
                    Some((t, normal, (u, v)))
                } else {
                    None
                }
            }
            PrimitiveGeometry::Sphere { radius } => {
                let r = radius.abs();
                let ox = local_ray.origin.x;
                let oy = local_ray.origin.y;
                let oz = local_ray.origin.z;

                let dx = local_ray.direction.x;
                let dy = local_ray.direction.y;
                let dz = local_ray.direction.z;

                let a = dx * dx + dy * dy + dz * dz;
                let b = 2.0 * (ox * dx + oy * dy + oz * dz);
                let c = ox * ox + oy * oy + oz * oz - r * r;

                let disc = b * b - 4.0 * a * c;
                if disc < 0.0 {
                    return None;
                }

                let sqrt_disc = disc.sqrt();
                let t1 = (-b - sqrt_disc) / (2.0 * a);
                let t2 = (-b + sqrt_disc) / (2.0 * a);

                let eps = 1e-9;
                let t = if t1 > eps {
                    t1
                } else if t2 > eps {
                    t2
                } else {
                    return None;
                };

                let hit = local_ray.at(t);
                let normal = hit / r;
                let theta = (hit.z / r).clamp(-1.0, 1.0).acos();
                let phi = hit.y.atan2(hit.x);
                let u = (phi + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
                let v = theta / std::f64::consts::PI;

                Some((t, normal, (u, v)))
            }
            PrimitiveGeometry::Cylinder { radius, half_length } => {
                let ox = local_ray.origin.x;
                let oy = local_ray.origin.y;
                let dx = local_ray.direction.x;
                let dy = local_ray.direction.y;

                let a = dx * dx + dy * dy;
                if a < 1e-12 {
                    return None;
                }

                let b = 2.0 * (ox * dx + oy * dy);
                let c = ox * ox + oy * oy - radius * radius;
                let disc = b * b - 4.0 * a * c;
                if disc < 0.0 {
                    return None;
                }

                let sqrt_disc = disc.sqrt();
                let t1 = (-b - sqrt_disc) / (2.0 * a);
                let t2 = (-b + sqrt_disc) / (2.0 * a);

                let eps = 1e-9;
                for &t in &[t1, t2] {
                    if t > eps {
                        let hit_z = local_ray.origin.z + t * local_ray.direction.z;
                        if hit_z.abs() <= half_length {
                            let hit_x = ox + t * dx;
                            let hit_y = oy + t * dy;
                            let normal = Vec3::new(hit_x / radius, hit_y / radius, 0.0);
                            let phi = hit_y.atan2(hit_x);
                            let u = (phi + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
                            let v = (hit_z + half_length) / (2.0 * half_length);
                            return Some((t, normal, (u, v)));
                        }
                    }
                }
                None
            }
            PrimitiveGeometry::Box { half_x, half_y, half_z } => {
                let ox = local_ray.origin.x;
                let oy = local_ray.origin.y;
                let oz = local_ray.origin.z;
                let dx = local_ray.direction.x;
                let dy = local_ray.direction.y;
                let dz = local_ray.direction.z;

                let mut t_near = -f64::INFINITY;
                let mut t_far = f64::INFINITY;
                let mut norm_near = Vec3::zeros();
                let mut norm_far = Vec3::zeros();

                // Axis X
                if dx.abs() > 1e-12 {
                    let inv_d = 1.0 / dx;
                    let t1 = (-half_x - ox) * inv_d;
                    let t2 = (half_x - ox) * inv_d;
                    let (tn, tf, nn, nf) = if inv_d >= 0.0 {
                        (t1, t2, Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
                    } else {
                        (t2, t1, Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0))
                    };
                    if tn > t_near {
                        t_near = tn;
                        norm_near = nn;
                    }
                    if tf < t_far {
                        t_far = tf;
                        norm_far = nf;
                    }
                } else if ox.abs() > half_x {
                    return None;
                }

                // Axis Y
                if dy.abs() > 1e-12 {
                    let inv_d = 1.0 / dy;
                    let t1 = (-half_y - oy) * inv_d;
                    let t2 = (half_y - oy) * inv_d;
                    let (tn, tf, nn, nf) = if inv_d >= 0.0 {
                        (t1, t2, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
                    } else {
                        (t2, t1, Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0))
                    };
                    if tn > t_near {
                        t_near = tn;
                        norm_near = nn;
                    }
                    if tf < t_far {
                        t_far = tf;
                        norm_far = nf;
                    }
                } else if oy.abs() > half_y {
                    return None;
                }

                // Axis Z
                if dz.abs() > 1e-12 {
                    let inv_d = 1.0 / dz;
                    let t1 = (-half_z - oz) * inv_d;
                    let t2 = (half_z - oz) * inv_d;
                    let (tn, tf, nn, nf) = if inv_d >= 0.0 {
                        (t1, t2, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0))
                    } else {
                        (t2, t1, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0))
                    };
                    if tn > t_near {
                        t_near = tn;
                        norm_near = nn;
                    }
                    if tf < t_far {
                        t_far = tf;
                        norm_far = nf;
                    }
                } else if oz.abs() > half_z {
                    return None;
                }

                if t_near > t_far || t_far <= 1e-9 {
                    return None;
                }

                if t_near > 1e-9 {
                    Some((t_near, norm_near, (0.5, 0.5)))
                } else {
                    Some((t_far, norm_far, (0.5, 0.5)))
                }
            }
            PrimitiveGeometry::Mesh { ref triangles } => {
                let mut best_t = f64::INFINITY;
                let mut best_hit = None;

                for tri in triangles {
                    if let Some(hit) = tri.intersect(local_ray, 1e-9, best_t) {
                        if hit.t > 1e-9 && hit.t < best_t {
                            best_t = hit.t;
                            best_hit = Some((hit.t, hit.normal, (0.0, 0.0)));
                        }
                    }
                }

                best_hit
            }
        }
    }

    /// Compute axis-aligned bounding box in local coordinates.
    pub fn local_aabb(&self) -> Aabb {
        match *self {
            PrimitiveGeometry::Plane {
                half_width,
                half_height,
            } => Aabb::from_points(&[
                Vec3::new(-half_width, -half_height, -0.001),
                Vec3::new(half_width, half_height, 0.001),
            ]),
            PrimitiveGeometry::Disk { outer_radius, .. } => Aabb::from_points(&[
                Vec3::new(-outer_radius, -outer_radius, -0.001),
                Vec3::new(outer_radius, outer_radius, 0.001),
            ]),
            PrimitiveGeometry::Sphere { radius } => {
                let r = radius.abs();
                Aabb::from_points(&[Vec3::new(-r, -r, -r), Vec3::new(r, r, r)])
            }
            PrimitiveGeometry::Cylinder { radius, half_length } => Aabb::from_points(&[
                Vec3::new(-radius, -radius, -half_length),
                Vec3::new(radius, radius, half_length),
            ]),
            PrimitiveGeometry::Box { half_x, half_y, half_z } => {
                Aabb::from_points(&[Vec3::new(-half_x, -half_y, -half_z), Vec3::new(half_x, half_y, half_z)])
            }
            PrimitiveGeometry::Mesh { ref triangles } => {
                let mut b = Aabb::empty();
                for t in triangles {
                    b.grow(t.v0);
                    b.grow(t.v1);
                    b.grow(t.v2);
                }
                b
            }
        }
    }
}
