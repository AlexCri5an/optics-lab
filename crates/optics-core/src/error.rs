//! Error types for optics calculations.

use thiserror::Error;

/// Errors arising during optical calculations and ray tracing.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum OpticsError {
    /// Ray encountered total internal reflection at an interface.
    #[error("Total internal reflection occurred")]
    TotalInternalReflection,

    /// A numerical singularity occurred, e.g. division by zero, non-finite vector, or zero-length direction.
    #[error("Numerical singularity: {0}")]
    NumericalSingularity(String),

    /// An invalid surface specification or geometric configuration was encountered.
    #[error("Invalid surface: {0}")]
    InvalidSurface(String),

    /// A parsing failure occurred when reading optical prescriptions or parameters.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Ray missed the optical surface entirely (no mathematical intersection).
    #[error("Ray missed optical surface: {0}")]
    RayMissedSurface(String),

    /// Conic sag or evaluation is undefined because the coordinate is outside the real conic aperture ((1+k)*c^2*r^2 > 1).
    #[error("Geometry point lies outside real conic aperture: (1+k)*c^2*r^2 > 1")]
    GeometryOutsideAperture,

    /// Ray was vignetted / clipped by an aperture stop or surface clear semi-diameter.
    #[error("Ray vignetted at surface {surface_index}: radius {r:.4} mm exceeds semi-diameter {max_r:.4} mm")]
    RayVignetted {
        /// Surface index where clipping occurred.
        surface_index: usize,
        /// Ray radial height at intersection.
        r: f64,
        /// Surface clear semi-diameter.
        max_r: f64,
    },

    /// Reflective mirror surfaces are out of scope for the current milestone.
    #[error("Reflective surfaces are not supported: {0}")]
    ReflectiveSurfaceNotSupported(String),

    /// A specified optical glass or material name could not be resolved.
    #[error("Material not found: {0}")]
    MaterialNotFound(String),
}

