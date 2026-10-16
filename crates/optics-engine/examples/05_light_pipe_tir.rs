//! Example 05: Non-Sequential Total Internal Reflection (TIR) Light Pipe
//!
//! Demonstrates lossless optical guidance through a solid dielectric light pipe:
//! - Modeling a rectangular glass light guide ($n = 1.517$) in ambient air ($n = 1.0$)
//! - Critical angle physics: $\theta_c = \arcsin(1/n) \approx 41.28^\circ$
//! - Launching an off-axis cone of rays entering the front entrance face
//! - Multiple internal bounces guided with 100% efficiency via Total Internal Reflection
//! - Radiometric flux collection on an exit face detector confirming transmission efficiency

use optics_core::{Ray, Vec3};
use optics_nonseq::{
    Detector2D, NonSeqMaterial, NonSeqObject, NonSeqRay, NonSeqScene, NonSeqTraceConfig,
    NonSeqTracer, OpticalMedium, PrimitiveGeometry, Transform3D,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 05: Non-Sequential TIR Light Pipe ===\n");

    let mut scene = NonSeqScene::new();

    // 1. Define dielectric light pipe:
    // Rectangular solid: width = 10 mm (half_x = 5), height = 10 mm (half_y = 5), length = 80 mm (half_z = 40)
    // Centered at (0, 0, 40) so entrance is at z = 0, exit is at z = 80
    let pipe_half_x = 5.0;
    let pipe_half_y = 5.0;
    let pipe_half_z = 40.0;

    let light_pipe = NonSeqObject {
        id: 1,
        name: "Rectangular_Glass_Light_Guide".to_string(),
        geometry: PrimitiveGeometry::Box {
            half_x: pipe_half_x,
            half_y: pipe_half_y,
            half_z: pipe_half_z,
        },
        transform: Transform3D::from_translation(0.0, 0.0, 40.0),
        material: NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.517 }),
        is_active: true,
    };
    scene.add_object(light_pipe);

    // 2. Add exit detector immediately behind the light pipe exit face (at z = 85 mm)
    let exit_detector = Detector2D::new(
        100,
        "Exit_Face_Detector",
        Vec3::new(0.0, 0.0, 85.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 0.0),
        20.0,
        20.0,
        40,
        40,
    )?;
    scene.add_detector(exit_detector);

    // 3. Launch rays into the entrance face at various angles
    // For n = 1.517, the critical angle is arcsin(1 / 1.517) = 41.24°
    // Any ray entering through the front face from air will strike the side walls at
    // an internal angle > 90° - arcsin(1/1.517) = 48.76° > theta_c, guaranteeing 100% TIR!
    println!("Launching rays into light pipe entrance at diverse injection angles...");
    let mut rays = Vec::new();
    let mut ray_id = 0;

    // Launch a grid of rays across [-3, 3] mm with angles up to 25°
    let launch_angles_deg: [f64; 5] = [0.0, 5.0, 10.0, 15.0, 20.0];
    for &angle_deg in &launch_angles_deg {
        let rad = angle_deg.to_radians();
        let dir = Vec3::new(rad.sin(), 0.0, rad.cos()).normalize();

        for x_step in -2..=2 {
            let origin = Vec3::new((x_step as f64) * 1.5, 0.0, -2.0); // 2 mm in front of entrance
            let ray = Ray::new(origin, dir, 550.0, 1.0)?;
            rays.push(NonSeqRay::new(ray, ray_id));
            ray_id += 1;
        }
    }

    println!("Total rays generated: {}", rays.len());

    // 4. Trace rays through the light pipe with multiple TIR bounces
    let tracer = NonSeqTracer::new(NonSeqTraceConfig {
        max_bounces: 30,
        min_power_threshold: 1e-6,
        enable_branching: false,
        record_paths: true,
    });

    println!("Tracing rays through light pipe with Total Internal Reflection...");
    let report = tracer.trace_rays(&mut scene, &rays, None)?;

    println!("\nTrace Execution Report:");
    println!("  Total Rays Traced:     {}", report.total_rays_traced);
    println!("  Total Interactions:    {}", report.total_interactions);
    println!(
        "  Launched Flux:         {:.4} Watts",
        report.launched_flux_watts
    );
    println!(
        "  Collected Exit Flux:   {:.4} Watts",
        report.detected_flux_watts
    );
    println!(
        "  Fresnel Reflection / Escape: {:.4} Watts",
        report.escaped_flux_watts
    );
    println!(
        "  Energy Conservation:   {:.6}",
        report.energy_conservation_ratio()
    );

    let det = &scene.detectors[0];
    println!("\nExit Detector Metrics ({}):", det.name);
    println!("  Captured Flux:         {:.4} Watts", det.total_flux_watts);
    println!("  Captured Luminous:     {:.4} Lumens", det.total_lumens);
    println!("  Total Ray Hits:        {}", det.total_rays_hit);
    let efficiency = (det.total_flux_watts / report.launched_flux_watts) * 100.0;
    println!("  Overall Transmission Efficiency: {:.2}%", efficiency);

    println!("\nNon-sequential light pipe TIR simulation successfully completed.");
    Ok(())
}
