//! Example 02: Comprehensive Cooke Triplet Analysis
//!
//! Demonstrates full optical characterization of the canonical Cooke Triplet F/4.5:
//! - Paraxial cardinal properties (EFL, BFL, pupil diameters/positions)
//! - Third-order Seidel aberration coefficients and Petzval sum
//! - Multi-field spot diagram blur radii (RMS and GEO)
//! - Wavefront OPD map and Noll Zernike polynomial decomposition
//! - Diffraction Modulation Transfer Function (MTF) vs spatial frequency

use optics_core::analysis::{
    PupilPattern, compute_diffraction_mtf, compute_seidel_coefficients, compute_wavefront_map, fit_zernike_noll,
    trace_spot_diagram,
};
use optics_core::prescription::calculate_paraxial_properties;
use optics_formats::presets::cooke_triplet_f4_5;
use optics_glass::catalog::GlassCatalogDb;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 02: Cooke Triplet F/4.5 Optical Analysis ===\n");

    let catalog = GlassCatalogDb::load_bundled()?;
    let prescription = cooke_triplet_f4_5()?;
    let resolved = prescription.resolve(&catalog)?;

    // 1. Paraxial Analysis
    let parax = calculate_paraxial_properties(&resolved, 587.56)?;
    println!("1. Paraxial Cardinal Points (at λ = 587.56 nm):");
    println!("  Effective Focal Length (EFL):    {:.4} mm", parax.efl_mm);
    println!("  Back Focal Length (BFL):         {:.4} mm", parax.bfl_mm);
    println!("  Working F-Number (F/#):          F/{:.2}", parax.f_number);
    println!(
        "  Entrance Pupil Position:         {:.4} mm",
        parax.entrance_pupil_pos_mm
    );
    println!("  Exit Pupil Position:             {:.4} mm\n", parax.exit_pupil_pos_mm);

    // 2. Seidel 3rd-Order Aberrations
    let seidel = compute_seidel_coefficients(&resolved, 0.0, 587.56)?;
    println!("2. Third-Order Seidel Aberration Coefficients:");
    println!("  S_I   (Spherical):      {:+.6}", seidel.s1_spherical);
    println!("  S_II  (Coma):           {:+.6}", seidel.s2_coma);
    println!("  S_III (Astigmatism):    {:+.6}", seidel.s3_astigmatism);
    println!("  S_IV  (Field Curvature):{:+.6}", seidel.s4_field_curvature);
    println!("  S_V   (Distortion):     {:+.6}", seidel.s5_distortion);
    println!("  Petzval Curvature Sum:   {:.6} mm^-1\n", seidel.petzval_sum);

    // 3. Multi-Field Spot Diagrams
    println!("3. Spot Diagram Analysis (Hexapolar 6 rings, 127 rays per field):");
    let fields = [0.0, 14.0, 20.0];
    for &field in &fields {
        let spot = trace_spot_diagram(
            &resolved,
            (0.0, field),
            587.56,
            PupilPattern::Hexapolar { num_rings: 6 },
        )?;
        println!(
            "  Field {:4.1}°: RMS Blur Radius = {:6.2} µm, GEO Radius = {:6.2} µm (Passed: {}/{})",
            field,
            spot.metrics.rms_radius_um,
            spot.metrics.geo_radius_um,
            spot.metrics.passed_rays_count,
            spot.metrics.total_rays_traced
        );
    }
    println!();

    // 4. Wavefront OPD & Zernike Fit (On-Axis)
    let wf = compute_wavefront_map(&resolved, 32, 0.0, 587.56, true)?;
    let zernike = fit_zernike_noll(&wf, 15)?;
    println!("4. Wavefront OPD Map & Zernike Decomposition (On-Axis):");
    println!("  Peak-to-Valley (P-V):   {:.4} waves", wf.pv_waves);
    println!("  Root-Mean-Square (RMS): {:.4} waves", wf.rms_waves);
    println!("  Estimated Strehl Ratio: {:.4}", wf.strehl_ratio);
    println!("  First 8 Noll Zernike Coefficients (waves):");
    for (i, &c) in zernike.coefficients.iter().take(8).enumerate() {
        println!("    Z{:02}: {:+8.5}", i + 1, c);
    }
    println!();

    // 5. Diffraction MTF
    let mtf = compute_diffraction_mtf(&resolved, 10, 0.0, 587.56)?;
    println!("5. On-Axis Diffraction Modulation Transfer Function (MTF):");
    println!("  Cutoff Frequency: {:.1} cycles/mm", mtf.cutoff_frequency_lpm);
    println!("  Freq (cyc/mm) | Tangential MTF | Sagittal MTF");
    println!("  -------------+----------------+-------------");
    for pt in &mtf.points {
        println!(
            "  {:12.1} | {:14.4} | {:12.4}",
            pt.spatial_frequency_lpm, pt.tangential_mtf, pt.sagittal_mtf
        );
    }

    println!("\nCooke Triplet analysis successfully completed.");
    Ok(())
}
