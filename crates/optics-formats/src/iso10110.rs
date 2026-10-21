//! ISO 10110 Optical Manufacturing Drawing Data Sheet Generator.
//!
//! Generates standardized ISO 10110 optical element drawing data sheets with:
//! - Part 1: Physical dimensions, diameter, thickness, and chamfers
//! - Part 2: `0/` Stress birefringence
//! - Part 3: `1/` Bubbles and inclusions
//! - Part 4: `2/` Inhomogeneity and striae
//! - Part 5: `3/` Surface form power and irregularity tolerances ($N / \Delta N$)
//! - Part 6: `4/` Centering tilt and wedge tolerances ($\sigma$)
//! - Part 7: `5/` Surface imperfection tolerances ($N \times A$)
//! - Part 8: Surface texture ($R_q$ roughness)
//! - Part 9: Surface treatment and AR coatings
//! - Part 10: Tabular data representation (Markdown and JSON formats)

use std::fmt::Write;

use optics_core::OpticalPrescription;
use optics_glass::GlassCatalogDb;
use serde::{Deserialize, Serialize};

/// Standard ISO 10110 optical surface manufacturing specifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Iso10110SurfaceData {
    /// Surface sequential index.
    pub surface_index: usize,
    /// Radius of curvature in millimeters ($R = 1/c$). Planar surfaces have `f64::INFINITY`.
    pub radius_mm: f64,
    /// Permissible radius tolerance in millimeters ($\pm \Delta R$).
    pub radius_tolerance_mm: f64,
    /// Clear aperture diameter in millimeters.
    pub clear_diameter_mm: f64,
    /// ISO 10110-5 Surface form tolerance indicator: `3/ N(Delta N)` (Power / Irregularity fringes).
    pub form_tolerance: String,
    /// ISO 10110-6 Centering tilt / wedge angle tolerance indicator: `4/ sigma` (arcmin).
    pub centering_tolerance: String,
    /// ISO 10110-7 Surface imperfection tolerance indicator: `5/ N x A`.
    pub imperfection_tolerance: String,
    /// ISO 10110-8 Surface texture / roughness ($R_q$ in $\mu\text{m}$).
    pub surface_texture_rq_um: f64,
    /// ISO 10110-9 Coating designation (e.g. "AR R < 0.5% @ 450-650 nm" or "Uncoated").
    pub coating: String,
    /// Protective edge chamfer / bevel in millimeters (e.g. "0.3 x 45 deg").
    pub protective_chamfer: String,
}

/// Standard ISO 10110 bulk optical material specifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Iso10110MaterialData {
    /// Glass material designation (e.g. "N-BK7", "F2", "Fused Silica").
    pub glass_name: String,
    /// Principal refractive index $n_d$ at 587.56 nm.
    pub nd: f64,
    /// Principal Abbe number $V_d$.
    pub vd: f64,
    /// ISO 10110-2 Stress birefringence indicator: `0/ OPD` (nm/cm).
    pub stress_birefringence: String,
    /// ISO 10110-3 Bubbles and inclusions indicator: `1/ N x A`.
    pub bubbles_inclusions: String,
    /// ISO 10110-4 Inhomogeneity and striae class indicator: `2/ class; striae`.
    pub striae_class: String,
}

/// A complete ISO 10110 lens element drawing data sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Iso10110LensElement {
    /// Element sequential index (1-based).
    pub element_index: usize,
    /// Front optical surface data.
    pub front_surface: Iso10110SurfaceData,
    /// Rear optical surface data.
    pub rear_surface: Iso10110SurfaceData,
    /// Bulk material specifications.
    pub material: Iso10110MaterialData,
    /// Center thickness in millimeters.
    pub center_thickness_mm: f64,
    /// Center thickness tolerance in millimeters ($\pm \Delta t$).
    pub center_thickness_tolerance_mm: f64,
    /// Outer mechanical diameter in millimeters.
    pub mechanical_diameter_mm: f64,
    /// Mechanical diameter tolerance in millimeters (lower, upper).
    pub mechanical_diameter_tolerance_mm: (f64, f64),
}

