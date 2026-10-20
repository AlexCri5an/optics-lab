//! High-performance optical optimization engine for sequential lens systems.
//!
//! Provides Damped Least Squares (Levenberg-Marquardt), merit function evaluations,
//! global optimization (multi-start random restarts, simulated annealing), and discrete
//! glass catalog substitution ("Hammer" optimizer).

pub mod dls;
pub mod error;
pub mod global;
pub mod hammer;
pub mod merit;
pub mod operands;
pub mod variables;

pub use dls::{ControlFlow, DlsConfig, DlsOptimizer, OptimizationCallback, OptimizationResult};
pub use error::OptimizeError;
pub use global::{
    MultiStartConfig, MultiStartOptimizer, SimulatedAnnealingConfig, SimulatedAnnealingOptimizer,
};
pub use hammer::{GlassHammerConfig, GlassHammerOptimizer};
pub use merit::{MeritEvaluation, MeritFunction};
pub use operands::{MeritOperand, OperandType};
pub use variables::{VariableDef, VariableSet, VariableType};

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::analysis::spot_fan::PupilPattern;
    use optics_core::analysis::trace_spot_diagram;
    use optics_core::prescription::{
        OpticalPrescription, OpticalSurface, calculate_paraxial_properties,
    };
    use optics_glass::catalog::GlassCatalogDb;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Helper to construct a simple singlet lens.
    fn make_singlet_prescription(r1: f64, r2: f64, d: f64, glass: &str) -> OpticalPrescription {
        OpticalPrescription {
            name: "Test Singlet".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: r1,
                    conic_constant: 0.0,
                    thickness_mm: d,
                    semi_diameter_mm: 12.0,
                    material_name: glass.to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: r2,
                    conic_constant: 0.0,
                    thickness_mm: 100.0, // initial air gap
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        }
    }

    /// Acceptance test: DLS optimizer bends a singlet lens to minimize spherical aberration
    /// while strictly holding the target focal length at 100.0 mm.
    #[test]
    fn test_dls_singlet_spherical_aberration_minimization() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let initial_presc = make_singlet_prescription(50.0, f64::INFINITY, 5.0, "N-BK7");

        // Set up variables: front curvature (c1) and back curvature (c2)
        let mut vars = VariableSet::new();
        vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.1, 0.1);
        vars.add_with_bounds(VariableType::Curvature { surface_index: 1 }, -0.1, 0.1);

        // Merit function: target EFL = 100 mm, target S1 (spherical aberration) = 0.0
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
            "MIN_S1",
            OperandType::SeidelSpherical {
                field_angle_deg: 0.0,
                wavelength_nm: 587.56,
            },
            0.0,
            10.0,
        ));

        let optimizer = DlsOptimizer::new(DlsConfig {
            max_iterations: 30,
            ..Default::default()
        });

        let result = optimizer.optimize(&initial_presc, &vars, &mf, &catalog, None)?;

        // Verify merit reduction
        assert!(
            result.final_merit < result.initial_merit,
            "Optimization must reduce merit: initial {}, final {}",
            result.initial_merit,
            result.final_merit
        );

        // Verify EFL constraint is preserved within 0.1%
        let resolved = result.optimized_prescription.resolve(&catalog)?;
        let parax = calculate_paraxial_properties(&resolved, 587.56)?;
        assert!(
            (parax.efl_mm - 100.0).abs() < 0.2,
            "Optimized EFL must equal 100mm, got {}",
            parax.efl_mm
        );

        Ok(())
    }

    /// Acceptance test: DLS optimizer minimizes RMS spot radius across field.
    #[test]
    fn test_dls_singlet_rms_spot_reduction() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let initial_presc = make_singlet_prescription(60.0, -120.0, 6.0, "N-BK7");

        let mut vars = VariableSet::new();
        vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.05, 0.05);
        vars.add_with_bounds(VariableType::Curvature { surface_index: 1 }, -0.05, 0.05);

        let mf = MeritFunction::default_rms_spot(&initial_presc, Some(100.0));

        let optimizer = DlsOptimizer::new(DlsConfig {
            max_iterations: 25,
            ..Default::default()
        });

        let result = optimizer.optimize(&initial_presc, &vars, &mf, &catalog, None)?;

        assert!(result.final_merit < result.initial_merit);

        let resolved_init = initial_presc.resolve(&catalog)?;
        let resolved_opt = result.optimized_prescription.resolve(&catalog)?;

        let spot_init = trace_spot_diagram(
            &resolved_init,
            (0.0, 0.0),
            587.56,
            PupilPattern::Hexapolar { num_rings: 3 },
        )?;
        let spot_opt = trace_spot_diagram(
            &resolved_opt,
            (0.0, 0.0),
            587.56,
            PupilPattern::Hexapolar { num_rings: 3 },
        )?;

        assert!(
            spot_opt.metrics.rms_radius_um <= spot_init.metrics.rms_radius_um,
            "Optimized spot ({} um) must be smaller than initial ({} um)",
            spot_opt.metrics.rms_radius_um,
            spot_init.metrics.rms_radius_um
        );

        Ok(())
    }

    /// Acceptance test: Multi-start global optimization and Simulated Annealing.
    #[test]
    fn test_global_multi_start_and_annealing() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let initial_presc = make_singlet_prescription(50.0, -200.0, 5.0, "N-BK7");

        let mut vars = VariableSet::new();
        vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.05, 0.05);
        vars.add_with_bounds(VariableType::Curvature { surface_index: 1 }, -0.05, 0.05);

        let mf = MeritFunction::default_rms_spot(&initial_presc, Some(100.0));

        // Test Multi-Start Global Search
        let ms_opt = MultiStartOptimizer::new(MultiStartConfig {
            num_trials: 4,
            time_budget_secs: 10.0,
            seed: 999,
            ..Default::default()
        });
        let ms_res = ms_opt.optimize(&initial_presc, &vars, &mf, &catalog, None)?;
        assert!(ms_res.final_merit <= ms_res.initial_merit);

        // Test Simulated Annealing
        let sa_opt = SimulatedAnnealingOptimizer::new(SimulatedAnnealingConfig {
            initial_temperature: 10.0,
            cooling_rate: 0.8,
            steps_per_temperature: 5,
            max_cooling_cycles: 5,
            seed: 888,
        });
        let sa_res = sa_opt.optimize(&initial_presc, &vars, &mf, &catalog, None)?;
        assert!(sa_res.final_merit <= sa_res.initial_merit);

        Ok(())
    }

    /// Acceptance test: Glass Hammer optimizer searches for optical glass substitutions.
    #[test]
    fn test_glass_hammer_optimizer() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let initial_presc = make_singlet_prescription(50.0, f64::INFINITY, 5.0, "F2");

        let mut vars = VariableSet::new();
        vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.05, 0.05);

        let mf = MeritFunction::default_rms_spot(&initial_presc, Some(100.0));

        let hammer = GlassHammerOptimizer::new(GlassHammerConfig {
            target_surfaces: vec![0],
            candidate_glasses: vec!["N-BK7".to_string(), "N-SK16".to_string(), "F2".to_string()],
            max_cycles: 1,
            time_budget_secs: 5.0,
            local_dls_config: DlsConfig {
                max_iterations: 5,
                ..Default::default()
            },
        });

        let res = hammer.optimize(&initial_presc, &vars, &mf, &catalog, None)?;
        assert!(res.final_merit <= res.initial_merit);
        Ok(())
    }

    /// Acceptance test: DLS simultaneously balances focal length, spherical aberration,
    /// and axial chromatic aberration on an achromatic crown-flint doublet.
    #[test]
    fn test_dls_achromatic_doublet_optimization() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let doublet_presc = OpticalPrescription {
            name: "Achromatic Doublet".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![486.13, 587.56, 656.27],
            primary_wavelength_index: 1,
            surfaces: vec![
                // Crown element (N-BK7)
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: 55.0,
                    conic_constant: 0.0,
                    thickness_mm: 6.0,
                    semi_diameter_mm: 12.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                // Cemented interface (F2 flint)
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: -35.0,
                    conic_constant: 0.0,
                    thickness_mm: 3.5,
                    semi_diameter_mm: 12.0,
                    material_name: "F2".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                // Back surface to air
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: -120.0,
                    conic_constant: 0.0,
                    thickness_mm: 95.0,
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
            ],
        };

        let mut vars = VariableSet::new();
        vars.add_with_bounds(VariableType::Curvature { surface_index: 0 }, -0.05, 0.05);
        vars.add_with_bounds(VariableType::Curvature { surface_index: 1 }, -0.05, 0.05);
        vars.add_with_bounds(VariableType::Curvature { surface_index: 2 }, -0.05, 0.05);

        let mut mf = MeritFunction::new();
        mf.add_operand(MeritOperand::new(
            "TARGET_EFL",
            OperandType::EffectiveFocalLength {
                wavelength_nm: 587.56,
            },
            100.0,
            100.0,
        ));
        mf.add_operand(MeritOperand::new(
            "MIN_AXIAL_COLOR",
            OperandType::SeidelAxialColor {
                field_angle_deg: 0.0,
                wavelength_nm: 587.56,
            },
            0.0,
            20.0,
        ));
        mf.add_operand(MeritOperand::new(
            "MIN_SPHERICAL",
            OperandType::SeidelSpherical {
                field_angle_deg: 0.0,
                wavelength_nm: 587.56,
            },
            0.0,
            5.0,
        ));

        let optimizer = DlsOptimizer::new(DlsConfig {
            max_iterations: 40,
            ..Default::default()
        });

        let res = optimizer.optimize(&doublet_presc, &vars, &mf, &catalog, None)?;
        assert!(res.final_merit < res.initial_merit);

        let resolved_opt = res.optimized_prescription.resolve(&catalog)?;
        let parax = calculate_paraxial_properties(&resolved_opt, 587.56)?;
        assert!(
            (parax.efl_mm - 100.0).abs() < 0.5,
            "Achromat EFL must be preserved near 100mm, got {}",
            parax.efl_mm
        );

        Ok(())
    }

    /// Acceptance test: DLS optimizes coarse unoptimized Cooke Triplet starting point.
    #[test]
    fn test_dls_cooke_triplet_benchmark() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let zmx = r#"NAME Cooke Triplet Benchmark
