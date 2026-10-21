//! Unified versioned OpticsLab project file schema and workspace management.
//!
//! Provides the primary `.optics` / `.json` project format (`schema_version = "1.0.0"`)
//! supporting multi-configuration optical prescriptions, glass catalog preferences,
//! metadata annotations, and direct conversion to/from `.zmx`, CODE V (`.seq`),
//! and OSLO (`.len`) file formats.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use optics_core::OpticalPrescription;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cad::{DxfConfig, SvgConfig, export_dxf_layout, export_obj_system, export_stl_lens, export_svg_layout};
use crate::codev::{CodeVSystem, parse_seq, write_seq};
use crate::model::FormatOpticalSystem;
use crate::oslo::{OsloSystem, parse_len, write_len};
use crate::parser::parse_zmx;
use crate::serializer::write_zmx;

/// The current project schema version string.
pub const OPTICS_PROJECT_SCHEMA_VERSION: &str = "1.0.0";

/// Errors that can occur during project I/O and format interchange.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// File I/O failure.
    #[error("Project I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization or deserialization failure.
    #[error("Project serialization error: {0}")]
    Json(#[from] serde_json::Error),
    /// Schema version incompatibility.
    #[error("Incompatible project schema version '{found}' (expected '{expected}')")]
    VersionMismatch {
        /// Found version.
        found: String,
        /// Expected version.
        expected: String,
    },
    /// Format import error.
    #[error("Import failure: {0}")]
    ImportError(String),
}

/// Project authorship and environmental metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Project author or engineer name.
    pub author: String,
    /// Organization or company.
    pub organization: String,
    /// Creation ISO timestamp or date.
    pub created_at: String,
    /// Last modified ISO timestamp or date.
    pub modified_at: String,
    /// Free-form engineering annotations and notes.
    pub notes: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            author: "OpticsLab User".to_string(),
            organization: "OpticsLab".to_string(),
            created_at: "2026-10-16T00:00:00Z".to_string(),
            modified_at: "2026-10-16T00:00:00Z".to_string(),
            notes: String::new(),
        }
    }
}

/// CAD and export preferences saved with the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectCadPreferences {
    /// SVG viewport width.
    pub svg_width: u32,
    /// SVG viewport height.
    pub svg_height: u32,
    /// Azimuthal divisions for 3D meshes.
    pub mesh_slices: u32,
    /// Radial concentric divisions for 3D meshes.
    pub mesh_rings: u32,
    /// Default export units ("mm", "cm", "in").
    pub units: String,
}

impl Default for ProjectCadPreferences {
    fn default() -> Self {
        Self {
            svg_width: 1200,
            svg_height: 600,
            mesh_slices: 32,
            mesh_rings: 12,
            units: "mm".to_string(),
        }
    }
}

/// Complete multi-configuration OpticsLab project workspace (`schema_version = "1.0.0"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticsProjectWorkspace {
    /// Project schema version string (e.g. "1.0.0").
    pub schema_version: String,
    /// Project title.
    pub name: String,
    /// Engineering metadata and notes.
    pub metadata: ProjectMetadata,
    /// CAD and rendering preferences.
    pub cad_preferences: ProjectCadPreferences,
    /// Multi-configuration optical prescriptions (e.g. zoom positions, thermal states).
    pub configurations: Vec<OpticalPrescription>,
    /// 0-based index of the currently active configuration.
    pub active_configuration: usize,
}

impl Default for OpticsProjectWorkspace {
    fn default() -> Self {
        Self {
            schema_version: OPTICS_PROJECT_SCHEMA_VERSION.to_string(),
            name: "New Optical Project".to_string(),
            metadata: ProjectMetadata::default(),
            cad_preferences: ProjectCadPreferences::default(),
            configurations: Vec::new(),
            active_configuration: 0,
        }
    }
}

