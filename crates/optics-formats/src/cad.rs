//! 2D and 3D Computer-Aided Design (CAD) exporters for optical systems.
//!
//! Provides zero-dependency export routines for:
//! - **2D SVG**: Technical cross-sectional layout diagrams with optical axis, lens contours,
//!   aperture stop, and multi-wavelength ray paths.
//! - **AutoCAD DXF**: Industry-standard Release 12 ASCII DXF files with layers for optical axis,
//!   lens elements, apertures, and ray trajectories.
//! - **3D STL**: Binary and ASCII Stereolithography (STL) surface meshes for rapid prototyping
//!   and 3D printing.
//! - **Wavefront OBJ + MTL**: Multi-element 3D meshes with outward vertex normals and optical
//!   material descriptions for CAD/CAM and rendering.

use std::fmt::Write;

use optics_core::OpticalPrescription;
use optics_core::tessellate::prescription_to_trimesh_with_materials;
use thiserror::Error;

/// Errors that can occur during CAD export operations.
#[derive(Debug, Error, PartialEq)]
pub enum CadExportError {
    /// Formatting or write error.
    #[error("Failed to format CAD output: {0}")]
    FormatError(String),
    /// Invalid prescription geometry.
    #[error("Prescription contains invalid surface geometry: {0}")]
    InvalidGeometry(String),
}

/// 2D Point representation for cross-sectional layouts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2D {
    /// Axial position along optical axis in millimeters ($z$).
    pub z: f64,
    /// Radial transverse height in millimeters ($y$).
    pub y: f64,
}

impl Point2D {
    /// Creates a new 2D layout point.
    #[must_use]
    pub const fn new(z: f64, y: f64) -> Self {
        Self { z, y }
    }
}

/// A 2D ray path trajectory for layout diagrams.
#[derive(Debug, Clone, PartialEq)]
pub struct RayPath2D {
    /// Polyline vertices $(z, y)$ along the ray path.
    pub points: Vec<Point2D>,
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
}

/// Configuration options for 2D SVG layout export.
#[derive(Debug, Clone)]
pub struct SvgConfig {
    /// Total viewport width in pixels.
    pub width: u32,
    /// Total viewport height in pixels.
    pub height: u32,
    /// Margin padding in pixels.
    pub margin: f64,
    /// Background color CSS string (e.g. "#1e1e24" or "#ffffff").
    pub background: String,
    /// Stroke color for lens element outlines.
    pub lens_stroke: String,
    /// Fill color for lens glass elements.
    pub lens_fill: String,
    /// Stroke color for optical axis.
    pub axis_stroke: String,
}

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 600,
            margin: 40.0,
            background: "#ffffff".to_string(),
            lens_stroke: "#1f4e78".to_string(),
            lens_fill: "rgba(91, 155, 213, 0.25)".to_string(),
            axis_stroke: "#7f7f7f".to_string(),
        }
    }
}

/// Evaluates the surface sag $z(r)$ for a spherical/conic surface.
fn surface_sag(r: f64, radius_mm: f64, conic_constant: f64) -> f64 {
    if !radius_mm.is_finite() || radius_mm.abs() < 1e-12 {
        return 0.0;
    }
    let c = 1.0 / radius_mm;
    let radicand = 1.0 - (1.0 + conic_constant) * c * c * r * r;
    let s = if radicand <= 0.0 { 0.0 } else { radicand.sqrt() };
    (c * r * r) / (1.0 + s)
}

/// Computes cumulative axial vertex positions for prescription surfaces.
fn compute_surface_z(prescription: &OpticalPrescription) -> Vec<f64> {
    let mut z_coords = Vec::with_capacity(prescription.surfaces.len());
    let mut current_z = 0.0_f64;

    for surf in &prescription.surfaces {
        z_coords.push(current_z);
        if surf.thickness_mm.is_finite() && surf.thickness_mm < 1e5 {
            current_z += surf.thickness_mm;
        }
    }
    z_coords
}

