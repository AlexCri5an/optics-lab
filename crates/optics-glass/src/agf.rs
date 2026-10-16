//! Zemax AGF (ASCII Glass File) catalog parser.
//!
//! Parses industry-standard `.agf` optical glass catalog files containing
//! glass names, dispersion formula types, coefficients, thermal parameters,
//! and internal transmittance measurements.

use optics_core::OpticsError;
use optics_spectral::thermal::SchottThermalModel;

use crate::material::{DispersionModel, GlassCatalog, GlassMaterial};

/// Parses a Zemax AGF glass catalog string into a vector of [`GlassMaterial`]s.
///
/// # Errors
/// Returns [`OpticsError::ParseError`] if the format is invalid.
pub fn parse_agf(agf_text: &str) -> Result<Vec<GlassMaterial>, OpticsError> {
    let mut glasses = Vec::new();
    let mut current_glass: Option<GlassMaterial> = None;
    let mut formula_code: u32 = 2; // Default to Sellmeier 1

    for line in agf_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        match tokens[0].to_uppercase().as_str() {
            "NM" => {
                // If there's an ongoing glass, flush it
                if let Some(glass) = current_glass.take() {
                    glasses.push(glass);
                }

                if tokens.len() < 2 {
                    return Err(OpticsError::ParseError(
                        "AGF 'NM' line missing glass name".to_string(),
                    ));
                }

                let name = tokens[1].to_string();
                formula_code = tokens
                    .get(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(2);
                let nd = tokens
                    .get(4)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.5);
                let vd = tokens
                    .get(5)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(50.0);

                current_glass = Some(GlassMaterial {
                    name,
                    catalog: GlassCatalog::Custom,
                    nd,
                    vd,
                    dispersion: DispersionModel::Constant { n: nd },
                    thermal: None,
                    internal_transmittance: Vec::new(),
                    test_thickness_mm: 10.0,
                });
            }
            "CD" => {
                if let Some(ref mut glass) = current_glass {
                    let mut coeffs = Vec::new();
                    for t in &tokens[1..] {
                        if let Ok(val) = t.parse::<f64>() {
                            coeffs.push(val);
                        }
                    }

                    glass.dispersion = map_agf_formula(formula_code, &coeffs, glass.nd);
                }
            }
            "TD" => {
                if let Some(ref mut glass) = current_glass {
                    if tokens.len() >= 7 {
                        let d0 = tokens[1].parse::<f64>().unwrap_or(0.0);
                        let d1 = tokens[2].parse::<f64>().unwrap_or(0.0);
                        let d2 = tokens[3].parse::<f64>().unwrap_or(0.0);
                        let e0 = tokens[4].parse::<f64>().unwrap_or(0.0);
                        let e1 = tokens[5].parse::<f64>().unwrap_or(0.0);
                        let lambda_tk = tokens[6].parse::<f64>().unwrap_or(0.170);

                        glass.thermal = Some(SchottThermalModel {
                            d0,
                            d1,
                            d2,
                            e0,
                            e1,
                            lambda_tk,
                        });
                    }
                }
            }
            "IT" => {
                if let Some(ref mut glass) = current_glass {
                    if tokens.len() >= 3 {
                        if let (Ok(wl_um), Ok(transmittance)) =
                            (tokens[1].parse::<f64>(), tokens[2].parse::<f64>())
                        {
                            let thickness = tokens
                                .get(3)
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(10.0);
                            glass
                                .internal_transmittance
                                .push((wl_um * 1000.0, transmittance));
                            glass.test_thickness_mm = thickness;
                        }
                    }
                }
            }
            _ => {
                // Ignore unrecognized operands (e.g. ED, LD, OD)
            }
        }
    }

    if let Some(glass) = current_glass {
        glasses.push(glass);
    }

    Ok(glasses)
}

