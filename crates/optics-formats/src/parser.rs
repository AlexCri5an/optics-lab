//! Optical `.zmx` format parser.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::error::FormatParseError;
use crate::model::{FormatOpticalSystem, FormatSurface, SurfaceType};

/// Parses an optical `.zmx` file from string content.
///
/// # Supported Operands
/// - System: `NAME`, `ENPD`, `FNUM`, `WAVL`, `WAVM`, `PWAV`, `FTYP`, `YFLD`, `YFLN`, `GCAT`, `UNIT`, `MODE`
/// - Surface: `SURF`, `TYPE`, `CURV`, `DISZ`, `GLAS`, `DIAM`, `CONI`, `STOP`
///
/// # Errors
/// Returns [`FormatParseError`] with line numbers and operand context if parsing fails.
pub fn parse_zmx(content: &str) -> Result<FormatOpticalSystem, FormatParseError> {
    let mut system = FormatOpticalSystem {
        wavelengths_um: Vec::new(),
        wavelength_weights: Vec::new(),
        field_angles_deg: Vec::new(),
        glass_catalogs: Vec::new(),
        surfaces: Vec::new(),
        ..Default::default()
    };

    // Buffer for indexed WAVM declarations
    let mut wavm_slots: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    let mut raw_wavl: Vec<(f64, f64)> = Vec::new();
    let mut expected_num_fields: Option<usize> = None;
    let mut expected_num_wavelengths: Option<usize> = None;

    let mut current_surface: Option<FormatSurface> = None;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Skip blank lines
        if trimmed.is_empty() {
            continue;
        }

        // Preserve comment lines
        if trimmed.starts_with('!') || trimmed.starts_with('#') {
            if let Some(ref mut s) = current_surface {
                s.extra_lines.push(trimmed.to_string());
            } else {
                system.extra_lines.push(trimmed.to_string());
            }
            continue;
        }

        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let operand = tokens[0].to_uppercase();

        match operand.as_str() {
            "FTYP" => {
                if let Some(tok) = tokens.get(1) {
                    system.field_type = (*tok).to_string();
                }
                // Token 3 (index 3 in tokens) is number of active fields: FTYP <type> <color> <num_fields> <num_wavelengths>
                if let Some(tok) = tokens.get(3) {
                    if let Ok(n) = tok.parse::<usize>() {
                        if n > 0 {
                            expected_num_fields = Some(n);
                        }
                    }
                }
                // Token 4 (index 4 in tokens) is number of active wavelengths
                if let Some(tok) = tokens.get(4) {
                    if let Ok(n) = tok.parse::<usize>() {
                        if n > 0 {
                            expected_num_wavelengths = Some(n);
                        }
                    }
                }
            }
            "COMM" => {
                if let Some(ref mut s) = current_surface {
                    let comm = tokens[1..].join(" ");
                    s.comment = Some(comm);
                }
            }
            "NAME" => {
                system.name = tokens[1..].join(" ");
            }
            "ENPD" => {
                let token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                    line: line_num,
                    operand: "ENPD",
                })?;
                system.entrance_pupil_diameter_mm = token.parse().map_err(|_| FormatParseError::InvalidFloat {
                    line: line_num,
                    operand: "ENPD",
                    token: (*token).to_string(),
                })?;
            }
            "FNUM" => {
                let token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                    line: line_num,
                    operand: "FNUM",
                })?;
                system.f_number = token.parse().map_err(|_| FormatParseError::InvalidFloat {
                    line: line_num,
                    operand: "FNUM",
                    token: (*token).to_string(),
                })?;
            }
            "WAVL" => {
                let wl_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                    line: line_num,
                    operand: "WAVL",
                })?;
                let wl: f64 = wl_token.parse().map_err(|_| FormatParseError::InvalidFloat {
                    line: line_num,
                    operand: "WAVL",
                    token: (*wl_token).to_string(),
                })?;
                let weight: f64 = tokens.get(2).and_then(|t| t.parse().ok()).unwrap_or(1.0);
                raw_wavl.push((wl, weight));
            }
            "WAVM" => {
                if let Some(first_tok) = tokens.get(1) {
                    if let Ok(slot) = first_tok.parse::<usize>() {
                        // Indexed format: WAVM <slot> <wavelength_um> [weight]
                        let wl_token = tokens.get(2).ok_or(FormatParseError::MissingOperandValue {
                            line: line_num,
                            operand: "WAVM",
                        })?;
                        let wl: f64 = wl_token.parse().map_err(|_| FormatParseError::InvalidFloat {
                            line: line_num,
                            operand: "WAVM",
                            token: (*wl_token).to_string(),
                        })?;
                        let weight: f64 = tokens.get(3).and_then(|t| t.parse().ok()).unwrap_or(1.0);
                        wavm_slots.insert(slot, (wl, weight));
                    } else {
                        // Multi-wavelength list format: WAVM <wl1> <wl2> <wl3> ...
                        for (idx, tok) in tokens.iter().skip(1).enumerate() {
                            if let Ok(wl) = tok.parse::<f64>() {
                                wavm_slots.insert(idx + 1, (wl, 1.0));
                            }
                        }
                    }
                }
            }
            "PWAV" => {
                let token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                    line: line_num,
                    operand: "PWAV",
                })?;
                let pwav_1based: usize = token.parse().map_err(|_| FormatParseError::InvalidInteger {
                    line: line_num,
                    operand: "PWAV",
                    token: (*token).to_string(),
                })?;
                system.primary_wavelength_index = pwav_1based.saturating_sub(1);
            }
            "YFLD" | "YFLN" => {
                let mut fields = Vec::new();
                for &field_token in &tokens[1..] {
                    if let Ok(y) = field_token.parse::<f64>() {
                        fields.push(y);
                    }
                }
                if let Some(n) = expected_num_fields {
                    if fields.len() > n {
                        fields.truncate(n);
                    }
                } else if fields.len() > 1 {
                    // Fixed 12-slot array pads unused entries with trailing 0.0
                    while fields.len() > 1 && fields.last() == Some(&0.0) {
                        fields.pop();
                    }
                }
                system.field_angles_deg.extend(fields);
            }
            "GCAT" => {
                for &cat in &tokens[1..] {
                    system.glass_catalogs.push(cat.to_string());
                }
            }
            "SURF" => {
                // Finalize previous surface
                if let Some(surf) = current_surface.take() {
                    system.surfaces.push(surf);
                }
                let idx_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                    line: line_num,
                    operand: "SURF",
                })?;
                let index: usize = idx_token.parse().map_err(|_| FormatParseError::InvalidInteger {
                    line: line_num,
                    operand: "SURF",
                    token: (*idx_token).to_string(),
                })?;
                current_surface = Some(FormatSurface {
                    index,
                    ..Default::default()
                });
            }
            "TYPE" => {
                if let Some(ref mut s) = current_surface {
                    let type_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "TYPE",
                    })?;
                    s.surface_type = SurfaceType::from_str(type_token).unwrap_or_default();
                }
            }
            "CURV" => {
                if let Some(ref mut s) = current_surface {
                    let curv_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "CURV",
                    })?;
                    let c: f64 = curv_token.parse().map_err(|_| FormatParseError::InvalidFloat {
                        line: line_num,
                        operand: "CURV",
                        token: (*curv_token).to_string(),
                    })?;
                    s.curvature = c;
                    s.radius_mm = if c.abs() < 1e-12 { f64::INFINITY } else { 1.0 / c };
                }
            }
            "DISZ" => {
                if let Some(ref mut s) = current_surface {
                    let token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "DISZ",
                    })?;
                    let t_upper = token.to_uppercase();
                    if t_upper == "INFINITY" || t_upper == "INF" {
                        s.thickness_mm = f64::INFINITY;
                    } else {
                        s.thickness_mm = token.parse().map_err(|_| FormatParseError::InvalidFloat {
                            line: line_num,
                            operand: "DISZ",
                            token: (*token).to_string(),
                        })?;
                    }
                }
            }
            "GLAS" => {
                if let Some(ref mut s) = current_surface {
                    let mat_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "GLAS",
                    })?;
                    let mat_upper = mat_token.to_uppercase();
                    if mat_upper == "AIR" {
                        s.material_name = String::new();
                    } else if mat_upper == "___BLANK" {
                        let nd_opt: Option<f64> = tokens.get(4).and_then(|t| t.parse().ok());
                        let vd_opt: Option<f64> = tokens.get(5).and_then(|t| t.parse().ok());
                        if let Some(nd) = nd_opt {
                            if nd > 1.05 {
                                let vd = vd_opt.unwrap_or(50.0);
                                s.material_name = format!("N_{:.4}_V{:.1}", nd, vd);
                            } else {
                                s.material_name = String::new();
                            }
                        } else {
                            s.material_name = String::new();
                        }
                    } else {
                        s.material_name = (*mat_token).to_string();
                    }
                }
            }
            "DIAM" => {
                if let Some(ref mut s) = current_surface {
                    let diam_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "DIAM",
                    })?;
                    s.semi_diameter_mm = diam_token.parse().map_err(|_| FormatParseError::InvalidFloat {
                        line: line_num,
                        operand: "DIAM",
                        token: (*diam_token).to_string(),
                    })?;
                }
            }
            "CONI" => {
                if let Some(ref mut s) = current_surface {
                    let coni_token = tokens.get(1).ok_or(FormatParseError::MissingOperandValue {
                        line: line_num,
                        operand: "CONI",
                    })?;
                    s.conic_constant = coni_token.parse().map_err(|_| FormatParseError::InvalidFloat {
                        line: line_num,
                        operand: "CONI",
                        token: (*coni_token).to_string(),
                    })?;
                }
            }
            "STOP" => {
                if let Some(ref mut s) = current_surface {
                    s.is_stop = true;
                }
            }
            "PARM" => {
                if let Some(ref mut s) = current_surface {
                    if let (Some(idx_tok), Some(val_tok)) = (tokens.get(1), tokens.get(2)) {
                        if let (Ok(idx), Ok(val)) = (idx_tok.parse::<usize>(), val_tok.parse::<f64>()) {
                            if idx > 0 {
                                if s.parameters.len() < idx {
                                    s.parameters.resize(idx, 0.0);
                                }
                                s.parameters[idx - 1] = val;
                            }
                        }
                    }
                }
            }
            "CLAP" | "OBST" => {
                if let Some(ref mut s) = current_surface {
                    let is_obscuration = operand == "OBST";
                    let min_r: f64 = tokens.get(1).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let max_r: f64 = tokens.get(2).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let x_size: f64 = tokens.get(3).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let y_size: f64 = tokens.get(4).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let x_dec: f64 = tokens.get(5).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let y_dec: f64 = tokens.get(6).and_then(|t| t.parse().ok()).unwrap_or(0.0);

                    let mut ap = if x_size > 1e-12 && y_size > 1e-12 {
                        optics_core::Aperture::rectangular(x_size, y_size)
                    } else if min_r > 1e-12 && max_r > min_r {
                        optics_core::Aperture::annular(min_r, max_r)
                    } else {
                        let r = if max_r > 1e-12 { max_r } else { s.semi_diameter_mm };
                        optics_core::Aperture::circular(r)
                    };
                    ap.decenter_x_mm = x_dec;
                    ap.decenter_y_mm = y_dec;
                    ap.is_obscuration = is_obscuration;
                    s.aperture = Some(ap);
                }
            }
            "APER" => {
                if let Some(ref mut s) = current_surface {
                    let shape_tok = tokens.get(1).map(|t| t.to_uppercase()).unwrap_or_default();
                    let min_r: f64 = tokens.get(2).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let max_r: f64 = tokens.get(3).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let x_size: f64 = tokens.get(4).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let y_size: f64 = tokens.get(5).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let x_dec: f64 = tokens.get(6).and_then(|t| t.parse().ok()).unwrap_or(0.0);
                    let y_dec: f64 = tokens.get(7).and_then(|t| t.parse().ok()).unwrap_or(0.0);

                    let mut ap = match shape_tok.as_str() {
                        "1" | "RECT" | "RECTANGULAR" => {
                            optics_core::Aperture::rectangular(x_size.max(0.1), y_size.max(0.1))
                        }
                        "2" | "ELLI" | "ELLIPTICAL" => {
                            optics_core::Aperture::elliptical(x_size.max(0.1), y_size.max(0.1))
                        }
                        _ => {
                            if min_r > 1e-12 && max_r > min_r {
                                optics_core::Aperture::annular(min_r, max_r)
                            } else {
                                let r = if max_r > 1e-12 { max_r } else { s.semi_diameter_mm };
                                optics_core::Aperture::circular(r)
                            }
                        }
                    };
                    ap.decenter_x_mm = x_dec;
                    ap.decenter_y_mm = y_dec;
                    s.aperture = Some(ap);
                }
            }
            "COAT" => {
                if let Some(ref mut s) = current_surface {
                    let coat_name = tokens.get(1).map(|t| t.trim_matches('"').trim()).unwrap_or_default();
                    if !coat_name.is_empty() {
                        s.coating = optics_core::SurfaceCoating::from_format_name(coat_name);
                    }
                    s.extra_lines.push(trimmed.to_string());
                }
            }
            _ => {
                // Preserve unrecognized operands for lossless round-tripping
                if let Some(ref mut s) = current_surface {
                    s.extra_lines.push(trimmed.to_string());
                } else {
                    system.extra_lines.push(trimmed.to_string());
                }
            }
        }
    }

    if let Some(surf) = current_surface {
        system.surfaces.push(surf);
    }

    if system.surfaces.is_empty() {
        return Err(FormatParseError::NoSurfaces);
    }

    // Consolidate wavelengths: WAVM slots take precedence if defined, else raw WAVL
    if !wavm_slots.is_empty() {
        for (_slot, (wl, weight)) in wavm_slots {
            system.wavelengths_um.push(wl);
            system.wavelength_weights.push(weight);
        }
    } else if !raw_wavl.is_empty() {
        for (wl, weight) in raw_wavl {
            system.wavelengths_um.push(wl);
            system.wavelength_weights.push(weight);
        }
    } else {
        // Fallback default d-line (0.58756 um)
        system.wavelengths_um.push(0.5875618);
        system.wavelength_weights.push(1.0);
    }

    // Apply expected wavelength count if declared in FTYP
    if let Some(n) = expected_num_wavelengths {
        if system.wavelengths_um.len() > n {
            system.wavelengths_um.truncate(n);
            system.wavelength_weights.truncate(n);
        }
    }

    // Deduplicate any exact duplicate wavelengths while preserving user-defined order
    let mut unique_wls = Vec::new();
    let mut unique_weights = Vec::new();
    for (wl, wt) in system.wavelengths_um.iter().zip(&system.wavelength_weights) {
        if !unique_wls.iter().any(|&u: &f64| (u - wl).abs() < 1e-5) {
            unique_wls.push(*wl);
            unique_weights.push(*wt);
        }
    }
    system.wavelengths_um = unique_wls;
    system.wavelength_weights = unique_weights;

    // Ensure primary wavelength index is within valid bounds
    if system.wavelengths_um.is_empty() {
        system.wavelengths_um.push(0.5875618);
        system.wavelength_weights.push(1.0);
    }
    system.primary_wavelength_index = system
        .primary_wavelength_index
        .min(system.wavelengths_um.len().saturating_sub(1));

    if let Some(n) = expected_num_fields {
        if system.field_angles_deg.len() > n {
            system.field_angles_deg.truncate(n);
        }
    }

    if system.field_angles_deg.is_empty() {
        system.field_angles_deg.push(0.0);
    }

    Ok(system)
}

/// Parses an optical `.zmx` file from raw byte slice, auto-detecting UTF-16 LE BOM or UTF-8.
///
/// # Errors
/// Returns [`FormatParseError`] if decoding or parsing fails.
pub fn parse_zmx_bytes(bytes: &[u8]) -> Result<FormatOpticalSystem, FormatParseError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        // UTF-16 LE with BOM
        let u16_slice: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let decoded = char::decode_utf16(u16_slice)
            .collect::<Result<String, _>>()
            .map_err(|e| FormatParseError::TextDecodeError(e.to_string()))?;
        parse_zmx(&decoded)
    } else if bytes.len() >= 4 && bytes[1] == 0 && bytes[3] == 0 {
        // UTF-16 LE without BOM
        let u16_slice: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let decoded = char::decode_utf16(u16_slice)
            .collect::<Result<String, _>>()
            .map_err(|e| FormatParseError::TextDecodeError(e.to_string()))?;
        parse_zmx(&decoded)
    } else {
        // UTF-8 or ASCII / Latin-1 fallback
        let decoded = String::from_utf8_lossy(bytes);
        parse_zmx(&decoded)
    }
}
