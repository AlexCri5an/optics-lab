//! Example 06: CAD & Multi-Format Optical Interoperability Export
//!
//! Demonstrates exporting optical prescriptions across CAD, CAM, and simulation file formats:
//! - 2D Vector SVG technical cross-section drawing
//! - AutoCAD Release 12 ASCII DXF (2D layout and 3D surface mesh)
//! - 3D Stereolithography (STL) binary mesh for 3D printing
//! - 3D Wavefront OBJ + MTL material library for CAD/CAM visualization
//! - Synopsys CODE V `.seq` sequential prescription format
//! - Lambda Research OSLO `.len` optical format
//! - ISO 10110 optical element drawing specification (Markdown)
//! - OpticsProjectWorkspace versioned JSON serialization

use optics_zemax::cad::{
    DxfConfig, SvgConfig, export_dxf_layout, export_obj_system, export_stl_lens, export_svg_layout,
};
use optics_zemax::codev::{CodeVSystem, write_seq};
use optics_zemax::iso10110::Iso10110DrawingSheet;
use optics_zemax::oslo::{OsloSystem, write_len};
use optics_zemax::presets::cooke_triplet_f4_5;
use optics_zemax::project::OpticsProjectWorkspace;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 06: CAD & Interoperability Format Export ===\n");

    let prescription = cooke_triplet_f4_5()?;
    println!("Loaded Optical Prescription: {}", prescription.name);
    println!("  Surfaces: {}", prescription.surfaces.len());
    println!(
        "  EPD:      {:.2} mm\n",
        prescription.entrance_pupil_diameter_mm
    );

    // 1. Export 2D SVG Technical Diagram
    let svg_config = SvgConfig::default();
    let svg_content = export_svg_layout(&prescription, None, &svg_config)?;
    println!("1. SVG 2D Layout Export:");
    println!("  Generated SVG size: {} bytes", svg_content.len());
    println!(
        "  Valid SVG Root:     {}\n",
        svg_content.contains("<svg") && svg_content.contains("</svg>")
    );

    // 2. Export AutoCAD DXF
    let dxf_2d_config = DxfConfig {
        export_3d: false,
        slices: 32,
        rings: 12,
    };
    let dxf_2d = export_dxf_layout(&prescription, None, &dxf_2d_config)?;
    let dxf_3d_config = DxfConfig {
        export_3d: true,
        slices: 32,
        rings: 12,
    };
    let dxf_3d = export_dxf_layout(&prescription, None, &dxf_3d_config)?;
    println!("2. AutoCAD DXF Export:");
    println!("  DXF 2D Cross-section size: {} bytes", dxf_2d.len());
    println!("  DXF 3D Surface mesh size:  {} bytes\n", dxf_3d.len());

    // 3. Export 3D STL Binary Mesh
    let stl_bytes = export_stl_lens(&prescription, true, 32, 12);
    let stl_ascii = export_stl_lens(&prescription, false, 16, 8);
    println!("3. 3D Stereolithography (STL) Mesh Export:");
    println!(
        "  Binary STL size: {} bytes (Header: 80 bytes, Records: 50 bytes/tri)",
        stl_bytes.len()
    );
    println!("  ASCII STL size:  {} bytes\n", stl_ascii.len());

    // 4. Export Wavefront OBJ + MTL
    let (obj_content, mtl_content) = export_obj_system(&prescription, 32, 12);
    println!("4. Wavefront 3D OBJ + MTL Export:");
    println!("  OBJ Geometry size: {} bytes", obj_content.len());
    println!("  MTL Material size: {} bytes\n", mtl_content.len());

    // 5. Export CODE V .seq Format
    let codev_sys = CodeVSystem::from_optical_prescription(&prescription);
    let seq_content = write_seq(&codev_sys);
    println!("5. Synopsys CODE V (.seq) Export:");
    println!("  CODE V Script size: {} bytes", seq_content.len());
    println!(
        "  Sample Header:\n{}\n",
        seq_content.lines().take(4).collect::<Vec<_>>().join("\n")
    );

    // 6. Export OSLO .len Format
    let oslo_sys = OsloSystem::from_optical_prescription(&prescription);
    let len_content = write_len(&oslo_sys);
    println!("6. Lambda Research OSLO (.len) Export:");
    println!("  OSLO Script size: {} bytes", len_content.len());
    println!(
        "  Sample Header:\n{}\n",
        len_content.lines().take(4).collect::<Vec<_>>().join("\n")
    );

    // 7. Generate ISO 10110 Technical Data Sheet
    let iso_sheet = Iso10110DrawingSheet::from_prescription(&prescription, None);
    let iso_md = iso_sheet.to_markdown_table();
    println!("7. ISO 10110 Optical Drawing Sheet:");
    println!("  ISO Sheet size: {} bytes", iso_md.len());
    println!(
        "  Sample Spec:\n{}\n",
        iso_md.lines().take(6).collect::<Vec<_>>().join("\n")
    );

    // 8. OpticsProjectWorkspace Versioned Serialization
    let workspace = OpticsProjectWorkspace::new("Cooke Triplet Project", prescription.clone());
    let json_bytes = workspace.to_json()?;
    println!("8. OpticsProjectWorkspace Versioned JSON Schema:");
    println!("  Schema Version:     {}", workspace.schema_version);
    println!("  Serialized JSON:    {} bytes", json_bytes.len());

    println!("\nMulti-format CAD and optical file export successfully completed.");
    Ok(())
}
