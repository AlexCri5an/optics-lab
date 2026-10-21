//! Optical `.zmx` text format serializer.

use std::fmt::Write;

use optics_core::OpticalPrescription;

use crate::model::FormatOpticalSystem;

/// Serializes a [`FormatOpticalSystem`] into standard `.zmx` format string.
#[must_use]
pub fn write_zmx(system: &FormatOpticalSystem) -> String {
    let mut out = String::new();

    // System Header
    let _ = writeln!(out, "NAME {}", system.name);
    let _ = writeln!(out, "ENPD {:.6}", system.entrance_pupil_diameter_mm);
    let _ = writeln!(out, "FNUM {:.6}", system.f_number);
    let _ = writeln!(out, "PWAV {}", system.primary_wavelength_index + 1);

    if !system.field_type.is_empty() {
        let _ = writeln!(out, "FTYP {}", system.field_type);
    }

    if !system.glass_catalogs.is_empty() {
        let _ = writeln!(out, "GCAT {}", system.glass_catalogs.join(" "));
    }

    // Header extra lines
    for line in &system.extra_lines {
        let _ = writeln!(out, "{line}");
    }

    // Wavelengths
    for (i, &wl) in system.wavelengths_um.iter().enumerate() {
        let weight = system.wavelength_weights.get(i).copied().unwrap_or(1.0);
        let _ = writeln!(out, "WAVL {wl:.7} {weight:.4}");
    }

    // Fields
    if !system.field_angles_deg.is_empty() {
        let field_strs: Vec<String> = system.field_angles_deg.iter().map(|f| format!("{f:.4}")).collect();
        let _ = writeln!(out, "YFLD {}", field_strs.join(" "));
    }

    // Surfaces
    for surf in &system.surfaces {
        let _ = writeln!(out, "SURF {}", surf.index);
        if let Some(ref comment) = surf.comment {
            let _ = writeln!(out, "  COMM {comment}");
        }
        if surf.is_stop {
            let _ = writeln!(out, "  STOP");
        }
        let _ = writeln!(out, "  TYPE {}", surf.surface_type);
        let _ = writeln!(out, "  CURV {:.10}", surf.curvature);

        if surf.thickness_mm.is_infinite() {
            let _ = writeln!(out, "  DISZ INFINITY");
        } else {
            let _ = writeln!(out, "  DISZ {:.6}", surf.thickness_mm);
        }

        if !surf.material_name.is_empty() && !surf.material_name.eq_ignore_ascii_case("AIR") {
            let _ = writeln!(out, "  GLAS {}", surf.material_name);
        }

        let _ = writeln!(out, "  DIAM {:.6}", surf.semi_diameter_mm);

        if surf.conic_constant.abs() > 1e-12 {
            let _ = writeln!(out, "  CONI {:.6}", surf.conic_constant);
        }

        for (idx, &param) in surf.parameters.iter().enumerate() {
            let _ = writeln!(out, "  PARM {} {:.10}", idx + 1, param);
        }

        if let Some(ref ap) = surf.aperture {
            let prefix = if ap.is_obscuration { "OBST" } else { "CLAP" };
            match &ap.shape {
                optics_core::surface::ApertureShape::Circular { radius_mm } => {
                    let _ = writeln!(
                        out,
                        "  {prefix} 0.0 {:.6} 0.0 0.0 {:.6} {:.6}",
                        radius_mm, ap.decenter_x_mm, ap.decenter_y_mm
                    );
                }
                optics_core::surface::ApertureShape::Rectangular {
                    half_width_mm,
                    half_height_mm,
                } => {
                    let _ = writeln!(
                        out,
                        "  {prefix} 0.0 0.0 {:.6} {:.6} {:.6} {:.6}",
                        half_width_mm, half_height_mm, ap.decenter_x_mm, ap.decenter_y_mm
                    );
                }
                optics_core::surface::ApertureShape::Elliptical {
                    semi_axis_x_mm,
                    semi_axis_y_mm,
                } => {
                    let _ = writeln!(
                        out,
                        "  APER ELLI 0.0 0.0 {:.6} {:.6} {:.6} {:.6}",
                        semi_axis_x_mm, semi_axis_y_mm, ap.decenter_x_mm, ap.decenter_y_mm
                    );
                }
                optics_core::surface::ApertureShape::Annular {
                    inner_radius_mm,
                    outer_radius_mm,
                } => {
                    let _ = writeln!(
                        out,
                        "  {prefix} {:.6} {:.6} 0.0 0.0 {:.6} {:.6}",
                        inner_radius_mm, outer_radius_mm, ap.decenter_x_mm, ap.decenter_y_mm
                    );
                }
                optics_core::surface::ApertureShape::Polygonal { .. } => {
                    let max_r = ap.max_radial_extent();
                    let _ = writeln!(
                        out,
                        "  APER POLY 0.0 {:.6} 0.0 0.0 {:.6} {:.6}",
                        max_r, ap.decenter_x_mm, ap.decenter_y_mm
                    );
                }
            }
        }

        if let Some(ref coat) = surf.coating {
            if !surf.extra_lines.iter().any(|l| l.trim_start().starts_with("COAT")) {
                let _ = writeln!(out, "  COAT {}", coat.to_format_name());
            }
        }

        // Surface extra lines
        for line in &surf.extra_lines {
            let _ = writeln!(out, "  {line}");
        }
    }

    out
}

/// Serializes an `optics-core` [`OpticalPrescription`] into standard `.zmx` format string.
#[must_use]
pub fn write_prescription_zmx(prescription: &OpticalPrescription) -> String {
    let system = FormatOpticalSystem::from_optical_prescription(prescription);
    write_zmx(&system)
}
