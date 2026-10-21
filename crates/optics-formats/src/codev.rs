//! CODE V `.seq` sequential prescription parser and serializer.
//!
//! Provides bidirectional conversion between Synopsys CODE V macro prescription files
//! and [`optics_core::OpticalPrescription`].
//!
//! # Supported Commands
//! - System: `TIT`, `EPD`, `DIM`, `WL`, `WTW`, `REF`, `XFL`, `YFL`, `GO`
//! - Surfaces: `SO`, `S`, `SI`, `RDY`, `CUY`, `THI`, `GLA`, `CCY`, `STO`, `SD`, `CIR`

use std::fmt::Write;

use optics_core::{OpticalPrescription, OpticalSurface};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when parsing a CODE V `.seq` file.
#[derive(Debug, Error, PartialEq)]
pub enum CodeVParseError {
    /// Line syntax or token error.
    #[error("Line {line}: missing value for CODE V command '{command}'")]
    MissingValue {
        /// 1-based line number.
        line: usize,
        /// Command name.
        command: String,
    },
    /// Invalid numeric value.
    #[error("Line {line}: invalid floating-point value '{token}' for '{command}'")]
    InvalidFloat {
        /// 1-based line number.
        line: usize,
        /// Command name.
        command: String,
        /// Offending token.
        token: String,
    },
    /// Invalid integer value.
    #[error("Line {line}: invalid integer value '{token}' for '{command}'")]
    InvalidInteger {
        /// 1-based line number.
        line: usize,
        /// Command name.
        command: String,
        /// Offending token.
        token: String,
    },
    /// No surfaces parsed.
    #[error("No optical surfaces found in CODE V file")]
    NoSurfaces,
}

/// A surface in a CODE V optical system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeVSurface {
    /// 0-based surface index (0 = Object, 1..N-1 = optics, N = Image).
    pub index: usize,
    /// Vertex radius of curvature in millimeters ($R = 1/c$). Planar surfaces have `f64::INFINITY`.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub radius_mm: f64,
    /// Axial thickness / spacing to next surface in millimeters.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub thickness_mm: f64,
    /// Glass material name or model specification. Empty for air/vacuum.
    pub glass: String,
    /// Conic constant ($k = 0$ for sphere, $-1$ for paraboloid).
    pub conic: f64,
    /// Clear semi-diameter in millimeters.
    pub semi_diameter_mm: f64,
    /// Whether this surface is the aperture stop (`STO`).
    pub is_stop: bool,
    /// Optional surface note or comment.
    pub comment: Option<String>,
    /// Preserved unparsed commands for this surface.
    pub extra_lines: Vec<String>,
}

impl Default for CodeVSurface {
    fn default() -> Self {
        Self {
            index: 0,
            radius_mm: f64::INFINITY,
            thickness_mm: 0.0,
            glass: String::new(),
            conic: 0.0,
            semi_diameter_mm: 10.0,
            is_stop: false,
            comment: None,
            extra_lines: Vec::new(),
        }
    }
}

/// A parsed CODE V optical system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeVSystem {
    /// System title (`TIT`).
    pub title: String,
    /// Entrance pupil diameter in millimeters (`EPD`).
    pub entrance_pupil_diameter_mm: f64,
    /// Dimensional unit (M = mm, C = cm, I = inches).
    pub dim_unit: String,
    /// Evaluated spectral wavelengths in nanometers (`WL`).
    pub wavelengths_nm: Vec<f64>,
    /// Spectral weights (`WTW`).
    pub wavelength_weights: Vec<f64>,
    /// 0-based index of primary/reference wavelength (`REF` - 1).
    pub primary_wavelength_index: usize,
    /// Field angles in degrees (`YFL` or `XFL`).
    pub field_angles_deg: Vec<f64>,
    /// Sequential surfaces (Object `SO` to Image `SI`).
    pub surfaces: Vec<CodeVSurface>,
    /// Preserved unparsed system-level lines.
    pub extra_lines: Vec<String>,
}

