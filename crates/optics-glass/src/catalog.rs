//! Embedded optical glass catalog parser and database.

use std::collections::HashMap;

use optics_core::OpticsError;
use serde::Deserialize;

use crate::material::{DispersionModel, GlassCatalog, GlassMaterial};

/// Raw schema entry as serialized in `optical_glasses.json`.
#[derive(Debug, Clone, Deserialize)]
struct RawGlassEntry {
    name: String,
    manufacturer: String,
    formula_type: u32,
    coefficients: Vec<f64>,
    nd: Option<f64>,
    vd: Option<f64>,
}

/// In-memory optical glass catalog database.
#[derive(Debug, Clone)]
pub struct GlassCatalogDb {
    glasses_by_name: HashMap<String, GlassMaterial>,
}

impl GlassCatalogDb {
    /// Parses the bundled `optical_glasses.json` dataset (646 glasses) compiled into the binary.
    ///
    /// # Errors
    /// Returns [`OpticsError::ParseError`] if the JSON is malformed.
    pub fn load_bundled() -> Result<Self, OpticsError> {
        let bytes = include_bytes!("../data/optical_glasses.json");
        Self::from_json_slice(bytes)
    }
}

impl Default for GlassCatalogDb {
    fn default() -> Self {
        Self::load_bundled().unwrap_or_else(|_| Self {
            glasses_by_name: HashMap::new(),
        })
    }
}

impl GlassCatalogDb {

    /// Parses glass catalog from a JSON byte slice.
    ///
    /// # Errors
    /// Returns [`OpticsError::ParseError`] if parsing fails.
    pub fn from_json_slice(slice: &[u8]) -> Result<Self, OpticsError> {
        let entries: Vec<RawGlassEntry> = serde_json::from_slice(slice).map_err(|e| {
            OpticsError::ParseError(format!("Failed to parse optical_glasses.json: {e}"))
        })?;

        let mut glasses_by_name = HashMap::with_capacity(entries.len());

        for raw in entries {
            let catalog = match raw.manufacturer.to_uppercase().as_str() {
                "SCHOTT" => GlassCatalog::Schott,
                "OHARA" => GlassCatalog::Ohara,
                "CDGM" => GlassCatalog::Cdgm,
                "HOYA" => GlassCatalog::Hoya,
                _ => GlassCatalog::Custom,
            };

            let dispersion = match raw.formula_type {
                2 if raw.coefficients.len() >= 7 => {
                    // Formula 2: Schott Sellmeier 1: [0, B1, C1, B2, C2, B3, C3]
                    DispersionModel::Sellmeier1 {
                        b: [
                            raw.coefficients[1],
                            raw.coefficients[3],
                            raw.coefficients[5],
                        ],
                        c: [
                            raw.coefficients[2],
                            raw.coefficients[4],
                            raw.coefficients[6],
                        ],
                    }
                }
                1 if raw.coefficients.len() >= 7 => {
                    // Formula 1 variant: Sellmeier 1 with offset
                    DispersionModel::Sellmeier1 {
                        b: [
                            raw.coefficients[1],
                            raw.coefficients[3],
                            raw.coefficients[5],
                        ],
                        c: [
                            raw.coefficients[2],
                            raw.coefficients[4],
                            raw.coefficients[6],
                        ],
                    }
                }
                _ => {
                    // Fallback constant index if non-standard formula
                    DispersionModel::Constant {
                        n: raw.nd.unwrap_or(1.5),
                    }
                }
            };

            let nd = raw.nd.unwrap_or_else(|| {
                // If nd missing, evaluate at helium yellow d line (587.56 nm)
                let mat = GlassMaterial {
                    name: raw.name.clone(),
                    catalog,
                    nd: 1.5,
                    vd: 50.0,
                    dispersion: dispersion.clone(),
                    thermal: None,
                    internal_transmittance: Vec::new(),
                    test_thickness_mm: 10.0,
                };
                mat.refractive_index(587.56)
            });

            let vd = raw.vd.unwrap_or(50.0);

            // Populate standard internal transmittance tables (Schott / Ohara standards)
            // for classic glasses where optical_glasses.json only has dispersion coefficients.
            let (internal_transmittance, test_thickness_mm) = match raw.name.to_uppercase().as_str() {
                "N-BK7" | "BK7" => (
                    vec![
                        (300.0, 0.050),
                        (320.0, 0.400),
                        (350.0, 0.720),
                        (400.0, 0.992),
                        (550.0, 0.998),
                        (700.0, 0.998),
                        (1060.0, 0.997),
                        (1530.0, 0.995),
                    ],
                    10.0,
                ),
                "SF11" => (
                    vec![
                        (370.0, 0.080),
                        (380.0, 0.220),
                        (400.0, 0.530),
                        (550.0, 0.996),
                        (700.0, 0.997),
                        (1060.0, 0.998),
                    ],
                    10.0,
                ),
                "N-SF11" => (
                    vec![
                        (370.0, 0.150),
                        (380.0, 0.380),
                        (400.0, 0.815),
                        (550.0, 0.992),
                        (700.0, 0.995),
                        (1060.0, 0.997),
                    ],
                    10.0,
                ),
                "F2" | "N-F2" => (
                    vec![
                        (350.0, 0.250),
                        (370.0, 0.750),
                        (400.0, 0.994),
                        (550.0, 0.999),
                        (700.0, 0.999),
                        (1060.0, 0.998),
                    ],
                    10.0,
                ),
                "LITHOSIL-Q" | "FUSED SILICA" | "SILICA" => (
                    vec![
                        (200.0, 0.920),
                        (250.0, 0.985),
                        (300.0, 0.995),
                        (400.0, 0.998),
                        (550.0, 0.999),
                        (700.0, 0.999),
                    ],
                    10.0,
                ),
                _ => (Vec::new(), 10.0),
            };

            let material = GlassMaterial {
                name: raw.name.clone(),
                catalog,
                nd,
                vd,
                dispersion,
                thermal: None,
                internal_transmittance,
                test_thickness_mm,
            };

            glasses_by_name.insert(raw.name.to_uppercase(), material);
        }

        Ok(Self { glasses_by_name })
    }

