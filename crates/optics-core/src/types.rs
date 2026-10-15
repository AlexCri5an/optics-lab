//! Core optical primitives, vector type aliases, and ray interaction records.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;

/// Type alias for 3D geometric vectors in double precision (`f64`).
///
/// We use `nalgebra::Vector3<f64>` directly to interoperate seamlessly with
/// nalgebra's linear algebra routines without newtype overhead.
pub type Vec3 = nalgebra::Vector3<f64>;

/// A single geometric light ray parameterized by origin, unit direction, wavelength, and radiometric energy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ray {
    /// Ray emission or interaction origin point in millimeters.
    pub origin: Vec3,
    /// Normalized propagation direction vector (must be unit length).
    pub direction: Vec3,
    /// Spectral wavelength in vacuum in nanometers (e.g. 589.3 nm for sodium D line).
    pub wavelength_nm: f64,
    /// Radiometric energy or relative flux carrying capability (default 1.0).
    pub energy: f64,
}

impl Ray {
    /// Creates a new ray, validating that the direction vector is non-zero, finite, and normalized.
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if the direction contains NaN/infinite
    /// components, or if its norm is zero or near-zero (< 1e-15).
    pub fn new(
        origin: Vec3,
        direction: Vec3,
        wavelength_nm: f64,
        energy: f64,
    ) -> Result<Self, OpticsError> {
        if !origin.iter().all(|x| x.is_finite()) || !direction.iter().all(|x| x.is_finite()) {
            return Err(OpticsError::NumericalSingularity(
                "Ray origin or direction contains non-finite values".to_string(),
            ));
        }

        let norm_sq = direction.norm_squared();
        if norm_sq < 1e-30 {
            return Err(OpticsError::NumericalSingularity(
                "Ray direction has near-zero norm".to_string(),
            ));
        }

        let norm = norm_sq.sqrt();
        let normalized_dir = direction / norm;
        debug_assert!(
            (normalized_dir.norm_squared() - 1.0).abs() < 1e-10,
            "Normalized direction must have unit norm"
        );

        Ok(Self {
            origin,
            direction: normalized_dir,
            wavelength_nm,
            energy,
        })
    }

    /// Evaluates the 3D position along the ray at parameter distance `t`.
    ///
    /// $$P(t) = \mathbf{origin} + t \cdot \mathbf{direction}$$
    #[inline]
    #[must_use]
    pub fn at(&self, t: f64) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Description of a ray-surface intersection event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HitRecord {
    /// Distance parameter $t$ along the ray where intersection occurred.
    pub t: f64,
    /// Exact 3D coordinates of the intersection point.
    pub point: Vec3,
    /// Outward/canonical unit normal vector at the hit point.
    pub normal: Vec3,
    /// Whether the ray struck the exterior (`true`) or interior (`false`) surface face.
    pub is_front_face: bool,
    /// Index of the optical surface within the prescription or scene.
    pub surface_index: usize,
}

/// The physical event type that concluded or continued a ray bounce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BounceEvent {
    /// Specular reflection at an optical boundary.
    Reflect,
    /// Specular refraction into a dielectric medium.
    Refract,
    /// Total internal reflection (critical angle exceeded, 100% reflected).
    TotalInternalReflection,
    /// Ray escaped the optical system into free space.
    Escape,
    /// Ray terminated at an aperture stop or absorbed by a baffle/substrate.
    Absorbed,
}

/// Comprehensive inspection payload recorded at each interaction/bounce of a ray.
///
/// Designed to feed both the interactive Single-Ray Inspector UI panel and diagnostic traces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RayBounce {
    /// Index of the optical surface where this bounce took place.
    pub surface_index: usize,
    /// 3D position of the bounce in scene space.
    pub point: Vec3,
    /// Unit surface normal oriented toward the incident medium ($\mathbf{i} \cdot \mathbf{n} \le 0$).
    pub normal: Vec3,
    /// Incident ray unit propagation direction.
    pub incident_dir: Vec3,
    /// Specularly reflected unit direction.
    pub reflected_dir: Vec3,
    /// Specularly refracted unit direction (None if TIR or absorbed).
    pub refracted_dir: Option<Vec3>,
    /// Refractive index of the incident medium at this wavelength.
    pub n1: f64,
    /// Refractive index of the transmitted medium at this wavelength.
    pub n2: f64,
    /// Angle of incidence in radians $\theta_i \in [0, \pi/2]$.
    pub incident_angle_rad: f64,
    /// Angle of refraction in radians $\theta_t \in [0, \pi/2]$ (None if TIR).
    pub refracted_angle_rad: Option<f64>,
    /// s-polarized power reflectance $R_s \in [0, 1]$.
    pub rs: f64,
    /// p-polarized power reflectance $R_p \in [0, 1]$.
    pub rp: f64,
    /// Unpolarized total reflectance $R_{\text{total}} = (R_s + R_p) / 2 \in [0, 1]$.
    pub r_total: f64,
    /// Total transmittance $T_{\text{total}} = 1 - R_{\text{total}} \in [0, 1]$.
    pub t_total: f64,
    /// Physical nature of this event.
    pub event: BounceEvent,
    /// Wavelength of the ray in nanometers.
    pub wavelength_nm: f64,
}