impl Default for CodeVSystem {
    fn default() -> Self {
        Self {
            title: "Untitled CODE V Lens".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            dim_unit: "M".to_string(),
            wavelengths_nm: vec![587.56],
            wavelength_weights: vec![1.0],
            primary_wavelength_index: 0,
            field_angles_deg: vec![0.0],
            surfaces: Vec::new(),
            extra_lines: Vec::new(),
        }
    }
}

/// Helper function to parse tokens while respecting single- or double-quoted strings.
fn tokenize_codev_line(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in line.chars() {
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            in_quote = Some(ch);
        } else if ch.is_whitespace() || ch == ';' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Parses a CODE V `.seq` string into a [`CodeVSystem`].
///
/// # Errors
/// Returns [`CodeVParseError`] if required operand values are missing or malformed.
pub fn parse_seq(content: &str) -> Result<CodeVSystem, CodeVParseError> {
    let mut system = CodeVSystem {
        wavelengths_nm: Vec::new(),
        wavelength_weights: Vec::new(),
        field_angles_deg: Vec::new(),
        surfaces: Vec::new(),
        ..Default::default()
    };

    let mut scale_to_mm = 1.0_f64;
    let mut current_surface: Option<CodeVSurface> = None;
    let mut current_index = 0_usize;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Comments start with ! or # or C or CC or REM
        if trimmed.is_empty() || trimmed.starts_with('!') || trimmed.starts_with('#') || trimmed.starts_with("/*") {
            if let Some(ref mut s) = current_surface {
                s.extra_lines.push(trimmed.to_string());
            } else {
                system.extra_lines.push(trimmed.to_string());
            }
            continue;
        }

        let tokens = tokenize_codev_line(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let cmd = tokens[0].to_uppercase();

        match cmd.as_str() {
            "TIT" | "TITLE" => {
                if tokens.len() > 1 {
                    system.title = tokens[1..].join(" ");
                }
            }
            "EPD" => {
                let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                    line: line_num,
                    command: "EPD".to_string(),
                })?;
                let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                    line: line_num,
                    command: "EPD".to_string(),
                    token: tok.clone(),
                })?;
                system.entrance_pupil_diameter_mm = val * scale_to_mm;
            }
            "DIM" => {
                if let Some(unit_tok) = tokens.get(1) {
                    let u = unit_tok.to_uppercase();
                    match u.as_str() {
                        "C" | "CM" => {
                            system.dim_unit = "C".to_string();
                            scale_to_mm = 10.0;
                        }
                        "I" | "IN" | "INCH" => {
                            system.dim_unit = "I".to_string();
                            scale_to_mm = 25.4;
                        }
                        _ => {
                            system.dim_unit = "M".to_string();
                            scale_to_mm = 1.0;
                        }
                    }
                }
            }
            "WL" => {
                for wl_tok in &tokens[1..] {
                    if let Ok(wl) = wl_tok.parse::<f64>() {
                        // CODE V wavelengths are typically in nanometers
                        let wl_nm = if wl < 10.0 { wl * 1000.0 } else { wl };
                        system.wavelengths_nm.push(wl_nm);
                    }
                }
            }
            "WTW" => {
                for wt_tok in &tokens[1..] {
                    if let Ok(wt) = wt_tok.parse::<f64>() {
                        system.wavelength_weights.push(wt);
                    }
                }
            }
            "REF" => {
                if let Some(ref_tok) = tokens.get(1) {
                    if let Ok(ref_idx) = ref_tok.parse::<usize>() {
                        system.primary_wavelength_index = ref_idx.saturating_sub(1);
                    }
                }
            }
            "YFL" | "XFL" => {
                for f_tok in &tokens[1..] {
                    if let Ok(f_val) = f_tok.parse::<f64>() {
                        system.field_angles_deg.push(f_val);
                    }
                }
            }
            "SO" => {
                // Surface Object (Index 0)
                if let Some(s) = current_surface.take() {
                    system.surfaces.push(s);
                }
                current_index = 0;
                let mut surf = CodeVSurface {
                    index: 0,
                    ..Default::default()
                };
                if let Some(t_tok) = tokens.get(1) {
                    if let Ok(t_val) = t_tok.parse::<f64>() {
                        surf.thickness_mm = t_val * scale_to_mm;
                    }
                }
                current_surface = Some(surf);
            }
            "S" => {
                if let Some(s) = current_surface.take() {
                    system.surfaces.push(s);
                }
                let idx = if let Some(idx_tok) = tokens.get(1) {
                    idx_tok.parse::<usize>().unwrap_or(current_index + 1)
                } else {
                    current_index + 1
                };
                current_index = idx;
                current_surface = Some(CodeVSurface {
                    index: idx,
                    ..Default::default()
                });
            }
            "SI" => {
                // Surface Image
                if let Some(s) = current_surface.take() {
                    system.surfaces.push(s);
                }
                current_index += 1;
                current_surface = Some(CodeVSurface {
                    index: current_index,
                    comment: Some("Image Plane".to_string()),
                    ..Default::default()
                });
            }
            "RDY" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                        line: line_num,
                        command: "RDY".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                        line: line_num,
                        command: "RDY".to_string(),
                        token: tok.clone(),
                    })?;
                    s.radius_mm = if val.abs() < 1e-12 {
                        f64::INFINITY
                    } else {
                        val * scale_to_mm
                    };
                }
            }
            "CUY" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                        line: line_num,
                        command: "CUY".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                        line: line_num,
                        command: "CUY".to_string(),
                        token: tok.clone(),
                    })?;
                    s.radius_mm = if val.abs() < 1e-12 {
                        f64::INFINITY
                    } else {
                        (1.0 / val) * scale_to_mm
                    };
                }
            }
            "THI" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                        line: line_num,
                        command: "THI".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                        line: line_num,
                        command: "THI".to_string(),
                        token: tok.clone(),
                    })?;
                    s.thickness_mm = val * scale_to_mm;
                }
            }
            "GLA" => {
                if let Some(ref mut s) = current_surface {
                    if let Some(mat_tok) = tokens.get(1) {
                        let trimmed_mat = mat_tok.trim_matches('\'').trim_matches('"');
                        if !trimmed_mat.eq_ignore_ascii_case("AIR") {
                            s.glass = trimmed_mat.to_string();
                        }
                    }
                }
            }
            "CCY" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                        line: line_num,
                        command: "CCY".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                        line: line_num,
                        command: "CCY".to_string(),
                        token: tok.clone(),
                    })?;
                    s.conic = val;
                }
            }
            "STO" => {
                if let Some(ref mut s) = current_surface {
                    s.is_stop = true;
                }
            }
            "SD" | "CIR" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| CodeVParseError::MissingValue {
                        line: line_num,
                        command: cmd.clone(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| CodeVParseError::InvalidFloat {
                        line: line_num,
                        command: cmd.clone(),
                        token: tok.clone(),
                    })?;
                    s.semi_diameter_mm = val * scale_to_mm;
                }
            }
            "GO" => {
                // End of lens data
                break;
            }
            _ => {
                // Record extra line for current surface or system
                if let Some(ref mut s) = current_surface {
                    s.extra_lines.push(trimmed.to_string());
                } else {
                    system.extra_lines.push(trimmed.to_string());
                }
            }
        }
    }

    if let Some(s) = current_surface {
        system.surfaces.push(s);
    }

    if system.surfaces.is_empty() {
        return Err(CodeVParseError::NoSurfaces);
    }

    if system.wavelengths_nm.is_empty() {
        system.wavelengths_nm.push(587.56);
        system.wavelength_weights.push(1.0);
    }

    if system.field_angles_deg.is_empty() {
        system.field_angles_deg.push(0.0);
    }

    Ok(system)
}

