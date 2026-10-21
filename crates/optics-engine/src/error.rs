//! Unified optical engine error models.

use thiserror::Error;

/// All errors emitted by the unified optical engine facade.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Optical core calculation error.
    #[error("Optics core error: {0}")]
    Core(#[from] optics_core::error::OpticsError),

    /// Optical format parser error.
    #[error("Format parse error: {0}")]
    Format(#[from] optics_formats::error::FormatParseError),

    /// Synopsys CODE V macro parser error.
    #[error("CODE V parse error: {0}")]
    CodeV(#[from] optics_formats::CodeVParseError),

    /// Lambda Research OSLO parser error.
    #[error("OSLO parse error: {0}")]
    Oslo(#[from] optics_formats::OsloParseError),

    /// CAD layout and mesh export error.
    #[error("CAD export error: {0}")]
    Cad(#[from] optics_formats::CadExportError),

    /// Project workspace persistence error.
    #[error("Project error: {0}")]
    Project(#[from] optics_formats::ProjectError),

    /// Lens optimization error.
    #[error("Optimization error: {0}")]
    Optimize(#[from] optics_optimize::OptimizeError),

    /// Tolerancing sensitivity or Monte Carlo error.
    #[error("Tolerancing error: {0}")]
    Tolerance(#[from] optics_tolerance::ToleranceError),

    /// Non-sequential ray tracing error.
    #[error("Non-sequential error: {0}")]
    NonSeq(#[from] optics_nonseq::NonSeqError),

    /// Parameter or specification validation error.
    #[error("Engine parameter error: {0}")]
    InvalidParameter(String),
}
