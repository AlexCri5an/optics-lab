//! End-to-end integration tests for prescription format parsing, ray tracing,
//! optical performance validation, and lossless round-trip serialization.

use optics_core::analysis::{PupilPattern, trace_spot_diagram};
use optics_core::prescription::calculate_paraxial_properties;
use optics_formats::presets::{
    COOKE_TRIPLET_F4_5_ZMX, DOUBLE_GAUSS_F2_0_ZMX, PLANO_CONVEX_100MM_ZMX, cooke_triplet_f4_5,
};
use optics_formats::{parse_zmx, write_zmx};
use optics_glass::GlassCatalogDb;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Acceptance criterion: Cooke Triplet paraxial EFL within 0.1% of nominal 50.0 mm (50.0 ± 0.05 mm).
#[test]
fn test_e2e_cooke_triplet_efl_within_0_1_percent() -> TestResult {
    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let paraxial = calculate_paraxial_properties(&resolved, 587.56)?;
    let nominal_efl = 50.0_f64;
    let error_fraction = (paraxial.efl_mm - nominal_efl).abs() / nominal_efl;

    assert!(
        error_fraction <= 0.001,
        "Cooke Triplet EFL error exceeds 0.1%: got {:.4} mm, error = {:.3}%",
        paraxial.efl_mm,
        error_fraction * 100.0
    );
    assert!(paraxial.efl_mm > 0.0);
    assert_eq!(resolved.surfaces.len(), 7);
    Ok(())
}

/// Acceptance criterion: Cooke Triplet spot diagram RMS radius < 50 µm at 0° field (on-axis).
#[test]
fn test_e2e_cooke_triplet_spot_rms_under_50um() -> TestResult {
    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let spot = trace_spot_diagram(&resolved, (0.0, 0.0), 587.56, PupilPattern::Hexapolar { num_rings: 6 })?;

    assert_eq!(spot.metrics.total_rays_traced, 127);
    assert_eq!(spot.metrics.passed_rays_count, 127);
    assert!(
        spot.metrics.rms_radius_um < 50.0,
        "Cooke Triplet on-axis RMS blur radius must be < 50 µm, got {:.2} µm",
        spot.metrics.rms_radius_um
    );
    assert!(
        spot.metrics.geo_radius_um < 50.0,
        "Cooke Triplet on-axis GEO blur radius must be < 50 µm, got {:.2} µm",
        spot.metrics.geo_radius_um
    );
    Ok(())
}

/// Acceptance criterion: Round-trip parse -> write -> parse .zmx gives identical optical systems.
#[test]
fn test_e2e_round_trip_zmx_all_presets() -> TestResult {
    let presets = [
        ("Cooke Triplet F/4.5", COOKE_TRIPLET_F4_5_ZMX),
        ("Double Gauss F/2.0", DOUBLE_GAUSS_F2_0_ZMX),
        ("Plano-Convex 100mm", PLANO_CONVEX_100MM_ZMX),
    ];

    for (name, zmx_src) in presets {
        let system_orig = parse_zmx(zmx_src)?;
        let serialized = write_zmx(&system_orig);
        let system_reparsed = parse_zmx(&serialized)?;

        assert_eq!(system_orig.name, system_reparsed.name, "{name}: name mismatch");
        assert!(
            (system_orig.entrance_pupil_diameter_mm - system_reparsed.entrance_pupil_diameter_mm).abs() < 1e-5,
            "{name}: EPD mismatch"
        );
        assert!(
            (system_orig.f_number - system_reparsed.f_number).abs() < 1e-5,
            "{name}: F-number mismatch"
        );
        assert_eq!(
            system_orig.primary_wavelength_index, system_reparsed.primary_wavelength_index,
            "{name}: primary wavelength index mismatch"
        );
        assert_eq!(
            system_orig.surfaces.len(),
            system_reparsed.surfaces.len(),
            "{name}: surface count mismatch"
        );

        for (i, (s1, s2)) in system_orig
            .surfaces
            .iter()
            .zip(system_reparsed.surfaces.iter())
            .enumerate()
        {
            assert_eq!(s1.index, s2.index, "{name} surf {i}: index mismatch");
            assert_eq!(s1.is_stop, s2.is_stop, "{name} surf {i}: is_stop mismatch");
            assert_eq!(s1.material_name, s2.material_name, "{name} surf {i}: material mismatch");
            assert_eq!(s1.surface_type, s2.surface_type, "{name} surf {i}: type mismatch");
            assert!(
                (s1.curvature - s2.curvature).abs() < 1e-8,
                "{name} surf {i}: curvature mismatch"
            );
            if s1.thickness_mm.is_infinite() {
                assert!(
                    s2.thickness_mm.is_infinite(),
                    "{name} surf {i}: expected infinite thickness"
                );
            } else {
                assert!(
                    (s1.thickness_mm - s2.thickness_mm).abs() < 1e-5,
                    "{name} surf {i}: thickness mismatch"
                );
            }
            assert!(
                (s1.semi_diameter_mm - s2.semi_diameter_mm).abs() < 1e-5,
                "{name} surf {i}: semi-diameter mismatch"
            );
        }
    }

    Ok(())
}