/// Serializes a [`CodeVSystem`] into standard CODE V `.seq` string format.
#[must_use]
pub fn write_seq(system: &CodeVSystem) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "TIT '{}'", system.title);
    let _ = writeln!(out, "EPD {:.6}", system.entrance_pupil_diameter_mm);
    let _ = writeln!(out, "DIM {}", system.dim_unit);

    if !system.wavelengths_nm.is_empty() {
        let wl_strs: Vec<String> = system.wavelengths_nm.iter().map(|w| format!("{w:.4}")).collect();
        let _ = writeln!(out, "WL {}", wl_strs.join(" "));

        let wt_strs: Vec<String> = system.wavelength_weights.iter().map(|w| format!("{w:.4}")).collect();
        if !wt_strs.is_empty() {
            let _ = writeln!(out, "WTW {}", wt_strs.join(" "));
        }
        let _ = writeln!(out, "REF {}", system.primary_wavelength_index + 1);
    }

    if !system.field_angles_deg.is_empty() {
        let f_strs: Vec<String> = system.field_angles_deg.iter().map(|f| format!("{f:.4}")).collect();
        let _ = writeln!(out, "YFL {}", f_strs.join(" "));
    }

    // System extra lines
    for line in &system.extra_lines {
        let _ = writeln!(out, "{line}");
    }

    // Surfaces
    for (i, surf) in system.surfaces.iter().enumerate() {
        if i == 0 {
            let _ = writeln!(out, "SO {:.6}", surf.thickness_mm);
        } else if i == system.surfaces.len().saturating_sub(1) {
            let _ = writeln!(out, "SI");
        } else {
            let _ = writeln!(out, "S {}", surf.index);
        }

        if let Some(ref comm) = surf.comment {
            let _ = writeln!(out, "  ! {comm}");
        }

        if surf.is_stop {
            let _ = writeln!(out, "  STO");
        }

        if surf.radius_mm.is_infinite() {
            let _ = writeln!(out, "  RDY 0.000000");
        } else {
            let _ = writeln!(out, "  RDY {:.6}", surf.radius_mm);
        }

        let _ = writeln!(out, "  THI {:.6}", surf.thickness_mm);

        if !surf.glass.is_empty() && !surf.glass.eq_ignore_ascii_case("AIR") {
            let _ = writeln!(out, "  GLA '{}'", surf.glass);
        }

        if surf.conic.abs() > 1e-12 {
            let _ = writeln!(out, "  CCY {:.6}", surf.conic);
        }

        let _ = writeln!(out, "  SD {:.6}", surf.semi_diameter_mm);

        for extra in &surf.extra_lines {
            let _ = writeln!(out, "  {extra}");
        }
    }

    let _ = writeln!(out, "GO");
    out
}

