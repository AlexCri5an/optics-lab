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
}