/// Acceptance criterion: Cooke Triplet Seidel aberration analysis produces balanced Petzval curvature.
#[test]
fn test_e2e_cooke_triplet_seidel_aberrations() -> TestResult {
    use optics_core::analysis::compute_seidel_coefficients;

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let seidel = compute_seidel_coefficients(&resolved, 0.0, 587.56)?;

    assert_eq!(seidel.surfaces.len(), 7);
    assert!(seidel.petzval_sum > 0.0 && seidel.petzval_sum < 0.02);
    assert!(seidel.s1_spherical.is_finite());
    assert!(seidel.s2_coma.abs() < 1e-10); // Zero on-axis coma
    assert!(seidel.w040_spherical.is_finite());
    Ok(())
}

/// Acceptance criterion: Cooke Triplet wavefront OPD map and Zernike decomposition.
#[test]
fn test_e2e_cooke_triplet_wavefront_and_zernike() -> TestResult {
    use optics_core::analysis::{compute_wavefront_map, fit_zernike_noll};

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let wf = compute_wavefront_map(&resolved, 32, 0.0, 587.56, true)?;

    assert_eq!(wf.grid_size, 32);
    assert!(wf.pv_waves > 0.0 && wf.pv_waves < 5.0);
    assert!(wf.rms_waves > 0.0 && wf.rms_waves < 1.0);
    assert!(wf.strehl_ratio >= 0.0 && wf.strehl_ratio <= 1.0);

    let zernike = fit_zernike_noll(&wf, 15)?;
    assert_eq!(zernike.coefficients.len(), 15);
    assert!(zernike.residual_rms_waves < 1.0);
    Ok(())
}

/// Acceptance criterion: Cooke Triplet FFT PSF computation and encircled energy.
#[test]
fn test_e2e_cooke_triplet_fft_psf_and_encircled_energy() -> TestResult {
    use optics_core::analysis::compute_fft_psf;

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let psf = compute_fft_psf(&resolved, 32, 64, 0.0, 587.56)?;

    assert_eq!(psf.grid_size, 64);
    assert!(psf.pixel_size_um > 0.0);
    assert!(psf.airy_radius_um > 0.0);
    assert!(psf.strehl_ratio >= 0.0 && psf.strehl_ratio <= 1.0);
    assert_eq!(psf.encircled_energy.len(), 30);

    // Encircled energy must be monotonically non-decreasing
    for w in psf.encircled_energy.windows(2) {
        assert!(w[0].1 <= w[1].1 + 1e-6);
    }
    Ok(())
}

/// Acceptance criterion: Cooke Triplet diffraction MTF and closed-form limit.
#[test]
fn test_e2e_cooke_triplet_diffraction_mtf() -> TestResult {
    use optics_core::analysis::compute_diffraction_mtf;

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let mtf = compute_diffraction_mtf(&resolved, 20, 0.0, 587.56)?;

    assert_eq!(mtf.points.len(), 20);
    assert!(mtf.cutoff_frequency_lpm > 100.0);
    assert_eq!(mtf.points[0].tangential_mtf, 1.0);
    assert_eq!(mtf.points[0].sagittal_mtf, 1.0);
    // At highest frequency, MTF approaches zero
    let last = mtf.points.last().ok_or("Empty MTF points")?;
    assert!(last.tangential_mtf <= 0.15);
    Ok(())
}

/// Acceptance criterion: Cooke Triplet field curves (distortion, field curvature, lateral color).
#[test]
fn test_e2e_cooke_triplet_field_curves() -> TestResult {
    use optics_core::analysis::compute_field_curves;

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let field_curves = compute_field_curves(&resolved, 5, 10.0, 587.56)?;

    assert_eq!(field_curves.points.len(), 5);
    assert_eq!(field_curves.points[0].field_angle_deg, 0.0);
    assert_eq!(field_curves.points[0].distortion_percent, 0.0);
    let last_fc = field_curves.points.last().ok_or("Empty field curves points")?;
    assert!(last_fc.relative_illumination_percent > 70.0);
    Ok(())
}

/// Acceptance criterion: Cooke Triplet surface-by-surface ray table and footprint.
#[test]
fn test_e2e_cooke_triplet_ray_table_and_footprint() -> TestResult {
    use optics_core::analysis::{PupilPattern, compute_footprint_diagram, compute_ray_table};

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let table = compute_ray_table(&resolved, 0.0, 0.5, 0.0, 587.56)?;
    assert_eq!(table.rows.len(), 7);
    assert!(!table.is_vignetted);
    assert!(table.total_opl_mm > 40.0);

    let footprint = compute_footprint_diagram(
        &resolved,
        3, // Aperture stop surface
        PupilPattern::Hexapolar { num_rings: 4 },
        0.0,
        587.56,
    )?;
    assert!(footprint.max_beam_radius_mm > 0.0);
    assert!(footprint.max_beam_radius_mm <= footprint.semi_diameter_mm + 1e-3);
    Ok(())
}

/// Acceptance criterion: Cooke Triplet two-bounce ghost analysis and prescription summary.
#[test]
fn test_e2e_cooke_triplet_ghosts_and_summary() -> TestResult {
    use optics_core::analysis::{compute_two_bounce_ghosts, generate_prescription_summary};

    let prescription = cooke_triplet_f4_5()?;
    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let summary = generate_prescription_summary(&resolved, 587.56)?;
    assert!((summary.efl_mm - 50.0).abs() < 0.1);
    assert_eq!(summary.surface_count, 7);
    assert_eq!(summary.stop_surface_index, 3);

    let ghosts = compute_two_bounce_ghosts(&resolved, 587.56, 0.005)?;
    assert!(!ghosts.ghosts.is_empty());
    assert!(ghosts.total_ghost_power_fraction > 0.0);
    Ok(())
}
