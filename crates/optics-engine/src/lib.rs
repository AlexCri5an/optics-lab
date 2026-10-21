//! High-level unified optical engine facade orchestrating all computational modules.
//!
//! Provides a single unified API entry point (`OpticalEngine`) for UI frontends,
//! CLI tooling, foreign language FFI bindings, and automated scripts.

#![deny(missing_docs)]

pub mod commands;
pub mod engine;
pub mod error;
pub mod events;
pub mod jobs;
pub mod types;
pub mod validation;

pub use commands::{EngineCommand, SurfaceUpdate, UndoStack};
pub use engine::OpticalEngine;
pub use error::EngineError;
pub use events::{EngineEvent, EventListener};
pub use jobs::{JobHandle, JobMessage};
pub use types::{AnalysisRequest, AnalysisResult, EngineStatus, GlassSummary, ParaxialAnalysisResult, ParaxialRayData};
pub use validation::{DiagnosticLevel, DiagnosticMessage, validate_prescription};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_engine_initialization_and_status() -> TestResult {
        let engine = OpticalEngine::new()?;
        let status = engine.status();

        assert_eq!(status.system_name, "Cooke Triplet F/4.5");
        assert_eq!(status.surface_count, 7);
        assert_eq!(status.stop_surface_index, 3);
        assert!((status.entrance_pupil_diameter_mm - 10.0).abs() < 1e-6);
        assert!(!status.is_dirty);
        assert!(!status.can_undo);
        assert!(!status.can_redo);

        if let Some(efl) = status.efl_mm {
            assert!((efl - 50.0).abs() / 50.0 < 0.001);
        } else {
            return Err("Expected finite EFL for Cooke Triplet".into());
        }

        Ok(())
    }

    #[test]
    fn test_engine_paraxial_and_spot_analysis() -> TestResult {
        let mut engine = OpticalEngine::new()?;

        let paraxial_res = engine.run_analysis(&AnalysisRequest::Paraxial)?;
        if let AnalysisResult::Paraxial(p) = paraxial_res {
            assert!((p.efl_mm - 50.0).abs() < 0.05);
            assert_eq!(p.marginal_ray.len(), 7);
            assert_eq!(p.chief_ray.len(), 7);
        } else {
            return Err("Expected Paraxial result".into());
        }

        let spot_res = engine.run_analysis(&AnalysisRequest::SpotDiagram {
            field_index: 0,
            wavelength_nm: None,
            ray_count: 64,
        })?;
        if let AnalysisResult::SpotDiagram(spot) = spot_res {
            assert!(spot.metrics.rms_radius_um < 50.0);
            assert!(spot.metrics.passed_rays_count > 0);
        } else {
            return Err("Expected SpotDiagram result".into());
        }

        Ok(())
    }

    #[test]
    fn test_engine_wavefront_and_seidel_analysis() -> TestResult {
        let mut engine = OpticalEngine::new()?;

        let wf_res = engine.run_analysis(&AnalysisRequest::Wavefront {
            field_index: 0,
            wavelength_nm: None,
            grid_size: 32,
            zernike_terms: 9,
        })?;
        if let AnalysisResult::Wavefront { wavefront, zernike } = wf_res {
            assert_eq!(wavefront.grid_size, 32);
            assert!(wavefront.rms_waves.is_finite());
            assert!(zernike.is_some());
        } else {
            return Err("Expected Wavefront result".into());
        }

        let seidel_res = engine.run_analysis(&AnalysisRequest::Seidel)?;
        if let AnalysisResult::Seidel(seidel) = seidel_res {
            assert!(seidel.surfaces.len() >= 6);
            assert!(seidel.w040_spherical.is_finite());
        } else {
            return Err("Expected Seidel result".into());
        }

        Ok(())
    }

    #[test]
    fn test_engine_field_curves_and_mtf() -> TestResult {
        let mut engine = OpticalEngine::new()?;

        let fc_res = engine.run_analysis(&AnalysisRequest::FieldCurves { num_points: 5 })?;
        if let AnalysisResult::FieldCurves(fc) = fc_res {
            assert_eq!(fc.points.len(), 5);
            assert!(fc.points[0].distortion_percent.abs() < 1e-4);
        } else {
            return Err("Expected FieldCurves result".into());
        }

        let mtf_res = engine.run_analysis(&AnalysisRequest::Mtf {
            field_index: 0,
            max_freq_lp_mm: 100.0,
            num_points: 10,
        })?;
        if let AnalysisResult::Mtf(mtf) = mtf_res {
            assert_eq!(mtf.points.len(), 10);
            assert!((mtf.points[0].tangential_mtf - 1.0).abs() < 1e-4);
        } else {
            return Err("Expected Mtf result".into());
        }

        Ok(())
    }

    #[test]
    fn test_engine_undo_redo_and_dirty_tracking() -> TestResult {
        let mut engine = OpticalEngine::new()?;
        let orig_r = engine.prescription().surfaces[1].radius_mm;

        // Edit surface 1 radius
        engine.execute(EngineCommand::EditSurface {
            index: 1,
            update: SurfaceUpdate {
                radius_mm: Some(25.0),
                ..Default::default()
            },
        })?;

        assert!(engine.is_dirty());
        assert!(engine.can_undo());
        assert!(!engine.can_redo());
        assert_eq!(engine.revision(), 2);
        assert!((engine.prescription().surfaces[1].radius_mm - 25.0).abs() < 1e-6);

        // Undo edit
        engine.undo()?;
        assert!(engine.can_redo());
        assert_eq!(engine.revision(), 3);
        assert!((engine.prescription().surfaces[1].radius_mm - orig_r).abs() < 1e-6);

        // Redo edit
        engine.redo()?;
        assert!(!engine.can_redo());
        assert_eq!(engine.revision(), 4);
        assert!((engine.prescription().surfaces[1].radius_mm - 25.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_engine_result_caching_and_invalidation() -> TestResult {
        let mut engine = OpticalEngine::new()?;

        let req = AnalysisRequest::Paraxial;
        let res1 = engine.run_analysis(&req)?;

        // Same request should hit cache
        let res2 = engine.run_analysis(&req)?;
        if let (AnalysisResult::Paraxial(p1), AnalysisResult::Paraxial(p2)) = (&res1, &res2) {
            assert!((p1.efl_mm - p2.efl_mm).abs() < 1e-12);
        } else {
            return Err("Expected Paraxial results".into());
        }

        // Modifying system increments revision and invalidates stale cached results
        engine.execute(EngineCommand::SetEntrancePupilDiameter { epd_mm: 12.0 })?;
        assert_eq!(engine.revision(), 2);

        let res3 = engine.run_analysis(&req)?;
        if let AnalysisResult::Paraxial(p3) = res3 {
            assert!((p3.entrance_pupil_diameter_mm - 12.0).abs() < 1e-6);
        }

        Ok(())
    }

    #[test]
    fn test_engine_cad_and_project_export() -> TestResult {
        let engine = OpticalEngine::new()?;

        let svg = engine.export_svg(None)?;
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Cooke Triplet"));

        let dxf = engine.export_dxf(None)?;
        assert!(dxf.contains("SECTION"));
        assert!(dxf.contains("OPTICAL_AXIS"));

        let stl_bin = engine.export_stl(true, 16, 8);
        assert!(stl_bin.len() > 84);

        let (obj, mtl) = engine.export_obj(16, 8);
        assert!(obj.contains("v "));
        assert!(mtl.contains("newmtl "));

        let iso_md = engine.export_iso10110_markdown();
        assert!(iso_md.contains("ISO 10110"));

        let zmx = engine.export_zmx()?;
        assert!(zmx.contains("SURF 1"));

        let proj_json = engine.export_project_json()?;
        assert!(proj_json.contains("schema_version"));

        Ok(())
    }

    #[test]
    fn test_engine_validation_diagnostics() -> TestResult {
        let mut engine = OpticalEngine::new()?;

        // Clean Cooke triplet should have no error diagnostics
        let diags = engine.validate();
        assert!(!diags.iter().any(|d| d.level == DiagnosticLevel::Error));

        // Inject severe error (clear aperture exceeds radius of curvature)
        engine.execute(EngineCommand::EditSurface {
            index: 1,
            update: SurfaceUpdate {
                radius_mm: Some(5.0),
                semi_diameter_mm: Some(10.0),
                ..Default::default()
            },
        })?;

        let diags_err = engine.validate();
        assert!(diags_err.iter().any(|d| d.level == DiagnosticLevel::Error));

        Ok(())
    }

    #[test]
    fn test_engine_event_subscription() -> TestResult {
        let mut engine = OpticalEngine::new()?;
        let counter = Arc::new(AtomicUsize::new(0));
        let c_clone = Arc::clone(&counter);

        engine.subscribe(move |_evt| {
            c_clone.fetch_add(1, Ordering::SeqCst);
        });

        engine.execute(EngineCommand::SetEntrancePupilDiameter { epd_mm: 15.0 })?;
        // Expect at least PrescriptionChanged and DiagnosticsUpdated
        assert!(counter.load(Ordering::SeqCst) >= 2);

        Ok(())
    }

    #[test]
    fn test_engine_job_worker_cancellation() -> TestResult {
        let handle = JobHandle::spawn(42, |cancel, tx| {
            let _ = tx.send(JobMessage::Progress {
                percent: 10.0,
                status: "Starting".to_string(),
            });
            for i in 1..=10 {
                if cancel.load(Ordering::SeqCst) {
                    let _ = tx.send(JobMessage::Cancelled);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
                let _ = tx.send(JobMessage::Progress {
                    percent: (i * 10) as f32,
                    status: format!("Step {i}"),
                });
            }
            let _ = tx.send(JobMessage::Completed("Done".to_string()));
        });

        assert_eq!(handle.id(), 42);
        handle.cancel();
        assert!(handle.is_cancelled());

        let mut received_cancel_or_complete = false;
        while let Ok(msg) = handle.recv() {
            if matches!(msg, JobMessage::Cancelled | JobMessage::Completed(_)) {
                received_cancel_or_complete = true;
                break;
            }
        }
        assert!(received_cancel_or_complete);

        Ok(())
    }
}
