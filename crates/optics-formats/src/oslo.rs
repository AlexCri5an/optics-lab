//! OSLO `.len` sequential prescription parser and serializer.
//!
//! Provides bidirectional conversion between Lambda Research OSLO prescription files
//! and [`optics_core::OpticalPrescription`].
//!
//! # Supported Commands
//! - System: `LEN`, `ENT`, `WV`, `WW`, `PK`, `ANG`, `FIELD`
//! - Surfaces: `SURF`, `IMAGE`, `RD`, `TH`, `AP`, `GLAS`, `AIR`, `AST`, `CC`

use std::fmt::Write;

use optics_core::{OpticalPrescription, OpticalSurface};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when parsing an OSLO `.len` file.
#[derive(Debug, Error, PartialEq)]
pub enum OsloParseError {
    /// Required operand value missing.
    #[error("Line {line}: missing value for OSLO command '{command}'")]
    MissingValue {
        /// 1-based line number.
        line: usize,
        /// Command name.
        command: String,
    },
    /// Invalid floating-point value.
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
    /// No surfaces found.
    #[error("No optical surfaces found in OSLO file")]
    NoSurfaces,
}

/// A single surface in an OSLO lens prescription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OsloSurface {
    /// 0-based surface index (0 = Object, 1..N-1 = optical elements, N = Image).
    pub index: usize,
    /// Radius of curvature in millimeters ($R = 1/c$). Planar surfaces have `f64::INFINITY`.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub radius_mm: f64,
    /// Axial thickness to next surface in millimeters.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub thickness_mm: f64,
    /// Aperture clear semi-diameter in millimeters (`AP`).
    pub aperture_radius_mm: f64,
    /// Glass material name or empty for air (`GLAS` or `AIR`).
    pub glass: String,
    /// Conic constant (`CC`).
    pub conic: f64,
    /// Whether this surface is the aperture stop (`AST`).
    pub is_stop: bool,
    /// Optional surface note or comment.
    pub comment: Option<String>,
    /// Preserved unparsed commands for this surface.
    pub extra_lines: Vec<String>,
}

impl Default for OsloSurface {
    fn default() -> Self {
        Self {
            index: 0,
            radius_mm: f64::INFINITY,
            thickness_mm: 0.0,
            aperture_radius_mm: 10.0,
            glass: String::new(),
            conic: 0.0,
            is_stop: false,
            comment: None,
            extra_lines: Vec::new(),
        }
    }
}

/// A parsed OSLO optical system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OsloSystem {
    /// System title (`LEN NEW "Title"`).
    pub title: String,
    /// Design focal length in millimeters.
    pub design_efl_mm: f64,
    /// Entrance beam radius in millimeters (`ENT`).
    pub entrance_beam_radius_mm: f64,
    /// Evaluated spectral wavelengths in micrometers (`WV`).
    pub wavelengths_um: Vec<f64>,
    /// Relative wavelength weights (`WW`).
    pub wavelength_weights: Vec<f64>,
    /// 0-based index of primary wavelength reference (`PK` - 1).
    pub primary_wavelength_index: usize,
    /// Field angles in degrees (`ANG`).
    pub field_angles_deg: Vec<f64>,
    /// Sequential surfaces (Object `SURF 0` to `IMAGE`).
    pub surfaces: Vec<OsloSurface>,
    /// Preserved unparsed system-level lines.
    pub extra_lines: Vec<String>,
}

impl Default for OsloSystem {
    fn default() -> Self {
        Self {
            title: "Untitled OSLO System".to_string(),
            design_efl_mm: 50.0,
            entrance_beam_radius_mm: 5.0,
            wavelengths_um: vec![0.5875618],
            wavelength_weights: vec![1.0],
            primary_wavelength_index: 0,
            field_angles_deg: vec![0.0],
            surfaces: Vec::new(),
            extra_lines: Vec::new(),
        }
    }
}

