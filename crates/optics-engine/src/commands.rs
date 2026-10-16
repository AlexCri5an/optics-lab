//! Command pattern and Undo/Redo history stack for non-destructive optical editing.

use serde::{Deserialize, Serialize};

use optics_core::OpticalPrescription;
use optics_core::prescription::OpticalSurface;

use crate::error::EngineError;

/// Partial update structure for modifying an individual optical surface.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SurfaceUpdate {
    /// Optional updated radius of curvature in millimeters.
    pub radius_mm: Option<f64>,
    /// Optional updated conic constant.
    pub conic_constant: Option<f64>,
    /// Optional updated axial thickness in millimeters.
    pub thickness_mm: Option<f64>,
    /// Optional updated semi-diameter in millimeters.
    pub semi_diameter_mm: Option<f64>,
    /// Optional updated material name.
    pub material_name: Option<String>,
    /// Optional updated aperture stop flag.
    pub is_stop: Option<bool>,
}

/// Commands executed on the optical prescription via the engine facade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "command", content = "args")]
pub enum EngineCommand {
    /// Completely replace the active prescription.
    SetPrescription(OpticalPrescription),
    /// Insert a new surface at the specified index.
    AddSurface {
        /// Insertion index.
        index: usize,
        /// New surface data.
        surface: OpticalSurface,
    },
    /// Remove the surface at the specified index.
    RemoveSurface {
        /// Removal index.
        index: usize,
    },
    /// Partially update the properties of an existing surface.
    EditSurface {
        /// Surface index.
        index: usize,
        /// Updated property fields.
        update: SurfaceUpdate,
    },
    /// Set the designated aperture stop surface.
    SetApertureStop {
        /// Surface index.
        index: usize,
    },
    /// Update entrance pupil diameter in millimeters.
    SetEntrancePupilDiameter {
        /// New EPD.
        epd_mm: f64,
    },
    /// Update design wavelengths and primary wavelength index.
    SetWavelengths {
        /// Primary wavelength index.
        primary_index: usize,
        /// Wavelengths in nanometers.
        wavelengths_nm: Vec<f64>,
    },
    /// Update field angles in degrees.
    SetFieldAngles {
        /// Field angles.
        field_angles_deg: Vec<f64>,
    },
}

impl EngineCommand {
    /// Applies this command to the given prescription, returning a human-readable description.
    pub fn apply(&self, p: &mut OpticalPrescription) -> Result<String, EngineError> {
        match self {
            Self::SetPrescription(new_p) => {
                let name = new_p.name.clone();
                *p = new_p.clone();
                Ok(format!("Replaced prescription with '{name}'"))
            }
            Self::AddSurface { index, surface } => {
                if *index > p.surfaces.len() {
                    return Err(EngineError::InvalidParameter(format!(
                        "Insertion index {} exceeds surface count {}",
                        index,
                        p.surfaces.len()
                    )));
                }
                let mut s = surface.clone();
                s.surface_index = *index;
                p.surfaces.insert(*index, s);
                // Re-index surfaces
                for (i, surf) in p.surfaces.iter_mut().enumerate() {
                    surf.surface_index = i;
                }
                Ok(format!("Added surface at index {index}"))
            }
            Self::RemoveSurface { index } => {
                if *index >= p.surfaces.len() {
                    return Err(EngineError::InvalidParameter(format!(
                        "Removal index {} out of bounds ({})",
                        index,
                        p.surfaces.len()
                    )));
                }
                if p.surfaces.len() <= 2 {
                    return Err(EngineError::InvalidParameter(
                        "Cannot remove surface: prescription must retain at least object and image planes".to_string()
                    ));
                }
                p.surfaces.remove(*index);
                for (i, surf) in p.surfaces.iter_mut().enumerate() {
                    surf.surface_index = i;
                }
                Ok(format!("Removed surface at index {index}"))
            }
            Self::EditSurface { index, update } => {
                let surf = p.surfaces.get_mut(*index).ok_or_else(|| {
                    EngineError::InvalidParameter(format!("Surface index {index} out of bounds"))
                })?;
                if let Some(r) = update.radius_mm {
                    surf.radius_mm = r;
                }
                if let Some(k) = update.conic_constant {
                    surf.conic_constant = k;
                }
                if let Some(t) = update.thickness_mm {
                    surf.thickness_mm = t;
                }
                if let Some(sd) = update.semi_diameter_mm {
                    surf.semi_diameter_mm = sd;
                }
                if let Some(ref m) = update.material_name {
                    surf.material_name = m.clone();
                }
                if let Some(is_stop) = update.is_stop {
                    if is_stop {
                        for s in p.surfaces.iter_mut() {
                            s.is_stop = false;
                        }
                    }
                    if let Some(target) = p.surfaces.get_mut(*index) {
                        target.is_stop = is_stop;
                    }
                }
                Ok(format!("Edited surface #{index}"))
            }
            Self::SetApertureStop { index } => {
                if *index >= p.surfaces.len() {
                    return Err(EngineError::InvalidParameter(format!(
                        "Stop index {index} out of bounds"
                    )));
                }
                for (i, s) in p.surfaces.iter_mut().enumerate() {
                    s.is_stop = i == *index;
                }
                Ok(format!("Designated surface #{index} as aperture stop"))
            }
            Self::SetEntrancePupilDiameter { epd_mm } => {
                if *epd_mm <= 0.0 || !epd_mm.is_finite() {
                    return Err(EngineError::InvalidParameter(format!(
                        "Invalid EPD: {epd_mm}"
                    )));
                }
                p.entrance_pupil_diameter_mm = *epd_mm;
                Ok(format!("Set entrance pupil diameter to {epd_mm:.2} mm"))
            }
            Self::SetWavelengths {
                primary_index,
                wavelengths_nm,
            } => {
                if wavelengths_nm.is_empty() {
                    return Err(EngineError::InvalidParameter(
                        "Wavelengths list cannot be empty".to_string(),
                    ));
                }
                if *primary_index >= wavelengths_nm.len() {
                    return Err(EngineError::InvalidParameter(format!(
                        "Primary index {primary_index} out of bounds"
                    )));
                }
                p.wavelengths_nm = wavelengths_nm.clone();
                p.primary_wavelength_index = *primary_index;
                Ok(format!(
                    "Updated {} design wavelengths",
                    wavelengths_nm.len()
                ))
            }
            Self::SetFieldAngles { field_angles_deg } => {
                if field_angles_deg.is_empty() {
                    return Err(EngineError::InvalidParameter(
                        "Field angles list cannot be empty".to_string(),
                    ));
                }
                p.field_angles_deg = field_angles_deg.clone();
                Ok(format!("Updated {} field angles", field_angles_deg.len()))
            }
        }
    }
}