impl OpticsProjectWorkspace {
    /// Creates a new workspace with a single initial optical prescription.
    #[must_use]
    pub fn new(name: impl Into<String>, prescription: OpticalPrescription) -> Self {
        let name_str = name.into();
        Self {
            schema_version: OPTICS_PROJECT_SCHEMA_VERSION.to_string(),
            name: name_str,
            metadata: ProjectMetadata::default(),
            cad_preferences: ProjectCadPreferences::default(),
            configurations: vec![prescription],
            active_configuration: 0,
        }
    }

    /// Returns the currently active optical prescription, if any exists.
    #[must_use]
    pub fn active_prescription(&self) -> Option<&OpticalPrescription> {
        self.configurations.get(self.active_configuration)
    }

    /// Serializes the project workspace to a formatted JSON string.
    ///
    /// # Errors
    /// Returns [`ProjectError::Json`] if serialization fails.
    pub fn to_json(&self) -> Result<String, ProjectError> {
        serde_json::to_string_pretty(self).map_err(ProjectError::Json)
    }

    /// Deserializes a project workspace from a JSON string, verifying schema version compatibility.
    ///
    /// # Errors
    /// Returns [`ProjectError`] if JSON parsing fails or schema version is incompatible.
    pub fn from_json(json: &str) -> Result<Self, ProjectError> {
        let project: Self = serde_json::from_str(json).map_err(ProjectError::Json)?;
        if !project.schema_version.starts_with("1.") {
            return Err(ProjectError::VersionMismatch {
                found: project.schema_version,
                expected: OPTICS_PROJECT_SCHEMA_VERSION.to_string(),
            });
        }
        Ok(project)
    }

