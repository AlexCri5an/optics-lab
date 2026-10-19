//! Advanced optical surface profiles, apertures, and coordinate transformations.

pub mod aperture;
pub mod asphere;
pub mod biconic;
pub mod coordinate_break;
pub mod diffractive;
pub mod forbes;
pub mod grin;
pub mod polynomial;

pub use aperture::{Aperture, ApertureShape};
pub use asphere::{EvenAsphere, OddAsphere};
pub use biconic::{BiconicSurface, ToroidalSurface};
pub use coordinate_break::{CoordinateBreak, SurfaceTransform};
pub use diffractive::DiffractivePhaseSurface;
pub use forbes::{ForbesQbfsSurface, ForbesQconSurface};
pub use grin::{GrinProfile, trace_grin_rk4};
pub use polynomial::{XyPolynomialSurface, XyTerm, ZernikeSagSurface};

use serde::{Deserialize, Serialize};

use crate::conic::{ConicSurface, conic_normal, conic_sag};
use crate::error::OpticsError;
use crate::types::Vec3;

/// Comprehensive optical surface profile representation supporting standard, aspheric, freeform, and ideal optical elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceProfile {
    /// Planar surface ($z \equiv 0$).
    Planar,
    /// Rotationally symmetric quadric/conic surface.
    Conic(ConicSurface),
    /// Even aspheric surface with coefficients up to $r^{20}$.
    EvenAsphere(EvenAsphere),
    /// Odd aspheric surface with even and odd powers.
    OddAsphere(OddAsphere),
    /// Biconic surface with independent $x$ and $y$ curvatures and conics.
    Biconic(BiconicSurface),
    /// Toroidal surface of revolution.
    Toroidal(ToroidalSurface),
    /// XY bivariate polynomial surface.
    XyPolynomial(XyPolynomialSurface),
    /// Zernike standard sag surface over a circular pupil.
    Zernike(ZernikeSagSurface),
    /// Forbes $Q_{\text{con}}$ orthogonal asphere.
    ForbesQcon(ForbesQconSurface),
    /// Forbes $Q_{\text{bfs}}$ orthogonal asphere.
    ForbesQbfs(ForbesQbfsSurface),
    /// Diffractive phase surface.
    Diffractive(DiffractivePhaseSurface),
    /// Ideal paraxial thin lens surface with effective focal length.
    Paraxial {
        /// Effective focal length in millimeters.
        focal_length_mm: f64,
    },
    /// Flat or curved reflective mirror surface.
    Mirror,
    /// Non-refracting reference/diagnostic dummy surface (e.g. entrance pupil or intermediate focal plane).
    Dummy,
}

impl SurfaceProfile {
    /// Evaluates surface sag $z(x, y)$ at coordinates $(x, y)$ in millimeters.
    ///
    /// # Errors
    /// Returns [`OpticsError::GeometryOutsideAperture`] if $(x, y)$ falls outside the real surface domain.
    pub fn sag(&self, x: f64, y: f64) -> Result<f64, OpticsError> {
        match self {
            Self::Planar | Self::Paraxial { .. } | Self::Dummy | Self::Mirror => Ok(0.0),
            Self::Conic(conic) => {
                let r = (x * x + y * y).sqrt();
                conic_sag(r, conic.curvature, conic.conic_constant)
            }
            Self::EvenAsphere(asph) => {
                let r = (x * x + y * y).sqrt();
                asph.sag(r)
            }
            Self::OddAsphere(asph) => {
                let r = (x * x + y * y).sqrt();
                asph.sag(r)
            }
            Self::Biconic(biconic) => biconic.sag(x, y),
            Self::Toroidal(toroid) => toroid.sag(x, y),
            Self::XyPolynomial(poly) => poly.sag(x, y),
            Self::Zernike(zernike) => zernike.sag(x, y),
            Self::ForbesQcon(qcon) => {
                let r = (x * x + y * y).sqrt();
                qcon.sag(r)
            }
            Self::ForbesQbfs(qbfs) => {
                let r = (x * x + y * y).sqrt();
                qbfs.sag(r)
            }
            Self::Diffractive(_) => Ok(0.0),
        }
    }

    /// Evaluates the outward unit surface normal pointing toward $-z$.
    ///
    /// # Errors
    /// Returns [`OpticsError`] if out of domain or singular.
    pub fn normal(&self, x: f64, y: f64) -> Result<Vec3, OpticsError> {
        match self {
            Self::Planar | Self::Paraxial { .. } | Self::Dummy | Self::Mirror => {
                Ok(Vec3::new(0.0, 0.0, -1.0))
            }
            Self::Conic(conic) => {
                let z = self.sag(x, y)?;
                let hit_pt = Vec3::new(x, y, z);
                conic_normal(hit_pt, conic)
            }
            Self::EvenAsphere(asph) => asph.normal(x, y),
            Self::OddAsphere(asph) => asph.normal(x, y),
            Self::Biconic(biconic) => biconic.normal(x, y),
            Self::Toroidal(toroid) => toroid.normal(x, y),
            Self::XyPolynomial(poly) => poly.normal(x, y),
            Self::Zernike(zernike) => zernike.normal(x, y),
            Self::ForbesQcon(qcon) => qcon.normal(x, y),
            Self::ForbesQbfs(qbfs) => qbfs.normal(x, y),
            Self::Diffractive(_) => Ok(Vec3::new(0.0, 0.0, -1.0)),
        }
    }

    /// Evaluates paraxial curvature contribution at vertex $(0, 0)$.
    #[must_use]
    pub fn paraxial_curvature(&self) -> f64 {
        match self {
            Self::Planar | Self::Dummy | Self::Mirror | Self::Diffractive(_) => 0.0,
            Self::Conic(conic) => conic.curvature,
            Self::EvenAsphere(asph) => asph.paraxial_curvature(),
            Self::OddAsphere(asph) => asph.curvature,
            Self::Biconic(biconic) => 0.5 * (biconic.cx + biconic.cy),
            Self::Toroidal(toroid) => 0.5 * (1.0 / toroid.rx_mm + toroid.cy),
            Self::XyPolynomial(poly) => poly.curvature,
            Self::Zernike(zernike) => zernike.curvature,
            Self::ForbesQcon(qcon) => qcon.curvature,
            Self::ForbesQbfs(qbfs) => qbfs.curvature,
            Self::Paraxial { focal_length_mm } => 1.0 / *focal_length_mm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_surface_profile_dispatch() -> TestResult {
        let planar = SurfaceProfile::Planar;
        assert_eq!(planar.sag(5.0, 5.0)?, 0.0);
        assert_eq!(planar.normal(5.0, 5.0)?, Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(planar.paraxial_curvature(), 0.0);

        let asph = EvenAsphere::new(0.02, -1.0, vec![1e-6], 20.0)?;
        let profile = SurfaceProfile::EvenAsphere(asph);
        assert!(profile.sag(5.0, 5.0)? > 0.0);
        let n = profile.normal(3.0, 4.0)?;
        assert!((n.norm() - 1.0).abs() < 1e-12);
        Ok(())
    }
}
