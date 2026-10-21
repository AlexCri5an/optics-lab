//! Comprehensive optical analysis suite.
//!
//! This module provides analytical, geometric, and physical-optics diagnostics:
//! - **Spot Diagrams & Transverse Ray Fans:** [`spot_fan`]
//! - **Third-Order Seidel Aberrations & Chromatic Contributions:** [`seidel`]
//! - **Wavefront OPD Maps & Zernike Circular Polynomial Decomposition:** [`wavefront`]
//! - **Point Spread Function (FFT PSF & Huygens Superposition):** [`psf`]
//! - **Modulation Transfer Function (Diffraction & Geometric MTF):** [`mtf`]
//! - **Field Curves (Distortion, Field Curvature, Relative Illumination):** [`field_curves`]
//! - **Surface Ray Tables, Footprint Diagrams & OPD Fans:** [`ray_table`]
//! - **Through-Focus Analysis, Two-Bounce Ghost Analysis & Prescription Summary:** [`focus_ghost`]

pub mod field_curves;
pub mod focus_ghost;
pub mod mtf;
pub mod psf;
pub mod ray_table;
pub mod seidel;
pub mod spot_fan;
pub mod wavefront;

// Re-export primary types and functions for ergonomics
pub use field_curves::{
    FieldCurves, FieldPointAnalysis, LongitudinalAberrationPoint, compute_field_curves,
    compute_longitudinal_spherical_aberration,
};
pub use focus_ghost::{
    GhostAnalysis, GhostReflection, PrescriptionSummary, ThroughFocusAnalysis, ThroughFocusSpotPoint,
    compute_through_focus_spot, compute_two_bounce_ghosts, generate_prescription_summary,
};
pub use mtf::{
    MtfCurve, MtfPoint, ThroughFocusMtfPoint, compute_diffraction_mtf, compute_geometric_mtf,
    compute_through_focus_geometric_mtf, diffraction_limit_mtf,
};
pub use psf::{PointSpreadFunction, compute_fft_psf};
pub use ray_table::{
    FootprintDiagram, FootprintPoint, OpdFanData, OpdFanPoint, RayTable, SurfaceRayRow, compute_footprint_diagram,
    compute_opd_fan, compute_ray_table,
};
pub use seidel::{SeidelCoefficients, SurfaceSeidel, compute_seidel_coefficients};
pub use spot_fan::{
    PupilCoord, PupilPattern, RayFanData, RayFanPoint, SpotDiagram, SpotMetrics, SpotPoint,
    generate_pupil_distribution, launch_pupil_ray, spot_diagram_metrics, trace_ray_fan, trace_spot_diagram,
};
pub use wavefront::{
    WavefrontMap, ZernikeFit, compute_wavefront_map, fit_zernike_noll, noll_index_to_nm, zernike_noll_eval,
};