    /// Case-insensitive lookup of an optical glass by designation (e.g. "N-BK7", "n-bk7", "SF11", "Fused Silica").
    ///
    /// Also supports automatic prefix/strip resolution for lead-free `N-` glass aliases
    /// (e.g. looking up "SK16" resolves to "N-SK16", and "N-F2" resolves to "F2").
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&GlassMaterial> {
        let key = name.trim().to_uppercase();
        if let Some(glass) = self.glasses_by_name.get(&key) {
            return Some(glass);
        }

        // Aliases for fused silica
        if key == "FUSED SILICA" || key == "FUSED_SILICA" || key == "SILICA" {
            return self.glasses_by_name.get("LITHOSIL-Q");
        }

        // Standard immersion Water
        if key == "WATER" || key == "H2O" {
            // Lazy/static or fallback
            return self.glasses_by_name.get("WATER");
        }

        // Classic Schott FK54
        if key == "FK54" || key == "N-FK54" {
            return self.glasses_by_name.get("FK54").or_else(|| self.glasses_by_name.get("N-FK51A"));
        }

        // Try N- prefix for classic Schott designations (e.g. "SK16" -> "N-SK16")
        if !key.starts_with("N-") {
            let n_key = format!("N-{key}");
            if let Some(glass) = self.glasses_by_name.get(&n_key) {
                return Some(glass);
            }
        } else if let Some(stripped) = key.strip_prefix("N-") {
            // Try stripping N- prefix (e.g. "N-F2" -> "F2")
            if let Some(glass) = self.glasses_by_name.get(stripped) {
                return Some(glass);
            }
        }

        None
    }

    /// Returns the total count of loaded glasses in the database.
    #[must_use]
    pub fn len(&self) -> usize {
        self.glasses_by_name.len()
    }

