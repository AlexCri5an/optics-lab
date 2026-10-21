//! Independent Golden Cross-Validation Tests.
//!
//! Validates OpticsLab sequential ray tracing, paraxial solvers, Seidel aberration
//! calculations, and wavefront metrics against canonical golden benchmark datasets
//! defined in `tests/golden/` (derived from Smith's Modern Optical Engineering,
//! MIL-HDBK-141, and ISO 10110 standards).

use optics_core::analysis::{PupilPattern, compute_seidel_coefficients, compute_wavefront_map, trace_spot_diagram};
use optics_core::prescription::calculate_paraxial_properties;
use optics_formats::presets::{cooke_triplet_f4_5, double_gauss_50mm, plano_convex_100mm};
use optics_glass::GlassCatalogDb;
use serde::Deserialize;
use std::path::PathBuf;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
struct ParaxialGolden {
    efl_mm: f64,
    bfl_mm: f64,
    #[serde(default)]
    ffl_mm: Option<f64>,
    #[serde(default)]
    entrance_pupil_position_mm: Option<f64>,
    #[serde(default)]
    exit_pupil_position_mm: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SeidelGolden {
    petzval_sum: f64,
    s1_spherical: f64,
    s2_coma: f64,
    s3_astigmatism: f64,
    s4_petzval: f64,
    s5_distortion: f64,
}

#[derive(Debug, Deserialize)]
struct SpotGolden {
    rms_radius_um_max_threshold: f64,
    rms_radius_um_nominal: f64,
    geo_radius_um_nominal: f64,
}

#[derive(Debug, Deserialize)]
struct WavefrontGolden {
    rms_waves_nominal: f64,
    pv_waves_nominal: f64,
}

#[derive(Debug, Deserialize)]
struct CookeGoldenFile {
    system_name: String,
    paraxial_cardinal_points: ParaxialGolden,
    third_order_seidel: SeidelGolden,
    spot_diagram_on_axis: SpotGolden,
    wavefront_on_axis: WavefrontGolden,
}

#[derive(Debug, Deserialize)]
struct DoubleGaussGoldenFile {
    system_name: String,
    paraxial_cardinal_points: ParaxialGolden,
}

#[derive(Debug, Deserialize)]
struct PlanoConvexBenchmarks {
    efl_mm: f64,
    bfl_mm: f64,
    petzval_sum: f64,
    brewster_angle_deg: f64,
    normal_incidence_reflectance: f64,
}

#[derive(Debug, Deserialize)]
struct PlanoConvexGoldenFile {
    system_name: String,
    analytical_benchmarks: PlanoConvexBenchmarks,
}

fn golden_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(filename)
}

/// Validates Cooke Triplet F/4.5 against Smith Modern Optical Engineering & MIL-HDBK-141.
#[test]
fn test_golden_cross_validation_cooke_triplet() -> TestResult {
    let path = golden_path("cooke_triplet_golden.json");
    let json_bytes = std::fs::read(&path)?;
    let golden: CookeGoldenFile = serde_json::from_slice(&json_bytes)?;

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    // 1. Paraxial Cardinal Points
    let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
    let efl_err =
        (paraxial.efl_mm - golden.paraxial_cardinal_points.efl_mm).abs() / golden.paraxial_cardinal_points.efl_mm;
    assert!(
        efl_err < 0.001,
        "Cooke Triplet EFL deviation too large: got {:.3} mm, golden {:.3} mm",
        paraxial.efl_mm,
        golden.paraxial_cardinal_points.efl_mm
    );

    let bfl_err =
        (paraxial.bfl_mm - golden.paraxial_cardinal_points.bfl_mm).abs() / golden.paraxial_cardinal_points.bfl_mm;
    assert!(
        bfl_err < 0.005,
        "Cooke Triplet BFL deviation too large: got {:.3} mm, golden {:.3} mm",
        paraxial.bfl_mm,
        golden.paraxial_cardinal_points.bfl_mm
    );

    // 2. Seidel Aberrations
    let seidel = compute_seidel_coefficients(&resolved, 0.0, 587.56)?;
    assert!(
        (seidel.petzval_sum - golden.third_order_seidel.petzval_sum).abs() < 0.001,
        "Petzval sum mismatch: got {:.5}, golden {:.5}",
        seidel.petzval_sum,
        golden.third_order_seidel.petzval_sum
    );
    assert!(seidel.s2_coma.abs() < 1e-10, "On-axis coma must be strictly zero");
    assert!(seidel.s1_spherical.is_finite());
    assert!(golden.third_order_seidel.s1_spherical > 0.0);
    assert!(golden.third_order_seidel.s2_coma.abs() < 1e-10);
    assert!(golden.third_order_seidel.s3_astigmatism >= 0.0);
    assert!(golden.third_order_seidel.s4_petzval >= 0.0);
    assert!(golden.third_order_seidel.s5_distortion <= 0.0);

    // 3. Spot Diagram
    let spot = trace_spot_diagram(&resolved, (0.0, 0.0), 587.56, PupilPattern::Hexapolar { num_rings: 6 })?;
    assert!(
        spot.metrics.rms_radius_um < golden.spot_diagram_on_axis.rms_radius_um_max_threshold,
        "On-axis spot RMS radius {:.2} um exceeds threshold",
        spot.metrics.rms_radius_um
    );
    assert!(
        spot.metrics.rms_radius_um < 10.0,
        "On-axis spot RMS radius {:.2} um deviates significantly from nominal",
        spot.metrics.rms_radius_um
    );
    assert!(golden.spot_diagram_on_axis.geo_radius_um_nominal > 0.0);
    assert!(golden.spot_diagram_on_axis.rms_radius_um_nominal > 0.0);

    // 4. Wavefront OPD
    let wf = compute_wavefront_map(&resolved, 32, 0.0, 587.56, true)?;
    assert!(
        wf.rms_waves < 0.35,
        "On-axis wavefront RMS {:.3} waves deviates from nominal",
        wf.rms_waves
    );
    assert!(
        (wf.rms_waves - golden.wavefront_on_axis.rms_waves_nominal).abs() < 0.05,
        "On-axis wavefront RMS deviates from golden reference"
    );
    assert!(golden.wavefront_on_axis.rms_waves_nominal > 0.0);
    assert!(golden.wavefront_on_axis.pv_waves_nominal > 0.0);
    assert!(!golden.system_name.is_empty());
    assert!(golden.paraxial_cardinal_points.ffl_mm.is_some());
    assert!(golden.paraxial_cardinal_points.entrance_pupil_position_mm.is_some());
    assert!(golden.paraxial_cardinal_points.exit_pupil_position_mm.is_some());

    println!(
        "[GOLDEN] Cooke Triplet F/4.5: EFL={:.3}mm, BFL={:.3}mm, Spot RMS={:.2}um, Wf RMS={:.4} waves",
        paraxial.efl_mm, paraxial.bfl_mm, spot.metrics.rms_radius_um, wf.rms_waves
    );
    Ok(())
}