impl CodeVSystem {
    /// Converts this [`CodeVSystem`] into an `optics-core` [`OpticalPrescription`].
    #[must_use]
    pub fn to_optical_prescription(&self) -> OpticalPrescription {
        let has_optical_surfaces = self.surfaces.iter().any(|s| s.index > 0);
        let optical_surfs: Vec<&CodeVSurface> = if has_optical_surfaces {
            self.surfaces.iter().filter(|s| s.index > 0).collect()
        } else {
            self.surfaces.iter().collect()
        };

        let surfaces: Vec<OpticalSurface> = optical_surfs
            .into_iter()
            .map(|s| OpticalSurface {
                surface_index: s.index,
                radius_mm: s.radius_mm,
                conic_constant: s.conic,
                thickness_mm: s.thickness_mm,
                semi_diameter_mm: s.semi_diameter_mm,
                material_name: if s.glass.is_empty() {
                    "AIR".to_string()
                } else {
                    s.glass.clone()
                },
                is_stop: s.is_stop,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
                profile: None,
                aperture: None,
                coating: None,
            })
            .collect();

        OpticalPrescription {
            name: self.title.clone(),
            entrance_pupil_diameter_mm: self.entrance_pupil_diameter_mm,
            field_angles_deg: if self.field_angles_deg.is_empty() {
                vec![0.0]
            } else {
                self.field_angles_deg.clone()
            },
            wavelengths_nm: if self.wavelengths_nm.is_empty() {
                vec![587.56]
            } else {
                self.wavelengths_nm.clone()
            },
            primary_wavelength_index: self
                .primary_wavelength_index
                .min(self.wavelengths_nm.len().saturating_sub(1)),
            surfaces,
        }
    }

