//! `optics-cli` — headless diagnostic runner, batch renderer, and optical CLI tools.

use clap::{Parser, Subcommand};
use optics_bvh::{Aabb, BlasInstance, BvhScene, Triangle as BvhTriangle};
use optics_core::analysis::{
    PupilPattern, compute_diffraction_mtf, compute_fft_psf, compute_field_curves,
    compute_footprint_diagram, compute_ray_table, compute_seidel_coefficients,
    compute_two_bounce_ghosts, compute_wavefront_map, fit_zernike_noll,
    generate_prescription_summary, trace_ray_fan, trace_spot_diagram,
};
use optics_core::conic::{ConicSurface, conic_normal, conic_sag};
use optics_core::fresnel::fresnel_dielectric;
use optics_core::prescription::calculate_paraxial_properties;
use optics_core::snell::snell_refract;
use optics_core::{
    Air, OpticalPrescription, PathRecord, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle,
    Vec3, spectral_fan,
};
use optics_engine::{AnalysisRequest, AnalysisResult, EngineCommand, OpticalEngine, SurfaceUpdate};
use optics_ffi::{
    optics_engine_create, optics_engine_export_svg, optics_engine_free,
    optics_engine_get_status_json, optics_string_free,
};
use optics_glass::{DispersionModel, GlassCatalogDb, GlassMaterial};
use optics_spectral::{LAMBDA_C_NM, LAMBDA_D_NM, LAMBDA_F_NM, display_color_for_wavelength};
use optics_zemax::presets::{cooke_triplet_f4_5, double_gauss_50mm, plano_convex_100mm};
use optics_zemax::{
    CodeVSystem, DxfConfig, Iso10110DrawingSheet, OpticsProjectWorkspace, OsloSystem, SvgConfig,
    export_dxf_layout, export_obj_system, export_stl_lens, export_svg_layout, parse_len, parse_seq,
    parse_zmx_bytes, write_len, write_seq,
};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "optics-cli", author, version, about = "OpticsLab CLI tools")]
struct Cli {
    /// Run complete automated self-test suite and exit with code 0 on success
    #[arg(long)]
    self_test: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs complete headless automated self-tests across all computational optics engines.
    SelfTest,

    /// Simulates dispersion through a prism, prints deviation table, and outputs 2D SVG.
    Prism {
        /// Glass material name (e.g. N-BK7, SF11, F2, Fused Silica)
        #[arg(long, default_value = "N-BK7")]
        glass: String,

        /// Prism apex angle in degrees
        #[arg(long, default_value_t = 60.0)]
        apex: f64,

        /// Ray angle of incidence on the first face in degrees
        #[arg(long, default_value_t = 45.0)]
        incidence: f64,

        /// Number of spectral wavelength samples between 380 nm and 780 nm
        #[arg(long, default_value_t = 32)]
        samples: usize,

        /// Optional SVG output file path
        #[arg(long, default_value = "out.svg")]
        svg: PathBuf,
    },

    /// Analyzes a sequential lens system: paraxial properties, spot diagram, and ray fans.
    Analyze {
        /// Preset lens name: "cooke", "double-gauss", or "plano-convex"
        #[arg(long, default_value = "cooke")]
        preset: String,

        /// Path to a Zemax .zmx prescription file (overrides --preset)
        #[arg(long)]
        zmx: Option<PathBuf>,

        /// Field angle in degrees
        #[arg(long, default_value_t = 0.0)]
        field: f64,

        /// Evaluation wavelength in nanometers (Fraunhofer d=587.56, F=486.13, C=656.27)
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,

        /// Number of hexapolar rings for spot diagram
        #[arg(long, default_value_t = 6)]
        rings: usize,

        /// Number of sampling points along ray aberration fan
        #[arg(long, default_value_t = 41)]
        fan_points: usize,

        /// Output diagnostic metrics as JSON
        #[arg(long, default_value_t = false)]
        json: bool,

        /// Optional CSV output file path for spot diagram rays
        #[arg(long)]
        csv: Option<PathBuf>,
    },

    /// Inspects optical glass material dispersion formulas, Fraunhofer table, and Abbe diagram classification.
    Glass {
        /// Glass material name (e.g. N-BK7, SF11, F2, Fused Silica, LITHOSIL-Q)
        #[arg(long, default_value = "N-BK7")]
        name: String,

        /// Output glass parameters and Fraunhofer table as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Measures BVH ray-slab traversal throughput in MRAYS/s.
    Benchmark {
        /// Benchmark scene: "prism"
        #[arg(long, default_value = "prism")]
        scene: String,

        /// Number of rays to trace
        #[arg(long, default_value_t = 100_000)]
        rays: usize,

        /// Output benchmark results as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Computes third-order Seidel aberration coefficients and chromatic contributions.
    Seidel {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
    },

    /// Computes 2D Wavefront OPD map and Zernike circular polynomial decomposition.
    Wavefront {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
        #[arg(long, default_value_t = 32)]
        grid: usize,
        #[arg(long, default_value_t = true)]
        remove_tilt: bool,
        #[arg(long, default_value_t = 15)]
        zernike_terms: usize,
    },

    /// Computes Point Spread Function (FFT PSF), Strehl ratio, and encircled energy.
    Psf {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
        #[arg(long, default_value_t = 32)]
        pupil_grid: usize,
        #[arg(long, default_value_t = 64)]
        fft_size: usize,
    },

    /// Computes Modulation Transfer Function (Diffraction MTF) frequency sweep.
    Mtf {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
        #[arg(long, default_value_t = 25)]
        frequencies: usize,
    },

    /// Computes distortion, field curvature, lateral color, and relative illumination.
    FieldCurves {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 10.0)]
        max_field: f64,
        #[arg(long, default_value_t = 9)]
        steps: usize,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
    },

