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

            let material = GlassMaterial {
                name: raw.name.clone(),
                catalog,
                nd,
                vd,
                dispersion,
                thermal: None,
                internal_transmittance: Vec::new(),
                test_thickness_mm: 10.0,
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

impl optics_core::MaterialResolver for GlassCatalogDb {
    fn resolve(
        &self,
        name: &str,
    ) -> Result<std::sync::Arc<dyn optics_core::RefractiveIndex + Send + Sync>, OpticsError> {
        self.lookup(name)
            .map(|g| {
                std::sync::Arc::new(g.clone())
                    as std::sync::Arc<dyn optics_core::RefractiveIndex + Send + Sync>
            })
            .ok_or_else(|| OpticsError::MaterialNotFound(name.to_string()))
    }
}
