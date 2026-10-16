//! Event models and subscription infrastructure for reactive UI updates.

use serde::{Deserialize, Serialize};

use crate::validation::DiagnosticMessage;

/// Events broadcast by [`crate::OpticalEngine`] to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event_type", content = "payload")]
pub enum EngineEvent {
    /// The entire optical prescription was replaced or loaded.
    PrescriptionLoaded {
        /// System title.
        title: String,
        /// Current revision number.
        revision: u64,
    },
    /// Prescription was modified via command or undo/redo.
    PrescriptionChanged {
        /// Updated revision sequence number.
        revision: u64,
        /// Human-readable description of change.
        description: String,
    },
    /// A single surface was edited.
    SurfaceModified {
        /// 0-based surface index.
        surface_index: usize,
        /// Updated revision sequence number.
        revision: u64,
    },
    /// An analysis calculation completed.
    AnalysisCompleted {
        /// Type of analysis (e.g. "SpotDiagram", "Mtf").
        analysis_type: String,
        /// Elapsed execution time in milliseconds.
        duration_ms: f64,
    },
    /// Background asynchronous job progress update.
    JobProgress {
        /// Job unique identifier.
        job_id: u64,
        /// Completion percentage (0.0 to 100.0).
        percent: f32,
        /// Progress description message.
        status_message: String,
    },
    /// Background job finished successfully.
    JobCompleted {
        /// Job unique identifier.
        job_id: u64,
    },
    /// Background job was aborted or cancelled.
    JobCancelled {
        /// Job unique identifier.
        job_id: u64,
    },
    /// System diagnostics/validation messages were refreshed.
    DiagnosticsUpdated {
        /// Active diagnostic messages.
        diagnostics: Vec<DiagnosticMessage>,
    },
}

/// Thread-safe event listener callback handler.
pub type EventListener = Box<dyn Fn(&EngineEvent) + Send + Sync>;
