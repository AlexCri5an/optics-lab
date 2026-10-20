//! Project serialization format, configuration models, and disk persistence.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::prescription::OpticalPrescription;
use crate::types::Vec3;

/// Current project schema format version.
pub const PROJECT_SCHEMA_VERSION: u32 = 1;

/// Camera orientation and projection configuration for project persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraConfig {
    /// World position of the camera in millimeters.
    pub position: Vec3,
    /// Point of interest in millimeters.
    pub look_at: Vec3,
    /// Camera up reference vector.
    pub up: Vec3,
    /// Vertical field of view in degrees.
    pub fov_deg: f64,
    /// Lens aperture diameter in millimeters (0.0 = ideal pinhole).
    pub aperture_mm: f64,
    /// Distance from the lens center to the plane of focus in millimeters.
    pub focus_dist_mm: f64,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, -100.0),
            look_at: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            fov_deg: 45.0,
            aperture_mm: 0.0,
            focus_dist_mm: 100.0,
        }
    }
}

/// Dynamic path tracing and presentation parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Maximum ray bounce depth.
    pub max_bounces: u32,
    /// Render diagnostic mode (0: Standard, 1: BVH Heatmap, 2: Surface Normals).
    pub render_mode: u32,
    /// Viewport width in pixels.
    pub width: u32,
    /// Viewport height in pixels.
    pub height: u32,
    /// Progressive samples per pixel target.
    pub samples_per_pixel: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            max_bounces: 6,
            render_mode: 0,
            width: 1280,
            height: 720,
            samples_per_pixel: 128,
        }
    }
}

/// High-level optical scene descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneConfig {
    /// Scene or project title.
    pub name: String,
    /// Primary glass material identifier (e.g. "N-BK7", "SF11").
    pub primary_glass: String,
    /// Evaluated spectral wavelength in nanometers.
    pub wavelength_nm: f64,
    /// User annotations and project notes.
    pub notes: String,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            name: "New Optical Project".to_string(),
            primary_glass: "N-BK7".to_string(),
            wavelength_nm: 587.56,
            notes: String::new(),
        }
    }
}

/// Complete serializable OpticsLab project model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticsProject {
    /// Format schema version.
    pub version: u32,
    /// Project title.
    pub name: String,
    /// High-level scene configuration.
    pub scene: SceneConfig,
    /// 3D camera settings.
    pub camera: CameraConfig,
    /// Sequential optical prescription (if active).
    pub prescription: Option<OpticalPrescription>,
    /// Path tracing rendering parameters.
    pub render_params: RenderConfig,
}

impl Default for OpticsProject {
    fn default() -> Self {
        Self {
            version: PROJECT_SCHEMA_VERSION,
            name: "New Optical Project".to_string(),
            scene: SceneConfig::default(),
            camera: CameraConfig::default(),
            prescription: None,
            render_params: RenderConfig::default(),
        }
    }
}

impl OpticsProject {
    /// Creates a new project with the specified name and optional optical prescription.
    #[must_use]
    pub fn new(name: impl Into<String>, prescription: Option<OpticalPrescription>) -> Self {
        let name_str = name.into();
        Self {
            version: PROJECT_SCHEMA_VERSION,
            name: name_str.clone(),
            scene: SceneConfig {
                name: name_str,
                ..Default::default()
            },
            camera: CameraConfig::default(),
            prescription,
            render_params: RenderConfig::default(),
        }
    }

    /// Serializes the project into a formatted JSON string.
    ///
    /// # Errors
    /// Returns [`OpticsError::SerializationError`] if serialization fails.
    pub fn to_json_string(&self) -> Result<String, OpticsError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| OpticsError::SerializationError(e.to_string()))
    }

    /// Deserializes a project from a JSON string, validating the schema version.
    ///
    /// # Errors
    /// Returns [`OpticsError::SerializationError`] if parsing fails or version is unsupported.
    pub fn from_json_str(json: &str) -> Result<Self, OpticsError> {
        let project: Self = serde_json::from_str(json)
            .map_err(|e| OpticsError::SerializationError(e.to_string()))?;
        if project.version > PROJECT_SCHEMA_VERSION {
            return Err(OpticsError::SerializationError(format!(
                "Unsupported project version {} (max supported: {})",
                project.version, PROJECT_SCHEMA_VERSION
            )));
        }
        Ok(project)
    }

    /// Saves the project to a file at the given path.
    ///
    /// # Errors
    /// Returns [`OpticsError::IoError`] if file creation or writing fails, or
    /// [`OpticsError::SerializationError`] if JSON encoding fails.
    pub fn save_to_json<P: AsRef<Path>>(&self, path: P) -> Result<(), OpticsError> {
        let json = self.to_json_string()?;
        let mut file = File::create(path.as_ref())
            .map_err(|e| OpticsError::IoError(format!("Failed to create project file: {e}")))?;
        file.write_all(json.as_bytes())
            .map_err(|e| OpticsError::IoError(format!("Failed to write project data: {e}")))?;
        Ok(())
    }

    /// Loads and parses a project file from the given path.
    ///
    /// # Errors
    /// Returns [`OpticsError::IoError`] if opening or reading the file fails, or
    /// [`OpticsError::SerializationError`] if JSON parsing fails.
    pub fn load_from_json<P: AsRef<Path>>(path: P) -> Result<Self, OpticsError> {
        let mut file = File::open(path.as_ref())
            .map_err(|e| OpticsError::IoError(format!("Failed to open project file: {e}")))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| OpticsError::IoError(format!("Failed to read project file: {e}")))?;
        Self::from_json_str(&contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_project_default_roundtrip() -> TestResult {
        let project = OpticsProject::default();
        let json = project.to_json_string()?;
        let loaded = OpticsProject::from_json_str(&json)?;
        assert_eq!(project, loaded);
        Ok(())
    }

    #[test]
    fn test_project_with_prescription_roundtrip() -> TestResult {
        let prescription = OpticalPrescription {
            name: "Doublet 50mm".to_string(),
            entrance_pupil_diameter_mm: 12.5,
            field_angles_deg: vec![0.0, 5.0, 10.0],
            wavelengths_nm: vec![486.13, 587.56, 656.27],
            primary_wavelength_index: 1,
            surfaces: vec![
                crate::prescription::OpticalSurface {
                    surface_index: 1,
                    radius_mm: 35.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 7.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                },
                crate::prescription::OpticalSurface {
                    surface_index: 2,
                    radius_mm: -35.0,
                    conic_constant: 0.0,
                    thickness_mm: 45.0,
                    semi_diameter_mm: 7.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                },
            ],
        };

        let mut project = OpticsProject::new("Doublet Project", Some(prescription));
        project.camera.position = Vec3::new(10.0, 20.0, -150.0);
        project.render_params.max_bounces = 12;

        let json = project.to_json_string()?;
        let loaded = OpticsProject::from_json_str(&json)?;
        assert_eq!(project, loaded);
        assert_eq!(loaded.camera.position, Vec3::new(10.0, 20.0, -150.0));
        assert_eq!(loaded.render_params.max_bounces, 12);
        assert!(loaded.prescription.is_some());
        Ok(())
    }

    #[test]
    fn test_project_file_io() -> TestResult {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("optics_lab_test_project.json");

        let project = OpticsProject::new("Disk Persistence Test", None);
        project.save_to_json(&file_path)?;

        let loaded = OpticsProject::load_from_json(&file_path)?;
        assert_eq!(project, loaded);

        let _ = std::fs::remove_file(file_path);
        Ok(())
    }
}
