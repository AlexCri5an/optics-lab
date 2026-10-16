//! Error types for the optical optimization engine.

use thiserror::Error;

/// Error type encompassing all failure modes in `optics-optimize`.
#[derive(Debug, Error)]
pub enum OptimizeError {
    /// Prescriptions failed resolution against the glass catalog.
    #[error("Catalog resolution error: {0}")]
    Catalog(String),

    /// Core optical tracing or evaluation error.
    #[error("Core optical error: {0}")]
    Core(#[from] optics_core::error::OpticsError),

    /// Variable index out of bounds or targeting nonexistent surface.
    #[error("Invalid variable specification: {0}")]
    InvalidVariable(String),

    /// Singular or uninvertible normal equations matrix in DLS.
    #[error("Singular matrix in normal equations: {0}")]
    SingularMatrix(String),

    /// Optimization failed to converge within allowable iterations or budget.
    #[error("Convergence failure: {0}")]
    ConvergenceFailure(String),

    /// Computation was cancelled early by user or progress callback.
    #[error("Optimization cancelled: {0}")]
    Cancelled(String),

    /// An operand evaluation failed due to invalid optical geometry.
    #[error("Operand evaluation failed: {0}")]
    OperandEvaluation(String),
}
