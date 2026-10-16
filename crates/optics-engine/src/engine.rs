//! Unified OpticalEngine facade orchestrating all computational modules.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use optics_core::OpticalPrescription;
use optics_core::analysis::{
    PupilPattern, compute_diffraction_mtf, compute_fft_psf, compute_field_curves,
    compute_ray_table, compute_seidel_coefficients, compute_through_focus_spot,
    compute_two_bounce_ghosts, compute_wavefront_map, fit_zernike_noll,
    generate_prescription_summary, trace_ray_fan, trace_spot_diagram,
};
use optics_core::prescription::{
    calculate_paraxial_properties, trace_paraxial_chief, trace_paraxial_marginal,
};
use optics_glass::GlassCatalogDb;
use optics_zemax::cad::{
    DxfConfig, SvgConfig, export_dxf_layout, export_obj_system, export_stl_lens, export_svg_layout,
};
use optics_zemax::codev::{CodeVSystem, parse_seq, write_seq};
use optics_zemax::iso10110::Iso10110DrawingSheet;
use optics_zemax::oslo::{OsloSystem, parse_len, write_len};
use optics_zemax::parser::parse_zmx;
use optics_zemax::project::OpticsProjectWorkspace;
use optics_zemax::serializer::write_prescription_zmx;

use crate::commands::{EngineCommand, UndoStack};
use crate::error::EngineError;
use crate::events::{EngineEvent, EventListener};
use crate::types::{
    AnalysisRequest, AnalysisResult, EngineStatus, GlassSummary, ParaxialAnalysisResult,
    ParaxialRayData,
};
use crate::validation::{DiagnosticMessage, validate_prescription};

/// Unified high-level optical engineering engine facade.
pub struct OpticalEngine {
    prescription: OpticalPrescription,
    catalog: Arc<GlassCatalogDb>,
    revision: u64,
    is_dirty: bool,
    undo_stack: UndoStack,
    listeners: Vec<EventListener>,
    cache: HashMap<(String, u64), AnalysisResult>,
}

impl OpticalEngine {
    /// Initializes a new optical engine with bundled glass database and a default Cooke Triplet F/4.5 preset.
    ///
    /// # Errors
    /// Returns [`EngineError`] if bundled glass catalog loading fails.
    pub fn new() -> Result<Self, EngineError> {
        let catalog = Arc::new(
            GlassCatalogDb::load_bundled()
                .map_err(|e| EngineError::InvalidParameter(e.to_string()))?,
        );
        let prescription = optics_zemax::presets::cooke_triplet_f4_5()?;
        Ok(Self {
            prescription,
            catalog,
            revision: 1,
            is_dirty: false,
            undo_stack: UndoStack::default(),
            listeners: Vec::new(),
            cache: HashMap::new(),
        })
    }

    /// Initializes an optical engine with an explicit optical prescription.
    ///
    /// # Errors
    /// Returns [`EngineError`] if bundled glass catalog loading fails.
    pub fn with_prescription(prescription: OpticalPrescription) -> Result<Self, EngineError> {
        let catalog = Arc::new(
            GlassCatalogDb::load_bundled()
                .map_err(|e| EngineError::InvalidParameter(e.to_string()))?,
        );
        Ok(Self {
            prescription,
            catalog,
            revision: 1,
            is_dirty: false,
            undo_stack: UndoStack::default(),
            listeners: Vec::new(),
            cache: HashMap::new(),
        })
    }

    /// Returns a shared reference to the active optical prescription.
    pub fn prescription(&self) -> &OpticalPrescription {
        &self.prescription
    }

    /// Returns a shared reference to the glass catalog database.
    pub fn glass_catalog(&self) -> &GlassCatalogDb {
        &self.catalog
    }