    /// Saves the project workspace to a file on disk.
    ///
    /// # Errors
    /// Returns [`ProjectError`] on I/O or encoding errors.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ProjectError> {
        let json = self.to_json()?;
        let mut file = File::create(path.as_ref())?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Loads and parses a project workspace file from disk.
    ///
    /// # Errors
    /// Returns [`ProjectError`] on I/O or parsing errors.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ProjectError> {
        let mut file = File::open(path.as_ref())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Self::from_json(&contents)
    }

    /// Imports an optical `.zmx` prescription file into a new project workspace.
    ///
    /// # Errors
    /// Returns [`ProjectError::ImportError`] if parsing fails.
    pub fn import_zmx(name: &str, zmx_content: &str) -> Result<Self, ProjectError> {
        let zmx_sys =
            parse_zmx(zmx_content).map_err(|e| ProjectError::ImportError(format!("ZMX import error: {e}")))?;
        let prescription = zmx_sys.to_optical_prescription();
        Ok(Self::new(name, prescription))
    }

    /// Imports a CODE V `.seq` prescription file into a new project workspace.
    ///
    /// # Errors
    /// Returns [`ProjectError::ImportError`] if parsing fails.
    pub fn import_codev(name: &str, seq_content: &str) -> Result<Self, ProjectError> {
        let codev_sys =
            parse_seq(seq_content).map_err(|e| ProjectError::ImportError(format!("CODE V import error: {e}")))?;
        let prescription = codev_sys.to_optical_prescription();
        Ok(Self::new(name, prescription))
    }

    /// Imports an OSLO `.len` prescription file into a new project workspace.
    ///
    /// # Errors
    /// Returns [`ProjectError::ImportError`] if parsing fails.
    pub fn import_oslo(name: &str, len_content: &str) -> Result<Self, ProjectError> {
        let oslo_sys =
            parse_len(len_content).map_err(|e| ProjectError::ImportError(format!("OSLO import error: {e}")))?;
        let prescription = oslo_sys.to_optical_prescription();
        Ok(Self::new(name, prescription))
    }

    /// Exports the active optical prescription to `.zmx` format string.
    #[must_use]
    pub fn export_active_zmx(&self) -> Option<String> {
        let rx = self.active_prescription()?;
        let sys = FormatOpticalSystem::from_optical_prescription(rx);
        Some(write_zmx(&sys))
    }

    /// Exports the active optical prescription to CODE V `.seq` format string.
    #[must_use]
    pub fn export_active_codev(&self) -> Option<String> {
        let rx = self.active_prescription()?;
        let sys = CodeVSystem::from_optical_prescription(rx);
        Some(write_seq(&sys))
    }

    /// Exports the active optical prescription to OSLO `.len` format string.
    #[must_use]
    pub fn export_active_oslo(&self) -> Option<String> {
        let rx = self.active_prescription()?;
        let sys = OsloSystem::from_optical_prescription(rx);
        Some(write_len(&sys))
    }

    /// Exports the active prescription to a 2D SVG layout string.
    #[must_use]
    pub fn export_active_svg(&self) -> Option<String> {
        let rx = self.active_prescription()?;
        let config = SvgConfig {
            width: self.cad_preferences.svg_width,
            height: self.cad_preferences.svg_height,
            ..Default::default()
        };
        export_svg_layout(rx, None, &config).ok()
    }

    /// Exports the active prescription to an AutoCAD DXF string.
    #[must_use]
    pub fn export_active_dxf(&self, is_3d: bool) -> Option<String> {
        let rx = self.active_prescription()?;
        let config = DxfConfig {
            export_3d: is_3d,
            slices: self.cad_preferences.mesh_slices,
            rings: self.cad_preferences.mesh_rings,
        };
        export_dxf_layout(rx, None, &config).ok()
    }

    /// Exports the active prescription to 3D STL binary bytes.
    #[must_use]
    pub fn export_active_stl_binary(&self) -> Option<Vec<u8>> {
        let rx = self.active_prescription()?;
        Some(export_stl_lens(
            rx,
            true,
            self.cad_preferences.mesh_slices,
            self.cad_preferences.mesh_rings,
        ))
    }

    /// Exports the active prescription to 3D Wavefront OBJ and MTL strings.
    #[must_use]
    pub fn export_active_obj(&self) -> Option<(String, String)> {
        let rx = self.active_prescription()?;
        Some(export_obj_system(
            rx,
            self.cad_preferences.mesh_slices,
            self.cad_preferences.mesh_rings,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::OpticalSurface;

    fn sample_prescription() -> OpticalPrescription {
        OpticalPrescription {
            name: "Test Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 6.0,
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
                    radius_mm: -100.0,
                    conic_constant: 0.0,
                    thickness_mm: 45.0,
                    semi_diameter_mm: 6.0,
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
        }
    }

    #[test]
    fn test_project_workspace_roundtrip() -> Result<(), ProjectError> {
        let rx = sample_prescription();
        let mut workspace = OpticsProjectWorkspace::new("Singlet Workspace", rx);
        workspace.metadata.author = "Alexander".to_string();

        let json = workspace.to_json()?;
        let loaded = OpticsProjectWorkspace::from_json(&json)?;

        assert_eq!(workspace.schema_version, loaded.schema_version);
        assert_eq!(workspace.name, loaded.name);
        assert_eq!(loaded.metadata.author, "Alexander");
        assert_eq!(loaded.configurations.len(), 1);
        assert_eq!(loaded.configurations[0].name, "Test Singlet");

        // Test export routines
        assert!(workspace.export_active_zmx().is_some());
        assert!(workspace.export_active_codev().is_some());
        assert!(workspace.export_active_oslo().is_some());
        assert!(workspace.export_active_svg().is_some());
        assert!(workspace.export_active_dxf(false).is_some());
        assert!(workspace.export_active_stl_binary().is_some());
        assert!(workspace.export_active_obj().is_some());

        Ok(())
    }

    #[test]
    fn test_project_disk_persistence() -> Result<(), ProjectError> {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("optics_lab_workspace_test.optics");

        let rx = sample_prescription();
        let workspace = OpticsProjectWorkspace::new("Disk Test Workspace", rx);
        workspace.save_to_file(&file_path)?;

        let loaded = OpticsProjectWorkspace::load_from_file(&file_path)?;
        assert_eq!(workspace.name, loaded.name);
        assert_eq!(loaded.configurations.len(), 1);

        let _ = std::fs::remove_file(file_path);
        Ok(())
    }
}
