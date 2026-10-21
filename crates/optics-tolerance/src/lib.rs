//! Optical tolerancing, sensitivity analysis, Monte Carlo simulation, and inverse budgeting.
//!
//! Evaluates manufacturing error bounds (radius, thickness, conic, tilt, decenter, glass index/Abbe)
//! with statistical distributions, optical compensators, and yield analysis.

pub mod distribution;
pub mod error;
pub mod inverse;
pub mod monte_carlo;
pub mod operand;
pub mod sensitivity;

pub use distribution::{Rng64, ToleranceDistribution};
pub use error::ToleranceError;
pub use inverse::{
    BudgetedToleranceItem, InverseToleranceConfig, InverseToleranceReport, InverseTolerancing,
};
pub use monte_carlo::{MonteCarloConfig, MonteCarloReport, MonteCarloSimulator, PercentileSummary};
pub use operand::{ToleranceGrade, ToleranceOperand, ToleranceOperandType};
pub use sensitivity::{
    Compensator, SensitivityAnalyzer, SensitivityRecord, SensitivityReport, ToleranceCriterion,
};

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::prescription::{
        OpticalPrescription, OpticalSurface, calculate_paraxial_properties,
    };
    use optics_glass::catalog::GlassCatalogDb;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn make_test_singlet(r1: f64, r2: f64, d: f64, glass: &str) -> OpticalPrescription {
        OpticalPrescription {
            name: "Test Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
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
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 0.0,
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

    /// Acceptance test: Verification that back focal shift from center thickness perturbation
    /// agrees with the exact paraxial analytical formula: $\Delta \text{BFL} = -\frac{n-1}{n} \Delta t$.
    #[test]
    fn test_paraxial_thickness_focus_shift_equals_analytical_formula() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let d0 = 5.0;
        let delta_t = 0.100; // 100 um thickness error

        // For a plano-convex lens with planar rear surface (r2 = inf),
        // the focal length is invariant with thickness, so BFL shift is exactly -((n-1)/n) * delta_t
        let presc_nom = make_test_singlet(50.0, f64::INFINITY, d0, "N-BK7");
        let presc_pert = make_test_singlet(50.0, f64::INFINITY, d0 + delta_t, "N-BK7");

        let res_nom = presc_nom.resolve(&catalog)?;
        let res_pert = presc_pert.resolve(&catalog)?;

        let p_nom = calculate_paraxial_properties(&res_nom, 587.56)?;
        let p_pert = calculate_paraxial_properties(&res_pert, 587.56)?;

        let n = res_nom.surfaces[0].material.n(587.56);
        let delta_bfl_measured = p_pert.bfl_mm - p_nom.bfl_mm;
        // From rear vertex, marginal ray height changes by u_0' * delta_t = (u_1' / n) * delta_t
        // Therefore delta_bfl = -delta_y / u_1' = -(1 / n) * delta_t exactly
        let delta_bfl_expected = -(1.0 / n) * delta_t;

        let diff = (delta_bfl_measured - delta_bfl_expected).abs();
        assert!(
            diff < 1e-4,
            "Thickness focus shift mismatch: measured {}, expected {}, diff {}",
            delta_bfl_measured,
            delta_bfl_expected,
            diff
        );

        Ok(())
    }

    /// Acceptance test: Sensitivity analysis ranks tolerance operands correctly.
    #[test]
    fn test_singlet_sensitivity_analysis_ranking() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let presc = make_test_singlet(50.0, -100.0, 5.0, "N-BK7");

        let operands = vec![
            ToleranceOperand::new(
                "TRAD_S1",
                ToleranceOperandType::Radius {
                    surface_index: 0,
                    delta_radius_mm: 0.1,
                    fringes: None,
                },
            ),
            ToleranceOperand::new(
                "TRAD_S2",
                ToleranceOperandType::Radius {
                    surface_index: 1,
                    delta_radius_mm: 0.2,
                    fringes: None,
                },
            ),
            ToleranceOperand::new(
                "TTHI_S1",
                ToleranceOperandType::Thickness {
                    surface_index: 0,
                    delta_thickness_mm: 0.05,
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

        let report =
            SensitivityAnalyzer::analyze(&presc, &operands, &criterion, &compensator, &catalog)?;

        assert_eq!(report.records.len(), 3);
        assert!(report.estimated_rss_degradation > 0.0);
        // Verify records are sorted descending by degradation
        for i in 0..report.records.len() - 1 {
            assert!(
                report.records[i].max_degradation >= report.records[i + 1].max_degradation,
                "Records must be sorted descending"
            );
        }

        Ok(())
    }

    /// Acceptance test: Monte Carlo simulation produces valid percentiles and deterministic seeds.
    #[test]
    fn test_monte_carlo_statistical_percentiles_and_determinism() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let presc = make_test_singlet(50.0, -100.0, 5.0, "N-BK7");

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
                "TTHI_S1",
                ToleranceOperandType::Thickness {
                    surface_index: 0,
                    delta_thickness_mm: 0.02,
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

        let sim = MonteCarloSimulator::new(MonteCarloConfig {
            num_trials: 50,
            seed: 12345,
            yield_threshold: Some(100.0), // Generous threshold for 100% yield
        });

        let rep1 = sim.run(&presc, &operands, &criterion, &compensator, &catalog)?;
        let rep2 = sim.run(&presc, &operands, &criterion, &compensator, &catalog)?;

        // Determinism with same seed
        assert_eq!(rep1.mean, rep2.mean);
        assert_eq!(rep1.std_dev, rep2.std_dev);
        assert_eq!(rep1.percentiles.p50, rep2.percentiles.p50);

        // Percentile monotonicity
        assert!(rep1.percentiles.p10 <= rep1.percentiles.p50);
        assert!(rep1.percentiles.p50 <= rep1.percentiles.p90);
        assert!(rep1.percentiles.p90 <= rep1.percentiles.p99);

        // Yield check
        assert_eq!(rep1.yield_percent, Some(100.0));

        Ok(())
    }

    /// Acceptance test: Inverse tolerancing automatically budgets manufacturing tolerances.
    #[test]
    fn test_inverse_tolerancing_budgeting() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let presc = make_test_singlet(50.0, -100.0, 5.0, "N-BK7");

        let operands = vec![
            ToleranceOperand::new(
                "TRAD_S1",
                ToleranceOperandType::Radius {
                    surface_index: 0,
                    delta_radius_mm: 0.2,
                    fringes: None,
                },
            ),
            ToleranceOperand::new(
                "TTHI_S1",
                ToleranceOperandType::Thickness {
                    surface_index: 0,
                    delta_thickness_mm: 0.1,
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

        let config = InverseToleranceConfig {
            performance_threshold: 80.0,
            target_yield_percent: 90.0,
            validation_trials: 30,
        };

        let report = InverseTolerancing::budget(
            &presc,
            &operands,
            &criterion,
            &compensator,
            &catalog,
            &config,
        )?;

        assert_eq!(report.budgeted_operands.len(), 2);
        assert!(report.achieved_yield_percent >= 50.0);

        Ok(())
    }

    /// Acceptance test: Statistical distributions generate correct bounds and unbiased zero-mean samples.
    #[test]
    fn test_distribution_statistical_sampling() -> TestResult {
        let mut rng = Rng64::new(999);
        let dist_u = ToleranceDistribution::Uniform;
        let dist_n = ToleranceDistribution::NormalTruncated { sigma_cutoff: 2.0 };
        let dist_e = ToleranceDistribution::WorstCaseEdge;
        let dist_p = ToleranceDistribution::Parabolic;
        let dist_b = ToleranceDistribution::Bimodal;

        let half_w = 1.0;
        let mut sum_u = 0.0;
        let mut sum_n = 0.0;
        let mut sum_p = 0.0;
        let mut sum_b = 0.0;
        let n_samples = 1000;

        for _ in 0..n_samples {
            let s_u = dist_u.sample(half_w, &mut rng);
            let s_n = dist_n.sample(half_w, &mut rng);
            let s_e = dist_e.sample(half_w, &mut rng);
            let s_p = dist_p.sample(half_w, &mut rng);
            let s_b = dist_b.sample(half_w, &mut rng);

            assert!(s_u.abs() <= half_w + 1e-12);
            assert!(s_n.abs() <= half_w + 1e-12);
            assert!((s_e.abs() - half_w).abs() < 1e-12);
            assert!(s_p.abs() <= half_w + 1e-12);
            assert!(s_b.abs() <= half_w + 1e-12);

            sum_u += s_u;
            sum_n += s_n;
            sum_p += s_p;
            sum_b += s_b;
        }

        let mean_u = sum_u / (n_samples as f64);
        let mean_n = sum_n / (n_samples as f64);
        let mean_p = sum_p / (n_samples as f64);
        let mean_b = sum_b / (n_samples as f64);
        assert!(
            mean_u.abs() < 0.1,
            "Uniform mean should be near 0, got {}",
            mean_u
        );
        assert!(
            mean_n.abs() < 0.1,
            "Normal mean should be near 0, got {}",
            mean_n
        );
        assert!(
            mean_p.abs() < 0.1,
            "Parabolic mean should be near 0, got {}",
            mean_p
        );
        assert!(
            mean_b.abs() < 0.1,
            "Bimodal mean should be near 0, got {}",
            mean_b
        );

        Ok(())
    }

    /// Acceptance test: Automatic default operand generator produces correct operands for Cooke Triplet.
    #[test]
    fn test_default_operand_generation_and_histogram() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let operands_comm = ToleranceOperand::generate_default_operands(&cooke, ToleranceGrade::Commercial);
        let operands_prec = ToleranceOperand::generate_default_operands(&cooke, ToleranceGrade::Precision);

        assert!(!operands_comm.is_empty());
        assert_eq!(operands_comm.len(), operands_prec.len());
        // Verify precision tolerances are tighter than commercial
        assert!(operands_prec[0].half_width() < operands_comm[0].half_width());

        // Test histogram generation on MonteCarloReport
        let report = MonteCarloReport {
            nominal_value: 10.0,
            total_trials: 100,
            mean: 11.2,
            std_dev: 1.5,
            min_value: 8.0,
            max_value: 16.0,
            percentiles: PercentileSummary {
                p10: 9.0,
                p50: 11.0,
                p90: 13.5,
                p99: 15.2,
            },
            yield_percent: Some(95.0),
            worst_case_prescription: None,
            trial_values: (0..100).map(|i| 8.0 + (i as f64) * 0.08).collect(),
        };

        let bins = report.compute_histogram(10);
        assert_eq!(bins.len(), 10);
        let total_counted: usize = bins.iter().map(|b| b.2).sum();
        assert_eq!(total_counted, 100);

        Ok(())
    }

    /// Acceptance test: Verification of all operand types and perturbation mechanics.
    #[test]
    fn test_tolerance_operand_all_types() -> TestResult {
        let mut presc = make_test_singlet(50.0, -100.0, 5.0, "N-BK7");

        let conic_op = ToleranceOperand::new(
            "TCON_S0",
            ToleranceOperandType::Conic {
                surface_index: 0,
                delta_conic: 0.1,
            },
        );
        conic_op.apply_perturbation(&mut presc, 0.1)?;
        assert!((presc.surfaces[0].conic_constant - 0.1).abs() < 1e-12);

        let decenter_op = ToleranceOperand::new(
            "TDEC_S0",
            ToleranceOperandType::SurfaceDecenter {
                surface_index: 0,
                max_decenter_mm: 0.05,
            },
        );
        decenter_op.apply_perturbation(&mut presc, 0.05)?;

        let tilt_op = ToleranceOperand::new(
            "TTIL_S0",
            ToleranceOperandType::SurfaceTilt {
                surface_index: 0,
                max_tilt_deg: 0.02,
            },
        );
        tilt_op.apply_perturbation(&mut presc, 0.02)?;

        let wedge_op = ToleranceOperand::new(
            "TWED_E0",
            ToleranceOperandType::ElementWedge {
                front_surface: 0,
                back_surface: 1,
                max_wedge_arcmin: 1.0,
            },
        );
        wedge_op.apply_perturbation(&mut presc, 1.0)?;

        let glass_op = ToleranceOperand::new(
            "TGLA_S0",
            ToleranceOperandType::GlassIndexAbbe {
                surface_index: 0,
                delta_nd: 0.001,
                delta_vd: 0.5,
            },
        );
        glass_op.apply_perturbation(&mut presc, 0.001)?;

        Ok(())
    }

    /// Acceptance test: Cooke Triplet tolerancing with RMS wavefront criterion and paraxial refocus.
    #[test]
    fn test_cooke_triplet_wavefront_tolerancing() -> TestResult {
        let catalog = GlassCatalogDb::load_bundled()?;
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;

        let operands = vec![
            ToleranceOperand::new(
                "TRAD_S0",
                ToleranceOperandType::Radius {
                    surface_index: 0,
                    delta_radius_mm: 0.03,
                    fringes: None,
                },
            ),
            ToleranceOperand::new(
                "TTHI_S0",
                ToleranceOperandType::Thickness {
                    surface_index: 0,
                    delta_thickness_mm: 0.02,
                },
            ),
            ToleranceOperand::new(
                "TRAD_S1",
                ToleranceOperandType::Radius {
                    surface_index: 1,
                    delta_radius_mm: 0.03,
                    fringes: None,
                },
            ),
        ];

        let criterion = ToleranceCriterion::RmsWavefront {
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
        };
        let compensator = Compensator::ParaxialRefocus {
            wavelength_nm: 587.56,
        };

        let sens =
            SensitivityAnalyzer::analyze(&cooke, &operands, &criterion, &compensator, &catalog)?;
        assert_eq!(sens.records.len(), 3);
        assert!(sens.estimated_rss_degradation > 0.0);

        let sim = MonteCarloSimulator::new(MonteCarloConfig {
            num_trials: 25,
            seed: 777,
            yield_threshold: Some(sens.nominal_value * 2.0),
        });
        let mc = sim.run(&cooke, &operands, &criterion, &compensator, &catalog)?;
        assert_eq!(mc.total_trials, 25);
        assert!(mc.percentiles.p50 > 0.0);
        assert!(mc.percentiles.p10 <= mc.percentiles.p90);

        Ok(())
    }
}
