//! Diagnostic validation layer detecting user errors, physical anomalies, and unfeasible geometries.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_glass::GlassCatalogDb;

/// Diagnostic severity classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    /// Informational advisory or optical property note.
    Info,
    /// Potentially unphysical or problematic configuration (e.g. extreme incidence angle, heavy vignetting).
    Warning,
    /// Severe invalid configuration that prevents physical ray tracing.
    Error,
}

/// A human-readable diagnostic message pinpointing location and explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticMessage {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Canonical diagnostic identifier code (e.g. "ERR_NO_SURFACES", "WARN_NEGATIVE_THICKNESS").
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// Optional affected optical surface index (0-based).
    pub surface_index: Option<usize>,
    /// Optional affected field angle index.
    pub field_index: Option<usize>,
    /// Remediation recommendation for the optical designer.
    pub recommendation: Option<String>,
}

/// Runs comprehensive diagnostic validation on an optical prescription.
pub fn validate_prescription(
    prescription: &OpticalPrescription,
    catalog: &GlassCatalogDb,
) -> Vec<DiagnosticMessage> {
    let mut diagnostics = Vec::new();

    // 1. Surface Count Check
    if prescription.surfaces.len() < 2 {
        diagnostics.push(DiagnosticMessage {
            level: DiagnosticLevel::Error,
            code: "ERR_INSUFFICIENT_SURFACES".to_string(),
            message: "Prescription must contain at least 2 surfaces (Object and Image planes)."
                .to_string(),
            surface_index: None,
            field_index: None,
            recommendation: Some(
                "Add at least an object surface and an image surface.".to_string(),
            ),
        });
        return diagnostics;
    }

    // 2. Entrance Pupil Diameter Check
    if prescription.entrance_pupil_diameter_mm <= 0.0
        || !prescription.entrance_pupil_diameter_mm.is_finite()
    {
        diagnostics.push(DiagnosticMessage {
            level: DiagnosticLevel::Error,
            code: "ERR_INVALID_EPD".to_string(),
            message: format!(
                "Entrance pupil diameter must be positive and finite (got {:.4}).",
                prescription.entrance_pupil_diameter_mm
            ),
            surface_index: None,
            field_index: None,
            recommendation: Some("Set EPD to a positive aperture diameter in mm.".to_string()),
        });
    }

    // 3. Wavelength Checks
    if prescription.wavelengths_nm.is_empty() {
        diagnostics.push(DiagnosticMessage {
            level: DiagnosticLevel::Error,
            code: "ERR_NO_WAVELENGTHS".to_string(),
            message: "No design wavelengths defined in optical prescription.".to_string(),
            surface_index: None,
            field_index: None,
            recommendation: Some(
                "Specify at least one primary wavelength (e.g. 587.56 nm d-line).".to_string(),
            ),
        });
    } else {
        if prescription.primary_wavelength_index >= prescription.wavelengths_nm.len() {
            diagnostics.push(DiagnosticMessage {
                level: DiagnosticLevel::Error,
                code: "ERR_PRIMARY_WAVELENGTH_INDEX".to_string(),
                message: format!(
                    "Primary wavelength index {} out of bounds (total: {}).",
                    prescription.primary_wavelength_index,
                    prescription.wavelengths_nm.len()
                ),
                surface_index: None,
                field_index: None,
                recommendation: Some(
                    "Set primary wavelength index to a valid wavelength position.".to_string(),
                ),
            });
        }
        for (w_idx, &w) in prescription.wavelengths_nm.iter().enumerate() {
            if !(100.0..=20000.0).contains(&w) || !w.is_finite() {
                diagnostics.push(DiagnosticMessage {
                    level: DiagnosticLevel::Warning,
                    code: "WARN_EXTREME_WAVELENGTH".to_string(),
                    message: format!(
                        "Wavelength #{w_idx} ({:.2} nm) is outside standard optical spectrum (100–20,000 nm).",
                        w
                    ),
                    surface_index: None,
                    field_index: None,
                    recommendation: Some("Verify wavelength is within optical transmission band.".to_string()),
                });
            }
        }
    }

    // 4. Aperture Stop Check
    let stop_count = prescription.surfaces.iter().filter(|s| s.is_stop).count();
    if stop_count == 0 {
        diagnostics.push(DiagnosticMessage {
            level: DiagnosticLevel::Warning,
            code: "WARN_NO_STOP".to_string(),
            message: "Prescription does not have an explicit aperture stop surface marked."
                .to_string(),
            surface_index: None,
            field_index: None,
            recommendation: Some(
                "Designate one optical surface as the aperture stop (is_stop = true).".to_string(),
            ),
        });
    } else if stop_count > 1 {
        diagnostics.push(DiagnosticMessage {
            level: DiagnosticLevel::Warning,
            code: "WARN_MULTIPLE_STOPS".to_string(),
            message: format!("Multiple aperture stops ({stop_count}) defined; only the first will act as system stop."),
            surface_index: None,
            field_index: None,
            recommendation: Some("Ensure exactly one surface is designated as aperture stop.".to_string()),
        });
    }

    // 5. Per-surface Inspection
    let num_surfaces = prescription.surfaces.len();
    for (idx, surf) in prescription.surfaces.iter().enumerate() {
        // Semi-diameter check
        if surf.semi_diameter_mm <= 0.0 || !surf.semi_diameter_mm.is_finite() {
            diagnostics.push(DiagnosticMessage {
                level: DiagnosticLevel::Error,
                code: "ERR_INVALID_SEMI_DIAMETER".to_string(),
                message: format!(
                    "Surface #{idx} semi-diameter must be strictly positive (got {:.4} mm).",
                    surf.semi_diameter_mm
                ),
                surface_index: Some(idx),
                field_index: None,
                recommendation: Some(
                    "Set clear aperture semi-diameter to positive value.".to_string(),
                ),
            });
        }

        // Thickness check
        if idx < num_surfaces - 1 && surf.thickness_mm < 0.0 {
            diagnostics.push(DiagnosticMessage {
                level: DiagnosticLevel::Warning,
                code: "WARN_NEGATIVE_THICKNESS".to_string(),
                message: format!(
                    "Surface #{idx} has negative axial thickness ({:.4} mm).",
                    surf.thickness_mm
                ),
                surface_index: Some(idx),
                field_index: None,
                recommendation: Some("Negative thickness in refractive sequential systems usually indicates backwards propagation without a fold mirror.".to_string()),
            });
        }

        // Curvature sag vs clear aperture domain check
        if surf.radius_mm.is_finite() && surf.radius_mm.abs() > 1e-12 {
            let r_curv = surf.radius_mm.abs();
            if surf.semi_diameter_mm >= r_curv {
                diagnostics.push(DiagnosticMessage {
                    level: DiagnosticLevel::Error,
                    code: "ERR_APERTURE_EXCEEDS_RADIUS".to_string(),
                    message: format!(
                        "Surface #{idx} semi-diameter ({:.2} mm) >= radius of curvature ({:.2} mm); spherical domain exceeded.",
                        surf.semi_diameter_mm, r_curv
                    ),
                    surface_index: Some(idx),
                    field_index: None,
                    recommendation: Some("Reduce clear aperture semi-diameter or increase radius of curvature.".to_string()),
                });
            }
        }

        // Glass Material resolution check
        let mat = surf.material_name.trim().to_uppercase();
        if !mat.is_empty()
            && mat != "AIR"
            && mat != "VACUUM"
            && catalog.lookup(&surf.material_name).is_none()
        {
            diagnostics.push(DiagnosticMessage {
                level: DiagnosticLevel::Warning,
                code: "WARN_UNKNOWN_GLASS".to_string(),
                message: format!(
                    "Surface #{idx} references glass '{}' which is not in the active catalog.",
                    surf.material_name
                ),
                surface_index: Some(idx),
                field_index: None,
                recommendation: Some("Use a standard catalog glass name (e.g. N-BK7, F2, SF11) or import custom AGF.".to_string()),
            });
        }
    }

    // 6. Field Angle Checks
    for (f_idx, &deg) in prescription.field_angles_deg.iter().enumerate() {
        if deg.abs() > 85.0 {
            diagnostics.push(DiagnosticMessage {
                level: DiagnosticLevel::Warning,
                code: "WARN_EXTREME_FIELD_ANGLE".to_string(),
                message: format!(
                    "Field #{f_idx} angle ({:.2}°) is near grazing incidence (> 85°).",
                    deg
                ),
                surface_index: None,
                field_index: Some(f_idx),
                recommendation: Some(
                    "Verify extreme wide-angle field angle is intended.".to_string(),
                ),
            });
        }
    }

    diagnostics
}