    /// Returns the current prescription revision sequence number.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns whether the engine contains unsaved or modified state.
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Marks the engine state as committed / not dirty.
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }

    /// Returns current optical engine status and summary metadata.
    pub fn status(&self) -> EngineStatus {
        let efl = if let Ok(res) = self.prescription.resolve(&*self.catalog) {
            calculate_paraxial_properties(&res, self.primary_wavelength())
                .ok()
                .map(|p| p.efl_mm)
        } else {
            None
        };

        let stop_surface_index = self
            .prescription
            .surfaces
            .iter()
            .position(|s| s.is_stop)
            .unwrap_or(0);

        let total_track_mm = self
            .prescription
            .surfaces
            .iter()
            .map(|s| s.thickness_mm)
            .sum();

        EngineStatus {
            revision: self.revision,
            system_name: self.prescription.name.clone(),
            surface_count: self.prescription.surfaces.len(),
            stop_surface_index,
            entrance_pupil_diameter_mm: self.prescription.entrance_pupil_diameter_mm,
            primary_wavelength_nm: self.primary_wavelength(),
            wavelengths_nm: self.prescription.wavelengths_nm.clone(),
            field_angles_deg: self.prescription.field_angles_deg.clone(),
            total_track_mm,
            efl_mm: efl,
            is_dirty: self.is_dirty,
            can_undo: self.undo_stack.can_undo(),
            can_redo: self.undo_stack.can_redo(),
        }
    }

    /// Primary design wavelength in nanometers.
    pub fn primary_wavelength(&self) -> f64 {
        self.prescription
            .wavelengths_nm
            .get(self.prescription.primary_wavelength_index)
            .copied()
            .unwrap_or(587.56)
    }

    /// Subscribes an event listener callback to engine state transitions.
    pub fn subscribe<F>(&mut self, listener: F)
    where
        F: Fn(&EngineEvent) + Send + Sync + 'static,
    {
        self.listeners.push(Box::new(listener));
    }

    /// Broadcasts an event to all registered subscribers.
    fn emit(&self, event: EngineEvent) {
        for listener in &self.listeners {
            listener(&event);
        }
    }

    /// Loads an optical prescription directly, replacing the current state.
    pub fn load_prescription(&mut self, p: OpticalPrescription) {
        self.prescription = p;
        self.revision += 1;
        self.is_dirty = true;
        self.undo_stack.clear();
        self.cache.clear();

        self.emit(EngineEvent::PrescriptionLoaded {
            title: self.prescription.name.clone(),
            revision: self.revision,
        });
        self.emit_diagnostics();
    }

    /// Loads an optical prescription from a Zemax `.zmx` formatted string.
    pub fn load_zmx_str(&mut self, zmx: &str) -> Result<(), EngineError> {
        let sys = parse_zmx(zmx)?;
        self.load_prescription(sys.to_optical_prescription());
        Ok(())
    }

    /// Loads an optical prescription from a Synopsys CODE V `.seq` macro string.
    pub fn load_seq_str(&mut self, seq: &str) -> Result<(), EngineError> {
        let sys = parse_seq(seq)?;
        self.load_prescription(sys.to_optical_prescription());
        Ok(())
    }

    /// Loads an optical prescription from a Lambda Research OSLO `.len` string.
    pub fn load_oslo_str(&mut self, len: &str) -> Result<(), EngineError> {
        let sys = parse_len(len)?;
        self.load_prescription(sys.to_optical_prescription());
        Ok(())
    }

    /// Loads an optical system from a JSON serialized [`OpticsProjectWorkspace`].
    pub fn load_project_json(&mut self, json_str: &str) -> Result<(), EngineError> {
        let ws: OpticsProjectWorkspace = serde_json::from_str(json_str)
            .map_err(|e| EngineError::InvalidParameter(e.to_string()))?;
        let p = ws
            .configurations
            .first()
            .cloned()
            .unwrap_or_else(OpticalPrescription::default);
        self.load_prescription(p);
        Ok(())
    }

    /// Executes an editing command on the prescription, updating revision and invalidating caches.
    pub fn execute(&mut self, cmd: EngineCommand) -> Result<String, EngineError> {
        let desc = self.undo_stack.execute(cmd, &mut self.prescription)?;
        self.revision += 1;
        self.is_dirty = true;
        self.cache.clear();

        self.emit(EngineEvent::PrescriptionChanged {
            revision: self.revision,
            description: desc.clone(),
        });
        self.emit_diagnostics();
        Ok(desc)
    }

    /// Undoes the most recent command.
    pub fn undo(&mut self) -> Result<String, EngineError> {
        let desc = self.undo_stack.undo(&mut self.prescription)?;
        self.revision += 1;
        self.is_dirty = true;
        self.cache.clear();

        self.emit(EngineEvent::PrescriptionChanged {
            revision: self.revision,
            description: desc.clone(),
        });
        self.emit_diagnostics();
        Ok(desc)
    }

    /// Redoes the previously undone command.
    pub fn redo(&mut self) -> Result<String, EngineError> {
        let desc = self.undo_stack.redo(&mut self.prescription)?;
        self.revision += 1;
        self.is_dirty = true;
        self.cache.clear();

        self.emit(EngineEvent::PrescriptionChanged {
            revision: self.revision,
            description: desc.clone(),
        });
        self.emit_diagnostics();
        Ok(desc)
    }

    /// Returns whether an undo operation can be performed.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether a redo operation can be performed.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Validates the current prescription, returning human-readable diagnostic messages.
    pub fn validate(&self) -> Vec<DiagnosticMessage> {
        validate_prescription(&self.prescription, &self.catalog)
    }

    fn emit_diagnostics(&self) {
        let diags = self.validate();
        self.emit(EngineEvent::DiagnosticsUpdated { diagnostics: diags });
    }

    /// Runs an optical analysis calculation, utilizing dirty-aware result caching.
    pub fn run_analysis(&mut self, req: &AnalysisRequest) -> Result<AnalysisResult, EngineError> {
        let req_key =
            serde_json::to_string(req).map_err(|e| EngineError::InvalidParameter(e.to_string()))?;
        let key = (req_key, self.revision);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }

        let start = Instant::now();
        let resolved = self.prescription.resolve(&*self.catalog)?;
        let primary_wl = self.primary_wavelength();

        let result = match req {
            AnalysisRequest::Paraxial => {
                let props = calculate_paraxial_properties(&resolved, primary_wl)?;
                let marginal = trace_paraxial_marginal(&resolved, primary_wl)?;
                let chief = trace_paraxial_chief(
                    &resolved,
                    self.prescription
                        .field_angles_deg
                        .first()
                        .copied()
                        .unwrap_or(0.0),
                    primary_wl,
                )?;

                AnalysisResult::Paraxial(ParaxialAnalysisResult {
                    efl_mm: props.efl_mm,
                    bfl_mm: props.bfl_mm,
                    f_number: props.f_number,
                    entrance_pupil_diameter_mm: self.prescription.entrance_pupil_diameter_mm,
                    entrance_pupil_position_mm: props.entrance_pupil_pos_mm,
                    exit_pupil_position_mm: props.exit_pupil_pos_mm,
                    marginal_ray: marginal
                        .into_iter()
                        .map(|r| ParaxialRayData {
                            y_mm: r.y,
                            u_rad: r.u,
                        })
                        .collect(),
                    chief_ray: chief
                        .into_iter()
                        .map(|r| ParaxialRayData {
                            y_mm: r.y,
                            u_rad: r.u,
                        })
                        .collect(),
                })
            }
            AnalysisRequest::SpotDiagram {
                field_index,
                wavelength_nm,
                ray_count,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let wl = wavelength_nm.unwrap_or(primary_wl);
                let num_rings = ((*ray_count as f64).sqrt() / 2.0).round().max(2.0) as usize;
                let spot = trace_spot_diagram(
                    &resolved,
                    (0.0, f_deg),
                    wl,
                    PupilPattern::Hexapolar { num_rings },
                )?;
                AnalysisResult::SpotDiagram(spot)
            }
            AnalysisRequest::RayFan {
                field_index,
                wavelength_nm,
                num_rays,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let wl = wavelength_nm.unwrap_or(primary_wl);
                let fan = trace_ray_fan(&resolved, (0.0, f_deg), wl, *num_rays)?;
                AnalysisResult::RayFan(fan)
            }
            AnalysisRequest::Wavefront {
                field_index,
                wavelength_nm,
                grid_size,
                zernike_terms,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let wl = wavelength_nm.unwrap_or(primary_wl);
                let wf = compute_wavefront_map(&resolved, *grid_size, f_deg, wl, true)?;
                let zernike = if *zernike_terms > 0 {
                    fit_zernike_noll(&wf, *zernike_terms).ok()
                } else {
                    None
                };
                AnalysisResult::Wavefront {
                    wavefront: wf,
                    zernike,
                }
            }
            AnalysisRequest::Seidel => {
                let seidel = compute_seidel_coefficients(&resolved, 0.0, primary_wl)?;
                AnalysisResult::Seidel(seidel)
            }
            AnalysisRequest::Psf {
                field_index,
                grid_size,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let pupil_grid = (*grid_size).min(64).next_power_of_two();
                let fft_size = pupil_grid.max(64);
                let psf = compute_fft_psf(&resolved, pupil_grid, fft_size, f_deg, primary_wl)?;
                AnalysisResult::Psf(psf)
            }
            AnalysisRequest::Mtf {
                field_index,
                max_freq_lp_mm: _,
                num_points,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let mtf = compute_diffraction_mtf(&resolved, *num_points, f_deg, primary_wl)?;
                AnalysisResult::Mtf(mtf)
            }
            AnalysisRequest::FieldCurves { num_points } => {
                let max_f = self
                    .prescription
                    .field_angles_deg
                    .iter()
                    .copied()
                    .fold(0.0_f64, f64::max)
                    .max(1.0);
                let fc = compute_field_curves(&resolved, *num_points, max_f, primary_wl)?;
                AnalysisResult::FieldCurves(fc)
            }
            AnalysisRequest::RayTable {
                field_index,
                pupil_x,
                pupil_y,
                wavelength_nm,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let wl = wavelength_nm.unwrap_or(primary_wl);
                let rt = compute_ray_table(&resolved, *pupil_x, *pupil_y, f_deg, wl)?;
                AnalysisResult::RayTable(rt)
            }
            AnalysisRequest::Ghosts { field_index: _ } => {
                let ghosts = compute_two_bounce_ghosts(&resolved, primary_wl, 0.005)?;
                AnalysisResult::Ghosts(ghosts)
            }
            AnalysisRequest::ThroughFocus {
                field_index,
                range_mm,
                steps,
            } => {
                let f_deg = self
                    .prescription
                    .field_angles_deg
                    .get(*field_index)
                    .copied()
                    .unwrap_or(0.0);
                let tf = compute_through_focus_spot(
                    &resolved,
                    PupilPattern::Hexapolar { num_rings: 4 },
                    f_deg,
                    primary_wl,
                    *range_mm,
                    *steps,
                )?;
                AnalysisResult::ThroughFocus(tf)
            }
            AnalysisRequest::FullReport => {
                let summary = generate_prescription_summary(&resolved, primary_wl)?;
                AnalysisResult::FullReport(summary)
            }
        };

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.cache.insert(key, result.clone());

        self.emit(EngineEvent::AnalysisCompleted {
            analysis_type: format!("{req:?}"),
            duration_ms: elapsed_ms,
        });

        Ok(result)
    }

    /// Exports the optical system to a 2D SVG layout vector drawing.
    pub fn export_svg(&self, config: Option<&SvgConfig>) -> Result<String, EngineError> {
        let default_cfg = SvgConfig::default();
        let cfg = config.unwrap_or(&default_cfg);
        Ok(export_svg_layout(&self.prescription, None, cfg)?)
    }

    /// Exports the optical system to AutoCAD Release 12 DXF format.
    pub fn export_dxf(&self, config: Option<&DxfConfig>) -> Result<String, EngineError> {
        let default_cfg = DxfConfig::default();
        let cfg = config.unwrap_or(&default_cfg);
        Ok(export_dxf_layout(&self.prescription, None, cfg)?)
    }

    /// Exports the 3D surface mesh in binary or ASCII STL format.
    pub fn export_stl(&self, binary: bool, slices: u32, rings: u32) -> Vec<u8> {
        export_stl_lens(&self.prescription, binary, slices, rings)
    }

    /// Exports the 3D optical assembly as Wavefront OBJ and MTL pair.
    pub fn export_obj(&self, slices: u32, rings: u32) -> (String, String) {
        export_obj_system(&self.prescription, slices, rings)
    }

    /// Generates ISO 10110 manufacturing drawing sheet data in Markdown format.
    pub fn export_iso10110_markdown(&self) -> String {
        Iso10110DrawingSheet::from_prescription(&self.prescription, Some(&*self.catalog))
            .to_markdown_table()
    }

    /// Generates ISO 10110 manufacturing drawing sheet data in JSON format.
    pub fn export_iso10110_json(&self) -> Result<String, EngineError> {
        Iso10110DrawingSheet::from_prescription(&self.prescription, Some(&*self.catalog))
            .to_json()
            .map_err(|e| EngineError::InvalidParameter(e.to_string()))
    }

    /// Exports the prescription to a Zemax `.zmx` formatted string.
    pub fn export_zmx(&self) -> Result<String, EngineError> {
        Ok(write_prescription_zmx(&self.prescription))
    }

    /// Exports the prescription to a Synopsys CODE V `.seq` macro string.
    pub fn export_seq(&self) -> Result<String, EngineError> {
        let sys = CodeVSystem::from_optical_prescription(&self.prescription);
        Ok(write_seq(&sys))
    }

    /// Exports the prescription to a Lambda Research OSLO `.len` macro string.
    pub fn export_len(&self) -> Result<String, EngineError> {
        let sys = OsloSystem::from_optical_prescription(&self.prescription);
        Ok(write_len(&sys))
    }

    /// Exports the system into a versioned [`OpticsProjectWorkspace`] JSON string.
    pub fn export_project_json(&self) -> Result<String, EngineError> {
        let ws =
            OpticsProjectWorkspace::new(self.prescription.name.clone(), self.prescription.clone());
        ws.to_json()
            .map_err(|e| EngineError::InvalidParameter(e.to_string()))
    }

    /// Searches the glass database by name substring.
    pub fn search_glasses(&self, query: &str) -> Vec<GlassSummary> {
        let q = query.trim().to_uppercase();
        self.catalog
            .all_glasses()
            .filter(|g| {
                g.name.to_uppercase().contains(&q) || g.catalog.name().to_uppercase().contains(&q)
            })
            .map(|g| GlassSummary {
                catalog: g.catalog.name().to_string(),
                name: g.name.clone(),
                nd: g.nd,
                vd: g.vd,
                density: None,
            })
            .collect()
    }

    /// Searches the glass database filtered by refractive index ($n_d$) and Abbe dispersion ($V_d$).
    pub fn search_glasses_by_nd_vd(
        &self,
        nd_min: f64,
        nd_max: f64,
        vd_min: f64,
        vd_max: f64,
    ) -> Vec<GlassSummary> {
        self.catalog
            .search_by_nd_vd((nd_min, nd_max), (vd_min, vd_max))
            .into_iter()
            .map(|g| GlassSummary {
                catalog: g.catalog.name().to_string(),
                name: g.name.clone(),
                nd: g.nd,
                vd: g.vd,
                density: None,
            })
            .collect()
    }
}
