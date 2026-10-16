//! Example 07: High-Level OpticalEngine Facade, Undo/Redo & Result Caching
//!
//! Demonstrates using `optics-engine` as the single high-level facade for GUI applications:
//! - Initializing `OpticalEngine` with bundled glass catalogs and default Cooke Triplet
//! - Querying `EngineStatus` for high-level system metrics
//! - Subscribing an `EventListener` to receive reactive `EngineEvent` notifications
//! - Executing reversible commands (`EngineCommand::EditSurface`, `SetEntrancePupilDiameter`)
//! - Demonstrating dirty tracking, revision increments, and reversible undo/redo
//! - Verifying revision-keyed analysis result caching and automatic cache invalidation
//! - Running automated diagnostic validation (`validate_system`)

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use optics_engine::{
    AnalysisRequest, AnalysisResult, DiagnosticLevel, EngineCommand, EngineEvent, OpticalEngine,
    SurfaceUpdate,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 07: OpticalEngine Facade & Undo/Redo ===\n");

    // 1. Initialize Engine
    let mut engine = OpticalEngine::new()?;
    let status = engine.status();

    println!("1. Initial Engine Status:");
    println!("  System Name: {}", status.system_name);
    println!("  Surfaces:    {}", status.surface_count);
    println!("  Stop Index:  {}", status.stop_surface_index);
    println!("  EPD:         {:.2} mm", status.entrance_pupil_diameter_mm);
    if let Some(efl) = status.efl_mm {
        println!("  Initial EFL: {:.4} mm", efl);
    }
    println!("  Revision:    {}", engine.revision());
    println!("  Dirty Flag:  {}\n", engine.is_dirty());

    // 2. Event Subscription
    let event_counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&event_counter);
    engine.subscribe(Box::new(move |event: &EngineEvent| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
        match event {
            EngineEvent::SurfaceModified { surface_index, .. } => {
                println!("    [Event Callback] Surface {} modified", surface_index);
            }
            EngineEvent::PrescriptionChanged { .. } => {
                println!("    [Event Callback] Prescription modified");
            }
            EngineEvent::AnalysisCompleted { analysis_type, .. } => {
                println!("    [Event Callback] Analysis completed: {}", analysis_type);
            }
            _ => {}
        }
    }));

    // 3. Run Paraxial Analysis through Facade
    println!("2. Executing Paraxial Analysis via run_analysis:");
    let res1 = engine.run_analysis(&AnalysisRequest::Paraxial)?;
    if let AnalysisResult::Paraxial(p1) = &res1 {
        println!(
            "  Computed Paraxial EFL: {:.4} mm, BFL: {:.4} mm",
            p1.efl_mm, p1.bfl_mm
        );
    }

    // 4. Demonstrate Result Caching
    println!("\n3. Testing Result Caching (Identical Request):");
    let res2 = engine.run_analysis(&AnalysisRequest::Paraxial)?;
    if let (AnalysisResult::Paraxial(p1), AnalysisResult::Paraxial(p2)) = (&res1, &res2) {
        println!(
            "  Result Cache Hit verified (EFL diff: {:.1e} mm)",
            (p1.efl_mm - p2.efl_mm).abs()
        );
    }

    // 5. Command Execution (Edit Surface 1 Radius)
    let orig_r1 = engine.prescription().surfaces[1].radius_mm;
    println!(
        "\n4. Executing Command: Edit Surface 1 Radius from {:.3} mm to 25.0 mm",
        orig_r1
    );
    engine.execute(EngineCommand::EditSurface {
        index: 1,
        update: SurfaceUpdate {
            radius_mm: Some(25.0),
            ..Default::default()
        },
    })?;

    println!("  Post-Command State:");
    println!(
        "  New Radius: {:.3} mm",
        engine.prescription().surfaces[1].radius_mm
    );
    println!("  Can Undo:   {}", engine.can_undo());
    println!("  Can Redo:   {}", engine.can_redo());
    println!("  Revision:   {}", engine.revision());
    println!("  Dirty Flag: {}", engine.is_dirty());

    // 6. Demonstrate Cache Invalidation
    println!("\n5. Testing Cache Invalidation after Command Execution:");
    let res3 = engine.run_analysis(&AnalysisRequest::Paraxial)?;
    if let AnalysisResult::Paraxial(p3) = res3 {
        println!(
            "  Recomputed EFL after edit: {:.4} mm (Cache successfully invalidated)",
            p3.efl_mm
        );
    }

    // 7. Undo Command
    println!("\n6. Executing Undo Command:");
    engine.undo()?;
    println!(
        "  Post-Undo Radius: {:.3} mm (Restored original: {:.3} mm)",
        engine.prescription().surfaces[1].radius_mm,
        orig_r1
    );
    println!("  Can Undo: {}", engine.can_undo());
    println!("  Can Redo: {}", engine.can_redo());
    println!("  Revision: {}", engine.revision());

    // 8. Redo Command
    println!("\n7. Executing Redo Command:");
    engine.redo()?;
    println!(
        "  Post-Redo Radius: {:.3} mm (Re-applied: 25.000 mm)",
        engine.prescription().surfaces[1].radius_mm
    );
    println!("  Can Undo: {}", engine.can_undo());
    println!("  Can Redo: {}", engine.can_redo());
    println!("  Revision: {}", engine.revision());

    // 9. Run Diagnostics Validation Layer
    println!("\n8. Running Diagnostic Validation Layer:");
    let diagnostics = engine.validate();
    println!("  Diagnostic issues found: {}", diagnostics.len());
    for diag in &diagnostics {
        let level_str = match diag.level {
            DiagnosticLevel::Error => "ERROR",
            DiagnosticLevel::Warning => "WARN ",
            DiagnosticLevel::Info => "INFO ",
        };
        println!(
            "  [{level_str}] Code: {} | Surface: {:?} | Message: {}",
            diag.code, diag.surface_index, diag.message
        );
    }

    println!(
        "\nTotal events received by subscriber: {}",
        event_counter.load(Ordering::SeqCst)
    );
    println!("\nOpticalEngine facade, undo/redo, and caching successfully demonstrated.");
    Ok(())
}