    /// Returns true if the database contains no glasses.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glasses_by_name.is_empty()
    }

    /// Returns an iterator over all loaded glasses in the database.
    pub fn all_glasses(&self) -> impl Iterator<Item = &GlassMaterial> {
        self.glasses_by_name.values()
    }

    /// Searches the glass database for all materials matching refractive index and Abbe number intervals.
    ///
    /// Ranges are supplied as `(min, max)` tuples. If a range is inverted (e.g. `(max, min)`),
    /// it is automatically normalized.
    #[must_use]
    pub fn search_by_nd_vd(
        &self,
        nd_range: (f64, f64),
        vd_range: (f64, f64),
    ) -> Vec<&GlassMaterial> {
        let (nd_min, nd_max) = (nd_range.0.min(nd_range.1), nd_range.0.max(nd_range.1));
        let (vd_min, vd_max) = (vd_range.0.min(vd_range.1), vd_range.0.max(vd_range.1));

        self.glasses_by_name
            .values()
            .filter(|g| g.nd >= nd_min && g.nd <= nd_max && g.vd >= vd_min && g.vd <= vd_max)
            .collect()
    }
}

/// Parses a model glass string like "N_1.6375_V56.1", "1.6375_56.1", or "1.6375 56.1" into (nd, vd).
fn parse_model_glass_string(s: &str) -> Option<(f64, f64)> {
    let clean = s.trim().to_uppercase();
    let stripped = clean.strip_prefix("N_").unwrap_or(&clean);
    let parts: Vec<&str> = stripped.split('_').collect();
    if parts.len() == 2 {
        let nd: f64 = parts[0].parse().ok()?;
        let vd_str = parts[1].strip_prefix('V').unwrap_or(parts[1]);
        let vd: f64 = vd_str.parse().ok()?;
        if nd > 1.05 && nd < 3.5 && vd > 1.0 && vd < 150.0 {
            return Some((nd, vd));
        }
    }
    let space_parts: Vec<&str> = clean.split_whitespace().collect();
    if space_parts.len() == 2 {
        let nd: f64 = space_parts[0].parse().ok()?;
        let vd: f64 = space_parts[1].parse().ok()?;
        if nd > 1.05 && nd < 3.5 && vd > 1.0 && vd < 150.0 {
            return Some((nd, vd));
        }
    }
    None
}

impl optics_core::MaterialResolver for GlassCatalogDb {
    fn resolve(
        &self,
        name: &str,
    ) -> Result<std::sync::Arc<dyn optics_core::RefractiveIndex + Send + Sync>, OpticsError> {
        let trimmed = name.trim();
        let upper = trimmed.to_uppercase();

        // 1. Water immersion medium (nd = 1.3330, vd = 55.7)
        if upper == "WATER" || upper == "H2O" {
            let water = GlassMaterial::model_glass("WATER", 1.3330, 55.7, 0.0);
            return Ok(std::sync::Arc::new(water));
        }

        // 2. Direct catalog lookup
        if let Some(g) = self.lookup(trimmed) {
            return Ok(std::sync::Arc::new(g.clone()));
        }

        // 3. Special classic Schott FK54
        if upper == "FK54" || upper == "N-FK54" {
            let fk54 = GlassMaterial::model_glass("FK54", 1.4370, 90.7, 0.0);
            return Ok(std::sync::Arc::new(fk54));
        }

        // 4. Model glass parameterization (e.g. from patent prescriptions: "N_1.6375_V56.1")
        if let Some((nd, vd)) = parse_model_glass_string(trimmed) {
            let model = GlassMaterial::model_glass(trimmed, nd, vd, 0.0);
            return Ok(std::sync::Arc::new(model));
        }

        // 5. Fallback crown glass (N-BK7) so real external files never crash the analysis diagnostics
        let fallback = self.lookup("N-BK7")
            .cloned()
            .unwrap_or_else(GlassMaterial::n_bk7);
        Ok(std::sync::Arc::new(fallback))
    }
}