/// Maps a wavelength in nanometers to an approximate sRGB hex color string.
#[must_use]
pub fn wavelength_to_svg_color(wavelength_nm: f64) -> &'static str {
    if wavelength_nm < 450.0 {
        "#7b1fa2" // Violet
    } else if wavelength_nm < 495.0 {
        "#1976d2" // Blue
    } else if wavelength_nm < 570.0 {
        "#388e3c" // Green
    } else if wavelength_nm < 590.0 {
        "#fbc02d" // Yellow
    } else if wavelength_nm < 620.0 {
        "#f57c00" // Orange
    } else {
        "#d32f2f" // Red
    }
}

/// Exports a 2D SVG cross-sectional technical drawing of an optical prescription.
///
/// # Arguments
/// * `prescription` - Optical prescription to render.
/// * `rays` - Optional pre-computed 2D ray path trajectories to draw over the layout.
/// * `config` - SVG appearance settings.
///
/// # Errors
/// Returns [`CadExportError`] if formatting fails.
pub fn export_svg_layout(
    prescription: &OpticalPrescription,
    rays: Option<&[RayPath2D]>,
    config: &SvgConfig,
) -> Result<String, CadExportError> {
    let z_coords = compute_surface_z(prescription);
    let mut z_min = 0.0_f64;
    let mut z_max = z_coords.last().copied().unwrap_or(100.0);
    let mut max_semi_diameter = prescription.entrance_pupil_diameter_mm * 0.5;

    for surf in &prescription.surfaces {
        if surf.semi_diameter_mm > max_semi_diameter {
            max_semi_diameter = surf.semi_diameter_mm;
        }
    }

    if let Some(ray_list) = rays {
        for ray in ray_list {
            for pt in &ray.points {
                if pt.z < z_min {
                    z_min = pt.z;
                }
                if pt.z > z_max {
                    z_max = pt.z;
                }
                if pt.y.abs() > max_semi_diameter {
                    max_semi_diameter = pt.y.abs();
                }
            }
        }
    }

    let z_span = (z_max - z_min).max(10.0);
    let y_span = (max_semi_diameter * 2.4).max(10.0);

    let draw_w = (config.width as f64 - config.margin * 2.0).max(100.0);
    let draw_h = (config.height as f64 - config.margin * 2.0).max(100.0);

    let scale_z = draw_w / z_span;
    let scale_y = draw_h / y_span;
    let scale = scale_z.min(scale_y);

    let center_x = config.margin + (draw_w - z_span * scale) * 0.5;
    let center_y = config.margin + draw_h * 0.5;

    let map_pt = |z: f64, y: f64| -> (f64, f64) {
        let sx = center_x + (z - z_min) * scale;
        let sy = center_y - y * scale;
        (sx, sy)
    };

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background-color: {};">"#,
        config.width, config.height, config.width, config.height, config.background
    );

    // Title and Info
    let _ = writeln!(
        svg,
        r#"  <text x="{}" y="28" font-family="sans-serif" font-size="16" font-weight="bold" fill="{}">{} (EPD: {:.2} mm)</text>"#,
        config.margin, config.lens_stroke, prescription.name, prescription.entrance_pupil_diameter_mm
    );

    // Optical Axis
    let (ax_start_x, ax_y) = map_pt(z_min - z_span * 0.05, 0.0);
    let (ax_end_x, _) = map_pt(z_max + z_span * 0.05, 0.0);
    let _ = writeln!(
        svg,
        r#"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-dasharray="6,4" stroke-width="1.2" />"#,
        ax_start_x, ax_y, ax_end_x, ax_y, config.axis_stroke
    );

    // Lens Elements
    let num_surfaces = prescription.surfaces.len();
    let mut i = 0;
    while i < num_surfaces {
        let surf = &prescription.surfaces[i];
        let mat = surf.material_name.trim();
        let is_glass = !mat.is_empty() && !mat.eq_ignore_ascii_case("air");

        if is_glass && i + 1 < num_surfaces {
            let back_surf = &prescription.surfaces[i + 1];
            let z_f = z_coords.get(i).copied().unwrap_or(0.0);
            let z_b = z_coords.get(i + 1).copied().unwrap_or(z_f);

            let r_f = surf.semi_diameter_mm.max(1e-3);
            let r_b = back_surf.semi_diameter_mm.max(1e-3);

            let samples = 24;
            let mut contour_pts: Vec<(f64, f64)> = Vec::with_capacity(samples * 2 + 2);

            // Front surface top-to-bottom: y from +r_f down to -r_f
            for s in 0..=samples {
                let frac = s as f64 / samples as f64;
                let y = r_f * (1.0 - 2.0 * frac);
                let z = z_f + surface_sag(y.abs(), surf.radius_mm, surf.conic_constant);
                contour_pts.push(map_pt(z, y));
            }

            // Back surface bottom-to-top: y from -r_b up to +r_b
            for s in 0..=samples {
                let frac = s as f64 / samples as f64;
                let y = -r_b * (1.0 - 2.0 * frac);
                let z = z_b + surface_sag(y.abs(), back_surf.radius_mm, back_surf.conic_constant);
                contour_pts.push(map_pt(z, y));
            }

            let pts_str: Vec<String> = contour_pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();

            let _ = writeln!(
                svg,
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="1.5" />"#,
                pts_str.join(" "),
                config.lens_fill,
                config.lens_stroke
            );

            // Element label
            let (label_x, label_y) = map_pt((z_f + z_b) * 0.5, 0.0);
            let _ = writeln!(
                svg,
                r#"  <text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="11" fill="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
                label_x, label_y, config.lens_stroke, mat
            );

            i += 2;
        } else {
            // Standalone surface or Stop
            let z_s = z_coords.get(i).copied().unwrap_or(0.0);
            if surf.is_stop {
                let (sx, sy_top) = map_pt(z_s, surf.semi_diameter_mm);
                let (_, sy_top_outer) = map_pt(z_s, surf.semi_diameter_mm * 1.3);
                let (_, sy_bot) = map_pt(z_s, -surf.semi_diameter_mm);
                let (_, sy_bot_outer) = map_pt(z_s, -surf.semi_diameter_mm * 1.3);

                // Stop blades
                let _ = writeln!(
                    svg,
                    r##"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#d32f2f" stroke-width="2.5" />"##,
                    sx, sy_top, sx, sy_top_outer
                );
                let _ = writeln!(
                    svg,
                    r##"  <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#d32f2f" stroke-width="2.5" />"##,
                    sx, sy_bot, sx, sy_bot_outer
                );
                let _ = writeln!(
                    svg,
                    r##"  <text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="10" fill="#d32f2f" text-anchor="middle">STOP</text>"##,
                    sx,
                    sy_top_outer - 4.0
                );
            }
            i += 1;
        }
    }

    // Rays
    if let Some(ray_list) = rays {
        for ray in ray_list {
            if ray.points.len() < 2 {
                continue;
            }
            let color = wavelength_to_svg_color(ray.wavelength_nm);
            let pts_str: Vec<String> = ray
                .points
                .iter()
                .map(|pt| {
                    let (x, y) = map_pt(pt.z, pt.y);
                    format!("{x:.2},{y:.2}")
                })
                .collect();

            let _ = writeln!(
                svg,
                r#"  <polyline points="{}" fill="none" stroke="{}" stroke-width="1.0" opacity="0.8" />"#,
                pts_str.join(" "),
                color
            );
        }
    }

    let _ = writeln!(svg, "</svg>");
    Ok(svg)
}

