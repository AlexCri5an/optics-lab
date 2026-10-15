//! `optics-cli` — headless diagnostic runner, batch renderer, and optical CLI tools.

use clap::{Parser, Subcommand};
use optics_core::{
    Air, PathRecord, Ray, RayBranchPolicy, RefractiveIndex, Scene, Triangle, Vec3, spectral_fan,
};
use optics_glass::{GlassCatalogDb, GlassMaterial};
use optics_spectral::display_color_for_wavelength;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "optics-cli", author, version, about = "OpticsLab CLI tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Prism {
            glass,
            apex,
            incidence,
            samples,
            svg,
        } => run_prism_command(&glass, apex, incidence, samples, &svg),
    }
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
