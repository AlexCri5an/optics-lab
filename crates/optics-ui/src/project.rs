//! Project serialization format and file schema.

use crate::app::AppMode;
use crate::theme::AppPreferences;
use serde::{Deserialize, Serialize};

/// Current project file schema version.
pub const PROJECT_FORMAT_VERSION: u32 = 1;

/// Complete serializable project state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Format schema version.
    pub format_version: u32,
    /// Active application mode.
    pub app_mode: AppMode,
    /// Prism angle of incidence in degrees.
    pub prism_incidence_deg: f64,
    /// Prism apex angle in degrees.
    pub prism_apex_deg: f64,
    /// Selected glass material name.
    pub prism_glass_name: String,
    /// Spectral sample ray count.
    pub prism_sample_count: usize,
    /// Currently inspected wavelength in nanometers.
    pub inspected_wavelength: f64,
    /// User preferences.
    pub prefs: AppPreferences,
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            format_version: PROJECT_FORMAT_VERSION,
            app_mode: AppMode::PrismDispersion,
            prism_incidence_deg: 45.0,
            prism_apex_deg: 60.0,
            prism_glass_name: "N-BK7".to_string(),
            prism_sample_count: 32,
            inspected_wavelength: 550.0,
            prefs: AppPreferences::default(),
        }
    }
}

impl ProjectFile {
    /// Serializes project state to formatted JSON bytes.
    ///
    /// # Errors
    /// Returns serialization error if conversion fails.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// Deserializes project state from JSON bytes, verifying format version.
    ///
    /// # Errors
    /// Returns error if parsing fails or version is unsupported.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, String> {
        let proj: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid project JSON: {e}"))?;
        if proj.format_version > PROJECT_FORMAT_VERSION {
            return Err(format!(
                "Unsupported project version {} (highest supported is {})",
                proj.format_version, PROJECT_FORMAT_VERSION
            ));
        }
        Ok(proj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_round_trip() {
        let proj = ProjectFile::default();
        let bytes_res = proj.to_json_bytes();
        assert!(bytes_res.is_ok(), "Serialization failed");
        if let Ok(bytes) = bytes_res {
            let parsed_res = ProjectFile::from_json_bytes(&bytes);
            assert!(parsed_res.is_ok(), "Deserialization failed");
            if let Ok(parsed) = parsed_res {
                assert_eq!(proj, parsed);
            }
        }
    }
}