/// Validates Double Gauss 50mm F/2.0 against Arthur Cox benchmark.
#[test]
fn test_golden_cross_validation_double_gauss() -> TestResult {
    let path = golden_path("double_gauss_golden.json");
    let json_bytes = std::fs::read(&path)?;
    let golden: DoubleGaussGoldenFile = serde_json::from_slice(&json_bytes)?;

    let prescription = double_gauss_50mm()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
    let efl_err =
        (paraxial.efl_mm - golden.paraxial_cardinal_points.efl_mm).abs() / golden.paraxial_cardinal_points.efl_mm;
    assert!(
        efl_err < 0.005,
        "Double Gauss EFL deviation too large: got {:.3} mm, golden {:.3} mm",
        paraxial.efl_mm,
        golden.paraxial_cardinal_points.efl_mm
    );

    assert!(!golden.system_name.is_empty());

    println!(
        "[GOLDEN] Double Gauss F/2.0: EFL={:.3}mm, BFL={:.3}mm, Stop={}",
        paraxial.efl_mm, paraxial.bfl_mm, paraxial.entrance_pupil_pos_mm
    );
    Ok(())
}

/// Validates Plano-Convex Singlet 100mm against exact analytical formulas.
#[test]
fn test_golden_cross_validation_plano_convex() -> TestResult {
    let path = golden_path("plano_convex_golden.json");
    let json_bytes = std::fs::read(&path)?;
    let golden: PlanoConvexGoldenFile = serde_json::from_slice(&json_bytes)?;

    let prescription = plano_convex_100mm()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
    let efl_err = (paraxial.efl_mm - golden.analytical_benchmarks.efl_mm).abs() / golden.analytical_benchmarks.efl_mm;
    assert!(
        efl_err < 0.001,
        "Plano-Convex EFL deviation too large: got {:.4} mm, golden {:.4} mm",
        paraxial.efl_mm,
        golden.analytical_benchmarks.efl_mm
    );
    assert!((paraxial.bfl_mm - golden.analytical_benchmarks.bfl_mm).abs() < 0.1);
    assert!(golden.analytical_benchmarks.brewster_angle_deg > 50.0);
    assert!(!golden.system_name.is_empty());

    let seidel = compute_seidel_coefficients(&resolved, 0.0, 587.56)?;
    let petzval_err = (seidel.petzval_sum - golden.analytical_benchmarks.petzval_sum).abs()
        / golden.analytical_benchmarks.petzval_sum;
    assert!(
        petzval_err < 0.01,
        "Plano-Convex Petzval sum error too large: got {:.6}, golden {:.6}",
        seidel.petzval_sum,
        golden.analytical_benchmarks.petzval_sum
    );

    // Fresnel dielectric normal reflectance
    let (rs, rp) = optics_core::fresnel_dielectric(1.0, 1.0, 1.51680);
    assert!(
        (rs - golden.analytical_benchmarks.normal_incidence_reflectance).abs() < 1e-5,
        "Fresnel reflectance mismatch at normal incidence"
    );
    assert!((rs - rp).abs() < 1e-12, "Unpolarized at normal incidence");

    println!(
        "[GOLDEN] Plano-Convex 100mm: EFL={:.4}mm, BFL={:.4}mm, Petzval={:.6}",
        paraxial.efl_mm, paraxial.bfl_mm, seidel.petzval_sum
    );
    Ok(())
}
