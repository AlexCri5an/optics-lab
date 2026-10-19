//! Coordinate breaks, spatial decenters, and 3D Euler rotations.
//!
//! Transforms rays and surface references between tilted and off-axis coordinate frames.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::{Ray, Vec3};

/// Coordinate break defining spatial decenters and Euler rotation tilts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateBreak {
    /// Decenter along $x$ in millimeters.
    pub decenter_x_mm: f64,
    /// Decenter along $y$ in millimeters.
    pub decenter_y_mm: f64,
    /// Decenter along $z$ in millimeters.
    pub decenter_z_mm: f64,
    /// Tilt angle about $x$-axis in degrees.
    pub tilt_x_deg: f64,
    /// Tilt angle about $y$-axis in degrees.
    pub tilt_y_deg: f64,
    /// Tilt angle about $z$-axis in degrees.
    pub tilt_z_deg: f64,
    /// Order flag: if true, rotation occurs before decenter translation.
    pub tilt_first: bool,
}

impl Default for CoordinateBreak {
    fn default() -> Self {
        Self {
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            decenter_z_mm: 0.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            tilt_z_deg: 0.0,
            tilt_first: false,
        }
    }
}

impl CoordinateBreak {
    /// Creates a 3D Euler rotation matrix $\mathbf{R} = \mathbf{R}_z \mathbf{R}_y \mathbf{R}_x$.
    #[must_use]
    pub fn rotation_matrix(&self) -> nalgebra::Matrix3<f64> {
        let to_rad = std::f64::consts::PI / 180.0;
        let rx = self.tilt_x_deg * to_rad;
        let ry = self.tilt_y_deg * to_rad;
        let rz = self.tilt_z_deg * to_rad;

        let rot_x = nalgebra::Matrix3::new(
            1.0,
            0.0,
            0.0,
            0.0,
            rx.cos(),
            -rx.sin(),
            0.0,
            rx.sin(),
            rx.cos(),
        );

        let rot_y = nalgebra::Matrix3::new(
            ry.cos(),
            0.0,
            ry.sin(),
            0.0,
            1.0,
            0.0,
            -ry.sin(),
            0.0,
            ry.cos(),
        );

        let rot_z = nalgebra::Matrix3::new(
            rz.cos(),
            -rz.sin(),
            0.0,
            rz.sin(),
            rz.cos(),
            0.0,
            0.0,
            0.0,
            1.0,
        );

        rot_z * rot_y * rot_x
    }

    /// Evaluates the 3D translation displacement vector (dx, dy, dz) in millimeters.
    #[must_use]
    pub fn translation(&self) -> Vec3 {
        Vec3::new(self.decenter_x_mm, self.decenter_y_mm, self.decenter_z_mm)
    }

    /// Transforms a ray into the new local coordinate system.
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if transformed direction has zero norm.
    pub fn transform_ray_forward(&self, ray: &Ray) -> Result<Ray, OpticsError> {
        let r_mat = self.rotation_matrix();
        let decenter = self.translation();

        let (new_origin, new_dir) = if self.tilt_first {
            // Rotation then translation: origin' = R * origin - decenter
            let rot_o = r_mat * ray.origin;
            let rot_d = r_mat * ray.direction;
            (rot_o - decenter, rot_d)
        } else {
            // Translation then rotation: origin' = R * (origin - decenter)
            let trans_o = ray.origin - decenter;
            (r_mat * trans_o, r_mat * ray.direction)
        };

        Ray::new(new_origin, new_dir, ray.wavelength_nm, ray.energy)
    }

    /// Transforms a ray from the local coordinate system back to the parent frame.
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if transformed direction has zero norm.
    pub fn transform_ray_inverse(&self, ray: &Ray) -> Result<Ray, OpticsError> {
        let r_mat_t = self.rotation_matrix().transpose();
        let decenter = self.translation();

        let (parent_origin, parent_dir) = if self.tilt_first {
            let trans_o = ray.origin + decenter;
            (r_mat_t * trans_o, r_mat_t * ray.direction)
        } else {
            let rot_o = r_mat_t * ray.origin;
            (rot_o + decenter, r_mat_t * ray.direction)
        };

        Ray::new(parent_origin, parent_dir, ray.wavelength_nm, ray.energy)
    }
}

/// Global 3D rigid transform (origin vertex position and orientation matrix) of an optical surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SurfaceTransform {
    /// 3D vertex position in global coordinates (mm).
    pub origin: Vec3,
    /// 3D orthonormal orientation rotation matrix transforming local vectors to global coordinates.
    pub rotation: nalgebra::Matrix3<f64>,
}

impl Default for SurfaceTransform {
    fn default() -> Self {
        Self {
            origin: Vec3::zeros(),
            rotation: nalgebra::Matrix3::identity(),
        }
    }
}

impl SurfaceTransform {
    /// Creates a transform from vertex position and rotation matrix.
    #[must_use]
    pub fn new(origin: Vec3, rotation: nalgebra::Matrix3<f64>) -> Self {
        Self { origin, rotation }
    }

    /// Transforms a global point into local surface coordinates:
    /// $$\mathbf{P}_{\text{local}} = \mathbf{R}^T (\mathbf{P}_{\text{global}} - \mathbf{O})$$
    #[must_use]
    pub fn to_local_point(&self, p_global: Vec3) -> Vec3 {
        self.rotation.transpose() * (p_global - self.origin)
    }

    /// Transforms a local point into global coordinates:
    /// $$\mathbf{P}_{\text{global}} = \mathbf{O} + \mathbf{R} \mathbf{P}_{\text{local}}$$
    #[must_use]
    pub fn to_global_point(&self, p_local: Vec3) -> Vec3 {
        self.origin + self.rotation * p_local
    }

    /// Transforms a global direction vector into local surface coordinates:
    /// $$\mathbf{D}_{\text{local}} = \mathbf{R}^T \mathbf{D}_{\text{global}}$$
    #[must_use]
    pub fn to_local_dir(&self, d_global: Vec3) -> Vec3 {
        self.rotation.transpose() * d_global
    }

    /// Transforms a local direction vector into global coordinates:
    /// $$\mathbf{D}_{\text{global}} = \mathbf{R} \mathbf{D}_{\text{local}}$$
    #[must_use]
    pub fn to_global_dir(&self, d_local: Vec3) -> Vec3 {
        self.rotation * d_local
    }

    /// Transforms a global ray into the local surface coordinate frame.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if normalized direction has zero or invalid norm.
    pub fn to_local_ray(&self, ray: &Ray) -> Result<Ray, OpticsError> {
        let local_o = self.to_local_point(ray.origin);
        let local_d = self.to_local_dir(ray.direction);
        Ray::new(local_o, local_d, ray.wavelength_nm, ray.energy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_coordinate_break_roundtrip() -> TestResult {
        let cb = CoordinateBreak {
            decenter_x_mm: 5.0,
            decenter_y_mm: -2.0,
            decenter_z_mm: 1.0,
            tilt_x_deg: 10.0,
            tilt_y_deg: -5.0,
            tilt_z_deg: 15.0,
            tilt_first: false,
        };

        let initial_ray = Ray::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.2, 0.9746794),
            587.56,
            1.0,
        )?;

        let forward = cb.transform_ray_forward(&initial_ray)?;
        let roundtrip = cb.transform_ray_inverse(&forward)?;

        assert!((roundtrip.origin - initial_ray.origin).norm() < 1e-12);
        assert!((roundtrip.direction - initial_ray.direction).norm() < 1e-12);
        Ok(())
    }
}
