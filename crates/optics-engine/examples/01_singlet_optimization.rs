//! Example 01: Damped Least Squares (Levenberg-Marquardt) Singlet Lens Optimization
//!
//! Demonstrates using `optics-optimize` to bend a singlet lens (optimizing front and back curvatures)
//! to minimize third-order spherical aberration while preserving an exact target focal length of 100.0 mm.

use optics_core::prescription::{
    OpticalPrescription, OpticalSurface, calculate_paraxial_properties,
};
use optics_glass::catalog::GlassCatalogDb;
use optics_optimize::{
    DlsConfig, DlsOptimizer, MeritFunction, MeritOperand, OperandType, VariableSet, VariableType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 01: Singlet Lens Optimization (DLS) ===\n");

    // 1. Load glass catalog and initialize prescription
    let catalog = GlassCatalogDb::load_bundled()?;
    let initial_prescription = OpticalPrescription {
        name: "Plano-Convex Singlet 100mm".to_string(),
        entrance_pupil_diameter_mm: 20.0,
        field_angles_deg: vec![0.0],
        wavelengths_nm: vec![587.56],
        primary_wavelength_index: 0,
        surfaces: vec![
            OpticalSurface {
                surface_index: 0,
                radius_mm: 50.0,
                conic_constant: 0.0,
                thickness_mm: 5.0,
                semi_diameter_mm: 12.0,
                material_name: "N-BK7".to_string(),
                is_stop: true,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
            },
            OpticalSurface {
                surface_index: 1,
                radius_mm: f64::INFINITY, // Flat back surface
                conic_constant: 0.0,
                thickness_mm: 96.7, // Initial estimated back focal distance
                semi_diameter_mm: 12.0,
                material_name: "AIR".to_string(),
                is_stop: false,
                asphere_coefficients: Vec::new(),
                coordinate_break: None,
                grin_profile: None,
            },
        ],
    };

    let resolved_initial = initial_prescription.resolve(&catalog)?;
    let parax_initial = calculate_paraxial_properties(&resolved_initial, 587.56)?;

    println!("Initial Prescription:");
    println!(
        "  Surface 1: R = {:.2} mm, Glass = N-BK7, t = 5.0 mm",
        initial_prescription.surfaces[0].radius_mm
    );
    println!("  Surface 2: R = Flat (inf), Glass = AIR");
    println!(
        "  Initial EFL: {:.3} mm, BFL: {:.3} mm\n",
        parax_initial.efl_mm, parax_initial.bfl_mm
    );

    // 2. Define optimization variables: surface 1 and surface 2 curvatures
    let mut vars = VariableSet::new();
    vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.1, 0.1);
    vars.add_with_bounds(VariableType::Curvature { surface_index: 1 }, -0.1, 0.1);

    // 3. Define merit function:
    //    - Operand 1: Target EFL = 100.0 mm (high weight)
    //    - Operand 2: Minimize Seidel spherical aberration S1 = 0.0
    let mut mf = MeritFunction::new();
    mf.add_operand(MeritOperand::new(
        "TARGET_EFL",
        OperandType::EffectiveFocalLength {
            wavelength_nm: 587.56,
        },
        100.0,
        500.0,
    ));
    mf.add_operand(MeritOperand::new(
        "MIN_SPHERICAL_S1",
        OperandType::SeidelSpherical {
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
        },
        0.0,
        10.0,
    ));

    let initial_eval = mf.evaluate(&initial_prescription, &catalog)?;
    println!("Initial Merit Function: {:.6}", initial_eval.total_merit);

    // 4. Run Damped Least Squares optimization
    let config = DlsConfig {
        max_iterations: 30,
        ..Default::default()
    };
    let optimizer = DlsOptimizer::new(config);

    println!("Executing Damped Least Squares optimization...");
    let result = optimizer.optimize(&initial_prescription, &vars, &mf, &catalog, None)?;

    println!(
        "Optimization converged in {} iterations.",
        result.iterations
    );
    println!("  Initial Merit: {:.6}", result.initial_merit);
    println!("  Final Merit:   {:.6}", result.final_merit);
    println!(
        "  Merit Reduction: {:.2}%\n",
        (1.0 - result.final_merit / result.initial_merit) * 100.0
    );

    // 5. Inspect optimized prescription and cardinal parameters
    let resolved_opt = result.optimized_prescription.resolve(&catalog)?;
    let parax_opt = calculate_paraxial_properties(&resolved_opt, 587.56)?;

    println!("Optimized Prescription:");
    println!(
        "  Surface 1: R = {:.3} mm (c = {:.6} mm^-1)",
        result.optimized_prescription.surfaces[0].radius_mm,
        1.0 / result.optimized_prescription.surfaces[0].radius_mm
    );
    println!(
        "  Surface 2: R = {:.3} mm (c = {:.6} mm^-1)",
        result.optimized_prescription.surfaces[1].radius_mm,
        1.0 / result.optimized_prescription.surfaces[1].radius_mm
    );
    println!(
        "  Optimized EFL: {:.3} mm (Target: 100.000 mm)",
        parax_opt.efl_mm
    );
    println!("  Optimized BFL: {:.3} mm\n", parax_opt.bfl_mm);

    println!("Optimization successfully completed without panics.");
    Ok(())
}