/// A recorded transaction in the undo stack.
#[derive(Debug, Clone)]
struct HistoryEntry {
    description: String,
    before: OpticalPrescription,
    after: OpticalPrescription,
}

/// Robust bounded undo/redo history stack for optical editing.
#[derive(Debug, Clone)]
pub struct UndoStack {
    undo_entries: Vec<HistoryEntry>,
    redo_entries: Vec<HistoryEntry>,
    max_depth: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(100)
    }
}

impl UndoStack {
    /// Creates a new undo stack with maximum history capacity.
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo_entries: Vec::new(),
            redo_entries: Vec::new(),
            max_depth,
        }
    }

    /// Executes a command, recording state transitions and clearing the redo buffer.
    pub fn execute(
        &mut self,
        cmd: EngineCommand,
        target: &mut OpticalPrescription,
    ) -> Result<String, EngineError> {
        let before = target.clone();
        let desc = cmd.apply(target)?;
        let after = target.clone();

        self.undo_entries.push(HistoryEntry {
            description: desc.clone(),
            before,
            after,
        });

        if self.undo_entries.len() > self.max_depth {
            self.undo_entries.remove(0);
        }

        // New modifications clear forward redo tree
        self.redo_entries.clear();
        Ok(desc)
    }

    /// Returns whether an undo operation can be performed.
    pub fn can_undo(&self) -> bool {
        !self.undo_entries.is_empty()
    }

    /// Returns whether a redo operation can be performed.
    pub fn can_redo(&self) -> bool {
        !self.redo_entries.is_empty()
    }

    /// Undoes the most recent command, restoring previous prescription state.
    pub fn undo(&mut self, target: &mut OpticalPrescription) -> Result<String, EngineError> {
        let entry = self
            .undo_entries
            .pop()
            .ok_or_else(|| EngineError::InvalidParameter("No actions to undo".to_string()))?;
        *target = entry.before.clone();
        let desc = format!("Undid: {}", entry.description);
        self.redo_entries.push(entry);
        Ok(desc)
    }

    /// Redoes the previously undone command.
    pub fn redo(&mut self, target: &mut OpticalPrescription) -> Result<String, EngineError> {
        let entry = self
            .redo_entries
            .pop()
            .ok_or_else(|| EngineError::InvalidParameter("No actions to redo".to_string()))?;
        *target = entry.after.clone();
        let desc = format!("Redid: {}", entry.description);
        self.undo_entries.push(entry);
        Ok(desc)
    }

    /// Clears both undo and redo histories.
    pub fn clear(&mut self) {
        self.undo_entries.clear();
        self.redo_entries.clear();
    }
}