    /// Converts an `optics-core` [`OpticalPrescription`] into a [`CodeVSystem`].
    #[must_use]
    pub fn from_optical_prescription(prescription: &OpticalPrescription) -> Self {
        let mut surfaces = Vec::with_capacity(prescription.surfaces.len() + 1);

        // Object surface at index 0
        surfaces.push(CodeVSurface {
            index: 0,
            radius_mm: f64::INFINITY,
            thickness_mm: 0.0,
            glass: "AIR".to_string(),
            conic: 0.0,
            semi_diameter_mm: prescription.entrance_pupil_diameter_mm * 0.5,
            is_stop: false,
            comment: Some("Object Plane".to_string()),
            extra_lines: Vec::new(),
        });

        for surf in &prescription.surfaces {
            surfaces.push(CodeVSurface {
                index: surf.surface_index,
                radius_mm: surf.radius_mm,
                thickness_mm: surf.thickness_mm,
                glass: surf.material_name.clone(),
                conic: surf.conic_constant,
                semi_diameter_mm: surf.semi_diameter_mm,
                is_stop: surf.is_stop,
                comment: None,
                extra_lines: Vec::new(),
            });
        }

        let weights = vec![1.0; prescription.wavelengths_nm.len()];

        Self {
            title: prescription.name.clone(),
            entrance_pupil_diameter_mm: prescription.entrance_pupil_diameter_mm,
            dim_unit: "M".to_string(),
            wavelengths_nm: prescription.wavelengths_nm.clone(),
            wavelength_weights: weights,
            primary_wavelength_index: prescription.primary_wavelength_index,
            field_angles_deg: prescription.field_angles_deg.clone(),
            surfaces,
            extra_lines: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_codev_parse_cooke_triplet() -> TestResult {
        let seq_data = r#"
TIT 'Cooke Triplet F/4.5'
EPD 10.000000
DIM M
WL 656.27 587.56 486.13
WTW 1.0 1.0 1.0
REF 2
YFL 0.0 10.0 20.0
SO 0.0
S 1
  RDY 22.01359
  THI 3.25896
  GLA 'SK16'
  SD 8.0
S 2
  RDY -435.76000
  THI 6.00754
  SD 8.0
S 3
  RDY -22.21328
  THI 0.99974
  GLA 'F2'
  SD 5.5
S 4
  STO
  RDY 20.29192
  THI 4.74971
  SD 5.0
S 5
  RDY 79.68360
  THI 2.95298
  GLA 'SK16'
  SD 7.5
S 6
  RDY -18.39533
  THI 42.20778
  SD 7.5
SI
  RDY 0.0
  THI 0.0
  SD 18.0
GO
"#;

        let system = parse_seq(seq_data)?;
        assert_eq!(system.title, "Cooke Triplet F/4.5");
        assert!((system.entrance_pupil_diameter_mm - 10.0).abs() < 1e-6);
        assert_eq!(system.wavelengths_nm.len(), 3);
        assert_eq!(system.primary_wavelength_index, 1);
        assert_eq!(system.surfaces.len(), 8); // SO, S1..S6, SI

        // Check Surface 1
        assert_eq!(system.surfaces[1].glass, "SK16");
        assert!((system.surfaces[1].radius_mm - 22.01359).abs() < 1e-4);
        assert!((system.surfaces[1].thickness_mm - 3.25896).abs() < 1e-4);

        // Check Stop
        assert!(system.surfaces[4].is_stop);

        // Roundtrip serialization
        let written = write_seq(&system);
        let reparsed = parse_seq(&written)?;

        assert_eq!(system.title, reparsed.title);
        assert_eq!(system.surfaces.len(), reparsed.surfaces.len());
        for (s1, s2) in system.surfaces.iter().zip(reparsed.surfaces.iter()) {
            assert_eq!(s1.index, s2.index);
            assert_eq!(s1.is_stop, s2.is_stop);
            assert_eq!(s1.glass, s2.glass);
            if s1.radius_mm.is_infinite() {
                assert!(s2.radius_mm.is_infinite());
            } else {
                assert!((s1.radius_mm - s2.radius_mm).abs() < 1e-4);
            }
        }

        // Test conversion to OpticalPrescription
        let prescription = system.to_optical_prescription();
        assert_eq!(prescription.name, "Cooke Triplet F/4.5");
        assert_eq!(prescription.surfaces.len(), 7); // Excludes Object surface
        assert_eq!(prescription.surfaces[0].material_name, "SK16");

        Ok(())
    }
}