/// Complete collection of ISO 10110 lens manufacturing drawings for an optical system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Iso10110DrawingSheet {
    /// Design project or assembly title.
    pub title: String,
    /// Individual optical lens elements.
    pub elements: Vec<Iso10110LensElement>,
}

impl Iso10110DrawingSheet {
    /// Derives an ISO 10110 manufacturing drawing sheet from an [`OpticalPrescription`].
    ///
    /// Automatically applies standard precision optical manufacturing tolerances:
    /// - Form: `3/ 3(0.5)` (~$\lambda/4$ irregularity)
    /// - Centering: `4/ 1.5'`
    /// - Imperfections: `5/ 3*0.16` (scratch/dig 40-20 equivalent)
    /// - Roughness: $R_q \le 0.005\,\mu\text{m}$ (optical polish)
    /// - Diameter: $+0.00 / -0.05$ mm
    /// - Thickness: $\pm 0.05$ mm
    #[must_use]
    pub fn from_prescription(prescription: &OpticalPrescription, catalog: Option<&GlassCatalogDb>) -> Self {
        let mut elements = Vec::new();
        let num_surfaces = prescription.surfaces.len();
        let mut element_idx = 1_usize;

        let mut i = 0;
        while i < num_surfaces {
            let surf = &prescription.surfaces[i];
            let mat = surf.material_name.trim();
            let is_glass = !mat.is_empty() && !mat.eq_ignore_ascii_case("air");

            if is_glass && i + 1 < num_surfaces {
                let back_surf = &prescription.surfaces[i + 1];

                let (nd, vd) = if let Some(db) = catalog {
                    if let Some(entry) = db.lookup(mat) {
                        (entry.nd, entry.vd)
                    } else {
                        (1.5168, 64.17)
                    }
                } else {
                    (1.5168, 64.17)
                };

                let clear_diam_f = surf.semi_diameter_mm * 2.0;
                let clear_diam_b = back_surf.semi_diameter_mm * 2.0;
                let mech_diam = clear_diam_f.max(clear_diam_b) + 1.0;

                let front_surface = Iso10110SurfaceData {
                    surface_index: surf.surface_index,
                    radius_mm: surf.radius_mm,
                    radius_tolerance_mm: if surf.radius_mm.is_infinite() {
                        0.0
                    } else {
                        (surf.radius_mm * 0.001).abs().max(0.02)
                    },
                    clear_diameter_mm: clear_diam_f,
                    form_tolerance: "3/ 3(0.5)".to_string(),
                    centering_tolerance: "4/ 1.5'".to_string(),
                    imperfection_tolerance: "5/ 3*0.16".to_string(),
                    surface_texture_rq_um: 0.005,
                    coating: "AR R < 0.5% @ 400-700nm".to_string(),
                    protective_chamfer: "0.3 x 45 deg".to_string(),
                };

                let rear_surface = Iso10110SurfaceData {
                    surface_index: back_surf.surface_index,
                    radius_mm: back_surf.radius_mm,
                    radius_tolerance_mm: if back_surf.radius_mm.is_infinite() {
                        0.0
                    } else {
                        (back_surf.radius_mm * 0.001).abs().max(0.02)
                    },
                    clear_diameter_mm: clear_diam_b,
                    form_tolerance: "3/ 3(0.5)".to_string(),
                    centering_tolerance: "4/ 1.5'".to_string(),
                    imperfection_tolerance: "5/ 3*0.16".to_string(),
                    surface_texture_rq_um: 0.005,
                    coating: "AR R < 0.5% @ 400-700nm".to_string(),
                    protective_chamfer: "0.3 x 45 deg".to_string(),
                };

                let material = Iso10110MaterialData {
                    glass_name: mat.to_string(),
                    nd,
                    vd,
                    stress_birefringence: "0/ 10".to_string(),
                    bubbles_inclusions: "1/ 2*0.1".to_string(),
                    striae_class: "2/ 1; 2".to_string(),
                };

                elements.push(Iso10110LensElement {
                    element_index: element_idx,
                    front_surface,
                    rear_surface,
                    material,
                    center_thickness_mm: surf.thickness_mm,
                    center_thickness_tolerance_mm: 0.05,
                    mechanical_diameter_mm: mech_diam,
                    mechanical_diameter_tolerance_mm: (-0.05, 0.00),
                });

                element_idx += 1;
                i += 2;
            } else {
                i += 1;
            }
        }

        Self {
            title: prescription.name.clone(),
            elements,
        }
    }

