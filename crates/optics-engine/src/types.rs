//! Serializable request, response, and summary data contracts for the unified optical engine facade.

use serde::{Deserialize, Serialize};

use optics_core::analysis::{
    FieldCurves, GhostAnalysis, MtfCurve, PointSpreadFunction, PrescriptionSummary, RayFanData, RayTable,
    SeidelCoefficients, SpotDiagram, ThroughFocusAnalysis, WavefrontMap, ZernikeFit,
};

/// Unified analysis request variants supported by the optical engine facade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "params")]
pub enum AnalysisRequest {
    /// Paraxial trace, cardinal points, pupils, and optical invariant.
    Paraxial,
    /// Spot diagram calculation at a specified field point and wavelength.
    SpotDiagram {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Optional explicit wavelength in nanometers (None uses primary).
        wavelength_nm: Option<f64>,
        /// Number of rays across pupil diameter.
        #[serde(default = "default_spot_rays")]
        ray_count: usize,
    },
    /// Transverse ray fan curves (tangential Ey vs Py and sagittal Ex vs Px).
    RayFan {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Optional explicit wavelength in nanometers.
        wavelength_nm: Option<f64>,
        /// Number of ray pupil evaluation points.
        #[serde(default = "default_fan_rays")]
        num_rays: usize,
    },
    /// Wavefront map (optical path difference OPD) with optional Zernike fit.
    Wavefront {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Optional explicit wavelength in nanometers.
        wavelength_nm: Option<f64>,
        /// Sampling grid resolution across entrance pupil.
        #[serde(default = "default_grid_size")]
        grid_size: usize,
        /// Number of Noll Zernike polynomial terms to fit (0 for none).
        #[serde(default = "default_zernike_terms")]
        zernike_terms: usize,
    },
    /// Seidel 3rd-order aberration coefficients (surface-by-surface and system sums).
    Seidel,
    /// Fast Fourier Transform (FFT) Point Spread Function (PSF).
    Psf {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// FFT grid resolution (e.g. 64, 128, 256).
        #[serde(default = "default_psf_grid")]
        grid_size: usize,
    },
    /// Modulation Transfer Function (Diffraction MTF).
    Mtf {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Maximum spatial frequency in line pairs per millimeter (lp/mm).
        #[serde(default = "default_max_freq")]
        max_freq_lp_mm: f64,
        /// Number of frequency sampling points.
        #[serde(default = "default_freq_points")]
        num_points: usize,
    },
    /// Astigmatic field curvature, distortion, and chromatic focal shifts.
    FieldCurves {
        /// Number of field sampling increments from on-axis to full field.
        #[serde(default = "default_field_points")]
        num_points: usize,
    },
    /// Comprehensive ray trace coordinates, angles, and OPL across every surface.
    RayTable {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Relative pupil coordinate X in [-1.0, 1.0].
        #[serde(default)]
        pupil_x: f64,
        /// Relative pupil coordinate Y in [-1.0, 1.0].
        #[serde(default = "default_pupil_y")]
        pupil_y: f64,
        /// Optional explicit wavelength in nanometers.
        wavelength_nm: Option<f64>,
    },
    /// Sequential two-bounce ghost reflection matrix and ghost pupil focus shifts.
    Ghosts {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
    },
    /// Through-focus RMS spot size and geometric MTF sweep.
    ThroughFocus {
        /// Field point index (0-based).
        #[serde(default)]
        field_index: usize,
        /// Defocus range in millimeters [±delta_z].
        #[serde(default = "default_defocus_range")]
        range_mm: f64,
        /// Number of focal steps across the range.
        #[serde(default = "default_focus_steps")]
        steps: usize,
    },
    /// Complete consolidated engineering summary of the optical system.
    FullReport,
}

fn default_spot_rays() -> usize {
    256
}
fn default_fan_rays() -> usize {
    64
}
fn default_grid_size() -> usize {
    64
}
fn default_zernike_terms() -> usize {
    36
}
fn default_psf_grid() -> usize {
    64
}
fn default_max_freq() -> f64 {
    100.0
}
fn default_freq_points() -> usize {
    50
}
fn default_field_points() -> usize {
    21
}
fn default_pupil_y() -> f64 {
    1.0
}
fn default_defocus_range() -> f64 {
    0.5
}
fn default_focus_steps() -> usize {
    41
}