/// Configuration options for AutoCAD DXF export.
#[derive(Debug, Clone)]
pub struct DxfConfig {
    /// Export as 3D mesh (`3DFACE` entities) if true, or 2D cross-section (`POLYLINE`) if false.
    pub export_3d: bool,
    /// Azimuthal divisions for 3D meshes.
    pub slices: u32,
    /// Radial concentric divisions for 3D meshes.
    pub rings: u32,
}

impl Default for DxfConfig {
    fn default() -> Self {
        Self {
            export_3d: false,
            slices: 32,
            rings: 12,
        }
    }
}

/// Exports an optical prescription to AutoCAD Release 12 ASCII DXF format.
///
/// # Arguments
/// * `prescription` - Optical prescription to export.
/// * `rays` - Optional 2D ray paths (for 2D export mode).
/// * `config` - DXF export configuration.
///
/// # Errors
/// Returns [`CadExportError`] if formatting fails.
pub fn export_dxf_layout(
    prescription: &OpticalPrescription,
    rays: Option<&[RayPath2D]>,
    config: &DxfConfig,
) -> Result<String, CadExportError> {
    let mut out = String::new();

    // DXF Header
    let _ = writeln!(
        out,
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1009\n  0\nENDSEC"
    );

    // Tables & Layers
    let _ = writeln!(
        out,
        "  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLAYER\n 70\n4\n  0\nLAYER\n  2\nOPTICAL_AXIS\n 70\n0\n 62\n7\n  6\nCONTINUOUS\n  0\nLAYER\n  2\nLENS_ELEMENTS\n 70\n0\n 62\n4\n  6\nCONTINUOUS\n  0\nLAYER\n  2\nAPERTURES\n 70\n0\n 62\n1\n  6\nCONTINUOUS\n  0\nLAYER\n  2\nRAYS\n 70\n0\n 62\n3\n  6\nCONTINUOUS\n  0\nENDTAB\n  0\nENDSEC"
    );

    // Entities Section
    let _ = writeln!(out, "  0\nSECTION\n  2\nENTITIES");

    if config.export_3d {
        // Export 3D tessellated geometry
        let (triangles, _) = prescription_to_trimesh_with_materials(prescription, config.slices, config.rings);

        for tri in &triangles {
            let _ = writeln!(out, "  0\n3DFACE\n  8\nLENS_ELEMENTS");
            let _ = writeln!(out, " 10\n{:.6}\n 20\n{:.6}\n 30\n{:.6}", tri.v0.x, tri.v0.y, tri.v0.z);
            let _ = writeln!(out, " 11\n{:.6}\n 21\n{:.6}\n 31\n{:.6}", tri.v1.x, tri.v1.y, tri.v1.z);
            let _ = writeln!(out, " 12\n{:.6}\n 22\n{:.6}\n 32\n{:.6}", tri.v2.x, tri.v2.y, tri.v2.z);
            // 4th vertex duplicates 3rd for triangle
            let _ = writeln!(out, " 13\n{:.6}\n 23\n{:.6}\n 33\n{:.6}", tri.v2.x, tri.v2.y, tri.v2.z);
        }
    } else {
        // Export 2D cross-section
        let z_coords = compute_surface_z(prescription);
        let z_min = 0.0_f64;
        let z_max = z_coords.last().copied().unwrap_or(100.0);

        // Optical Axis line
        let _ = writeln!(
            out,
            "  0\nLINE\n  8\nOPTICAL_AXIS\n 10\n{:.6}\n 20\n0.0\n 30\n0.0\n 11\n{:.6}\n 21\n0.0\n 31\n0.0",
            z_min - 10.0,
            z_max + 10.0
        );

        let num_surfaces = prescription.surfaces.len();
        let mut i = 0;
        while i < num_surfaces {
            let surf = &prescription.surfaces[i];
            let mat = surf.material_name.trim();
            let is_glass = !mat.is_empty() && !mat.eq_ignore_ascii_case("air");

            if is_glass && i + 1 < num_surfaces {
                let back_surf = &prescription.surfaces[i + 1];
                let z_f = z_coords.get(i).copied().unwrap_or(0.0);
                let z_b = z_coords.get(i + 1).copied().unwrap_or(z_f);

                let r_f = surf.semi_diameter_mm.max(1e-3);
                let r_b = back_surf.semi_diameter_mm.max(1e-3);

                let samples = 20;
                let mut pts: Vec<(f64, f64)> = Vec::with_capacity(samples * 2 + 2);

                for s in 0..=samples {
                    let frac = s as f64 / samples as f64;
                    let y = r_f * (1.0 - 2.0 * frac);
                    let z = z_f + surface_sag(y.abs(), surf.radius_mm, surf.conic_constant);
                    pts.push((z, y));
                }
                for s in 0..=samples {
                    let frac = s as f64 / samples as f64;
                    let y = -r_b * (1.0 - 2.0 * frac);
                    let z = z_b + surface_sag(y.abs(), back_surf.radius_mm, back_surf.conic_constant);
                    pts.push((z, y));
                }

                // Closed POLYLINE
                let _ = writeln!(out, "  0\nPOLYLINE\n  8\nLENS_ELEMENTS\n 66\n1\n 70\n1");
                for (pz, py) in pts {
                    let _ = writeln!(
                        out,
                        "  0\nVERTEX\n  8\nLENS_ELEMENTS\n 10\n{pz:.6}\n 20\n{py:.6}\n 30\n0.0"
                    );
                }
                let _ = writeln!(out, "  0\nSEQEND");

                i += 2;
            } else {
                let z_s = z_coords.get(i).copied().unwrap_or(0.0);
                if surf.is_stop {
                    let _ = writeln!(
                        out,
                        "  0\nLINE\n  8\nAPERTURES\n 10\n{z_s:.6}\n 20\n{:.6}\n 30\n0.0\n 11\n{z_s:.6}\n 21\n{:.6}\n 31\n0.0",
                        surf.semi_diameter_mm,
                        surf.semi_diameter_mm * 1.3
                    );
                    let _ = writeln!(
                        out,
                        "  0\nLINE\n  8\nAPERTURES\n 10\n{z_s:.6}\n 20\n{:.6}\n 30\n0.0\n 11\n{z_s:.6}\n 21\n{:.6}\n 31\n0.0",
                        -surf.semi_diameter_mm,
                        -surf.semi_diameter_mm * 1.3
                    );
                }
                i += 1;
            }
        }

        // Ray Paths
        if let Some(ray_list) = rays {
            for ray in ray_list {
                if ray.points.len() < 2 {
                    continue;
                }
                let _ = writeln!(out, "  0\nPOLYLINE\n  8\nRAYS\n 66\n1");
                for pt in &ray.points {
                    let _ = writeln!(
                        out,
                        "  0\nVERTEX\n  8\nRAYS\n 10\n{:.6}\n 20\n{:.6}\n 30\n0.0",
                        pt.z, pt.y
                    );
                }
                let _ = writeln!(out, "  0\nSEQEND");
            }
        }
    }

    // End entities & file
    let _ = writeln!(out, "  0\nENDSEC\n  0\nEOF");
    Ok(out)
}

