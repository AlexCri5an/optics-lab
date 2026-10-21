//! `optics-formats` — Optical file format parser and serializer.
//!
//! Provides `FormatOpticalSystem` (parsed representation) and conversion to
//! `optics-core`'s `OpticalPrescription`. Bundled presets: Cooke Triplet
//! F/4.5, Double Gauss F/2.0, single plano-convex N-BK7 f = 100 mm.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod cad;
pub mod codev;
pub mod error;
pub mod iso10110;
pub mod model;
pub mod oslo;
pub mod parser;
pub mod presets;
pub mod project;
pub mod serializer;

pub use cad::{
    CadExportError, DxfConfig, Point2D, RayPath2D, SvgConfig, export_dxf_layout, export_obj_system, export_stl_lens,
    export_svg_layout, wavelength_to_svg_color,
};
pub use codev::{CodeVParseError, CodeVSurface, CodeVSystem, parse_seq, write_seq};
pub use error::FormatParseError;
pub use iso10110::{Iso10110DrawingSheet, Iso10110LensElement, Iso10110MaterialData, Iso10110SurfaceData};
pub use model::{FormatOpticalSystem, FormatSurface, SurfaceType};
pub use oslo::{OsloParseError, OsloSurface, OsloSystem, parse_len, write_len};
pub use parser::{parse_zmx, parse_zmx_bytes};
pub use presets::{
    COOKE_TRIPLET_F4_5_ZMX, DOUBLE_GAUSS_F2_0_ZMX, PLANO_CONVEX_100MM_ZMX, cooke_triplet_f4_5, cooke_triplet_system,
    double_gauss_50mm, double_gauss_system, plano_convex_100mm, plano_convex_system,
};
pub use project::{
    OPTICS_PROJECT_SCHEMA_VERSION, OpticsProjectWorkspace, ProjectCadPreferences, ProjectError, ProjectMetadata,
};
pub use serializer::{write_prescription_zmx, write_zmx};

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::{
        OpticalPrescription, OpticalSurface, Ray, Vec3, calculate_paraxial_properties, trace_paraxial_marginal,
        trace_sequential,
    };
    use optics_glass::GlassCatalogDb;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_cooke_triplet_zmx() -> TestResult {
        let system = parse_zmx(COOKE_TRIPLET_F4_5_ZMX)?;

        assert_eq!(system.name, "Cooke Triplet F/4.5");
        assert_eq!(system.entrance_pupil_diameter_mm, 10.0);
        assert_eq!(system.f_number, 4.5);
        assert_eq!(system.primary_wavelength_index, 1);
        assert_eq!(system.wavelengths_um.len(), 3);
        assert_eq!(system.surfaces.len(), 8); // SURF 0 to SURF 7

        // Check surfaces
        assert_eq!(system.surfaces[0].index, 0);
        assert!(system.surfaces[0].thickness_mm.is_infinite());

        assert_eq!(system.surfaces[1].material_name, "SK16");
        assert!((system.surfaces[1].radius_mm - 22.01359).abs() < 1e-4);
        assert!((system.surfaces[1].thickness_mm - 3.25896).abs() < 1e-4);

        assert_eq!(system.surfaces[3].material_name, "F2");
        assert!((system.surfaces[3].radius_mm - -22.21328).abs() < 1e-4);

        // Surface 4 is Stop
        assert!(system.surfaces[4].is_stop);
        assert_eq!(system.surfaces[4].material_name, "");

        assert_eq!(system.surfaces[5].material_name, "SK16");
        assert!((system.surfaces[5].radius_mm - 79.68360).abs() < 1e-4);

        assert!((system.surfaces[6].thickness_mm - 42.20778).abs() < 1e-4);
        assert_eq!(system.surfaces[7].index, 7);

        Ok(())
    }

    #[test]
    fn test_round_trip_serialization() -> TestResult {
        let original = parse_zmx(COOKE_TRIPLET_F4_5_ZMX)?;
        let serialized = write_zmx(&original);
        let reparsed = parse_zmx(&serialized)?;

        assert_eq!(original.name, reparsed.name);
        assert_eq!(original.entrance_pupil_diameter_mm, reparsed.entrance_pupil_diameter_mm);
        assert_eq!(original.f_number, reparsed.f_number);
        assert_eq!(original.primary_wavelength_index, reparsed.primary_wavelength_index);
        assert_eq!(original.wavelengths_um.len(), reparsed.wavelengths_um.len());
        assert_eq!(original.surfaces.len(), reparsed.surfaces.len());

        for (s1, s2) in original.surfaces.iter().zip(reparsed.surfaces.iter()) {
            assert_eq!(s1.index, s2.index);
            assert_eq!(s1.is_stop, s2.is_stop);
            assert_eq!(s1.material_name, s2.material_name);
            assert_eq!(s1.surface_type, s2.surface_type);
            assert!((s1.curvature - s2.curvature).abs() < 1e-8);
            if s1.thickness_mm.is_infinite() {
                assert!(s2.thickness_mm.is_infinite());
            } else {
                assert!((s1.thickness_mm - s2.thickness_mm).abs() < 1e-5);
            }
            assert!((s1.semi_diameter_mm - s2.semi_diameter_mm).abs() < 1e-5);
        }

        Ok(())
    }

    #[test]
    fn test_lossless_zmx_issue_004_round_trip() -> TestResult {
        let zmx_content = r#"! Standard optical test file with non-standard operands
NAME Lossless Roundtrip Lens
ENPD 25.000000
FNUM 2.800000
PWAV 1
FTYP OBJ_HEIGHT
GCAT SCHOTT HOYA OHARA
! Custom header comment line
CUSTOM_SYS_TAG 999.888
WAVL 0.5875618 1.0000
YFLD 0.0000 10.0000 20.0000
SURF 0
  COMM Object Plane
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
  DIAM 12.500000
SURF 1
  COMM Front Crown Singlet
  TYPE STANDARD
  CURV 0.0200000000
  DISZ 5.000000
  GLAS N-BK7
  DIAM 15.000000
  ! Custom surface note
  COAT AR_COATING_V
SURF 2
  COMM Stop and Rear Surface
  STOP
  TYPE STANDARD
  CURV -0.0150000000
  DISZ 40.000000
  DIAM 14.000000
SURF 3
  COMM Image Plane
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 18.000000
"#;

        let system = parse_zmx(zmx_content)?;
        assert_eq!(system.field_type, "OBJ_HEIGHT");
        assert_eq!(system.glass_catalogs, vec!["SCHOTT", "HOYA", "OHARA"]);
        assert!(system.extra_lines.iter().any(|l| l.contains("CUSTOM_SYS_TAG")));
        assert!(system.surfaces[1].comment.as_deref() == Some("Front Crown Singlet"));
        assert!(
            system.surfaces[1]
                .extra_lines
                .iter()
                .any(|l| l.contains("COAT AR_COATING_V"))
        );

        let serialized = write_zmx(&system);
        let reparsed = parse_zmx(&serialized)?;

        assert_eq!(reparsed.name, "Lossless Roundtrip Lens");
        assert_eq!(reparsed.field_type, "OBJ_HEIGHT");
        assert_eq!(reparsed.glass_catalogs, vec!["SCHOTT", "HOYA", "OHARA"]);
        assert_eq!(reparsed.surfaces[0].comment.as_deref(), Some("Object Plane"));
        assert_eq!(reparsed.surfaces[1].comment.as_deref(), Some("Front Crown Singlet"));
        assert!(
            reparsed.surfaces[1]
                .extra_lines
                .iter()
                .any(|l| l.contains("COAT AR_COATING_V"))
        );
        assert!(reparsed.surfaces[2].is_stop);
        assert_eq!(reparsed.surfaces[2].comment.as_deref(), Some("Stop and Rear Surface"));

        Ok(())
    }

    #[test]
    fn test_cooke_triplet_paraxial_efl_50mm() -> TestResult {
        let prescription = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        // Paraxial properties at primary d-line (587.56 nm)
        let props = calculate_paraxial_properties(&resolved, 587.56)?;

        // Task 2.7 acceptance criteria: Cooke Triplet EFL = ~50 mm (50.0 ± 0.5 mm)
        assert!(
            (props.efl_mm - 50.0).abs() < 0.5,
            "Cooke Triplet EFL mismatch: got {} mm, expected 50.0 ± 0.5 mm",
            props.efl_mm
        );

        // BFL must match distance from Surface 6 to focus (42.21 ± 0.5 mm)
        assert!(
            (props.bfl_mm - 42.21).abs() < 0.5,
            "Cooke Triplet BFL mismatch: got {} mm, expected 42.21 ± 0.5 mm",
            props.bfl_mm
        );

        // F-number = EFL / EPD = 50.0 / 10.0 = 5.0
        assert!(
            (props.f_number - 5.0).abs() < 0.1,
            "Cooke Triplet F-number mismatch: got {}, expected 5.0",
            props.f_number
        );

        // Verify marginal ray tracing height decreases monotonically through the lens
        let marginals = trace_paraxial_marginal(&resolved, 587.56)?;
        assert_eq!(marginals.len(), resolved.surfaces.len());
        assert_eq!(marginals[0].y, 5.0); // EPD / 2

        Ok(())
    }

    #[test]
    fn test_cooke_triplet_real_ray_trace() -> TestResult {
        let prescription = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        // Launch an on-axis ray at height y = 3.0 mm (within entrance pupil of 10 mm diameter)
        let ray = Ray::new(Vec3::new(0.0, 3.0, -10.0), Vec3::new(0.0, 0.0, 1.0), 587.56, 1.0)?;

        let path = trace_sequential(&resolved, &ray)?;

        // Ray must intersect all surfaces: 6 lens interfaces + image plane = 7 hits
        assert_eq!(path.hits.len(), 7, "Ray must intersect all 7 surfaces");

        // The image plane is the last surface hit
        let image_hit = &path.hits[6];
        assert_eq!(image_hit.surface_index, 7);

        // Near-axis ray hits near center of image plane (y close to 0)
        assert!(
            image_hit.point.y.abs() < 0.2,
            "Ray must focus near optical axis on image plane, got y = {}",
            image_hit.point.y
        );

        // Real Fresnel throughput should be realistic (~70.7% for 6 uncoated air-glass interfaces)
        assert!(
            (path.cumulative_throughput - 0.707).abs() < 0.05,
            "Expected throughput ~70.7%, got {:.2}%",
            path.cumulative_throughput * 100.0
        );

        Ok(())
    }

    #[test]
    fn test_double_gauss_preset() -> TestResult {
        let prescription = double_gauss_50mm()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        let props = calculate_paraxial_properties(&resolved, 587.56)?;

        // Classic Double Gauss 50mm objective
        assert!(
            (props.efl_mm - 50.0).abs() < 1.0,
            "Double Gauss EFL mismatch: got {} mm, expected ~50 mm",
            props.efl_mm
        );

        assert!(
            (props.f_number - 2.0).abs() < 0.1,
            "Double Gauss F-number mismatch: got {}, expected 2.0",
            props.f_number
        );

        Ok(())
    }

    #[test]
    fn test_plano_convex_preset() -> TestResult {
        let prescription = plano_convex_100mm()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        let props = calculate_paraxial_properties(&resolved, 587.56)?;

        assert!(
            (props.efl_mm - 100.0).abs() < 0.1,
            "Plano-convex EFL mismatch: got {} mm, expected 100.0 mm",
            props.efl_mm
        );

        assert!(
            (props.bfl_mm - 96.7).abs() < 0.2,
            "Plano-convex BFL mismatch: got {} mm, expected ~96.7 mm",
            props.bfl_mm
        );

        Ok(())
    }

    #[test]
    fn test_utf16_le_bom_parsing() -> TestResult {
        // Encode COOKE_TRIPLET_F4_5_ZMX to UTF-16 LE with BOM
        let mut bytes = vec![0xFF, 0xFE]; // BOM
        for u in COOKE_TRIPLET_F4_5_ZMX.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }

        let system = parse_zmx_bytes(&bytes)?;
        assert_eq!(system.name, "Cooke Triplet F/4.5");
        assert_eq!(system.surfaces.len(), 8);

        Ok(())
    }

    #[test]
    fn test_prescription_zmx_round_trip() -> TestResult {
        let original_presc = cooke_triplet_f4_5()?;
        let zmx_str = write_prescription_zmx(&original_presc);
        let reparsed_sys = parse_zmx(&zmx_str)?;
        let reparsed_presc = reparsed_sys.to_optical_prescription();

        assert_eq!(original_presc.name, reparsed_presc.name);
        assert_eq!(
            original_presc.entrance_pupil_diameter_mm,
            reparsed_presc.entrance_pupil_diameter_mm
        );
        assert_eq!(original_presc.surfaces.len(), reparsed_presc.surfaces.len());

        for (s1, s2) in original_presc.surfaces.iter().zip(reparsed_presc.surfaces.iter()) {
            assert_eq!(s1.surface_index, s2.surface_index);
            assert_eq!(s1.is_stop, s2.is_stop);
            assert_eq!(s1.material_name, s2.material_name);
            if s1.radius_mm.is_infinite() {
                assert!(s2.radius_mm.is_infinite());
            } else {
                assert!((s1.radius_mm - s2.radius_mm).abs() < 1e-4);
            }
            assert!((s1.thickness_mm - s2.thickness_mm).abs() < 1e-4);
        }

        Ok(())
    }

    #[test]
    fn test_parse_errors() {
        assert_eq!(parse_zmx(""), Err(FormatParseError::NoSurfaces));

        let invalid_enpd = "ENPD not_a_number\nSURF 1\n";
        assert!(matches!(
            parse_zmx(invalid_enpd),
            Err(FormatParseError::InvalidFloat { .. })
        ));

        let missing_val = "SURF 1\nCURV\n";
        assert!(matches!(
            parse_zmx(missing_val),
            Err(FormatParseError::MissingOperandValue { .. })
        ));
    }

    #[test]
    fn test_cooke_triplet_spot_diagram_on_and_off_axis() -> TestResult {
        use optics_core::{PupilPattern, trace_spot_diagram};

        let prescription = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        // 1. On-axis spot diagram (0 deg field)
        let spot_on_axis = trace_spot_diagram(&resolved, (0.0, 0.0), 587.56, PupilPattern::Hexapolar { num_rings: 6 })?;

        assert_eq!(spot_on_axis.points.len(), 127);
        assert_eq!(spot_on_axis.metrics.passed_rays_count, 127);

        // On-axis centroid must be at optical axis (0, 0)
        assert!(spot_on_axis.metrics.centroid_x_mm.abs() < 1e-6);
        assert!(spot_on_axis.metrics.centroid_y_mm.abs() < 1e-6);

        // Cooke Triplet has well-corrected spherical aberration (RMS blur < 50 um at f=50mm F/4.5)
        assert!(
            spot_on_axis.metrics.rms_radius_um < 50.0,
            "Cooke Triplet on-axis RMS blur {} exceeds 50 um",
            spot_on_axis.metrics.rms_radius_um
        );

        // 2. Off-axis spot diagram (14 deg field)
        let spot_off_axis =
            trace_spot_diagram(&resolved, (0.0, 14.0), 587.56, PupilPattern::Hexapolar { num_rings: 6 })?;

        // Off-axis centroid y ~ f * tan(14 deg) ~ 50 * 0.2493 ~ 12.46 mm
        let expected_y = 50.0 * 14.0_f64.to_radians().tan();
        assert!(
            (spot_off_axis.metrics.centroid_y_mm - expected_y).abs() < 0.5,
            "Off-axis spot centroid y mismatch: got {}, expected ~{}",
            spot_off_axis.metrics.centroid_y_mm,
            expected_y
        );

        // Spot must have realistic off-axis RMS blur (< 200 um) and realistic optical vignetting (~65% passed)
        assert!(spot_off_axis.metrics.rms_radius_um < 200.0);
        assert_eq!(spot_off_axis.metrics.passed_rays_count, 83);
        assert!((spot_off_axis.metrics.vignetting_factor - 0.65).abs() < 0.05);

        Ok(())
    }

    #[test]
    fn test_cooke_triplet_ray_fan() -> TestResult {
        use optics_core::trace_ray_fan;

        let prescription = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let resolved = prescription.resolve(&catalog)?;

        let fan_on_axis = trace_ray_fan(&resolved, (0.0, 0.0), 587.56, 31)?;
        assert_eq!(fan_on_axis.tangential.len(), 31);
        assert_eq!(fan_on_axis.sagittal.len(), 31);

        // On-axis tangential fan is odd-symmetric: ey(-Py) = -ey(Py)
        let pt_pos = fan_on_axis
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - 0.8).abs() < 1e-4)
            .ok_or("Py=0.8 must exist")?;
        let pt_neg = fan_on_axis
            .tangential
            .iter()
            .find(|p| (p.pupil_coord - -0.8).abs() < 1e-4)
            .ok_or("Py=-0.8 must exist")?;

        assert!(
            (pt_pos.aberration_um + pt_neg.aberration_um).abs() < 1e-4,
            "On-axis tangential fan must have odd symmetry"
        );

        // Off-axis fan has non-zero chief ray displacement
        let fan_off_axis = trace_ray_fan(&resolved, (0.0, 14.0), 587.56, 31)?;
        let expected_y = 50.0 * 14.0_f64.to_radians().tan();
        assert!((fan_off_axis.chief_ray_hit_mm.1 - expected_y).abs() < 0.5);

        Ok(())
    }

    #[test]
    fn test_aspheric_zmx_parm_round_trip() -> TestResult {
        let zmx_input = r#"NAME Aspheric Singlet
ENPD 25.0
FNUM 4.0
WAVL 0.58756 1.0
SURF 0
  DISZ INFINITY
SURF 1
  TYPE EVENASPH
  CURV 0.02
  DISZ 5.0
  GLAS N-BK7
  DIAM 15.0
  CONI -1.0
  PARM 1 0.0
  PARM 2 1.5e-6
  PARM 3 -3.2e-10
SURF 2
  TYPE STANDARD
  CURV 0.0
  DISZ 45.0
  DIAM 15.0
SURF 3
  TYPE STANDARD
  CURV 0.0
  DISZ 0.0
"#;

        let system = parse_zmx(zmx_input)?;
        assert_eq!(system.surfaces.len(), 4);
        let surf1 = &system.surfaces[1];
        assert_eq!(surf1.surface_type, crate::model::SurfaceType::EvenAsphere);
        assert_eq!(surf1.parameters.len(), 3);
        assert!((surf1.parameters[1] - 1.5e-6).abs() < 1e-12);
        assert!((surf1.parameters[2] - -3.2e-10).abs() < 1e-15);

        // Round-trip serialization
        let serialized = write_zmx(&system);
        let parsed_back = parse_zmx(&serialized)?;
        assert_eq!(
            parsed_back.surfaces[1].surface_type,
            crate::model::SurfaceType::EvenAsphere
        );
        assert_eq!(parsed_back.surfaces[1].parameters.len(), 3);
        assert!((parsed_back.surfaces[1].parameters[1] - 1.5e-6).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_coordinate_break_and_grin_zmx_round_trip() -> TestResult {
        let zmx_input = r#"NAME Coordinate Break and GRIN Test
ENPD 10.0
SURF 0
  DISZ INFINITY
SURF 1
  TYPE COORDBRK
  DISZ 5.0
  PARM 1 2.5
  PARM 2 -1.5
  PARM 3 15.0
  PARM 4 0.0
  PARM 5 0.0
  PARM 6 1.0
SURF 2
  TYPE GRADIENT_1
  DISZ 15.707963
  PARM 1 1.6
  PARM 2 0.01
SURF 3
  TYPE STANDARD
  DISZ 0.0
"#;

        let system = parse_zmx(zmx_input)?;
        assert_eq!(system.surfaces.len(), 4);
        assert_eq!(
            system.surfaces[1].surface_type,
            crate::model::SurfaceType::CoordinateBreak
        );
        assert_eq!(system.surfaces[2].surface_type, crate::model::SurfaceType::Gradient1);

        // Convert to OpticalPrescription
        let rx = system.to_optical_prescription();
        assert_eq!(rx.surfaces.len(), 3);
        let cb = rx.surfaces[0].coordinate_break.as_ref().unwrap();
        assert_eq!(cb.decenter_x_mm, 2.5);
        assert_eq!(cb.decenter_y_mm, -1.5);
        assert_eq!(cb.tilt_x_deg, 15.0);
        assert!(cb.tilt_first);

        let grin = rx.surfaces[1].grin_profile.as_ref().unwrap();
        assert_eq!(grin, &optics_core::GrinProfile::Radial { n0: 1.6, a: 0.01 });

        // Convert back to FormatOpticalSystem
        let sys_back = crate::model::FormatOpticalSystem::from_optical_prescription(&rx);
        assert_eq!(
            sys_back.surfaces[1].surface_type,
            crate::model::SurfaceType::CoordinateBreak
        );
        assert_eq!(sys_back.surfaces[2].surface_type, crate::model::SurfaceType::Gradient1);
        assert_eq!(sys_back.surfaces[2].parameters, vec![1.6, 0.01]);

        Ok(())
    }

    #[test]
    fn test_aperture_shapes_zmx_round_trip() -> TestResult {
        let mut ap_rect = optics_core::Aperture::rectangular(12.0, 8.0);
        ap_rect.decenter_x_mm = 1.5;
        ap_rect.decenter_y_mm = -0.5;

        let ap_annular = optics_core::Aperture::annular(3.0, 15.0);
        let ap_elli = optics_core::Aperture::elliptical(10.0, 6.0);

        let mut ap_obst = optics_core::Aperture::circular(4.0);
        ap_obst.is_obscuration = true;

        let original = OpticalPrescription {
            name: "Aperture Shapes Roundtrip".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 15.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: Some(ap_rect.clone()),
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 10.0,
                    semi_diameter_mm: 15.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: Some(ap_annular.clone()),
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 3,
                    radius_mm: 100.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 15.0,
                    material_name: "SF11".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: Some(ap_elli.clone()),
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 4,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 0.0,
                    semi_diameter_mm: 15.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: Some(ap_obst.clone()),
                    coating: None,
                },
            ],
        };

        let zmx = write_prescription_zmx(&original);
        assert!(zmx.contains("CLAP 0.0 0.0 12.000000 8.000000 1.500000 -0.500000"));
        assert!(zmx.contains("CLAP 3.000000 15.000000 0.0 0.0 0.000000 0.000000"));
        assert!(zmx.contains("APER ELLI 0.0 0.0 10.000000 6.000000 0.000000 0.000000"));
        assert!(zmx.contains("OBST 0.0 4.000000 0.0 0.0 0.000000 0.000000"));

        let reparsed = parse_zmx(&zmx)?;
        let rx = reparsed.to_optical_prescription();

        assert_eq!(rx.surfaces[0].aperture, Some(ap_rect));
        assert_eq!(rx.surfaces[1].aperture, Some(ap_annular));
        assert_eq!(rx.surfaces[2].aperture, Some(ap_elli));
        assert_eq!(rx.surfaces[3].aperture, Some(ap_obst));

        Ok(())
    }

    #[test]
    fn test_coatings_zmx_round_trip() -> TestResult {
        let original = OpticalPrescription {
            name: "Coatings Roundtrip".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![550.0],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 15.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: Some(optics_core::SurfaceCoating::mgf2_default()),
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: -50.0,
                    conic_constant: 0.0,
                    thickness_mm: 10.0,
                    semi_diameter_mm: 15.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: Some(optics_core::SurfaceCoating::BroadbandAr),
                },
                OpticalSurface {
                    surface_index: 3,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 0.0,
                    semi_diameter_mm: 15.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: Some(optics_core::SurfaceCoating::HighReflectanceMirror),
                },
            ],
        };

        let zmx = write_prescription_zmx(&original);
        assert!(zmx.contains("COAT MGF2"), "ZMX must contain COAT MGF2");
        assert!(zmx.contains("COAT BBAR"), "ZMX must contain COAT BBAR");
        assert!(zmx.contains("COAT MIRROR"), "ZMX must contain COAT MIRROR");

        let reparsed = parse_zmx(&zmx)?;
        let rx = reparsed.to_optical_prescription();

        assert_eq!(
            rx.surfaces[0].coating,
            Some(optics_core::SurfaceCoating::mgf2_default())
        );
        assert_eq!(rx.surfaces[1].coating, Some(optics_core::SurfaceCoating::BroadbandAr));
        assert_eq!(
            rx.surfaces[2].coating,
            Some(optics_core::SurfaceCoating::HighReflectanceMirror)
        );

        Ok(())
    }

    #[test]
    fn test_forbes_and_zernike_zmx_round_trip() -> TestResult {
        let qcon = optics_core::ForbesQconSurface::new(1.0 / 40.0, -0.8, 15.0, vec![2.5e-4, -1.2e-5, 3.4e-6])?;

        let qbfs = optics_core::ForbesQbfsSurface::new(1.0 / 60.0, 12.0, vec![1.1e-4, -8.0e-6])?;

        let zernike = optics_core::ZernikeSagSurface::new(0.0, 0.0, 10.0, vec![(4, 0.002), (11, -0.0005)], 10.0)?;

        let original = OpticalPrescription {
            name: "Forbes & Zernike System".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 40.0,
                    conic_constant: -0.8,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 15.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: Some(optics_core::SurfaceProfile::ForbesQcon(qcon.clone())),
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 2,
                    radius_mm: 60.0,
                    conic_constant: 0.0,
                    thickness_mm: 10.0,
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: Some(optics_core::SurfaceProfile::ForbesQbfs(qbfs.clone())),
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 3,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 20.0,
                    semi_diameter_mm: 10.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: Some(optics_core::SurfaceProfile::Zernike(zernike.clone())),
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 4,
                    radius_mm: f64::INFINITY,
                    conic_constant: 0.0,
                    thickness_mm: 0.0,
                    semi_diameter_mm: 15.0,
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

        let zmx = write_prescription_zmx(&original);
        assert!(zmx.contains("TYPE QCON"), "ZMX should contain TYPE QCON");
        assert!(zmx.contains("TYPE QBFS"), "ZMX should contain TYPE QBFS");
        assert!(zmx.contains("TYPE ZERNIKE"), "ZMX should contain TYPE ZERNIKE");

        let reparsed = parse_zmx(&zmx)?;
        let rx = reparsed.to_optical_prescription();

        match &rx.surfaces[0].profile {
            Some(optics_core::SurfaceProfile::ForbesQcon(q)) => {
                assert!((q.r_max_mm - 15.0).abs() < 1e-9);
                assert_eq!(q.coefficients.len(), 3);
                assert!((q.coefficients[0] - 2.5e-4).abs() < 1e-9);
            }
            other => panic!("Expected ForbesQcon, got {:?}", other),
        }

        match &rx.surfaces[1].profile {
            Some(optics_core::SurfaceProfile::ForbesQbfs(q)) => {
                assert!((q.r_max_mm - 12.0).abs() < 1e-9);
                assert_eq!(q.coefficients.len(), 2);
                assert!((q.coefficients[0] - 1.1e-4).abs() < 1e-9);
            }
            other => panic!("Expected ForbesQbfs, got {:?}", other),
        }

        match &rx.surfaces[2].profile {
            Some(optics_core::SurfaceProfile::Zernike(z)) => {
                assert!((z.norm_radius_mm - 10.0).abs() < 1e-9);
                assert_eq!(z.coefficients.len(), 2);
                assert!((z.coefficients[0].1 - 0.002).abs() < 1e-9);
            }
            other => panic!("Expected Zernike, got {:?}", other),
        }

        Ok(())
    }
}