/// Strongly-typed results emitted by the optical engine facade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result_type", content = "data")]
pub enum AnalysisResult {
    /// Paraxial cardinal points, focal lengths, and pupil specifications.
    Paraxial(ParaxialAnalysisResult),
    /// Geometric spot diagram with RMS radius, centroid, and Airy disc.
    SpotDiagram(SpotDiagram),
    /// Transverse ray fan curves (tangential and sagittal).
    RayFan(RayFanData),
    /// Wavefront map with optional orthogonal Zernike expansion fit.
    Wavefront {
        /// Raw OPD wavefront grid map.
        wavefront: WavefrontMap,
        /// Optional Zernike fit (Noll indices).
        zernike: Option<ZernikeFit>,
    },
    /// Seidel third-order aberration coefficients.
    Seidel(SeidelCoefficients),
    /// FFT Point Spread Function, Strehl ratio, and encircled energy.
    Psf(PointSpreadFunction),
    /// Diffraction Modulation Transfer Function curve (sagittal and tangential).
    Mtf(MtfCurve),
    /// Astigmatic field curvature (sagittal and tangential) and distortion curves.
    FieldCurves(FieldCurves),
    /// Surface-by-surface ray intersection coordinates, direction cosines, and OPL.
    RayTable(RayTable),
    /// Two-bounce ghost reflection analysis.
    Ghosts(GhostAnalysis),
    /// Through-focus focal sweep analysis.
    ThroughFocus(ThroughFocusAnalysis),
    /// Full engineering report.
    FullReport(PrescriptionSummary),
}

/// Paraxial marginal and chief ray surface intersection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParaxialRayData {
    /// Paraxial ray height in millimeters at the surface.
    pub y_mm: f64,
    /// Paraxial ray slope angle in radians.
    pub u_rad: f64,
}

/// Paraxial analysis results container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParaxialAnalysisResult {
    /// Effective focal length in millimeters.
    pub efl_mm: f64,
    /// Back focal length in millimeters.
    pub bfl_mm: f64,
    /// Working F-number ($N_w = \text{EFL} / \text{EPD}$).
    pub f_number: f64,
    /// Entrance pupil diameter in millimeters.
    pub entrance_pupil_diameter_mm: f64,
    /// Entrance pupil position relative to surface 1 in millimeters.
    pub entrance_pupil_position_mm: f64,
    /// Exit pupil position relative to image surface in millimeters.
    pub exit_pupil_position_mm: f64,
    /// Paraxial marginal ray coordinates surface-by-surface.
    pub marginal_ray: Vec<ParaxialRayData>,
    /// Paraxial chief ray coordinates surface-by-surface (on primary field).
    pub chief_ray: Vec<ParaxialRayData>,
}

/// Summary information for optical glass materials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlassSummary {
    /// Glass catalog identifier (e.g. "SCHOTT", "OHARA").
    pub catalog: String,
    /// Glass name (e.g. "N-BK7").
    pub name: String,
    /// Refractive index at d-line (587.56 nm).
    pub nd: f64,
    /// Abbe dispersion number $V_d = (n_d - 1) / (n_F - n_C)$.
    pub vd: f64,
    /// Density in g/cm³.
    pub density: Option<f64>,
}

/// Optical engine state metadata and status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineStatus {
    /// Prescription revision sequence number (monotonically incrementing on every modification).
    pub revision: u64,
    /// Name of the loaded optical system.
    pub system_name: String,
    /// Number of optical surfaces (including object and image).
    pub surface_count: usize,
    /// Index of the optical aperture stop surface.
    pub stop_surface_index: usize,
    /// Entrance pupil diameter in millimeters.
    pub entrance_pupil_diameter_mm: f64,
    /// Primary wavelength in nanometers.
    pub primary_wavelength_nm: f64,
    /// All active design wavelengths in nanometers.
    pub wavelengths_nm: Vec<f64>,
    /// Field point angles in degrees.
    pub field_angles_deg: Vec<f64>,
    /// Total track length in millimeters (vertex 1 to image).
    pub total_track_mm: f64,
    /// Effective focal length in millimeters (if finite).
    pub efl_mm: Option<f64>,
    /// Whether the engine contains uncommitted or modified state.
    pub is_dirty: bool,
    /// Whether an undo operation is currently possible.
    pub can_undo: bool,
    /// Whether a redo operation is currently possible.
    pub can_redo: bool,
}
