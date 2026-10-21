//! Example 04: Non-Sequential Spectral Ray Tracing & Prism Dispersion
//!
//! Demonstrates non-sequential ray tracing through a dispersive 3D triangular prism (SF11 flint glass):
//! - Constructing a 3D triangular prism mesh with watertight boundary topology
//! - Configuring high-dispersion Sellmeier optical material from the glass catalog
//! - Launching multi-spectral ray bundles across visible wavelengths (450 nm, 550 nm, 650 nm)
//! - Tracing 3D vector Snell refraction and internal reflection paths
//! - Capturing spectral separation (angular dispersion) on a 2D pixelated radiometric detector

use optics_core::{OpticsError, Ray, Triangle, Vec3};
use optics_glass::catalog::GlassCatalogDb;
use optics_nonseq::{
    Detector2D, NonSeqMaterial, NonSeqObject, NonSeqRay, NonSeqScene, NonSeqTraceConfig, NonSeqTracer, OpticalMedium,
    PrimitiveGeometry, Transform3D,
};

fn create_triangular_prism_mesh() -> Result<Vec<Triangle>, OpticsError> {
    // Triangular prism cross section in X-Z:
    // Base along z = 0 from x = -15 to x = +15
    // Apex at x = 0, z = 20
    // Extruded along Y from y = -10 to y = +10
    let v0 = Vec3::new(-15.0, -10.0, 0.0);
    let v1 = Vec3::new(15.0, -10.0, 0.0);
    let v2 = Vec3::new(0.0, -10.0, 20.0);

    let v3 = Vec3::new(-15.0, 10.0, 0.0);
    let v4 = Vec3::new(15.0, 10.0, 0.0);
    let v5 = Vec3::new(0.0, 10.0, 20.0);

    Ok(vec![
        // Bottom face (z = 0)
        Triangle::new(v0, v1, v4, 0, 1, 0)?,
        Triangle::new(v0, v4, v3, 0, 1, 0)?,
        // Left slanted face
        Triangle::new(v0, v3, v5, 0, 1, 0)?,
        Triangle::new(v0, v5, v2, 0, 1, 0)?,
        // Right slanted face
        Triangle::new(v1, v2, v5, 0, 1, 0)?,
        Triangle::new(v1, v5, v4, 0, 1, 0)?,
        // Front triangular cap (y = -10)
        Triangle::new(v0, v2, v1, 0, 1, 0)?,
        // Back triangular cap (y = +10)
        Triangle::new(v3, v4, v5, 0, 1, 0)?,
    ])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 04: Non-Sequential Prism Dispersion ===\n");

    let catalog = GlassCatalogDb::load_bundled()?;
    let mut scene = NonSeqScene::new();

    // 1. Create SF11 dispersive prism object
    let prism_mesh = create_triangular_prism_mesh()?;
    let prism_object = NonSeqObject {
        id: 1,
        name: "SF11_Dispersive_Prism".to_string(),
        geometry: PrimitiveGeometry::Mesh { triangles: prism_mesh },
        transform: Transform3D::from_translation(0.0, 0.0, 10.0),
        material: NonSeqMaterial::dielectric(OpticalMedium::GlassCatalog {
            name: "SF11".to_string(),
        }),
        is_active: true,
    };
    scene.add_object(prism_object);

    // 2. Set up 2D detector behind the prism
    let detector = Detector2D::new(
        10,
        "Spectral_Dispersion_Screen",
        Vec3::new(0.0, 0.0, 45.0), // position
        Vec3::new(0.0, 0.0, 1.0),  // surface normal facing +z
        Vec3::new(0.0, 1.0, 0.0),  // up vector
        60.0,                      // width mm
        40.0,                      // height mm
        60,                        // 60 pixels horizontal
        40,                        // 40 pixels vertical
    )?;
    scene.add_detector(detector);

    // 3. Launch rays at 450 nm (Blue), 550 nm (Green), 650 nm (Red)
    // Launched toward the prism at an angle to induce asymmetric refraction
    let wavelengths = [
        (450.0, "Blue  (450 nm)"),
        (550.0, "Green (550 nm)"),
        (650.0, "Red   (650 nm)"),
    ];

    println!("Launching multi-spectral collimated ray bundles toward prism...");
    let mut all_rays = Vec::new();
    let mut ray_id = 0;

    for &(wl, label) in &wavelengths {
        println!("  Generating bundle for {label}");
        // Launch a small pencil of rays around x = -6.0 entering the left face
        for dy in -2..=2 {
            let origin = Vec3::new(-6.0, (dy as f64) * 1.5, 0.0);
            let direction = Vec3::new(0.2, 0.0, 1.0).normalize();
            let ray = Ray::new(origin, direction, wl, 1.0)?;
            all_rays.push(NonSeqRay::new(ray, ray_id));
            ray_id += 1;
        }
    }

    // 4. Trace rays non-sequentially with Snell/Fresnel physics
    let tracer = NonSeqTracer::new(NonSeqTraceConfig {
        max_bounces: 15,
        min_power_threshold: 1e-6,
        enable_branching: false,
        record_paths: true,
    });

    println!("\nTracing {} rays with vector Snell refraction...", all_rays.len());
    let report = tracer.trace_rays(&mut scene, &all_rays, Some(&catalog))?;

    println!("Non-sequential trace completed:");
    println!("  Rays Traced:        {}", report.total_rays_traced);
    println!("  Interactions:       {}", report.total_interactions);
    println!("  Launched Flux:      {:.4} Watts", report.launched_flux_watts);
    println!("  Detected Flux:      {:.4} Watts", report.detected_flux_watts);
    println!("  Escaped Flux:       {:.4} Watts", report.escaped_flux_watts);
    println!("  Conservation Ratio: {:.6}\n", report.energy_conservation_ratio());

    let det = &scene.detectors[0];
    println!("Detector Results ({})", det.name);
    println!("  Total Captured Power: {:.4} Watts", det.total_flux_watts);
    println!("  Total Luminous Flux:  {:.4} Lumens", det.total_lumens);
    println!("  Total Ray Hits:       {}", det.total_rays_hit);

    println!("\nNon-sequential prism dispersion simulation successfully completed.");
    Ok(())
}
