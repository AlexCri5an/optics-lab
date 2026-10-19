//! Example 03: Optical Tolerancing, Sensitivity Ranking & Monte Carlo Yield Simulation
//!
//! Demonstrates evaluating manufacturing tolerances on an optical system:
//! - Setting up manufacturing tolerances: surface radius, center thickness, surface decenter
//! - Worst-case sensitivity analysis ranking operands by performance degradation
//! - Root-Sum-Square (RSS) statistical error budget estimation
//! - Deterministic Monte Carlo simulation (100 trials) with paraxial refocus compensator
//! - Yield calculation and statistical percentiles (P10, P50, P90, P99)

use optics_core::prescription::{OpticalPrescription, OpticalSurface};
use optics_glass::catalog::GlassCatalogDb;
use optics_tolerance::{
    Compensator, MonteCarloConfig, MonteCarloSimulator, SensitivityAnalyzer, ToleranceCriterion,
    ToleranceOperand, ToleranceOperandType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 03: Tolerancing & Monte Carlo Simulation ===\n");

    let catalog = GlassCatalogDb::load_bundled()?;

    // 1. Define optical system: Doublet lens
    let prescription = OpticalPrescription {
        name: "Achromatic Doublet 100mm".to_string(),
        entrance_pupil_diameter_mm: 20.0,
        field_angles_deg: vec![0.0],
        wavelengths_nm: vec![587.56],
        primary_wavelength_index: 0,
        surfaces: vec![
            OpticalSurface {
                surface_index: 0,
                radius_mm: 60.0,
                conic_constant: 0.0,
                thickness_mm: 6.0,
                semi_diameter_mm: 12.0,
                material_name: "N-BK7".to_string(),
                is_stop: true,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
                profile: None,
            },
            OpticalSurface {
                surface_index: 1,
                radius_mm: -45.0,
                conic_constant: 0.0,
                thickness_mm: 3.0,
                semi_diameter_mm: 12.0,
                material_name: "SF11".to_string(),
                is_stop: false,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
                profile: None,
            },
            OpticalSurface {
                surface_index: 2,
                radius_mm: -150.0,
                conic_constant: 0.0,
                thickness_mm: 92.5,
                semi_diameter_mm: 12.0,
                material_name: "AIR".to_string(),
                is_stop: false,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
                profile: None,
            },
        ],
    };

    // 2. Define tolerance operands representing manufacturing shop tolerances
    let operands = vec![
        ToleranceOperand::new(
            "TRAD_S1",
            ToleranceOperandType::Radius {
                surface_index: 0,
                delta_radius_mm: 0.05,
                fringes: None,
            },
        ),
        ToleranceOperand::new(
            "TRAD_S2",
            ToleranceOperandType::Radius {
                surface_index: 1,
                delta_radius_mm: 0.05,
                fringes: None,
            },
        ),
        ToleranceOperand::new(
            "TRAD_S3",
            ToleranceOperandType::Radius {
                surface_index: 2,
                delta_radius_mm: 0.10,
                fringes: None,
            },
        ),
        ToleranceOperand::new(
            "TTHI_S1",
            ToleranceOperandType::Thickness {
                surface_index: 0,
                delta_thickness_mm: 0.03,
            },
        ),
        ToleranceOperand::new(
            "TTHI_S2",
            ToleranceOperandType::Thickness {
                surface_index: 1,
                delta_thickness_mm: 0.03,
            },
        ),
        ToleranceOperand::new(
            "TDEC_S2",
            ToleranceOperandType::SurfaceDecenter {
                surface_index: 1,
                max_decenter_mm: 0.02,
            },
        ),
    ];

    let criterion = ToleranceCriterion::RmsSpotRadius {
        field_angle_deg: 0.0,
        wavelength_nm: 587.56,
    };
    let compensator = Compensator::ParaxialRefocus {
        wavelength_nm: 587.56,
    };

    // 3. Sensitivity Analysis
    println!("Running Sensitivity Analysis with Paraxial Refocus Compensator...");
    let sens_report =
        SensitivityAnalyzer::analyze(&prescription, &operands, &criterion, &compensator, &catalog)?;

    println!(
        "Nominal Performance (RMS Spot Radius): {:.4} µm\n",
        sens_report.nominal_value
    );
    println!("Tolerance Sensitivity Ranking (Sorted Descending):");
    println!("  Rank | Operand ID   | Nominal (µm) | Worst-Case Δ | Sensitivity Rate");
    println!("  -----+--------------+--------------+--------------+-----------------");
    for (i, rec) in sens_report.records.iter().enumerate() {
        println!(
            "  {:4} | {:12} | {:12.4} | {:10.4} µm | {:16.4}",
            i + 1,
            rec.id,
            rec.nominal_value,
            rec.max_degradation,
            rec.sensitivity_rate
        );
    }
    println!(
        "\n  Estimated RSS Criterion Degradation: {:.4} µm",
        sens_report.estimated_rss_degradation
    );
    println!(
        "  Estimated Worst-Case Linear Sum:     {:.4} µm\n",
        sens_report.worst_case_degradation
    );

    // 4. Monte Carlo Yield Simulation
    println!("Running Monte Carlo Simulation (100 trials, seed = 42)...");
    let mc_config = MonteCarloConfig {
        num_trials: 100,
        seed: 42,
        yield_threshold: Some(sens_report.nominal_value + 15.0),
    };
    let mc_sim = MonteCarloSimulator::new(mc_config);
    let mc_report = mc_sim.run(&prescription, &operands, &criterion, &compensator, &catalog)?;

    println!("Monte Carlo Statistical Summary:");
    println!("  Trials Executed:     {}", mc_report.total_trials);
    println!("  Mean Criterion:      {:.4} µm", mc_report.mean);
    println!("  Std Deviation:       {:.4} µm", mc_report.std_dev);
    println!("  Percentile P10 (10%):{:.4} µm", mc_report.percentiles.p10);
    println!("  Percentile P50 (Med):{:.4} µm", mc_report.percentiles.p50);
    println!("  Percentile P90 (90%):{:.4} µm", mc_report.percentiles.p90);
    println!("  Percentile P99 (99%):{:.4} µm", mc_report.percentiles.p99);
    if let Some(yield_val) = mc_report.yield_percent {
        println!("  Estimated Yield:     {:.1}%\n", yield_val);
    }

    println!("Tolerancing and Monte Carlo simulation successfully completed.");
    Ok(())
}