fn map_agf_formula(formula: u32, coeffs: &[f64], fallback_nd: f64) -> DispersionModel {
    match formula {
        1 if coeffs.len() >= 6 => {
            // Schott formula
            let mut a = [0.0; 6];
            for (i, val) in coeffs.iter().take(6).enumerate() {
                a[i] = *val;
            }
            DispersionModel::Schott { a }
        }
        2 if coeffs.len() >= 6 => {
            // Sellmeier 1
            DispersionModel::Sellmeier1 {
                b: [coeffs[0], coeffs[2], coeffs[4]],
                c: [coeffs[1], coeffs[3], coeffs[5]],
            }
        }
        3 if coeffs.len() >= 5 => {
            // Herzberger
            let mut a = [0.0; 5];
            for (i, val) in coeffs.iter().take(5).enumerate() {
                a[i] = *val;
            }
            DispersionModel::Herzberger { coeffs: a }
        }
        4 if coeffs.len() >= 5 => {
            // Sellmeier 2: A, B1, C1, B2, C2
            DispersionModel::Sellmeier2 {
                a: coeffs[0],
                b: [coeffs[1], coeffs[3]],
                c: [coeffs[2], coeffs[4]],
            }
        }
        5 if coeffs.len() >= 3 => {
            // Conrady: n0, A, B
            DispersionModel::Conrady {
                n0: coeffs[0],
                a: coeffs[1],
                b: coeffs[2],
            }
        }
        6 if coeffs.len() >= 8 => {
            // Sellmeier 3 (4 oscillators)
            DispersionModel::Sellmeier3 {
                k: [coeffs[0], coeffs[2], coeffs[4], coeffs[6]],
                l: [coeffs[1], coeffs[3], coeffs[5], coeffs[7]],
            }
        }
        7 if coeffs.len() >= 4 => {
            // Handbook of Optics 1
            DispersionModel::HandbookOfOptics1 {
                a: coeffs[0],
                b: coeffs[1],
                c: coeffs[2],
                d: coeffs[3],
            }
        }
        8 if coeffs.len() >= 4 => {
            // Handbook of Optics 2
            DispersionModel::HandbookOfOptics2 {
                a: coeffs[0],
                b: coeffs[1],
                c: coeffs[2],
                d: coeffs[3],
            }
        }
        9 if coeffs.len() >= 5 => {
            // Sellmeier 4
            DispersionModel::Sellmeier4 {
                a: coeffs[0],
                b: coeffs[1],
                c: coeffs[2],
                d: coeffs[3],
                e: coeffs[4],
            }
        }
        10 if coeffs.len() >= 8 => {
            // Extended 1
            let mut a = [0.0; 8];
            for (i, val) in coeffs.iter().take(8).enumerate() {
                a[i] = *val;
            }
            DispersionModel::Extended1 { a }
        }
        11 if coeffs.len() >= 10 => {
            // Sellmeier 5 (5 oscillators)
            DispersionModel::Sellmeier5 {
                k: [coeffs[0], coeffs[2], coeffs[4], coeffs[6], coeffs[8]],
                l: [coeffs[1], coeffs[3], coeffs[5], coeffs[7], coeffs[9]],
            }
        }
        12 if coeffs.len() >= 7 => {
            // Extended 2
            let mut a = [0.0; 7];
            for (i, val) in coeffs.iter().take(7).enumerate() {
                a[i] = *val;
            }
            DispersionModel::Extended2 { a }
        }
        _ => DispersionModel::Constant { n: fallback_nd },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_agf_snippet() -> TestResult {
        let agf_sample = r#"
# Sample AGF Catalog
NM TEST_CROWN 2 123456 1.5168 64.17 0.0 0.0
CD 1.03961212 0.00600069867 0.231792344 0.0200179144 1.01046945 103.560653
TD 1.86e-6 1.31e-8 -1.37e-11 4.34e-7 6.27e-10 0.170
IT 0.400 0.992 10.0
IT 0.550 0.998 10.0

NM TEST_FLINT 1 654321 1.6200 36.37 0.0 0.0
CD 2.60 0.01 0.02 0.001 0.0 0.0
"#;

        let glasses = parse_agf(agf_sample)?;
        assert_eq!(glasses.len(), 2);

        let crown = &glasses[0];
        assert_eq!(crown.name, "TEST_CROWN");
        assert_eq!(crown.nd, 1.5168);
        assert!(crown.thermal.is_some());
        assert_eq!(crown.internal_transmittance.len(), 2);
        assert!((crown.refractive_index(587.56) - 1.5168).abs() < 1e-3);

        let flint = &glasses[1];
        assert_eq!(flint.name, "TEST_FLINT");
        assert!(matches!(flint.dispersion, DispersionModel::Schott { .. }));
        Ok(())
    }
}
