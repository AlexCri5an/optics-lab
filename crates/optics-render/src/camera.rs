//! Thin-lens and orthographic camera models for ray generation and GPU uniform buffer binding.

use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Vector3};
use optics_core::{OpticsError, Ray, Vec3};

use crate::error::RenderError;

/// GPU uniform structure for camera parameters, matching WGSL layout.
///
/// 16-byte aligned, size 160 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct CameraUniform {
    /// Inverse view-projection transformation matrix.
    pub inv_view_proj: [[f32; 4]; 4],
    /// World space eye position $(x, y, z, 1.0)$.
    pub position: [f32; 4],
    /// Camera parameters: $[fov\_rad, aperture\_radius\_mm, focus\_distance\_mm, aspect\_ratio]$.
    pub params: [f32; 4],
    /// Right vector in world space $(r_x, r_y, r_z, 0.0)$.
    pub right: [f32; 4],
    /// Up vector in world space $(u_x, u_y, u_z, 0.0)$.
    pub up: [f32; 4],
    /// Forward vector in world space $(f_x, f_y, f_z, 0.0)$.
    pub forward: [f32; 4],
}

/// Physical thin-lens camera supporting depth-of-field and realistic bokeh.
#[derive(Clone, Debug, PartialEq)]
pub struct ThinLensCamera {
    /// World position of the lens center in millimeters.
    pub position: Vec3,
    /// Point of interest / target in millimeters.
    pub look_at: Vec3,
    /// Upward reference orientation vector.
    pub up: Vec3,
    /// Vertical field of view in degrees.
    pub fov_deg: f64,
    /// Lens aperture diameter in millimeters (0.0 for ideal pinhole).
    pub aperture_mm: f64,
    /// Distance from the lens center to the plane of perfect focus in millimeters.
    pub focus_dist_mm: f64,
}

impl ThinLensCamera {
    /// Creates a new thin-lens camera.
    #[must_use]
    pub fn new(
        position: Vec3,
        look_at: Vec3,
        up: Vec3,
        fov_deg: f64,
        aperture_mm: f64,
        focus_dist_mm: f64,
    ) -> Self {
        Self {
            position,
            look_at,
            up,
            fov_deg,
            aperture_mm,
            focus_dist_mm: focus_dist_mm.max(1e-4),
        }
    }

    /// Evaluates camera coordinate basis vectors $(\mathbf{forward}, \mathbf{right}, \mathbf{up})$.
    ///
    /// # Errors
    /// Returns [`RenderError::CameraError`] if `position == look_at` or vectors are collinear.
    pub fn basis(&self) -> Result<(Vec3, Vec3, Vec3), RenderError> {
        let forward = self.look_at - self.position;
        let f_len = forward.norm();
        if f_len < 1e-12 {
            return Err(RenderError::CameraError(
                "Camera position and look_at target are identical".to_string(),
            ));
        }
        let forward = forward / f_len;

        let right = self.up.cross(&forward);
        let r_len = right.norm();
        if r_len < 1e-12 {
            return Err(RenderError::CameraError(
                "Camera view direction and up vector are collinear".to_string(),
            ));
        }
        let right = right / r_len;
        let up = forward.cross(&right).normalize();

        Ok((forward, right, up))
    }