/// Helper function to parse tokens while respecting quoted strings.
fn tokenize_oslo_line(line: &str) -> Vec<String> {
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
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch.is_whitespace() || ch == ',' {
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

/// Parses an OSLO `.len` prescription file from string content.
///
/// # Errors
/// Returns [`OsloParseError`] if required values cannot be parsed.
pub fn parse_len(content: &str) -> Result<OsloSystem, OsloParseError> {
    let mut system = OsloSystem {
        wavelengths_um: Vec::new(),
        wavelength_weights: Vec::new(),
        field_angles_deg: Vec::new(),
        surfaces: Vec::new(),
        ..Default::default()
    };

    let mut current_surface: Option<OsloSurface> = None;
    let mut current_index = 0_usize;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Skip comments or preserve them
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('!')
            || trimmed.starts_with('#')
            || trimmed.starts_with('*')
        {
            if let Some(ref mut s) = current_surface {
                s.extra_lines.push(trimmed.to_string());
            } else {
                system.extra_lines.push(trimmed.to_string());
            }
            continue;
        }

        let tokens = tokenize_oslo_line(trimmed);
        if tokens.is_empty() {
            continue;
        }

        let cmd = tokens[0].to_uppercase();

        match cmd.as_str() {
            "LEN" => {
                // LEN NEW "Name" [EFL] [Surfaces]
                let mut title_idx = 1;
                if tokens.get(1).is_some_and(|t| t.eq_ignore_ascii_case("NEW")) {
                    title_idx = 2;
                }
                if let Some(name_tok) = tokens.get(title_idx) {
                    system.title = name_tok.clone();
                }
                if let Some(efl_tok) = tokens.get(title_idx + 1) {
                    if let Ok(efl) = efl_tok.parse::<f64>() {
                        system.design_efl_mm = efl;
                    }
                }
            }
            "ENT" => {
                let tok = tokens.get(1).ok_or_else(|| OsloParseError::MissingValue {
                    line: line_num,
                    command: "ENT".to_string(),
                })?;
                let val: f64 = tok.parse().map_err(|_| OsloParseError::InvalidFloat {
                    line: line_num,
                    command: "ENT".to_string(),
                    token: tok.clone(),
                })?;
                system.entrance_beam_radius_mm = val;
            }
            "WV" => {
                for wl_tok in &tokens[1..] {
                    if let Ok(wl) = wl_tok.parse::<f64>() {
                        // In OSLO, wavelengths are in micrometers
                        system.wavelengths_um.push(wl);
                    }
                }
            }
            "WW" => {
                for wt_tok in &tokens[1..] {
                    if let Ok(wt) = wt_tok.parse::<f64>() {
                        system.wavelength_weights.push(wt);
                    }
                }
            }
            "PK" => {
                if let Some(pk_tok) = tokens.get(1) {
                    if let Ok(pk) = pk_tok.parse::<usize>() {
                        system.primary_wavelength_index = pk.saturating_sub(1);
                    }
                }
            }
            "ANG" | "FIELD" => {
                for ang_tok in &tokens[1..] {
                    if let Ok(ang) = ang_tok.parse::<f64>() {
                        system.field_angles_deg.push(ang);
                    }
                }
            }
            "SURF" => {
                if let Some(s) = current_surface.take() {
                    system.surfaces.push(s);
                }
                let idx = if let Some(idx_tok) = tokens.get(1) {
                    idx_tok.parse::<usize>().unwrap_or(current_index + 1)
                } else {
                    current_index + 1
                };
                current_index = idx;
                current_surface = Some(OsloSurface {
                    index: idx,
                    ..Default::default()
                });
            }
            "IMAGE" | "IMS" => {
                if let Some(s) = current_surface.take() {
                    system.surfaces.push(s);
                }
                current_index += 1;
                current_surface = Some(OsloSurface {
                    index: current_index,
                    comment: Some("Image Plane".to_string()),
                    ..Default::default()
                });
            }
            "RD" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| OsloParseError::MissingValue {
                        line: line_num,
                        command: "RD".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| OsloParseError::InvalidFloat {
                        line: line_num,
                        command: "RD".to_string(),
                        token: tok.clone(),
                    })?;
                    s.radius_mm = if val.abs() < 1e-12 { f64::INFINITY } else { val };
                }
            }
            "TH" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| OsloParseError::MissingValue {
                        line: line_num,
                        command: "TH".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| OsloParseError::InvalidFloat {
                        line: line_num,
                        command: "TH".to_string(),
                        token: tok.clone(),
                    })?;
                    s.thickness_mm = val;
                }
            }
            "AP" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| OsloParseError::MissingValue {
                        line: line_num,
                        command: "AP".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| OsloParseError::InvalidFloat {
                        line: line_num,
                        command: "AP".to_string(),
                        token: tok.clone(),
                    })?;
                    s.aperture_radius_mm = val;
                }
            }
            "GLAS" => {
                if let Some(ref mut s) = current_surface {
                    if let Some(mat_tok) = tokens.get(1) {
                        let trimmed_mat = mat_tok.trim_matches('"').trim_matches('\'');
                        if !trimmed_mat.eq_ignore_ascii_case("AIR") {
                            s.glass = trimmed_mat.to_string();
                        }
                    }
                }
            }
            "AIR" => {
                if let Some(ref mut s) = current_surface {
                    s.glass = String::new();
                }
            }
            "AST" => {
                if let Some(ref mut s) = current_surface {
                    s.is_stop = true;
                }
            }
            "CC" => {
                if let Some(ref mut s) = current_surface {
                    let tok = tokens.get(1).ok_or_else(|| OsloParseError::MissingValue {
                        line: line_num,
                        command: "CC".to_string(),
                    })?;
                    let val: f64 = tok.parse().map_err(|_| OsloParseError::InvalidFloat {
                        line: line_num,
                        command: "CC".to_string(),
                        token: tok.clone(),
                    })?;
                    s.conic = val;
                }
            }
            _ => {
                // Preserve unknown command
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
        return Err(OsloParseError::NoSurfaces);
    }

    if system.wavelengths_um.is_empty() {
        system.wavelengths_um.push(0.5875618);
        system.wavelength_weights.push(1.0);
    }

    if system.field_angles_deg.is_empty() {
        system.field_angles_deg.push(0.0);
    }

    Ok(system)
}