ENPD 10.000000
FNUM 5.000000
PWAV 2
WAVL 0.4861327 1.0000
WAVL 0.5875618 1.0000
WAVL 0.6562725 1.0000
YFLD 0.0000 7.0000 10.0000
SURF 0
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
SURF 1
  TYPE STANDARD
  CURV 0.0333333333
  DISZ 3.250000
  GLAS SK16
  DIAM 6.000000
SURF 2
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 6.000000
  DIAM 6.000000
SURF 3
  TYPE STANDARD
  CURV -0.0333333333
  DISZ 1.000000
  GLAS F2
  DIAM 4.500000
SURF 4
  STOP
  TYPE STANDARD
  CURV 0.0333333333
  DISZ 4.750000
  DIAM 4.500000
SURF 5
  TYPE STANDARD
  CURV 0.0250000000
  DISZ 2.950000
  GLAS SK16
  DIAM 5.500000
SURF 6
  TYPE STANDARD
  CURV -0.0333333333
  DISZ 43.500000
  DIAM 5.500000
SURF 7
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 10.000000
"#;
        let sys = optics_zemax::parse_zmx(zmx)?;
        let initial_presc = sys.to_optical_prescription();

        let mut vars = VariableSet::new();
        // Variable curvatures for all 6 active surfaces
        for surf_idx in 0..6 {
            vars.add_with_bounds(VariableType::Curvature { surface_index: surf_idx }, -0.1, 0.1);
        }

        let mf = MeritFunction::default_rms_spot(&initial_presc, Some(50.0));

        let optimizer = DlsOptimizer::new(DlsConfig {
            max_iterations: 20,
            ..Default::default()
        });

        let res = optimizer.optimize(&initial_presc, &vars, &mf, &catalog, None)?;
        println!(
            "Cooke Benchmark Merit: initial {} -> final {} (reduction {:.1}%)",
            res.initial_merit,
            res.final_merit,
            (1.0 - res.final_merit / res.initial_merit) * 100.0
        );
        assert!(res.final_merit < res.initial_merit);
        assert!(
            res.final_merit < res.initial_merit * 0.8,
            "Expect at least 20% merit reduction on unoptimized benchmark"
        );

        Ok(())
    }
}
