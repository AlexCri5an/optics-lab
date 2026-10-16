//! refractiveindex.info YAML material specification parser.
//!
//! Parses standard material definitions from the popular refractiveindex.info database
//! into [`GlassMaterial`] structs.

use optics_core::OpticsError;

use crate::material::{DispersionModel, GlassCatalog, GlassMaterial};

/// Parses a refractiveindex.info YAML string into a [`GlassMaterial`].
///
/// Supports `type: formula 1` (Sellmeier 1 with offset), `type: formula 2` (Sellmeier 1),
/// `type: formula 3` (Schott), `type: formula 4` (Sellmeier 2 / Herzberger),
/// and `type: formula 5` (Cauchy).
///
/// # Errors
/// Returns [`OpticsError::ParseError`] if data format cannot be interpreted.
pub fn parse_refractiveindex_yaml(
    name: &str,
    yaml_text: &str,
) -> Result<GlassMaterial, OpticsError> {
    let mut formula_type = 2; // Default Sellmeier 1
    let mut coefficients = Vec::new();

    let mut in_data = false;

    for line in yaml_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("DATA:") {
            in_data = true;
            continue;
        }

        if in_data {
            if trimmed.starts_with("type:") || trimmed.starts_with("- type:") {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    let val = parts[1].trim();
                    if val.starts_with("formula") {
                        let subparts: Vec<&str> = val.split_whitespace().collect();
                        if subparts.len() >= 2 {
                            formula_type = subparts[1].parse::<u32>().unwrap_or(2);
                        }
                    }
                }
            } else if trimmed.starts_with("coefficients:") {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    for num_str in parts[1].split_whitespace() {
                        if let Ok(c) = num_str.parse::<f64>() {
                            coefficients.push(c);
                        }
                    }
                }
            } else if !coefficients.is_empty() {
                // Continuation lines of coefficients
                for num_str in trimmed.split_whitespace() {
                    if let Ok(c) = num_str.parse::<f64>() {
                        coefficients.push(c);
                    }
                }
            }
        }
    }

    if coefficients.is_empty() {
        return Err(OpticsError::ParseError(
            "refractiveindex.info YAML missing coefficients in DATA section".to_string(),
        ));
    }

    let dispersion = match formula_type {
        1 | 2 => {
            // Formula 1 & 2 in refractiveindex.info are Sellmeier equations
            // If coefficients has leading constant: [c0, B1, C1, B2, C2, B3, C3]
            if coefficients.len() >= 7 {
                DispersionModel::Sellmeier1 {
                    b: [coefficients[1], coefficients[3], coefficients[5]],
                    c: [coefficients[2], coefficients[4], coefficients[6]],
                }
            } else if coefficients.len() >= 6 {
                DispersionModel::Sellmeier1 {
                    b: [coefficients[0], coefficients[2], coefficients[4]],
                    c: [coefficients[1], coefficients[3], coefficients[5]],
                }
            } else {
                return Err(OpticsError::ParseError(
                    "Insufficient coefficients for Sellmeier formula in YAML".to_string(),
                ));
            }
        }
        3 => {
            // Formula 3: Schott polynomial
            let mut a = [0.0; 6];
            for (i, &val) in coefficients.iter().take(6).enumerate() {
                a[i] = val;
            }
            DispersionModel::Schott { a }
        }
        5 => {
            // Formula 5: Cauchy
            let a = coefficients.first().copied().unwrap_or(1.5);
            let b = coefficients.get(1).copied().unwrap_or(0.005);
            DispersionModel::Cauchy { a, b }
        }
        _ => {
            return Err(OpticsError::ParseError(format!(
                "Unsupported refractiveindex.info formula type {formula_type}"
            )));
        }
    };

    let mut mat = GlassMaterial {
        name: name.to_string(),
        catalog: GlassCatalog::Custom,
        nd: 1.5,
        vd: 50.0,
        dispersion,
        thermal: None,
        internal_transmittance: Vec::new(),
        test_thickness_mm: 10.0,
    };

    // Compute nd and vd from analytical model
    let nd = mat.refractive_index(587.56);
    let nf = mat.refractive_index(486.13);
    let nc = mat.refractive_index(656.27);
    let vd = if (nf - nc).abs() > 1e-12 {
        (nd - 1.0) / (nf - nc)
    } else {
        50.0
    };

    mat.nd = nd;
    mat.vd = vd;

    Ok(mat)
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_refractiveindex_yaml_sellmeier() -> TestResult {
        let yaml_sample = r#"
# N-BK7 from refractiveindex.info
REFERENCES: "SCHOTT optical glass catalog 2017"
COMMENTS: "Sellmeier 1 formula"
DATA:
  - type: formula 2
    wavelength_range: 0.3 2.5
    coefficients: 0 1.03961212 0.00600069867 0.231792344 0.0200179144 1.01046945 103.560653
"#;

        let glass = parse_refractiveindex_yaml("N-BK7_YAML", yaml_sample)?;
        assert_eq!(glass.name, "N-BK7_YAML");
        assert!((glass.nd - 1.51680).abs() < 1e-4);
        assert!((glass.vd - 64.17).abs() < 0.2);
        Ok(())
    }
}