/// Serializes an [`OsloSystem`] into standard OSLO `.len` text format.
#[must_use]
pub fn write_len(system: &OsloSystem) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "LEN NEW \"{}\" {:.4} {}",
        system.title,
        system.design_efl_mm,
        system.surfaces.len()
    );
    let _ = writeln!(out, "ENT {:.6}", system.entrance_beam_radius_mm);

    if !system.wavelengths_um.is_empty() {
        let wv_strs: Vec<String> = system.wavelengths_um.iter().map(|w| format!("{w:.6}")).collect();
        let _ = writeln!(out, "WV {}", wv_strs.join(" "));

        let ww_strs: Vec<String> = system.wavelength_weights.iter().map(|w| format!("{w:.4}")).collect();
        if !ww_strs.is_empty() {
            let _ = writeln!(out, "WW {}", ww_strs.join(" "));
        }
        let _ = writeln!(out, "PK {}", system.primary_wavelength_index + 1);
    }

    if !system.field_angles_deg.is_empty() {
        let ang_strs: Vec<String> = system.field_angles_deg.iter().map(|a| format!("{a:.4}")).collect();
        let _ = writeln!(out, "ANG {}", ang_strs.join(" "));
    }

    // System extra lines
    for line in &system.extra_lines {
        let _ = writeln!(out, "{line}");
    }

    // Surfaces
    for (i, surf) in system.surfaces.iter().enumerate() {
        if i == system.surfaces.len().saturating_sub(1) {
            let _ = writeln!(out, "IMAGE");
        } else {
            let _ = writeln!(out, "SURF {}", surf.index);
        }

        if let Some(ref comm) = surf.comment {
            let _ = writeln!(out, "  // {comm}");
        }

        if surf.is_stop {
            let _ = writeln!(out, "  AST");
        }

        if surf.radius_mm.is_infinite() {
            let _ = writeln!(out, "  RD 0.000000");
        } else {
            let _ = writeln!(out, "  RD {:.6}", surf.radius_mm);
        }

        let _ = writeln!(out, "  TH {:.6}", surf.thickness_mm);
        let _ = writeln!(out, "  AP {:.6}", surf.aperture_radius_mm);

        if !surf.glass.is_empty() && !surf.glass.eq_ignore_ascii_case("AIR") {
            let _ = writeln!(out, "  GLAS {}", surf.glass);
        } else {
            let _ = writeln!(out, "  AIR");
        }

        if surf.conic.abs() > 1e-12 {
            let _ = writeln!(out, "  CC {:.6}", surf.conic);
        }

        for extra in &surf.extra_lines {
            let _ = writeln!(out, "  {extra}");
        }
    }

    out
}

