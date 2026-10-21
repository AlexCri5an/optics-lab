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
    pub fn new(origin: Vec3, direction: Vec3, wavelength_nm: f64, energy: f64) -> Result<Self, OpticsError> {
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

/// Canonical optical wavelength with unit conversions and Fraunhofer spectral reference lines.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Wavelength {
    /// Wavelength in micrometers (µm).
    microns: f64,
}

impl Wavelength {
    /// Fraunhofer d-line (Helium yellow): 0.5875618 µm (587.5618 nm).
    pub const D_LINE: Self = Self { microns: 0.587_561_8 };
    /// Fraunhofer e-line (Mercury green): 0.5460740 µm (546.0740 nm).
    pub const E_LINE: Self = Self { microns: 0.546_074_0 };
    /// Fraunhofer F-line (Hydrogen blue): 0.4861327 µm (486.1327 nm).
    pub const F_LINE: Self = Self { microns: 0.486_132_7 };
    /// Fraunhofer C-line (Hydrogen red): 0.6562725 µm (656.2725 nm).
    pub const C_LINE: Self = Self { microns: 0.656_272_5 };
    /// Fraunhofer g-line (Mercury violet): 0.4358343 µm (435.8343 nm).
    pub const G_LINE: Self = Self { microns: 0.435_834_3 };

    /// Creates a wavelength from micrometers (µm).
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if the value is non-finite or non-positive.
    pub fn from_microns(um: f64) -> Result<Self, OpticsError> {
        if !um.is_finite() || um <= 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Wavelength in microns must be a positive, finite number".to_string(),
            ));
        }
        Ok(Self { microns: um })
    }

    /// Creates a wavelength from nanometers (nm).
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if the value is non-finite or non-positive.
    pub fn from_nanometers(nm: f64) -> Result<Self, OpticsError> {
        if !nm.is_finite() || nm <= 0.0 {
            return Err(OpticsError::InvalidParameter(
                "Wavelength in nanometers must be a positive, finite number".to_string(),
            ));
        }
        Ok(Self { microns: nm / 1000.0 })
    }

    /// Wavelength value in micrometers (µm).
    #[inline]
    #[must_use]
    pub const fn microns(&self) -> f64 {
        self.microns
    }

    /// Wavelength value in nanometers (nm).
    #[inline]
    #[must_use]
    pub fn nanometers(&self) -> f64 {
        self.microns * 1000.0
    }
}

/// Normalized pupil coordinate on the aperture stop unit disk ($P_x^2 + P_y^2 \le 1.0$).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pupil {
    /// Normalized horizontal (sagittal) coordinate in $[-1.0, 1.0]$.
    pub px: f64,
    /// Normalized vertical (tangential) coordinate in $[-1.0, 1.0]$.
    pub py: f64,
}

impl Pupil {
    /// Creates a normalized pupil coordinate, validating that it lies within the unit disk.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if non-finite or $P_x^2 + P_y^2 > 1.0 + \epsilon$.
    pub fn new(px: f64, py: f64) -> Result<Self, OpticsError> {
        if !px.is_finite() || !py.is_finite() {
            return Err(OpticsError::InvalidParameter(
                "Pupil coordinates must be finite".to_string(),
            ));
        }
        let rho_sq = px * px + py * py;
        if rho_sq > 1.0 + 1e-12 {
            return Err(OpticsError::InvalidParameter(format!(
                "Pupil coordinate ({px}, {py}) exceeds unit disk (rho^2 = {rho_sq:.4} > 1.0)"
            )));
        }
        Ok(Self { px, py })
    }

    /// Chief ray / pupil center $(0.0, 0.0)$.
    #[must_use]
    pub const fn center() -> Self {
        Self { px: 0.0, py: 0.0 }
    }

    /// Normalized radial distance $\rho = \sqrt{P_x^2 + P_y^2} \in [0.0, 1.0]$.
    #[inline]
    #[must_use]
    pub fn rho(&self) -> f64 {
        (self.px * self.px + self.py * self.py).sqrt().min(1.0)
    }

    /// Azimuthal angle $\theta = \text{atan2}(P_y, P_x)$ in radians.
    #[inline]
    #[must_use]
    pub fn theta(&self) -> f64 {
        self.py.atan2(self.px)
    }
}

/// Field specification for optical systems.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Field {
    /// Object field specified in angular degrees (e.g. for infinite conjugates).
    AngleDegrees {
        /// Horizontal field angle in degrees.
        x: f64,
        /// Vertical field angle in degrees.
        y: f64,
    },
    /// Object field specified as physical height on the object plane in millimeters.
    HeightMm {
        /// Horizontal object height in mm.
        x: f64,
        /// Vertical object height in mm.
        y: f64,
    },
    /// Normalized field coordinate in $[-1.0, 1.0]$.
    Normalized {
        /// Normalized horizontal field coordinate.
        x: f64,
        /// Normalized vertical field coordinate.
        y: f64,
    },
}

impl Field {
    /// On-axis field point (zero angle/height).
    #[must_use]
    pub const fn on_axis() -> Self {
        Self::AngleDegrees { x: 0.0, y: 0.0 }
    }
}

/// Strongly-typed optical surface index / identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SurfaceId(pub usize);

impl SurfaceId {
    /// Surface 0 (commonly object or first optical surface).
    pub const ZERO: Self = Self(0);

    /// Inner raw index.
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

impl From<usize> for SurfaceId {
    #[inline]
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<SurfaceId> for usize {
    #[inline]
    fn from(val: SurfaceId) -> Self {
        val.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_wavelength_units_and_constants() -> TestResult {
        let wl = Wavelength::from_nanometers(550.0)?;
        assert!((wl.microns() - 0.55).abs() < 1e-12);
        assert!((wl.nanometers() - 550.0).abs() < 1e-12);

        let d_line = Wavelength::D_LINE;
        assert!((d_line.nanometers() - 587.5618).abs() < 1e-6);

        // Validation rejects negative and NaN
        assert!(Wavelength::from_microns(-0.5).is_err());
        assert!(Wavelength::from_nanometers(f64::NAN).is_err());
        Ok(())
    }

    #[test]
    fn test_pupil_coordinates_and_bounds() -> TestResult {
        let center = Pupil::center();
        assert_eq!(center.px, 0.0);
        assert_eq!(center.py, 0.0);
        assert_eq!(center.rho(), 0.0);

        let edge = Pupil::new(0.0, 1.0)?;
        assert!((edge.rho() - 1.0).abs() < 1e-12);
        assert!((edge.theta() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);

        // Beyond unit circle is rejected
        assert!(Pupil::new(0.8, 0.8).is_err());
        Ok(())
    }

    #[test]
    fn test_field_and_surface_id() {
        let on_axis = Field::on_axis();
        assert_eq!(on_axis, Field::AngleDegrees { x: 0.0, y: 0.0 });

        let s0 = SurfaceId::ZERO;
        assert_eq!(s0.index(), 0);
        let s3 = SurfaceId::from(3);
        assert_eq!(s3.index(), 3);
        assert_eq!(usize::from(s3), 3);
    }
}