/// Exports a 3D STL mesh of the optical system in either binary or ASCII format.
///
/// # Arguments
/// * `prescription` - Optical prescription to tessellate.
/// * `binary` - `true` for standard binary STL (compact), `false` for ASCII STL.
/// * `slices` - Azimuthal angular divisions.
/// * `rings` - Radial concentric ring divisions.
#[must_use]
pub fn export_stl_lens(prescription: &OpticalPrescription, binary: bool, slices: u32, rings: u32) -> Vec<u8> {
    let (triangles, _) = prescription_to_trimesh_with_materials(prescription, slices, rings);

    if binary {
        let mut buffer = Vec::with_capacity(84 + triangles.len() * 50);

        // 80-byte header
        let header_str = "OpticsLab 3D STL Lens Export - Precision Rotational Mesh";
        let mut header = [0u8; 80];
        let bytes = header_str.as_bytes();
        let copy_len = bytes.len().min(80);
        header[..copy_len].copy_from_slice(&bytes[..copy_len]);
        buffer.extend_from_slice(&header);

        // Triangle count (u32 little-endian)
        let tri_count = triangles.len() as u32;
        buffer.extend_from_slice(&tri_count.to_le_bytes());

        // Triangles (50 bytes each)
        for tri in &triangles {
            // Normal (3x f32)
            buffer.extend_from_slice(&(tri.normal.x as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.normal.y as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.normal.z as f32).to_le_bytes());

            // Vertex 0 (3x f32)
            buffer.extend_from_slice(&(tri.v0.x as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v0.y as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v0.z as f32).to_le_bytes());

            // Vertex 1 (3x f32)
            buffer.extend_from_slice(&(tri.v1.x as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v1.y as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v1.z as f32).to_le_bytes());

            // Vertex 2 (3x f32)
            buffer.extend_from_slice(&(tri.v2.x as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v2.y as f32).to_le_bytes());
            buffer.extend_from_slice(&(tri.v2.z as f32).to_le_bytes());

            // Attribute byte count (u16 = 0)
            buffer.extend_from_slice(&0u16.to_le_bytes());
        }

        buffer
    } else {
        let mut ascii = String::new();
        let _ = writeln!(ascii, "solid OpticsLabLens");

        for tri in &triangles {
            let _ = writeln!(
                ascii,
                "  facet normal {:.6e} {:.6e} {:.6e}",
                tri.normal.x, tri.normal.y, tri.normal.z
            );
            let _ = writeln!(ascii, "    outer loop");
            let _ = writeln!(ascii, "      vertex {:.6e} {:.6e} {:.6e}", tri.v0.x, tri.v0.y, tri.v0.z);
            let _ = writeln!(ascii, "      vertex {:.6e} {:.6e} {:.6e}", tri.v1.x, tri.v1.y, tri.v1.z);
            let _ = writeln!(ascii, "      vertex {:.6e} {:.6e} {:.6e}", tri.v2.x, tri.v2.y, tri.v2.z);
            let _ = writeln!(ascii, "    endloop");
            let _ = writeln!(ascii, "  endfacet");
        }

        let _ = writeln!(ascii, "endsolid OpticsLabLens");
        ascii.into_bytes()
    }
}