    /// Generates a standardized ISO 10110 manufacturing data table in GitHub-flavored Markdown.
    #[must_use]
    pub fn to_markdown_table(&self) -> String {
        let mut md = String::new();

        let _ = writeln!(md, "# ISO 10110 Manufacturing Drawing Data Sheet");
        let _ = writeln!(md, "**Optical Assembly**: {}\n", self.title);

        for el in &self.elements {
            let _ = writeln!(md, "## Element #{}: {}", el.element_index, el.material.glass_name);
            let _ = writeln!(
                md,
                "- **Glass**: {} ($n_d = {:.4}$, $V_d = {:.2}$)",
                el.material.glass_name, el.material.nd, el.material.vd
            );
            let _ = writeln!(
                md,
                "- **Center Thickness**: {:.3} mm $\\pm$ {:.3} mm",
                el.center_thickness_mm, el.center_thickness_tolerance_mm
            );
            let _ = writeln!(
                md,
                "- **Mechanical Diameter**: {:.2} mm ({:+.2}/{:+.2} mm)",
                el.mechanical_diameter_mm, el.mechanical_diameter_tolerance_mm.0, el.mechanical_diameter_tolerance_mm.1
            );
            let _ = writeln!(
                md,
                "- **Material Tolerances**: `{}` (Birefringence), `{}` (Inclusions), `{}` (Striae)\n",
                el.material.stress_birefringence, el.material.bubbles_inclusions, el.material.striae_class
            );

            // Table of surfaces
            let _ = writeln!(
                md,
                "| Surface | Radius (mm) | Clear Diam (mm) | Form 3/ | Tilt 4/ | Defects 5/ | Texture Rq | Coating | Bevel |"
            );
            let _ = writeln!(md, "| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |");

            for (label, surf) in [("Front (S1)", &el.front_surface), ("Rear (S2)", &el.rear_surface)] {
                let r_str = if surf.radius_mm.is_infinite() {
                    "Planar".to_string()
                } else {
                    format!("{:.3} (±{:.3})", surf.radius_mm, surf.radius_tolerance_mm)
                };

                let _ = writeln!(
                    md,
                    "| {} | {} | {:.2} | `{}` | `{}` | `{}` | {:.3} µm | {} | {} |",
                    label,
                    r_str,
                    surf.clear_diameter_mm,
                    surf.form_tolerance,
                    surf.centering_tolerance,
                    surf.imperfection_tolerance,
                    surf.surface_texture_rq_um,
                    surf.coating,
                    surf.protective_chamfer
                );
            }
            let _ = writeln!(md);
        }

        md
    }

    /// Serializes the ISO 10110 drawing sheet to pretty JSON string.
    ///
    /// # Errors
    /// Returns error string if serialization fails.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::OpticalSurface;

    #[test]
    fn test_iso10110_drawing_generation() {
        let prescription = OpticalPrescription {
            name: "Test Doublet".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 4.0,
                    semi_diameter_mm: 10.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: -50.0,
                    conic_constant: 0.0,
                    thickness_mm: 80.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        };

        let sheet = Iso10110DrawingSheet::from_prescription(&prescription, None);
        assert_eq!(sheet.elements.len(), 1);
        let el = &sheet.elements[0];
        assert_eq!(el.material.glass_name, "N-BK7");
        assert_eq!(el.front_surface.radius_mm, 50.0);
        assert_eq!(el.rear_surface.radius_mm, -50.0);
        assert_eq!(el.center_thickness_mm, 4.0);

        let md = sheet.to_markdown_table();
        assert!(md.contains("ISO 10110 Manufacturing Drawing Data Sheet"));
        assert!(md.contains("Element #1: N-BK7"));
        assert!(md.contains("3/ 3(0.5)"));
        assert!(md.contains("4/ 1.5'"));
        assert!(md.contains("5/ 3*0.16"));

        let json = sheet.to_json().unwrap_or_default();
        assert!(json.contains("Test Doublet"));
        assert!(json.contains("protective_chamfer"));
    }
}