    /// Generates a primary ray for normalized screen coordinates $(u, v) \in [0, 1]^2$.
    ///
    /// Jitters the lens sample across the aperture disk for depth-of-field.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if ray direction validation fails.
    pub fn generate_ray(
        &self,
        u: f64,
        v: f64,
        lens_sample: (f64, f64),
        aspect_ratio: f64,
        wavelength_nm: f64,
    ) -> Result<Ray, OpticsError> {
        let (forward, right, up) = match self.basis() {
            Ok(b) => b,
            Err(_) => (
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        };

        let theta_rad = self.fov_deg.to_radians();
        let half_h = (theta_rad * 0.5).tan() * self.focus_dist_mm;
        let half_w = half_h * aspect_ratio;

        // Position on the focal plane
        let p_focus = self.position
            + forward * self.focus_dist_mm
            + right * ((2.0 * u - 1.0) * half_w)
            + up * ((1.0 - 2.0 * v) * half_h);

        // Sample concentric lens disk
        let (lens_dx, lens_dy) = concentric_disk_sample(lens_sample.0, lens_sample.1);
        let lens_radius = self.aperture_mm * 0.5;
        let p_lens = self.position + (right * lens_dx + up * lens_dy) * lens_radius;

        let dir = (p_focus - p_lens).normalize();

        Ray::new(p_lens, dir, wavelength_nm, 1.0)
    }

    /// Packs camera parameters into a GPU-compatible [`CameraUniform`].
    #[must_use]
    pub fn to_uniform(&self, aspect_ratio: f64) -> CameraUniform {
        let (forward, right, up) = self.basis().unwrap_or((
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ));

        let eye = Point3::new(self.position.x, self.position.y, self.position.z);
        let target = Point3::new(self.look_at.x, self.look_at.y, self.look_at.z);
        let up_vec = Vector3::new(self.up.x, self.up.y, self.up.z);

        let view = Matrix4::look_at_rh(&eye, &target, &up_vec);
        let proj = Matrix4::new_perspective(
            aspect_ratio as f32,
            self.fov_deg.to_radians() as f32,
            0.1,
            10000.0,
        );

        let view_f32 = view.cast::<f32>();
        let vp = proj * view_f32;
        let inv_vp = vp.try_inverse().unwrap_or_else(Matrix4::identity);

        let mut inv_vp_arr = [[0.0_f32; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                inv_vp_arr[r][c] = inv_vp[(r, c)];
            }
        }

        CameraUniform {
            inv_view_proj: inv_vp_arr,
            position: [
                self.position.x as f32,
                self.position.y as f32,
                self.position.z as f32,
                1.0,
            ],
            params: [
                self.fov_deg.to_radians() as f32,
                (self.aperture_mm * 0.5) as f32,
                self.focus_dist_mm as f32,
                aspect_ratio as f32,
            ],
            right: [right.x as f32, right.y as f32, right.z as f32, 0.0],
            up: [up.x as f32, up.y as f32, up.z as f32, 0.0],
            forward: [forward.x as f32, forward.y as f32, forward.z as f32, 0.0],
        }
    }
}

/// Orthographic camera for technical cross-section views and paraxial system layout.
#[derive(Clone, Debug, PartialEq)]
pub struct OrthographicCamera {
    /// World position of the camera in millimeters.
    pub position: Vec3,
    /// Target point in millimeters.
    pub look_at: Vec3,
    /// Upward reference vector.
    pub up: Vec3,
    /// Visible width in millimeters.
    pub viewport_width_mm: f64,
    /// Visible height in millimeters.
    pub viewport_height_mm: f64,
}

impl OrthographicCamera {
    /// Creates a new orthographic camera.
    #[must_use]
    pub fn new(
        position: Vec3,
        look_at: Vec3,
        up: Vec3,
        viewport_width_mm: f64,
        viewport_height_mm: f64,
    ) -> Self {
        Self {
            position,
            look_at,
            up,
            viewport_width_mm,
            viewport_height_mm,
        }
    }

    /// Generates a parallel ray for normalized screen coordinates $(u, v) \in [0, 1]^2$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if ray direction is degenerate.
    pub fn generate_ray(&self, u: f64, v: f64, wavelength_nm: f64) -> Result<Ray, OpticsError> {
        let forward = (self.look_at - self.position).normalize();
        let right = self.up.cross(&forward).normalize();
        let up = forward.cross(&right).normalize();

        let origin = self.position
            + right * ((u - 0.5) * self.viewport_width_mm)
            + up * ((0.5 - v) * self.viewport_height_mm);

        Ray::new(origin, forward, wavelength_nm, 1.0)
    }
}

/// Maps uniform square samples $(\xi_1, \xi_2) \in [0, 1]^2$ to concentric unit disk.
#[must_use]
pub fn concentric_disk_sample(u1: f64, u2: f64) -> (f64, f64) {
    let sx = 2.0 * u1 - 1.0;
    let sy = 2.0 * u2 - 1.0;

    if sx == 0.0 && sy == 0.0 {
        return (0.0, 0.0);
    }

    let (r, theta) = if sx.abs() > sy.abs() {
        (sx, (std::f64::consts::FRAC_PI_4) * (sy / sx))
    } else {
        (
            sy,
            (std::f64::consts::FRAC_PI_2) - (std::f64::consts::FRAC_PI_4) * (sx / sy),
        )
    };

    (r * theta.cos(), r * theta.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_camera_uniform_layout() -> TestResult {
        assert_eq!(
            std::mem::align_of::<CameraUniform>(),
            16,
            "CameraUniform must be 16-byte aligned"
        );
        Ok(())
    }

    #[test]
    fn test_thin_lens_pinhole_ray_generation() -> TestResult {
        let camera = ThinLensCamera::new(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            60.0,
            0.0, // pinhole
            10.0,
        );

        // Center pixel (0.5, 0.5)
        let ray = camera.generate_ray(0.5, 0.5, (0.0, 0.0), 1.0, 587.56)?;
        assert_eq!(ray.origin, Vec3::new(0.0, 0.0, -10.0));
        assert!((ray.direction.z - 1.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_orthographic_camera_ray_generation() -> TestResult {
        let camera = OrthographicCamera::new(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            20.0,
            20.0,
        );

        let ray = camera.generate_ray(0.5, 0.5, 587.56)?;
        assert_eq!(ray.origin, Vec3::new(0.0, 0.0, -10.0));
        assert_eq!(ray.direction, Vec3::new(0.0, 0.0, 1.0));

        let top_left_ray = camera.generate_ray(0.0, 0.0, 587.56)?;
        assert_eq!(top_left_ray.origin, Vec3::new(-10.0, 10.0, -10.0));
        assert_eq!(top_left_ray.direction, Vec3::new(0.0, 0.0, 1.0));

        Ok(())
    }
}
