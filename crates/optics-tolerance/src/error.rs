//! Error types for optical tolerancing and sensitivity simulations.

use thiserror::Error;

/// Error type encompassing all failure modes in `optics-tolerance`.
#[derive(Debug, Error)]
pub enum ToleranceError {
    /// Failure resolving prescription against glass catalog.
    #[error("Catalog resolution error: {0}")]
    Catalog(String),

    /// Core ray tracing or paraxial evaluation failure.
    #[error("Core optical error: {0}")]
    Core(#[from] optics_core::error::OpticsError),

    /// Optimization error during compensator adjustment.
    #[error("Compensator optimization error: {0}")]
    Optimize(#[from] optics_optimize::error::OptimizeError),

    /// Surface or element index out of range.
    #[error("Invalid surface specification: {0}")]
    InvalidSurface(String),

    /// Invalid statistical distribution parameter.
    #[error("Invalid tolerance distribution: {0}")]
    InvalidDistribution(String),

    /// Empty Monte Carlo sample or missing tolerance operands.
    #[error("Empty tolerance specification: {0}")]
    EmptySpecification(String),
}