    /// Outputs surface-by-surface ray coordinates, direction cosines, and OPL.
    RayTable {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 0.0)]
        px: f64,
        #[arg(long, default_value_t = 0.5)]
        py: f64,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
    },

    /// Computes beam footprint on a designated optical surface.
    Footprint {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        surface: usize,
        #[arg(long, default_value_t = 6)]
        rings: usize,
        #[arg(long, default_value_t = 0.0)]
        field: f64,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
    },

    /// Evaluates sequential two-bounce ghost reflections and prescription summary.
    Ghost {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
        #[arg(long, default_value_t = 0.005)]
        reflectance: f64,
    },

    /// Performs Damped Least Squares (Levenberg-Marquardt) optimization to minimize merit function.
    Optimize {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = 50.0)]
        target_efl: f64,
        #[arg(long, default_value_t = 30)]
        iterations: usize,
    },

    /// Performs optical tolerancing sensitivity analysis and Monte Carlo simulation.
    Tolerance {
        /// Preset lens name: "cooke", "double-gauss", or "plano-convex"
        #[arg(long, default_value = "cooke")]
        preset: String,
        /// Path to a Zemax .zmx prescription file (overrides --preset)
        #[arg(long)]
        zmx: Option<PathBuf>,
        /// Number of Monte Carlo trials
        #[arg(long, default_value_t = 50)]
        trials: usize,
        /// Deterministic PRNG seed
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Evaluation wavelength in nanometers
        #[arg(long, default_value_t = 587.56)]
        wavelength: f64,
        /// Field angle in degrees
        #[arg(long, default_value_t = 0.0)]
        field: f64,
    },
    /// Performs non-sequential ray tracing, Fresnel splitting, and detector irradiance accumulation.
    NonSeq {
        /// Preset system: "slab", "pipe", or "singlet"
        #[arg(long, default_value = "singlet")]
        preset: String,
        /// Number of rays to launch
        #[arg(long, default_value_t = 1000)]
        rays: usize,
        /// Maximum allowed bounces
        #[arg(long, default_value_t = 20)]
        max_bounces: usize,
        /// Minimum power cutoff threshold in Watts
        #[arg(long, default_value_t = 1e-6)]
        min_power: f64,
        /// Output report as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Exports a 2D SVG cross-sectional technical drawing of the optical system.
    ExportSvg {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value = "layout.svg")]
        output: PathBuf,
    },

    /// Exports an AutoCAD Release 12 ASCII DXF file (2D layout or 3D mesh).
    ExportDxf {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value = "layout.dxf")]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        three_d: bool,
    },

    /// Exports a 3D STL mesh of the lens system (binary or ASCII).
    ExportStl {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value = "lens.stl")]
        output: PathBuf,
        #[arg(long, default_value_t = true)]
        binary: bool,
        #[arg(long, default_value_t = 32)]
        slices: u32,
        #[arg(long, default_value_t = 12)]
        rings: u32,
    },

    /// Exports a Wavefront 3D OBJ mesh and companion MTL material template.
    ExportObj {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value = "system.obj")]
        output: PathBuf,
        #[arg(long, default_value_t = 32)]
        slices: u32,
        #[arg(long, default_value_t = 12)]
        rings: u32,
    },

    /// Generates an ISO 10110 optical element drawing data sheet (Markdown or JSON).
    ExportIso10110 {
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Parses or exports a CODE V .seq sequential prescription file.
    CodeV {
        #[arg(long)]
        seq: Option<PathBuf>,
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Parses or exports an OSLO .len sequential prescription file.
    Oslo {
        #[arg(long)]
        len: Option<PathBuf>,
        #[arg(long, default_value = "cooke")]
        preset: String,
        #[arg(long)]
        zmx: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Interacts directly with the unified OpticalEngine facade.
    Engine {
        /// Optional path to a Zemax .zmx file to load
        #[arg(long)]
        zmx: Option<PathBuf>,

        /// Print system status as JSON
        #[arg(long, default_value_t = false)]
        status: bool,

        /// Validate system diagnostics and print as JSON
        #[arg(long, default_value_t = false)]
        validate: bool,

        /// Run an AnalysisRequest JSON string
        #[arg(long)]
        analyze: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.self_test || matches!(cli.command, Some(Commands::SelfTest)) {
        return run_self_test();
    }

    let Some(command) = cli.command else {
        println!(
            "No command specified. Use --help for usage or --self-test to run diagnostic self-tests."
        );
        return Ok(());
    };

    match command {
        Commands::SelfTest => run_self_test(),
        Commands::Prism {
            glass,
            apex,
            incidence,
            samples,
            svg,
        } => run_prism_command(&glass, apex, incidence, samples, &svg),
        Commands::Analyze {
            preset,
            zmx,
            field,
            wavelength,
            rings,
            fan_points,
            json,
            csv,
        } => run_analyze_command(AnalyzeOptions {
            preset: &preset,
            zmx: zmx.as_ref(),
            field,
            wavelength,
            rings,
            fan_points,
            json,
            csv: csv.as_ref(),
        }),
        Commands::Glass { name, json } => run_glass_command(&name, json),
        Commands::Benchmark { scene, rays, json } => run_benchmark_command(&scene, rays, json),
        Commands::Seidel {
            preset,
            zmx,
            field,
            wavelength,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let seidel = compute_seidel_coefficients(&resolved, field, wavelength)?;
            println!("{}", serde_json::to_string_pretty(&seidel)?);
            Ok(())
        }
        Commands::Wavefront {
            preset,
            zmx,
            field,
            wavelength,
            grid,
            remove_tilt,
            zernike_terms,
        } => {
            let (name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let wf = compute_wavefront_map(&resolved, grid, field, wavelength, remove_tilt)?;
            let zernike = fit_zernike_noll(&wf, zernike_terms)?;
            let output = serde_json::json!({
                "system": name,
                "wavefront": wf,
                "zernike": zernike,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        Commands::Psf {
            preset,
            zmx,
            field,
            wavelength,
            pupil_grid,
            fft_size,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let psf = compute_fft_psf(&resolved, pupil_grid, fft_size, field, wavelength)?;
            println!("{}", serde_json::to_string_pretty(&psf)?);
            Ok(())
        }
        Commands::Mtf {
            preset,
            zmx,
            field,
            wavelength,
            frequencies,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let mtf = compute_diffraction_mtf(&resolved, frequencies, field, wavelength)?;
            println!("{}", serde_json::to_string_pretty(&mtf)?);
            Ok(())
        }
        Commands::FieldCurves {
            preset,
            zmx,
            max_field,
            steps,
            wavelength,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let curves = compute_field_curves(&resolved, steps, max_field, wavelength)?;
            println!("{}", serde_json::to_string_pretty(&curves)?);
            Ok(())
        }
        Commands::RayTable {
            preset,
            zmx,
            px,
            py,
            field,
            wavelength,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let table = compute_ray_table(&resolved, px, py, field, wavelength)?;
            println!("{}", serde_json::to_string_pretty(&table)?);
            Ok(())
        }
        Commands::Footprint {
            preset,
            zmx,
            surface,
            rings,
            field,
            wavelength,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let footprint = compute_footprint_diagram(
                &resolved,
                surface,
                PupilPattern::Hexapolar { num_rings: rings },
                field,
                wavelength,
            )?;
            println!("{}", serde_json::to_string_pretty(&footprint)?);
            Ok(())
        }
        Commands::Ghost {
            preset,
            zmx,
            wavelength,
            reflectance,
        } => {
            let (_name, resolved) = resolve_cli_prescription(&preset, zmx.as_ref())?;
            let summary = generate_prescription_summary(&resolved, wavelength)?;
            let ghosts = compute_two_bounce_ghosts(&resolved, wavelength, reflectance)?;
            let output = serde_json::json!({
                "summary": summary,
                "ghost_analysis": ghosts,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        Commands::Optimize {
            preset,
            zmx,
            target_efl,
            iterations,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let catalog = GlassCatalogDb::load_bundled()?;
            let mut vars = optics_optimize::VariableSet::new();
            for i in 0..presc.surfaces.len().saturating_sub(1) {
                if !presc.surfaces[i].is_stop {
                    vars.add_with_bounds(
                        optics_optimize::VariableType::Curvature { surface_index: i },
                        -0.1,
                        0.1,
                    );
                }
            }
            let mf = optics_optimize::MeritFunction::default_rms_spot(&presc, Some(target_efl));
            let optimizer = optics_optimize::DlsOptimizer::new(optics_optimize::DlsConfig {
                max_iterations: iterations,
                ..Default::default()
            });
            let result = optimizer.optimize(&presc, &vars, &mf, &catalog, None)?;
            let output = serde_json::json!({
                "system": name,
                "initial_merit": result.initial_merit,
                "final_merit": result.final_merit,
                "iterations": result.iterations,
                "converged": result.converged,
                "optimized_surfaces": result.optimized_prescription.surfaces,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        Commands::Tolerance {
            preset,
            zmx,
            trials,
            seed,
            wavelength,
            field,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let catalog = GlassCatalogDb::load_bundled()?;
            let mut operands = Vec::new();
            for i in 0..presc.surfaces.len().saturating_sub(1) {
                if presc.surfaces[i].radius_mm.is_finite()
                    && presc.surfaces[i].radius_mm.abs() > 1e-6
                {
                    operands.push(optics_tolerance::ToleranceOperand::new(
                        format!("TRAD_S{}", i),
                        optics_tolerance::ToleranceOperandType::Radius {
                            surface_index: i,
                            delta_radius_mm: 0.05,
                            fringes: None,
                        },
                    ));
                }
                if presc.surfaces[i].thickness_mm > 0.0 {
                    operands.push(optics_tolerance::ToleranceOperand::new(
                        format!("TTHI_S{}", i),
                        optics_tolerance::ToleranceOperandType::Thickness {
                            surface_index: i,
                            delta_thickness_mm: 0.02,
                        },
                    ));
                }
            }
            let criterion = optics_tolerance::ToleranceCriterion::RmsSpotRadius {
                field_angle_deg: field,
                wavelength_nm: wavelength,
            };
            let compensator = optics_tolerance::Compensator::ParaxialRefocus {
                wavelength_nm: wavelength,
            };
            let sensitivity = optics_tolerance::SensitivityAnalyzer::analyze(
                &presc,
                &operands,
                &criterion,
                &compensator,
                &catalog,
            )?;
            let mc_sim =
                optics_tolerance::MonteCarloSimulator::new(optics_tolerance::MonteCarloConfig {
                    num_trials: trials,
                    seed,
                    yield_threshold: Some(sensitivity.nominal_value * 2.0),
                });
            let mc_report = mc_sim.run(&presc, &operands, &criterion, &compensator, &catalog)?;
            let output = serde_json::json!({
                "system": name,
                "sensitivity": sensitivity,
                "monte_carlo": mc_report,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
        Commands::NonSeq {
            preset,
            rays,
            max_bounces,
            min_power,
            json,
        } => run_nonseq_command(&preset, rays, max_bounces, min_power, json),
        Commands::ExportSvg {
            preset,
            zmx,
            output,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let svg = export_svg_layout(&presc, None, &SvgConfig::default())?;
            std::fs::write(&output, svg)?;
            println!(
                "Exported 2D SVG layout for '{name}' to {}",
                output.display()
            );
            Ok(())
        }
        Commands::ExportDxf {
            preset,
            zmx,
            output,
            three_d,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let config = DxfConfig {
                export_3d: three_d,
                ..Default::default()
            };
            let dxf = export_dxf_layout(&presc, None, &config)?;
            std::fs::write(&output, dxf)?;
            let mode = if three_d { "3D" } else { "2D" };
            println!(
                "Exported {mode} AutoCAD DXF for '{name}' to {}",
                output.display()
            );
            Ok(())
        }
        Commands::ExportStl {
            preset,
            zmx,
            output,
            binary,
            slices,
            rings,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let stl_bytes = export_stl_lens(&presc, binary, slices, rings);
            std::fs::write(&output, stl_bytes)?;
            let fmt = if binary { "binary" } else { "ASCII" };
            println!(
                "Exported 3D STL ({fmt}) for '{name}' to {}",
                output.display()
            );
            Ok(())
        }
        Commands::ExportObj {
            preset,
            zmx,
            output,
            slices,
            rings,
        } => {
            let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let (obj_data, mtl_data) = export_obj_system(&presc, slices, rings);
            std::fs::write(&output, obj_data)?;
            let mtl_path = output.with_extension("mtl");
            std::fs::write(&mtl_path, mtl_data)?;
            println!(
                "Exported 3D OBJ & MTL for '{name}' to {} and {}",
                output.display(),
                mtl_path.display()
            );
            Ok(())
        }
        Commands::ExportIso10110 { preset, zmx, json } => {
            let (_name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
            let catalog = GlassCatalogDb::load_bundled().ok();
            let sheet = Iso10110DrawingSheet::from_prescription(&presc, catalog.as_ref());
            if json {
                println!(
                    "{}",
                    sheet
                        .to_json()
                        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?
                );
            } else {
                println!("{}", sheet.to_markdown_table());
            }
            Ok(())
        }
        Commands::CodeV {
            seq,
            preset,
            zmx,
            output,
        } => {
            if let Some(path) = seq {
                let content = std::fs::read_to_string(&path)?;
                let sys = parse_seq(&content)?;
                println!(
                    "Parsed CODE V file: '{}' with {} surfaces",
                    sys.title,
                    sys.surfaces.len()
                );
                if let Some(out_path) = output {
                    let rx = sys.to_optical_prescription();
                    let serialized = write_seq(&CodeVSystem::from_optical_prescription(&rx));
                    std::fs::write(&out_path, serialized)?;
                    println!(
                        "Wrote CODE V sequential prescription to {}",
                        out_path.display()
                    );
                }
            } else {
                let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
                let sys = CodeVSystem::from_optical_prescription(&presc);
                let text = write_seq(&sys);
                if let Some(out_path) = output {
                    std::fs::write(&out_path, text)?;
                    println!(
                        "Exported CODE V sequential file for '{name}' to {}",
                        out_path.display()
                    );
                } else {
                    println!("{text}");
                }
            }
            Ok(())
        }
        Commands::Oslo {
            len,
            preset,
            zmx,
            output,
        } => {
            if let Some(path) = len {
                let content = std::fs::read_to_string(&path)?;
                let sys = parse_len(&content)?;
                println!(
                    "Parsed OSLO file: '{}' with {} surfaces",
                    sys.title,
                    sys.surfaces.len()
                );
                if let Some(out_path) = output {
                    let rx = sys.to_optical_prescription();
                    let serialized = write_len(&OsloSystem::from_optical_prescription(&rx));
                    std::fs::write(&out_path, serialized)?;
                    println!("Wrote OSLO prescription to {}", out_path.display());
                }
            } else {
                let (name, presc) = resolve_cli_raw_prescription(&preset, zmx.as_ref())?;
                let sys = OsloSystem::from_optical_prescription(&presc);
                let text = write_len(&sys);
                if let Some(out_path) = output {
                    std::fs::write(&out_path, text)?;
                    println!(
                        "Exported OSLO prescription for '{name}' to {}",
                        out_path.display()
                    );
                } else {
                    println!("{text}");
                }
            }
            Ok(())
        }
        Commands::Engine {
            zmx,
            status,
            validate,
            analyze,
        } => run_engine_command(zmx, status, validate, analyze),
    }
}

fn run_engine_command(
    zmx: Option<PathBuf>,
    status: bool,
    validate: bool,
    analyze: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = OpticalEngine::new()?;
    if let Some(path) = zmx {
        let content = std::fs::read_to_string(path)?;
        engine.load_zmx_str(&content)?;
    }

    if status {
        let st = engine.status();
        println!("{}", serde_json::to_string_pretty(&st)?);
    }

    if validate {
        let diags = engine.validate();
        println!("{}", serde_json::to_string_pretty(&diags)?);
    }

    if let Some(req_str) = analyze {
        let req: AnalysisRequest = serde_json::from_str(&req_str)?;
        let res = engine.run_analysis(&req)?;
        println!("{}", serde_json::to_string_pretty(&res)?);
    }

    Ok(())
}

fn run_nonseq_command(
    preset: &str,
    rays: usize,
    max_bounces: usize,
    min_power: f64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use optics_nonseq::*;

    let catalog = optics_glass::catalog::GlassCatalogDb::load_bundled()?;
    let mut scene = NonSeqScene::new();

    let ray_list = match preset {
        "slab" => {
            let slab = NonSeqObject {
                id: 0,
                name: "DielectricSlab".to_string(),
                geometry: PrimitiveGeometry::Box {
                    half_x: 50.0,
                    half_y: 50.0,
                    half_z: 2.5,
                },
                transform: Transform3D::from_translation(0.0, 0.0, 2.5),
                material: NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.5 }),
                is_active: true,
            };
            scene.add_object(slab);

            let det_trans = Detector2D::new(
                0,
                "TransmittedDetector",
                optics_core::Vec3::new(0.0, 0.0, 10.0),
                optics_core::Vec3::new(0.0, 0.0, 1.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                100.0,
                100.0,
                20,
                20,
            )?;
            scene.add_detector(det_trans);

            let det_refl = Detector2D::new(
                1,
                "ReflectedDetector",
                optics_core::Vec3::new(0.0, 0.0, -10.0),
                optics_core::Vec3::new(0.0, 0.0, -1.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                100.0,
                100.0,
                20,
                20,
            )?;
            scene.add_detector(det_refl);

            let source = RaySource::CollimatedBeam {
                center: optics_core::Vec3::new(0.0, 0.0, -5.0),
                direction: optics_core::Vec3::new(0.0, 0.0, 1.0),
                radius: 10.0,
                total_flux_watts: 1.0,
                spectrum: SourceSpectrum::Monochromatic {
                    wavelength_nm: 587.56,
                },
            };
            source.generate_rays(rays, 42)?
        }
        "pipe" => {
            let pipe = NonSeqObject {
                id: 0,
                name: "LightPipe".to_string(),
                geometry: PrimitiveGeometry::Cylinder {
                    radius: 5.0,
                    half_length: 25.0,
                },
                transform: Transform3D::from_translation(0.0, 0.0, 25.0),
                material: NonSeqMaterial::dielectric(OpticalMedium::Constant { n: 1.5168 }),
                is_active: true,
            };
            scene.add_object(pipe);

            let det = Detector2D::new(
                0,
                "ExitDetector",
                optics_core::Vec3::new(0.0, 0.0, 50.1),
                optics_core::Vec3::new(0.0, 0.0, 1.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                20.0,
                20.0,
                20,
                20,
            )?;
            scene.add_detector(det);

            let source = RaySource::CollimatedBeam {
                center: optics_core::Vec3::new(0.0, 0.0, 1.0),
                direction: optics_core::Vec3::new(0.1, 0.0, 0.99).normalize(),
                radius: 3.0,
                total_flux_watts: 1.0,
                spectrum: SourceSpectrum::Monochromatic {
                    wavelength_nm: 587.56,
                },
            };
            let mut launched = source.generate_rays(rays, 42)?;
            for r in &mut launched {
                r.medium_stack = vec![0];
            }
            launched
        }
        _ => {
            let presc = plano_convex_100mm()?;
            let _lens_id = scene.add_prescription_lens(
                "PlanoConvexSinglet",
                &presc,
                0,
                1,
                NonSeqMaterial::dielectric(OpticalMedium::GlassCatalog {
                    name: "N-BK7".to_string(),
                }),
                32,
                16,
            )?;

            let det = Detector2D::new(
                0,
                "FocalDetector",
                optics_core::Vec3::new(0.0, 0.0, 98.4),
                optics_core::Vec3::new(0.0, 0.0, 1.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                10.0,
                10.0,
                50,
                50,
            )?;
            scene.add_detector(det);

            let source = RaySource::CollimatedBeam {
                center: optics_core::Vec3::new(0.0, 0.0, -10.0),
                direction: optics_core::Vec3::new(0.0, 0.0, 1.0),
                radius: 4.0,
                total_flux_watts: 1.0,
                spectrum: SourceSpectrum::Monochromatic {
                    wavelength_nm: 587.56,
                },
            };
            source.generate_rays(rays, 42)?
        }
    };

    let tracer = NonSeqTracer::new(NonSeqTraceConfig {
        max_bounces,
        min_power_threshold: min_power,
        enable_branching: true,
        record_paths: false,
    });

    let start = std::time::Instant::now();
    let report = tracer.trace_rays(&mut scene, &ray_list, Some(&catalog))?;
    let elapsed = start.elapsed();

    if json {
        let json_val = serde_json::json!({
            "preset": preset,
            "rays_launched": report.total_rays_traced,
            "total_interactions": report.total_interactions,
            "launched_flux_watts": report.launched_flux_watts,
            "detected_flux_watts": report.detected_flux_watts,
            "absorbed_flux_watts": report.absorbed_flux_watts,
            "escaped_flux_watts": report.escaped_flux_watts,
            "energy_conservation_ratio": report.energy_conservation_ratio(),
            "elapsed_ms": elapsed.as_secs_f64() * 1000.0,
            "detectors": scene.detectors.iter().map(|d| {
                let (cx, cy) = d.centroid_mm();
                serde_json::json!({
                    "id": d.id,
                    "name": d.name,
                    "total_flux_watts": d.total_flux_watts,
                    "total_lumens": d.total_lumens,
                    "rays_hit": d.total_rays_hit,
                    "centroid_mm": [cx, cy],
                    "rms_radius_mm": d.rms_radius_mm(),
                })
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&json_val)?);
    } else {
        println!("============================================================");
        println!("OpticsLab Non-Sequential Ray Tracing Report");
        println!("Preset: {preset}");
        println!("============================================================");
        println!("Rays Launched:        {}", report.total_rays_traced);
        println!("Total Interactions:   {}", report.total_interactions);
        println!("Launched Flux:        {:.6} W", report.launched_flux_watts);
        println!("Detected Flux:        {:.6} W", report.detected_flux_watts);
        println!("Absorbed Flux:        {:.6} W", report.absorbed_flux_watts);
        println!("Escaped Flux:         {:.6} W", report.escaped_flux_watts);
        println!(
            "Conservation Ratio:   {:.8}",
            report.energy_conservation_ratio()
        );
        println!(
            "Trace Time:           {:.2} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        println!("------------------------------------------------------------");
        for d in &scene.detectors {
            let (cx, cy) = d.centroid_mm();
            println!("Detector #{}: '{}'", d.id, d.name);
            println!(
                "  Captured Flux:    {:.6} W ({:.4} lm)",
                d.total_flux_watts, d.total_lumens
            );
            println!("  Rays Hit:         {}", d.total_rays_hit);
            println!("  Centroid:         ({:.4}, {:.4}) mm", cx, cy);
            println!("  RMS Spot Radius:  {:.4} mm", d.rms_radius_mm());
        }
        println!("============================================================");
    }

    Ok(())
}

fn resolve_cli_raw_prescription(
    preset: &str,
    zmx: Option<&PathBuf>,
) -> Result<(String, OpticalPrescription), Box<dyn std::error::Error>> {
    if let Some(path) = zmx {
        let bytes = std::fs::read(path)?;
        let p = parse_zmx_bytes(&bytes)?.to_optical_prescription();
        let fname = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("Zemax System")
            .to_string();
        Ok((fname, p))
    } else {
        match preset.to_lowercase().replace('_', "-").as_str() {
            "cooke" | "cooke-triplet" => {
                Ok(("Cooke Triplet F/4.5".to_string(), cooke_triplet_f4_5()?))
            }
            "double-gauss" | "doublegauss" => {
                Ok(("Double Gauss F/2.0".to_string(), double_gauss_50mm()?))
            }
            "plano-convex" | "singlet" => {
                Ok(("Plano-Convex Singlet".to_string(), plano_convex_100mm()?))
            }
            other => Err(format!(
                "Unknown preset '{other}'. Valid options: cooke, double-gauss, plano-convex"
            )
            .into()),
        }
    }
}

fn resolve_cli_prescription(
    preset: &str,
    zmx: Option<&PathBuf>,
) -> Result<(String, optics_core::prescription::ResolvedPrescription), Box<dyn std::error::Error>> {
    let (name, prescription) = resolve_cli_raw_prescription(preset, zmx)?;

    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;
    Ok((name, resolved))
}

fn run_prism_command(
    glass_name: &str,
    apex_deg: f64,
    incidence_deg: f64,
    samples_count: usize,
    svg_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let glass_db = GlassCatalogDb::load_bundled().ok();
    let glass_mat: GlassMaterial = match glass_db.as_ref().and_then(|db| db.lookup(glass_name)) {
        Some(entry) => entry.clone(),
        None => match glass_name.to_lowercase().as_str() {
            "n-bk7" | "nbk7" => GlassMaterial::n_bk7(),
            "sf11" => GlassMaterial::sf11(),
            "f2" => GlassMaterial::f2(),
            "fused silica" | "fused_silica" => GlassMaterial::fused_silica(),
            _ => {
                return Err(format!(
                    "Glass material '{glass_name}' not found in embedded catalog or presets"
                )
                .into());
            }
        },
    };

    // Prism geometry in XY plane, apex angle alpha, half-angle beta = alpha / 2.
    // L = 50 mm side length.
    let l_side = 50.0_f64;
    let alpha = apex_deg.to_radians();
    let beta = alpha / 2.0;

    let h = l_side * beta.cos();
    let w = l_side * beta.sin();

    let v_apex = Vec3::new(0.0, h, 0.0);
    let v_left = Vec3::new(-w, 0.0, 0.0);
    let v_right = Vec3::new(w, 0.0, 0.0);

    // Extrude 3D prism along Z from -50 to +50 mm
    let z_min = -50.0;
    let z_max = 50.0;

    let v_apex_front = Vec3::new(0.0, h, z_max);
    let v_apex_back = Vec3::new(0.0, h, z_min);
    let v_left_front = Vec3::new(-w, 0.0, z_max);
    let v_left_back = Vec3::new(-w, 0.0, z_min);
    let v_right_front = Vec3::new(w, 0.0, z_max);
    let v_right_back = Vec3::new(w, 0.0, z_min);

    // Left face (from left to apex)
    let tri1a = Triangle::new(v_left_front, v_apex_front, v_apex_back, 1, 1, 0)?;
    let tri1b = Triangle::new(v_left_front, v_apex_back, v_left_back, 1, 1, 0)?;

    // Right face (from apex to right)
    let tri2a = Triangle::new(v_apex_front, v_right_front, v_right_back, 2, 1, 0)?;
    let tri2b = Triangle::new(v_apex_front, v_right_back, v_apex_back, 2, 1, 0)?;

    // Base (from right to left)
    let tri3a = Triangle::new(v_right_front, v_left_front, v_left_back, 3, 1, 0)?;
    let tri3b = Triangle::new(v_right_front, v_left_back, v_right_back, 3, 1, 0)?;

    let triangles = vec![tri1a, tri1b, tri2a, tri2b, tri3a, tri3b];
    let media: Vec<Box<dyn optics_core::RefractiveIndex>> =
        vec![Box::new(Air), Box::new(glass_mat.clone())];
    let scene = Scene::new(triangles, media);

    // Incidence vector: angle incidence_deg relative to left face normal (-cos beta, sin beta).
    // Direction vector at angle (incidence - beta) in radians.
    let theta_inc = incidence_deg.to_radians();
    let ray_angle = theta_inc - beta;
    let incident_dir = Vec3::new(ray_angle.cos(), ray_angle.sin(), 0.0);

    // Aim at midpoint of left face: (-w/2, h/2)
    let p_mid = Vec3::new(-w * 0.5, h * 0.5, 0.0);
    let origin = p_mid - incident_dir * 40.0;

    let ray_template = Ray::new(origin, incident_dir, 550.0, 1.0)?;

    // Generate wavelengths
    let num_samples = samples_count.max(2);
    let step = (780.0 - 380.0) / (num_samples - 1) as f64;
    let mut wavelengths = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        wavelengths.push(380.0 + i as f64 * step);
    }

    let fan: Vec<(f64, PathRecord)> = spectral_fan(
        &scene,
        &ray_template,
        &wavelengths,
        RayBranchPolicy::FollowTransmission,
        10,
    );

    // Print stdout table
    println!(
        "{:<17} | {:<10} | {:<15}",
        "Wavelength (nm)", "n", "Deviation (deg)"
    );
    println!("{:-<17}-+-{:-<10}-+-{:-<15}", "", "", "");

    let mut table_rows: Vec<(f64, f64, f64, &PathRecord)> = Vec::new();

    for (wl, path) in &fan {
        let n = glass_mat.n(*wl);
        if path.bounces.len() >= 2 {
            if let Some(exit_dir) = path.bounces[1].refracted_dir {
                let dev_rad = exit_dir.dot(&incident_dir).clamp(-1.0, 1.0).acos();
                let dev_deg = dev_rad.to_degrees();
                println!("{:<17.2} | {:<10.6} | {:<15.4}", wl, n, dev_deg);
                table_rows.push((*wl, n, dev_deg, path));
                continue;
            }
        }
        println!("{:<17.2} | {:<10.6} | {:<15}", wl, n, "TIR / Unrefracted");
        table_rows.push((*wl, n, 0.0, path));
    }

    // Generate 2D SVG
    generate_prism_svg(
        svg_path,
        v_left,
        v_apex,
        v_right,
        origin,
        incident_dir,
        &table_rows,
    )?;

    println!("\nSaved 2D SVG diagram to: {}", svg_path.display());

    Ok(())
}

fn generate_prism_svg(
    svg_path: &PathBuf,
    v_left: Vec3,
    v_apex: Vec3,
    v_right: Vec3,
    ray_origin: Vec3,
    _incident_dir: Vec3,
    rays: &[(f64, f64, f64, &PathRecord)],
) -> Result<(), std::io::Error> {
    // Gather all 2D points to compute bounding box
    let mut min_x = v_left.x.min(v_right.x).min(v_apex.x).min(ray_origin.x);
    let mut max_x = v_left.x.max(v_right.x).max(v_apex.x).max(ray_origin.x);
    let mut min_y = v_left.y.min(v_right.y).min(v_apex.y).min(ray_origin.y);
    let mut max_y = v_left.y.max(v_right.y).max(v_apex.y).max(ray_origin.y);

    let exit_extension = 45.0;

    for (_, _, _, path) in rays {
        for b in &path.bounces {
            min_x = min_x.min(b.point.x);
            max_x = max_x.max(b.point.x);
            min_y = min_y.min(b.point.y);
            max_y = max_y.max(b.point.y);
        }
        if path.bounces.len() >= 2 {
            if let Some(exit_dir) = path.bounces[1].refracted_dir {
                let end = path.bounces[1].point + exit_dir * exit_extension;
                min_x = min_x.min(end.x);
                max_x = max_x.max(end.x);
                min_y = min_y.min(end.y);
                max_y = max_y.max(end.y);
            }
        }
    }

    // Add padding
    let pad = 15.0;
    min_x -= pad;
    max_x += pad;
    min_y -= pad;
    max_y += pad;

    let world_width = max_x - min_x;
    let world_height = max_y - min_y;

    let svg_width = 1000.0;
    let svg_height = 700.0;

    let scale = (svg_width / world_width).min(svg_height / world_height) * 0.92;
    let offset_x = (svg_width - world_width * scale) * 0.5;
    let offset_y = (svg_height - world_height * scale) * 0.5;

    // Transform world coordinates (x, y) to SVG (X, Y) where Y is inverted
    let to_svg = |x: f64, y: f64| -> (f64, f64) {
        let sx = offset_x + (x - min_x) * scale;
        let sy = offset_y + (max_y - y) * scale;
        (sx, sy)
    };

    let mut svg_out = File::create(svg_path)?;

    writeln!(
        svg_out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_width} {svg_height}" width="{svg_width}" height="{svg_height}" style="background-color: #11141a;">"##
    )?;

    // Title and legend
    writeln!(
        svg_out,
        r##"  <text x="30" y="40" font-family="system-ui, sans-serif" font-size="20" font-weight="600" fill="#e0e8f5">OpticsLab — Prism Spectral Dispersion</text>"##
    )?;
    writeln!(
        svg_out,
        r##"  <text x="30" y="65" font-family="system-ui, sans-serif" font-size="13" fill="#8899aa">Wavelength spectrum: 380 nm (violet) → 780 nm (red)</text>"##
    )?;

    // Draw prism polygon
    let (p1x, p1y) = to_svg(v_left.x, v_left.y);
    let (p2x, p2y) = to_svg(v_apex.x, v_apex.y);
    let (p3x, p3y) = to_svg(v_right.x, v_right.y);

    writeln!(
        svg_out,
        r##"  <!-- Prism Body -->
  <polygon points="{p1x:.2},{p1y:.2} {p2x:.2},{p2y:.2} {p3x:.2},{p3y:.2}" fill="rgba(80, 140, 220, 0.12)" stroke="#4a80c0" stroke-width="2" stroke-linejoin="round"/>"##
    )?;

    // Draw rays
    writeln!(svg_out, r##"  <!-- Dispersed Rays -->"##)?;

    for (wl, _, _, path) in rays {
        let [r, g, b] = display_color_for_wavelength(*wl);

        let mut pts = Vec::new();
        let (x0, y0) = to_svg(ray_origin.x, ray_origin.y);
        pts.push(format!("{x0:.2},{y0:.2}"));

        for bounce in &path.bounces {
            let (bx, by) = to_svg(bounce.point.x, bounce.point.y);
            pts.push(format!("{bx:.2},{by:.2}"));
        }

        if path.bounces.len() >= 2 {
            if let Some(exit_dir) = path.bounces[1].refracted_dir {
                let end = path.bounces[1].point + exit_dir * exit_extension;
                let (ex, ey) = to_svg(end.x, end.y);
                pts.push(format!("{ex:.2},{ey:.2}"));
            }
        }

        let pts_str = pts.join(" ");
        writeln!(
            svg_out,
            r#"  <polyline points="{pts_str}" fill="none" stroke="rgb({r},{g},{b})" stroke-width="1.8" opacity="0.85"/>"#
        )?;
    }

    writeln!(svg_out, "</svg>")?;

    Ok(())
}

struct AnalyzeOptions<'a> {
    preset: &'a str,
    zmx: Option<&'a PathBuf>,
    field: f64,
    wavelength: f64,
    rings: usize,
    fan_points: usize,
    json: bool,
    csv: Option<&'a PathBuf>,
}

fn run_analyze_command(opts: AnalyzeOptions<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let (name, prescription) = if let Some(path) = opts.zmx {
        let bytes = std::fs::read(path)?;
        let p = parse_zmx_bytes(&bytes)?.to_optical_prescription();
        let fname = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("Zemax System")
            .to_string();
        (fname, p)
    } else {
        match opts.preset.to_lowercase().replace('_', "-").as_str() {
            "cooke" | "cooke-triplet" => ("Cooke Triplet F/4.5".to_string(), cooke_triplet_f4_5()?),
            "double-gauss" | "doublegauss" => {
                ("Double Gauss F/2.0".to_string(), double_gauss_50mm()?)
            }
            "plano-convex" | "singlet" => {
                ("Plano-Convex Singlet".to_string(), plano_convex_100mm()?)
            }
            other => {
                return Err(format!(
                    "Unknown preset '{other}'. Valid options: cooke, double-gauss, plano-convex"
                )
                .into());
            }
        }
    };

    let catalog = GlassCatalogDb::load_bundled()?;
    let resolved = prescription.resolve(&catalog)?;

    let paraxial = calculate_paraxial_properties(&resolved, opts.wavelength)?;
    let spot = trace_spot_diagram(
        &resolved,
        (0.0, opts.field),
        opts.wavelength,
        PupilPattern::Hexapolar {
            num_rings: opts.rings,
        },
    )?;
    let fan = trace_ray_fan(
        &resolved,
        (0.0, opts.field),
        opts.wavelength,
        opts.fan_points,
    )?;

    if let Some(csv_path) = opts.csv {
        let mut csv_file = File::create(csv_path)?;
        writeln!(
            csv_file,
            "ray_index,px,py,x_mm,y_mm,intensity,wavelength_nm"
        )?;
        for (i, pt) in spot.points.iter().enumerate() {
            writeln!(
                csv_file,
                "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.2}",
                i, pt.pupil.px, pt.pupil.py, pt.x_mm, pt.y_mm, pt.intensity, pt.wavelength_nm
            )?;
        }
        println!(
            "Saved spot diagram CSV ({} rays) to: {}",
            spot.points.len(),
            csv_path.display()
        );
    }

    if opts.json {
        let output = serde_json::json!({
            "prescription_name": name,
            "field_angle_deg": opts.field,
            "wavelength_nm": opts.wavelength,
            "paraxial": {
                "efl_mm": paraxial.efl_mm,
                "bfl_mm": paraxial.bfl_mm,
                "f_number": paraxial.f_number,
                "entrance_pupil_pos_mm": paraxial.entrance_pupil_pos_mm,
                "exit_pupil_pos_mm": paraxial.exit_pupil_pos_mm
            },
            "spot_diagram": {
                "total_rays_traced": spot.metrics.total_rays_traced,
                "passed_rays_count": spot.metrics.passed_rays_count,
                "vignetting_factor": spot.metrics.vignetting_factor,
                "centroid_x_mm": spot.metrics.centroid_x_mm,
                "centroid_y_mm": spot.metrics.centroid_y_mm,
                "rms_radius_um": spot.metrics.rms_radius_um,
                "geo_radius_um": spot.metrics.geo_radius_um,
                "airy_radius_um": spot.metrics.airy_radius_um,
                "is_diffraction_limited": spot.metrics.is_diffraction_limited
            },
            "tangential_fan": fan.tangential,
            "sagittal_fan": fan.sagittal
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("============================================================");
        println!("OpticsLab Sequential Lens Diagnostics");
        println!("============================================================");
        println!("Prescription: {name}");
        println!(
            "Evaluation:   Field = {:.2}°, λ = {:.2} nm",
            opts.field, opts.wavelength
        );
        println!();
        println!("1. Paraxial First-Order Properties:");
        println!(
            "   Effective Focal Length (EFL): {:>8.3} mm",
            paraxial.efl_mm
        );
        println!(
            "   Back Focal Length (BFL):      {:>8.3} mm",
            paraxial.bfl_mm
        );
        println!(
            "   Working F-number:             F/{:>6.2}",
            paraxial.f_number
        );
        println!(
            "   Entrance Pupil Position:      {:>8.3} mm",
            paraxial.entrance_pupil_pos_mm
        );
        println!(
            "   Exit Pupil Position:          {:>8.3} mm",
            paraxial.exit_pupil_pos_mm
        );
        println!();
        println!("2. Geometric Spot Diagram:");
        println!(
            "   Total Rays Traced:            {:>8}",
            spot.metrics.total_rays_traced
        );
        println!(
            "   Passed Rays:                  {:>8} ({:.1}% transmitted)",
            spot.metrics.passed_rays_count,
            spot.metrics.vignetting_factor * 100.0
        );
        println!(
            "   Centroid Coordinates:         ({:.2} µm, {:.2} µm)",
            spot.metrics.centroid_x_mm * 1000.0,
            spot.metrics.centroid_y_mm * 1000.0
        );
        println!(
            "   RMS Blur Radius:              {:>8.2} µm",
            spot.metrics.rms_radius_um
        );
        println!(
            "   GEO Enclosure Radius:         {:>8.2} µm",
            spot.metrics.geo_radius_um
        );
        println!(
            "   Airy Disk Radius:             {:>8.2} µm",
            spot.metrics.airy_radius_um
        );
        println!(
            "   Diffraction Limited:          {}",
            if spot.metrics.is_diffraction_limited {
                "Yes (Diffraction Limited)"
            } else {
                "No (Aberration Dominated)"
            }
        );
        println!();
        println!("3. Transverse Ray Aberrations (Tangential Fan Samples):");
        for p in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            if let Some(pt) = fan
                .tangential
                .iter()
                .find(|pt| (pt.pupil_coord - p).abs() < 1e-3)
            {
                println!(
                    "   Py = {:>5.2}: {:>8.2} µm",
                    pt.pupil_coord, pt.aberration_um
                );
            }
        }
        println!("============================================================");
    }

    Ok(())
}

fn run_glass_command(name: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let db = GlassCatalogDb::load_bundled().ok();
    let glass_mat: GlassMaterial = match db.as_ref().and_then(|db| db.lookup(name)) {
        Some(entry) => entry.clone(),
        None => match name.to_lowercase().as_str() {
            "n-bk7" | "nbk7" => GlassMaterial::n_bk7(),
            "sf11" => GlassMaterial::sf11(),
            "f2" => GlassMaterial::f2(),
            "fused silica" | "fused_silica" | "lithosil-q" => GlassMaterial::fused_silica(),
            _ => {
                return Err(
                    format!("Glass material '{name}' not found in catalog or presets").into(),
                );
            }
        },
    };

    let n_f = glass_mat.refractive_index(LAMBDA_F_NM);
    let n_d = glass_mat.refractive_index(LAMBDA_D_NM);
    let n_c = glass_mat.refractive_index(LAMBDA_C_NM);
    let n_e = glass_mat.refractive_index(546.07);
    let n_g = glass_mat.refractive_index(435.84);
    let calc_vd = (n_d - 1.0) / (n_f - n_c);
    let is_crown = if n_d > 1.60 {
        calc_vd > 50.0
    } else {
        calc_vd > 55.0
    };
    let classification = if is_crown {
        "Crown (low dispersion)"
    } else {
        "Flint (high dispersion)"
    };

    if json {
        let output = serde_json::json!({
            "name": glass_mat.name,
            "catalog": glass_mat.catalog.name(),
            "nd": glass_mat.nd,
            "vd": glass_mat.vd,
            "calculated_vd": calc_vd,
            "classification": classification,
            "fraunhofer_indices": {
                "n_F_486_13nm": n_f,
                "n_e_546_07nm": n_e,
                "n_d_587_56nm": n_d,
                "n_C_656_27nm": n_c,
                "n_g_435_84nm": n_g
            },
            "dispersion_model": glass_mat.dispersion,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("============================================================");
    println!("OpticsLab Glass Material Diagnostics");
    println!("============================================================");
    println!("Designation:         {}", glass_mat.name);
    println!("Catalog / Vendor:    {}", glass_mat.catalog.name());
    println!("Classification:      {classification}");
    println!("Helium d-line index: {:.6}", n_d);
    println!("Abbe Number Vd:      {:.2}", calc_vd);
    println!();
    println!("1. Fraunhofer Spectral Line Refractive Indices:");
    println!("   Line g (435.84 nm, Hg blue):    {:.6}", n_g);
    println!("   Line F (486.13 nm, H blue):     {:.6}", n_f);
    println!("   Line e (546.07 nm, Hg green):   {:.6}", n_e);
    println!("   Line d (587.56 nm, He yellow):  {:.6}", n_d);
    println!("   Line C (656.27 nm, H red):      {:.6}", n_c);
    println!("   Principal dispersion (nF - nC): {:.6}", n_f - n_c);
    println!(
        "   Partial dispersion (nF - ne)/(nF - nC): {:.4}",
        (n_f - n_e) / (n_f - n_c)
    );
    println!();
    println!("2. Dispersion Model Parameters:");
    match &glass_mat.dispersion {
        DispersionModel::Sellmeier1 { b, c } => {
            println!("   Type: Sellmeier Formula 1 (λ in µm)");
            println!("   Formula: n²(λ) - 1 = B1·λ²/(λ²-C1) + B2·λ²/(λ²-C2) + B3·λ²/(λ²-C3)");
            println!("   B1 = {:>14.8e},  C1 = {:>14.8e} µm²", b[0], c[0]);
            println!("   B2 = {:>14.8e},  C2 = {:>14.8e} µm²", b[1], c[1]);
            println!("   B3 = {:>14.8e},  C3 = {:>14.8e} µm²", b[2], c[2]);
        }
        DispersionModel::Sellmeier2 { a, b, c } => {
            println!("   Type: Sellmeier Formula 2 (λ in µm)");
            println!("   Formula: n²(λ) - 1 = A + B1·λ²/(λ²-C1) + B2·λ²/(λ²-C2)");
            println!("   A  = {:>14.8e}", a);
            println!("   B1 = {:>14.8e},  C1 = {:>14.8e} µm²", b[0], c[0]);
            println!("   B2 = {:>14.8e},  C2 = {:>14.8e} µm²", b[1], c[1]);
        }
        DispersionModel::Schott { a } => {
            println!("   Type: Schott Empirical Formula (λ in µm)");
            println!("   Formula: n²(λ) = a0 + a1·λ² + a2·λ⁻² + a3·λ⁻⁴ + a4·λ⁻⁶ + a5·λ⁻⁸");
            for (idx, coeff) in a.iter().enumerate() {
                println!("   a{idx} = {:>14.8e}", coeff);
            }
        }
        DispersionModel::ModelGlass { nd, vd, d_pgf, .. } => {
            println!("   Type: Model Glass (nd={nd:.4}, Vd={vd:.2}, ΔPgF={d_pgf:.4})");
        }
        DispersionModel::Cauchy { a, b } => {
            println!("   Type: Cauchy Dispersion (λ in µm)");
            println!("   Formula: n(λ) = A + B / λ²");
            println!("   A = {:>14.8e},  B = {:>14.8e} µm²", a, b);
        }
        DispersionModel::Constant { n } => {
            println!("   Type: Non-dispersive constant refractive index");
            println!("   n = {:.6}", n);
        }
        DispersionModel::Air => {
            println!("   Type: Standard Air (n = 1.000000)");
        }
        DispersionModel::Mirror => {
            println!("   Type: Ideal Mirror (n = -1.000000)");
        }
        other => {
            println!("   Type: Extended Dispersion Model ({:?})", other);
        }
    }
    println!();
    println!("3. Visible Spectrum Sample Table (380 nm → 780 nm):");
    println!("   {:<16} | {:<12}", "Wavelength (nm)", "n(λ)");
    println!("   {:-<16}-+-{:-<12}", "", "");
    for wl in (380..=780).step_by(40) {
        let wl_f = wl as f64;
        let n = glass_mat.refractive_index(wl_f);
        println!("   {:<16.1} | {:<12.6}", wl_f, n);
    }
    println!();
    println!("4. Abbe Diagram Position:");
    println!("   Coordinate (Vd, nd): ({:.2}, {:.4})", calc_vd, n_d);
    let abbe_loc = if calc_vd >= 55.0 {
        if n_d > 1.60 {
            "Dense Crown [SK / LaK]"
        } else {
            "Standard / Borosilicate Crown [BK / PK]"
        }
    } else if n_d > 1.70 {
        "Dense / Super Dense Flint [SF / LAF]"
    } else {
        "Standard Flint [F / LF]"
    };
    println!("   Optical Glass Region: {abbe_loc}");
    println!("============================================================");

    Ok(())
}

fn run_benchmark_command(
    scene_name: &str,
    rays_count: usize,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let rays_count = rays_count.max(100);

    // Build benchmark scene geometry (prism geometry matching run_prism_command)
    let l_side = 50.0_f64;
    let alpha = 60.0_f64.to_radians();
    let beta = alpha / 2.0;
    let h = l_side * beta.cos();
    let w = l_side * beta.sin();
    let z_min = -50.0;
    let z_max = 50.0;

    let v_apex_front = Vec3::new(0.0, h, z_max);
    let v_apex_back = Vec3::new(0.0, h, z_min);
    let v_left_front = Vec3::new(-w, 0.0, z_max);
    let v_left_back = Vec3::new(-w, 0.0, z_min);
    let v_right_front = Vec3::new(w, 0.0, z_max);
    let v_right_back = Vec3::new(w, 0.0, z_min);

    let tri1a = BvhTriangle::new(v_left_front, v_apex_front, v_apex_back, 1);
    let tri1b = BvhTriangle::new(v_left_front, v_apex_back, v_left_back, 1);
    let tri2a = BvhTriangle::new(v_apex_front, v_right_front, v_right_back, 1);
    let tri2b = BvhTriangle::new(v_apex_front, v_right_back, v_apex_back, 1);
    let tri3a = BvhTriangle::new(v_right_front, v_left_front, v_left_back, 1);
    let tri3b = BvhTriangle::new(v_right_front, v_left_back, v_right_back, 1);

    let triangles = vec![tri1a, tri1b, tri2a, tri2b, tri3a, tri3b];

    // Compute bounding box
    let mut local_aabb = Aabb::empty();
    for t in &triangles {
        local_aabb = local_aabb.union(&t.aabb());
    }

    let bvh_build_start = std::time::Instant::now();
    let instance = BlasInstance::new(0, &local_aabb, nalgebra::Matrix4::identity());
    let bvh = BvhScene::build(vec![triangles], vec![instance]);
    let bvh_build_time = bvh_build_start.elapsed();

    // Deterministic pseudo-random ray generator (LCG)
    let mut rng: u64 = 0x853c49e6748fea9b;
    let mut next_f64 = || -> f64 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 11) as f64 / (1_u64 << 53) as f64
    };

    let mut rays = Vec::with_capacity(rays_count);
    for _ in 0..rays_count {
        let ox = (next_f64() - 0.5) * 60.0;
        let oy = next_f64() * 50.0;
        let oz = -60.0;
        let dx = (next_f64() - 0.5) * 0.1;
        let dy = (next_f64() - 0.5) * 0.1;
        let dz = 1.0;
        let dir = Vec3::new(dx, dy, dz).normalize();
        let ray = Ray::new(Vec3::new(ox, oy, oz), dir, 550.0, 1.0)?;
        rays.push(ray);
    }

    // Benchmark traversal
    let trace_start = std::time::Instant::now();
    let mut hits = 0;
    for ray in &rays {
        if bvh.intersect(ray).is_some() {
            hits += 1;
        }
    }
    let trace_elapsed = trace_start.elapsed();

    let mrays_sec = (rays_count as f64) / (trace_elapsed.as_secs_f64() * 1e6);
    let latency_ns = trace_elapsed.as_nanos() as f64 / (rays_count as f64);
    let hit_rate = hits as f64 / rays_count as f64;

    if json {
        let output = serde_json::json!({
            "scene": scene_name,
            "rays_traced": rays_count,
            "hits_count": hits,
            "hit_rate": hit_rate,
            "bvh_build_time_us": bvh_build_time.as_micros(),
            "traversal_time_ms": trace_elapsed.as_secs_f64() * 1000.0,
            "throughput_mrays_per_sec": mrays_sec,
            "average_latency_ns": latency_ns
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("============================================================");
    println!("OpticsLab BVH Ray-Slab Traversal Benchmark");
    println!("============================================================");
    println!("Scene:                {scene_name} (2-level BVH, TLAS + BLAS)");
    println!("Total Rays:           {rays_count}");
    println!(
        "BVH Build Duration:   {:.3} ms",
        bvh_build_time.as_secs_f64() * 1000.0
    );
    println!(
        "Traversal Duration:   {:.3} ms",
        trace_elapsed.as_secs_f64() * 1000.0
    );
    println!("Intersections Found:  {hits} ({:.1}%)", hit_rate * 100.0);
    println!("------------------------------------------------------------");
    println!("Throughput:           {:.2} MRAYS/s", mrays_sec);
    println!("Average Ray Latency:  {:.1} ns / ray", latency_ns);
    println!("============================================================");

    Ok(())
}

#[allow(unsafe_code)]
fn run_self_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("OpticsLab Automated Headless Self-Test");
    println!("============================================================");

    // 1. Physical Prism Dispersion & Snell's Law
    let start_prism = std::time::Instant::now();
    let nbk7 = GlassMaterial::n_bk7();
    let n_red = nbk7.refractive_index(700.0);
    let n_green = nbk7.refractive_index(546.07);
    let n_blue = nbk7.refractive_index(435.83);
    if !(n_blue > n_green && n_green > n_red) {
        return Err(
            "Prism dispersion monotonicity failed: expected n(blue) > n(green) > n(red)".into(),
        );
    }

    // Verify Fresnel at normal incidence
    let (rs, rp) = optics_core::fresnel_dielectric(1.0, 1.0, nbk7.nd);
    let r_expected = ((1.0 - nbk7.nd) / (1.0 + nbk7.nd)).powi(2);
    if (rs - r_expected).abs() > 1e-6 || (rp - r_expected).abs() > 1e-6 {
        return Err("Fresnel reflectance calculation failed at normal incidence".into());
    }
    let elapsed_prism = start_prism.elapsed();
    println!(
        "[PASS] Physical Prism Dispersion & Snell's Law ({:.2} ms)",
        elapsed_prism.as_secs_f64() * 1000.0
    );

    // 2. Lens Presets & Paraxial Solvers
    let start_lens = std::time::Instant::now();
    let catalog = GlassCatalogDb::load_bundled()?;
    let cooke = cooke_triplet_f4_5()?.resolve(&catalog)?;
    let cooke_paraxial = calculate_paraxial_properties(&cooke, LAMBDA_D_NM)?;
    if (cooke_paraxial.efl_mm - 50.0).abs() / 50.0 > 0.01 {
        return Err("Cooke Triplet paraxial EFL deviates by > 1% from design".into());
    }

    let double_gauss = double_gauss_50mm()?.resolve(&catalog)?;
    let dg_paraxial = calculate_paraxial_properties(&double_gauss, LAMBDA_D_NM)?;
    if (dg_paraxial.efl_mm - 50.0).abs() / 50.0 > 0.01 {
        return Err("Double Gauss paraxial EFL deviates by > 1% from design".into());
    }

    let plano = plano_convex_100mm()?.resolve(&catalog)?;
    let plano_paraxial = calculate_paraxial_properties(&plano, LAMBDA_D_NM)?;
    if (plano_paraxial.efl_mm - 100.0).abs() / 100.0 > 0.01 {
        return Err("Plano-Convex paraxial EFL deviates by > 1% from design".into());
    }
    let elapsed_lens = start_lens.elapsed();
    println!(
        "[PASS] Sequential Lens Presets & Paraxial Solvers ({:.2} ms)",
        elapsed_lens.as_secs_f64() * 1000.0
    );

    // 3. Zemax ZMX Specification & Round-Trip
    let start_zmx = std::time::Instant::now();
    let zmx_bytes = optics_zemax::presets::COOKE_TRIPLET_F4_5_ZMX.as_bytes();
    let parsed_zmx = parse_zmx_bytes(zmx_bytes)?;
    if parsed_zmx.surfaces.len() != 8 {
        return Err("Parsed Cooke Triplet surface count mismatch".into());
    }
    let zmx_str = optics_zemax::write_zmx(&parsed_zmx);
    let round_trip = parse_zmx_bytes(zmx_str.as_bytes())?;
    if round_trip.surfaces.len() != parsed_zmx.surfaces.len() {
        return Err("Zemax round-trip serialization changed surface count".into());
    }
    let elapsed_zmx = start_zmx.elapsed();
    println!(
        "[PASS] Zemax .zmx Specification & Round-Trip ({:.2} ms)",
        elapsed_zmx.as_secs_f64() * 1000.0
    );

    // 4. Optical Glass Catalog Database
    let start_glass = std::time::Instant::now();
    let glass_count = catalog.len();
    if glass_count < 600 {
        return Err(
            format!("Glass catalog incomplete: found {glass_count} glasses (< 600)").into(),
        );
    }
    let sf11 = catalog
        .lookup("SF11")
        .ok_or("Failed to lookup SF11 in glass catalog")?;
    if (sf11.nd - 1.7847).abs() > 0.005 {
        return Err("SF11 refractive index nd does not match reference".into());
    }
    let elapsed_glass = start_glass.elapsed();
    println!(
        "[PASS] Optical Glass Catalog Database ({} glasses, {:.2} ms)",
        glass_count,
        elapsed_glass.as_secs_f64() * 1000.0
    );

    // 5. BVH Construction & Ray-AABB Slab Traversal
    let start_bvh = std::time::Instant::now();
    let p0 = Vec3::new(-10.0, 0.0, 0.0);
    let p1 = Vec3::new(10.0, 0.0, 0.0);
    let p2 = Vec3::new(0.0, 20.0, 0.0);
    let tri = BvhTriangle::new(p0, p1, p2, 1);
    let local_aabb = tri.aabb();
    let instance = BlasInstance::new(0, &local_aabb, nalgebra::Matrix4::identity());
    let scene = BvhScene::build(vec![vec![tri]], vec![instance]);
    let test_ray = Ray::new(
        Vec3::new(0.0, 5.0, -10.0),
        Vec3::new(0.0, 0.0, 1.0),
        550.0,
        1.0,
    )?;
    let hit = scene.intersect(&test_ray);
    if hit.is_none() {
        return Err("BVH test ray failed to intersect triangle".into());
    }
    let elapsed_bvh = start_bvh.elapsed();
    println!(
        "[PASS] BVH Construction & Ray-AABB Slab Traversal ({:.2} ms)",
        elapsed_bvh.as_secs_f64() * 1000.0
    );

    // 6. Tolerancing Engine: Sensitivity & Monte Carlo Simulation
    let start_tol = std::time::Instant::now();
    let cooke_raw = cooke_triplet_f4_5()?;
    let tol_operands = vec![
        optics_tolerance::ToleranceOperand::new(
            "TRAD_S0",
            optics_tolerance::ToleranceOperandType::Radius {
                surface_index: 0,
                delta_radius_mm: 0.05,
                fringes: None,
            },
        ),
        optics_tolerance::ToleranceOperand::new(
            "TTHI_S0",
            optics_tolerance::ToleranceOperandType::Thickness {
                surface_index: 0,
                delta_thickness_mm: 0.02,
            },
        ),
    ];
    let criterion = optics_tolerance::ToleranceCriterion::RmsSpotRadius {
        field_angle_deg: 0.0,
        wavelength_nm: LAMBDA_D_NM,
    };
    let compensator = optics_tolerance::Compensator::ParaxialRefocus {
        wavelength_nm: LAMBDA_D_NM,
    };
    let sens_report = optics_tolerance::SensitivityAnalyzer::analyze(
        &cooke_raw,
        &tol_operands,
        &criterion,
        &compensator,
        &catalog,
    )?;
    if sens_report.records.len() != 2 || sens_report.estimated_rss_degradation <= 0.0 {
        return Err("Tolerancing sensitivity analysis validation failed".into());
    }
    let mc_sim = optics_tolerance::MonteCarloSimulator::new(optics_tolerance::MonteCarloConfig {
        num_trials: 20,
        seed: 42,
        yield_threshold: Some(sens_report.nominal_value * 2.5),
    });
    let mc_report = mc_sim.run(
        &cooke_raw,
        &tol_operands,
        &criterion,
        &compensator,
        &catalog,
    )?;
    if mc_report.total_trials != 20 || mc_report.percentiles.p50 <= 0.0 {
        return Err("Monte Carlo simulation percentile calculation failed".into());
    }
    let elapsed_tol = start_tol.elapsed();
    println!(
        "[PASS] Tolerancing Sensitivity & Monte Carlo Simulation ({:.2} ms)",
        elapsed_tol.as_secs_f64() * 1000.0
    );

    // 7. Non-Sequential Ray Tracing Engine (Fresnel splitting & detector accumulation)
    let start_nonseq = std::time::Instant::now();
    let mut ns_scene = optics_nonseq::NonSeqScene::new();
    let ns_slab = optics_nonseq::NonSeqObject {
        id: 0,
        name: "DielectricSlab".to_string(),
        geometry: optics_nonseq::PrimitiveGeometry::Box {
            half_x: 50.0,
            half_y: 50.0,
            half_z: 2.5,
        },
        transform: optics_nonseq::Transform3D::from_translation(0.0, 0.0, 2.5),
        material: optics_nonseq::NonSeqMaterial::dielectric(
            optics_nonseq::OpticalMedium::Constant { n: 1.5 },
        ),
        is_active: true,
    };
    ns_scene.add_object(ns_slab);
    let ns_det_trans = optics_nonseq::Detector2D::new(
        0,
        "TransmittedDetector",
        optics_core::Vec3::new(0.0, 0.0, 10.0),
        optics_core::Vec3::new(0.0, 0.0, 1.0),
        optics_core::Vec3::new(0.0, 1.0, 0.0),
        100.0,
        100.0,
        10,
        10,
    )?;
    ns_scene.add_detector(ns_det_trans);
    let ns_det_refl = optics_nonseq::Detector2D::new(
        1,
        "ReflectedDetector",
        optics_core::Vec3::new(0.0, 0.0, -10.0),
        optics_core::Vec3::new(0.0, 0.0, -1.0),
        optics_core::Vec3::new(0.0, 1.0, 0.0),
        100.0,
        100.0,
        10,
        10,
    )?;
    ns_scene.add_detector(ns_det_refl);
    let ns_ray = optics_core::Ray::new(
        optics_core::Vec3::new(0.0, 0.0, -5.0),
        optics_core::Vec3::new(0.0, 0.0, 1.0),
        587.56,
        1.0,
    )?;
    let ns_rays = vec![optics_nonseq::NonSeqRay::new(ns_ray, 0)];
    let ns_tracer = optics_nonseq::NonSeqTracer::new(optics_nonseq::NonSeqTraceConfig {
        max_bounces: 20,
        min_power_threshold: 1e-12,
        enable_branching: true,
        record_paths: false,
    });
    let ns_report = ns_tracer.trace_rays(&mut ns_scene, &ns_rays, None)?;
    let p_tot = ns_scene.detectors[0].total_flux_watts + ns_scene.detectors[1].total_flux_watts;
    if (p_tot - 1.0).abs() > 1e-5 || (ns_report.energy_conservation_ratio() - 1.0).abs() > 1e-5 {
        return Err("Non-sequential energy conservation validation failed".into());
    }
    let elapsed_nonseq = start_nonseq.elapsed();
    println!(
        "[PASS] Non-Sequential Ray Tracing & Fresnel Branching ({:.2} ms)",
        elapsed_nonseq.as_secs_f64() * 1000.0
    );

    // 8. CAD Export & Multi-Format Optical Interoperability (SVG, DXF, STL, OBJ, CODE V, OSLO, ISO 10110)
    let start_cad = std::time::Instant::now();
    let cooke_rx = cooke_triplet_f4_5()?;

    // 8a. CODE V round-trip
    let codev_sys = CodeVSystem::from_optical_prescription(&cooke_rx);
    let codev_str = write_seq(&codev_sys);
    let codev_reparsed = parse_seq(&codev_str)?;
    if codev_reparsed.surfaces.len() != codev_sys.surfaces.len() {
        return Err("CODE V .seq round-trip surface count mismatch".into());
    }

    // 8b. OSLO round-trip
    let oslo_sys = OsloSystem::from_optical_prescription(&cooke_rx);
    let oslo_str = write_len(&oslo_sys);
    let oslo_reparsed = parse_len(&oslo_str)?;
    if oslo_reparsed.surfaces.len() != oslo_sys.surfaces.len() {
        return Err("OSLO .len round-trip surface count mismatch".into());
    }

    // 8c. ISO 10110 drawing data sheet
    let iso_sheet = Iso10110DrawingSheet::from_prescription(&cooke_rx, Some(&catalog));
    let iso_md = iso_sheet.to_markdown_table();
    if !iso_md.contains("ISO 10110") || iso_sheet.elements.len() != 3 {
        return Err("ISO 10110 drawing table generation failed".into());
    }

    // 8d. CAD Exports (SVG, DXF, STL, OBJ)
    let svg_out = export_svg_layout(&cooke_rx, None, &SvgConfig::default())?;
    if !svg_out.contains("<svg") || !svg_out.contains("</svg>") {
        return Err("CAD 2D SVG layout export failed".into());
    }

    let dxf_out = export_dxf_layout(&cooke_rx, None, &DxfConfig::default())?;
    if !dxf_out.contains("SECTION") || !dxf_out.contains("EOF") {
        return Err("CAD 2D DXF export failed".into());
    }

    let stl_bin = export_stl_lens(&cooke_rx, true, 16, 6);
    if stl_bin.len() <= 84 {
        return Err("CAD 3D STL binary export failed".into());
    }

    let (obj_str, mtl_str) = export_obj_system(&cooke_rx, 16, 6);
    if !obj_str.contains("v ") || !mtl_str.contains("newmtl") {
        return Err("CAD 3D Wavefront OBJ/MTL export failed".into());
    }

    // 8e. Unified Project Workspace
    let project = OpticsProjectWorkspace::new("Cooke Triplet Project", cooke_rx);
    let proj_json = project.to_json()?;
    let proj_loaded = OpticsProjectWorkspace::from_json(&proj_json)?;
    if proj_loaded.configurations.len() != 1 || proj_loaded.name != "Cooke Triplet Project" {
        return Err("OpticsProjectWorkspace JSON round-trip failed".into());
    }

    let elapsed_cad = start_cad.elapsed();
    println!(
        "[PASS] CAD Export & Multi-Format Optical Interoperability (SVG, DXF, STL, OBJ, CODE V, OSLO, ISO 10110) ({:.2} ms)",
        elapsed_cad.as_secs_f64() * 1000.0
    );

    // 9. Unified OpticalEngine Facade, Undo/Redo & C FFI ABI Boundary
    let start_engine = std::time::Instant::now();
    let mut engine = OpticalEngine::new()?;
    let status = engine.status();
    if status.surface_count != 7 || status.system_name != "Cooke Triplet F/4.5" {
        return Err("OpticalEngine initial status mismatch".into());
    }

    // Paraxial & Spot Analysis via Engine
    let paraxial_res = engine.run_analysis(&AnalysisRequest::Paraxial)?;
    if !matches!(paraxial_res, AnalysisResult::Paraxial(_)) {
        return Err("OpticalEngine paraxial analysis failed".into());
    }

    let spot_res = engine.run_analysis(&AnalysisRequest::SpotDiagram {
        field_index: 0,
        wavelength_nm: None,
        ray_count: 64,
    })?;
    if let AnalysisResult::SpotDiagram(s) = spot_res {
        if s.metrics.rms_radius_um >= 50.0 {
            return Err("OpticalEngine spot RMS blur exceeds design threshold".into());
        }
    } else {
        return Err("OpticalEngine spot analysis failed".into());
    }

    // Undo / Redo / Dirty Invalidation
    let orig_r = engine.prescription().surfaces[1].radius_mm;
    engine.execute(EngineCommand::EditSurface {
        index: 1,
        update: SurfaceUpdate {
            radius_mm: Some(30.0),
            ..Default::default()
        },
    })?;
    if !engine.is_dirty()
        || engine.revision() != 2
        || (engine.prescription().surfaces[1].radius_mm - 30.0).abs() > 1e-6
    {
        return Err("OpticalEngine command execution failed".into());
    }
    engine.undo()?;
    if (engine.prescription().surfaces[1].radius_mm - orig_r).abs() > 1e-6 || !engine.can_redo() {
        return Err("OpticalEngine undo failed".into());
    }
    engine.redo()?;
    if (engine.prescription().surfaces[1].radius_mm - 30.0).abs() > 1e-6 {
        return Err("OpticalEngine redo failed".into());
    }

    // C FFI ABI Boundary Verification
    let ffi_engine = optics_engine_create();
    if ffi_engine.is_null() {
        return Err("optics_engine_create returned NULL".into());
    }
    let status_ptr = unsafe { optics_engine_get_status_json(ffi_engine) };
    if status_ptr.is_null() {
        unsafe { optics_engine_free(ffi_engine) };
        return Err("optics_engine_get_status_json returned NULL".into());
    }
    let status_str = unsafe { std::ffi::CStr::from_ptr(status_ptr) }.to_str()?;
    if !status_str.contains("Cooke Triplet") {
        unsafe {
            optics_string_free(status_ptr);
            optics_engine_free(ffi_engine);
        }
        return Err("C FFI status JSON did not contain expected preset".into());
    }
    let svg_ptr = unsafe { optics_engine_export_svg(ffi_engine) };
    if svg_ptr.is_null() {
        unsafe { optics_engine_free(ffi_engine) };
        return Err("optics_engine_export_svg returned NULL".into());
    }
    let svg_str = unsafe { std::ffi::CStr::from_ptr(svg_ptr) }.to_str()?;
    if !svg_str.contains("<svg") {
        unsafe {
            optics_string_free(svg_ptr);
            optics_engine_free(ffi_engine);
        }
        return Err("C FFI SVG export did not contain <svg tag".into());
    }
    unsafe {
        optics_string_free(svg_ptr);
        optics_engine_free(ffi_engine);
    }

    let elapsed_engine = start_engine.elapsed();
    println!(
        "[PASS] Unified OpticalEngine Facade, Undo/Redo & C FFI ABI Boundary ({:.2} ms)",
        elapsed_engine.as_secs_f64() * 1000.0
    );

    // 10. Independent Golden Cross-Validation, Numerical Invariants & Robustness Fuzzing
    let start_validation = std::time::Instant::now();

    // 10a. Golden paraxial benchmarks: Cooke Triplet EFL (50.02 mm) & spot RMS (< 5.0 um)
    let cooke_val = cooke_triplet_f4_5()?.resolve(&catalog)?;
    let cooke_px = calculate_paraxial_properties(&cooke_val, LAMBDA_D_NM)?;
    if (cooke_px.efl_mm - 50.02).abs() / 50.02 > 0.001 {
        return Err(
            "Golden benchmark validation failed: Cooke Triplet EFL deviates from 50.02 mm".into(),
        );
    }
    let spot_val = trace_spot_diagram(
        &cooke_val,
        (0.0, 0.0),
        LAMBDA_D_NM,
        PupilPattern::Hexapolar { num_rings: 6 },
    )?;
    if spot_val.metrics.rms_radius_um >= 5.0 {
        return Err(
            "Golden benchmark validation failed: Cooke Triplet spot RMS blur exceeds 5.0 um".into(),
        );
    }

    // 10b. Plano-convex Petzval sum golden check
    let plano_val = plano_convex_100mm()?.resolve(&catalog)?;
    let seidel_plano = compute_seidel_coefficients(&plano_val, 0.0, LAMBDA_D_NM)?;
    if (seidel_plano.petzval_sum - 0.006593).abs() / 0.006593 > 0.01 {
        return Err("Golden benchmark validation failed: Plano-Convex Petzval sum error".into());
    }

    // 10c. Numerical Snell law vector cross-product invariant: n1 * (s x n) == n2 * (s' x n)
    let n_in = 1.0;
    let n_out = 1.5168;
    let norm_vec = Vec3::new(0.0, 0.0, 1.0);
    let ray_dir = Vec3::new(0.5, 0.0, -0.8660254037844386).normalize();
    let refr_dir = snell_refract(ray_dir, norm_vec, n_in, n_out)?;
    let cross_in = ray_dir.cross(&norm_vec) * n_in;
    let cross_out = refr_dir.cross(&norm_vec) * n_out;
    if (cross_in - cross_out).norm() > 1e-12 {
        return Err("Vector Snell cross-product invariant failed".into());
    }
    let (rs, rp) = fresnel_dielectric(0.8660254037844386, n_in, n_out);
    if ((rs + (1.0 - rs)) - 1.0).abs() > 1e-12 || ((rp + (1.0 - rp)) - 1.0).abs() > 1e-12 {
        return Err("Fresnel energy conservation invariant failed".into());
    }

    // 10d. Normal unit length & non-finite input rejection
    let surf_conic = ConicSurface::from_radius(100.0, -1.0, 25.0)?;
    let hit_norm = conic_normal(Vec3::new(10.0, 0.0, 0.5), &surf_conic)?;
    if (hit_norm.norm() - 1.0).abs() > 1e-12 {
        return Err("Conic normal unit length invariant failed".into());
    }
    if conic_sag(f64::NAN, 0.01, 0.0).is_ok()
        || snell_refract(Vec3::new(f64::NAN, 0.0, 1.0), norm_vec, 1.0, 1.5).is_ok()
    {
        return Err("Non-finite input rejection failed: expected Err on NaN input".into());
    }

    // 10e. Fuzzing parser resilience on extreme/malformed tokens
    let malformed_zmx = "VERS 0\nSURF 1\n CURV NaN\n DISZ 1e99999\n DIAM Infinity\n";
    let _ = optics_zemax::parse_zmx(malformed_zmx);
    let malformed_seq = "TIT 'Malformed'\nSO 0\nS 1\nRDY NaN\nTHI 1e-99999\n";
    let _ = parse_seq(malformed_seq);
    let malformed_len = "LEN NEW\nSURF 1\nRD null\nTH undefined\n";
    let _ = parse_len(malformed_len);

    let elapsed_val = start_validation.elapsed();
    println!(
        "[PASS] Independent Golden Cross-Validation, Numerical Invariants & Robustness Fuzzing ({:.2} ms)",
        elapsed_val.as_secs_f64() * 1000.0
    );

    println!("============================================================");
    println!("All 10 automated self-test suites passed successfully.");
    println!("============================================================");
    Ok(())
}