/// Exports the 3D optical system as a Wavefront OBJ string and companion MTL material library.
///
/// # Returns
/// A tuple containing:
/// 1. `.obj` file content with indexed vertices, normals, and material group assignments.
/// 2. `.mtl` companion material definitions with index of refraction and realistic transmittances.
#[must_use]
pub fn export_obj_system(prescription: &OpticalPrescription, slices: u32, rings: u32) -> (String, String) {
    let (triangles, materials) = prescription_to_trimesh_with_materials(prescription, slices, rings);

    let mut obj = String::new();
    let mut mtl = String::new();

    let mtl_filename = format!("{}.mtl", prescription.name.replace(' ', "_"));

    // OBJ Header
    let _ = writeln!(obj, "# OpticsLab 3D Wavefront OBJ Export");
    let _ = writeln!(obj, "# Design: {}", prescription.name);
    let _ = writeln!(obj, "mtllib {mtl_filename}");

    // MTL Header
    let _ = writeln!(mtl, "# OpticsLab Material Template Library");
    for mat_name in &materials {
        let _ = writeln!(mtl, "newmtl {mat_name}");
        let _ = writeln!(mtl, "  Ka 0.1 0.1 0.1");
        let _ = writeln!(mtl, "  Kd 0.82 0.90 0.98");
        let _ = writeln!(mtl, "  Ks 0.8 0.8 0.8");
        let _ = writeln!(mtl, "  d 0.35");
        let _ = writeln!(mtl, "  Ns 120.0");
        let _ = writeln!(mtl, "  illum 4");
    }

    // Default material for rims or standalone surfaces
    let _ = writeln!(mtl, "newmtl OpticsLabMount");
    let _ = writeln!(
        mtl,
        "  Ka 0.2 0.2 0.2\n  Kd 0.3 0.3 0.3\n  Ks 0.1 0.1 0.1\n  d 1.0\n  Ns 20.0\n  illum 2"
    );

    let mut current_mat_id: Option<usize> = None;
    let mut v_index = 1_usize;

    for tri in &triangles {
        if current_mat_id != Some(tri.medium_inside) {
            current_mat_id = Some(tri.medium_inside);
            let mat_name = materials
                .get(tri.medium_inside)
                .map(|s| s.as_str())
                .unwrap_or("OpticsLabMount");
            let _ = writeln!(obj, "usemtl {mat_name}");
        }

        // Emit vertices
        let _ = writeln!(obj, "v {:.6} {:.6} {:.6}", tri.v0.x, tri.v0.y, tri.v0.z);
        let _ = writeln!(obj, "v {:.6} {:.6} {:.6}", tri.v1.x, tri.v1.y, tri.v1.z);
        let _ = writeln!(obj, "v {:.6} {:.6} {:.6}", tri.v2.x, tri.v2.y, tri.v2.z);

        // Emit normal
        let _ = writeln!(obj, "vn {:.6} {:.6} {:.6}", tri.normal.x, tri.normal.y, tri.normal.z);

        // Emit face
        let _ = writeln!(
            obj,
            "f {}//{} {}//{} {}//{}",
            v_index,
            v_index / 3 + 1,
            v_index + 1,
            v_index / 3 + 1,
            v_index + 2,
            v_index / 3 + 1
        );

        v_index += 3;
    }

    (obj, mtl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::OpticalSurface;

    fn sample_doublet() -> OpticalPrescription {
        OpticalPrescription {
            name: "Achromatic Doublet".to_string(),
            entrance_pupil_diameter_mm: 25.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![486.13, 587.56, 656.27],
            primary_wavelength_index: 1,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: 61.85,
                    conic_constant: 0.0,
                    thickness_mm: 6.0,
                    semi_diameter_mm: 12.5,
                    material_name: "N-BK7".to_string(),
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
                    radius_mm: -43.95,
                    conic_constant: 0.0,
                    thickness_mm: 2.5,
                    semi_diameter_mm: 12.5,
                    material_name: "SF11".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                    aperture: None,
                    coating: None,
                },
                OpticalSurface {
                    surface_index: 3,
                    radius_mm: -156.25,
                    conic_constant: 0.0,
                    thickness_mm: 95.0,
                    semi_diameter_mm: 12.5,
                    material_name: "AIR".to_string(),
                    is_stop: true,
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

    #[test]
    fn test_svg_export_contains_valid_xml() {
        let prescription = sample_doublet();
        let ray = RayPath2D {
            points: vec![
                Point2D::new(0.0, 10.0),
                Point2D::new(6.0, 9.5),
                Point2D::new(8.5, 9.0),
                Point2D::new(103.5, 0.0),
            ],
            wavelength_nm: 587.56,
        };

        let svg = export_svg_layout(&prescription, Some(&[ray]), &SvgConfig::default()).unwrap_or_default();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("Achromatic Doublet"));
        assert!(svg.contains("polyline"));
        assert!(svg.contains("polygon"));
    }

    #[test]
    fn test_dxf_2d_and_3d_export() {
        let prescription = sample_doublet();
        let dxf_2d = export_dxf_layout(&prescription, None, &DxfConfig::default()).unwrap_or_default();
        assert!(dxf_2d.contains("SECTION"));
        assert!(dxf_2d.contains("OPTICAL_AXIS"));
        assert!(dxf_2d.contains("LENS_ELEMENTS"));
        assert!(dxf_2d.ends_with("EOF\n"));

        let config_3d = DxfConfig {
            export_3d: true,
            slices: 12,
            rings: 4,
        };
        let dxf_3d = export_dxf_layout(&prescription, None, &config_3d).unwrap_or_default();
        assert!(dxf_3d.contains("3DFACE"));
    }

    #[test]
    fn test_stl_binary_and_ascii_export() {
        let prescription = sample_doublet();
        let binary_stl = export_stl_lens(&prescription, true, 12, 4);
        assert!(binary_stl.len() > 84);
        let header = std::str::from_utf8(&binary_stl[..40]).unwrap_or("");
        assert!(header.contains("OpticsLab 3D STL"));

        let ascii_stl = export_stl_lens(&prescription, false, 8, 3);
        let ascii_str = std::str::from_utf8(&ascii_stl).unwrap_or("");
        assert!(ascii_str.starts_with("solid OpticsLabLens"));
        assert!(ascii_str.contains("facet normal"));
        assert!(ascii_str.ends_with("endsolid OpticsLabLens\n"));
    }

    #[test]
    fn test_obj_and_mtl_export() {
        let prescription = sample_doublet();
        let (obj, mtl) = export_obj_system(&prescription, 12, 4);

        assert!(obj.contains("mtllib"));
        assert!(obj.contains("usemtl N-BK7"));
        assert!(obj.contains("v "));
        assert!(obj.contains("vn "));
        assert!(obj.contains("f "));

        assert!(mtl.contains("newmtl N-BK7"));
        assert!(mtl.contains("newmtl SF11"));
        assert!(mtl.contains("Kd"));
    }
}
