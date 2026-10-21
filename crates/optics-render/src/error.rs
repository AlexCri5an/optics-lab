//! Error definitions for the rendering and GPU pipeline.

use thiserror::Error;

/// Rendering and GPU initialization/execution errors.
#[derive(Debug, Error)]
pub enum RenderError {
    /// Failed to find a suitable GPU adapter.
    #[error("Failed to request compatible GPU adapter")]
    AdapterRequestFailed,

    /// Failed to create or acquire a logical GPU device.
    #[error("Failed to create GPU device: {0}")]
    DeviceRequestFailed(String),

    /// Failed to create or configure rendering surface.
    #[error("Surface creation failed: {0}")]
    SurfaceError(String),

    /// Shader compilation or pipeline creation failed.
    #[error("Pipeline creation error: {0}")]
    PipelineError(String),

    /// Ray generation singularity or invalid camera configuration.
    #[error("Camera configuration error: {0}")]
    CameraError(String),

    /// Readback or buffer mapping failed.
    #[error("Readback error: {0}")]
    ReadbackError(String),
}