impl OsloSystem {
    /// Converts this [`OsloSystem`] into an `optics-core` [`OpticalPrescription`].
    #[must_use]
    pub fn to_optical_prescription(&self) -> OpticalPrescription {
        let wavelengths_nm: Vec<f64> = self.wavelengths_um.iter().map(|&w| w * 1000.0).collect();

        let has_optical_surfaces = self.surfaces.iter().any(|s| s.index > 0);
        let optical_surfs: Vec<&OsloSurface> = if has_optical_surfaces {
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
                semi_diameter_mm: s.aperture_radius_mm,
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
            entrance_pupil_diameter_mm: self.entrance_beam_radius_mm * 2.0,
            field_angles_deg: if self.field_angles_deg.is_empty() {
                vec![0.0]
            } else {
                self.field_angles_deg.clone()
            },
            wavelengths_nm: if wavelengths_nm.is_empty() {
                vec![587.56]
            } else {
                wavelengths_nm
            },
            primary_wavelength_index: self
                .primary_wavelength_index
                .min(self.wavelengths_um.len().saturating_sub(1)),
            surfaces,
        }
    }

    /// Converts an `optics-core` [`OpticalPrescription`] into an [`OsloSystem`].
    #[must_use]
    pub fn from_optical_prescription(prescription: &OpticalPrescription) -> Self {
        let wavelengths_um: Vec<f64> = prescription.wavelengths_nm.iter().map(|&w| w / 1000.0).collect();

        let mut surfaces = Vec::with_capacity(prescription.surfaces.len() + 1);

        // Object surface at index 0
        surfaces.push(OsloSurface {
            index: 0,
            radius_mm: f64::INFINITY,
            thickness_mm: 0.0,
            aperture_radius_mm: prescription.entrance_pupil_diameter_mm * 0.5,
            glass: String::new(),
            conic: 0.0,
            is_stop: false,
            comment: Some("Object Plane".to_string()),
            extra_lines: Vec::new(),
        });

        for surf in &prescription.surfaces {
            surfaces.push(OsloSurface {
                index: surf.surface_index,
                radius_mm: surf.radius_mm,
                thickness_mm: surf.thickness_mm,
                aperture_radius_mm: surf.semi_diameter_mm,
                glass: surf.material_name.clone(),
                conic: surf.conic_constant,
                is_stop: surf.is_stop,
                comment: None,
                extra_lines: Vec::new(),
            });
        }

        let weights = vec![1.0; wavelengths_um.len()];

        Self {
            title: prescription.name.clone(),
            design_efl_mm: 50.0,
            entrance_beam_radius_mm: prescription.entrance_pupil_diameter_mm * 0.5,
            wavelengths_um,
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
    fn test_oslo_parse_and_round_trip() -> TestResult {
        let len_content = r#"// OSLO len file
LEN NEW "Cooke Triplet" 50.0 8
ENT 5.000000
WV 0.58756 0.65627 0.48613
WW 1.0 1.0 1.0
PK 1
ANG 0.0 10.0 20.0
SURF 0
  RD 0.0
  TH 0.0
  AP 5.0
  AIR
SURF 1
  RD 22.01359
  TH 3.25896
  AP 8.00000
  GLAS SK16
SURF 2
  RD -435.76000
  TH 6.00754
  AP 8.00000
  AIR
SURF 3
  RD -22.21328
  TH 0.99974
  AP 5.50000
  GLAS F2
SURF 4
  AST
  RD 20.29192
  TH 4.74971
  AP 5.00000
  AIR
SURF 5
  RD 79.68360
  TH 2.95298
  AP 7.50000
  GLAS SK16
SURF 6
  RD -18.39533
  TH 42.20778
  AP 7.50000
  AIR
IMAGE
  RD 0.0
  TH 0.0
  AP 18.0
  AIR
"#;

        let system = parse_len(len_content)?;
        assert_eq!(system.title, "Cooke Triplet");
        assert!((system.entrance_beam_radius_mm - 5.0).abs() < 1e-6);
        assert_eq!(system.wavelengths_um.len(), 3);
        assert_eq!(system.primary_wavelength_index, 0);
        assert_eq!(system.surfaces.len(), 8);

        // Check Surface 1
        assert_eq!(system.surfaces[1].glass, "SK16");
        assert!((system.surfaces[1].radius_mm - 22.01359).abs() < 1e-4);

        // Check Stop
        assert!(system.surfaces[4].is_stop);

        // Test roundtrip
        let written = write_len(&system);
        let reparsed = parse_len(&written)?;

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
        assert_eq!(prescription.name, "Cooke Triplet");
        assert_eq!(prescription.entrance_pupil_diameter_mm, 10.0);
        assert_eq!(prescription.surfaces.len(), 7);

        Ok(())
    }
}
