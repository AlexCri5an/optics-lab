//! Error types for non-sequential ray tracing.

use thiserror::Error;

/// Errors arising during non-sequential ray tracing and scene operations.
#[derive(Debug, Error)]
pub enum NonSeqError {
    /// Non-finite or degenerate geometry parameters.
    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),

    /// Singular or uninvertible transformation matrix.
    #[error("Singular transform matrix: {0}")]
    SingularTransform(String),

    /// Ray exceeded maximum bounce limit.
    #[error("Maximum ray bounce limit ({0}) exceeded")]
    MaxBouncesExceeded(usize),

    /// Missing or invalid material definition.
    #[error("Material error: {0}")]
    Material(String),

    /// Error during glass catalog lookup.
    #[error("Catalog lookup error: {0}")]
    Catalog(String),

    /// Error from underlying core optics calculation.
    #[error("Core optics error: {0}")]
    Core(#[from] optics_core::error::OpticsError),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}
