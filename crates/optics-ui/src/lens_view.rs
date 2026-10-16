//! Minimalist academic UI rendering for lens systems and optical aberration analysis.
//!
//! Provides clean technical workstation plots:
//! - 2D lens cross-section layout with glass elements and meridional rays
//! - 2D geometric spot diagram scatter plot with Airy disk and RMS blur overlays
//! - Transverse ray aberration fan plots (tangential & sagittal)
//! - Tabular lens prescription editor with live recalculation

use egui::epaint::PathShape;
use egui::{
    Align2, Color32, DragValue, Grid, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, TextEdit,
    Ui, Vec2,
};
use optics_core::prescription::OpticalSurface;

use crate::formatting::format_percent;
use crate::lens_sim::{LensPreset, LensSimulation, LensSubview};
use crate::theme::{OpticsTokens, ray_color_for_background};
use crate::view_transform::ViewTransform;

/// Renders the complete Lens System & Aberration analysis workbench.
pub fn render_lens_system_panel(
    ui: &mut Ui,
    sim: &mut LensSimulation,
    view: &mut ViewTransform,
    tokens: &OpticsTokens,
) {
    // 1. Top Sub-Navigation Bar (academic minimalist toolbar)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);

        // Preset Selector
        ui.label("Preset:");
        let current_preset_name = sim.preset.map_or("Custom", |p| p.name());
        egui::ComboBox::from_id_salt("lens_preset_combo")
            .selected_text(current_preset_name)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(
                        sim.preset == Some(LensPreset::CookeTriplet),
                        LensPreset::CookeTriplet.name(),
                    )
                    .clicked()
                {
                    sim.select_preset(LensPreset::CookeTriplet);
                }
                if ui
                    .selectable_label(
                        sim.preset == Some(LensPreset::DoubleGauss),
                        LensPreset::DoubleGauss.name(),
                    )
                    .clicked()
                {
                    sim.select_preset(LensPreset::DoubleGauss);
                }
                if ui
                    .selectable_label(
                        sim.preset == Some(LensPreset::PlanoConvex),
                        LensPreset::PlanoConvex.name(),
                    )
                    .clicked()
                {
                    sim.select_preset(LensPreset::PlanoConvex);
                }
            });

        ui.separator();

        // Subview Tab Buttons
        let tabs = [
            (LensSubview::CrossSection, "Layout & Rays"),
            (LensSubview::SpotDiagram, "Spot Diagram"),
            (LensSubview::RayFan, "Aberration Fans"),
            (LensSubview::PrescriptionTable, "Prescription Table"),
        ];

        for (subview, label) in tabs {
            let is_selected = sim.active_subview == subview;
            if ui.selectable_label(is_selected, label).clicked() {
                sim.active_subview = subview;
            }
        }

        ui.separator();

        // Field Angle Quick Buttons
        ui.label("Field:");
        let field_angles = [0.0, 10.0, 14.0];
        for angle in field_angles {
            let is_sel = (sim.field_angle_deg - angle).abs() < 1e-3;
            let text = if angle == 0.0 {
                "0° (On-axis)".to_string()
            } else {
                format!("{angle:.0}°")
            };
            if ui.selectable_label(is_sel, text).clicked() {
                sim.field_angle_deg = angle;
                sim.recalculate();
            }
        }

        ui.separator();

        // Wavelength Quick Buttons
        ui.label("Line:");
        let wavelengths = [
            (587.56, "d: 587.6 nm (Yellow)"),
            (486.13, "F: 486.1 nm (Blue)"),
            (656.27, "C: 656.3 nm (Red)"),
        ];
        for (w, label) in wavelengths {
            let is_sel = (sim.wavelength_nm - w).abs() < 1e-2;
            if ui.selectable_label(is_sel, label).clicked() {
                sim.wavelength_nm = w;
                sim.recalculate();
            }
        }
    });

    ui.separator();

    // 2. Main Content Split: Left Summary sidebar + Center Active Subview
    ui.horizontal(|ui| {
        // Left Paraxial & Diagnostics Sidebar
        ui.vertical(|ui| {
            ui.set_width(240.0);
            render_lens_sidebar(ui, sim, tokens);
        });

        ui.separator();

        // Center Subview Content
        ui.vertical(|ui| match sim.active_subview {
            LensSubview::CrossSection => {
                render_lens_cross_section_view(ui, sim, view, tokens);
            }
            LensSubview::SpotDiagram => {
                render_spot_diagram_view(ui, sim, tokens);
            }
            LensSubview::RayFan => {
                render_ray_fan_view(ui, sim, tokens);
            }
            LensSubview::PrescriptionTable => {
                render_prescription_table_view(ui, sim, tokens);
            }
        });
    });
}

/// Renders the left-hand diagnostic summary card.
fn render_lens_sidebar(ui: &mut Ui, sim: &LensSimulation, tokens: &OpticsTokens) {
    ui.group(|ui| {
        ui.label(
            egui::RichText::new("Paraxial Properties")
                .strong()
                .color(tokens.text_primary),
        );
        ui.add_space(4.0);

        if let Some(parax) = &sim.paraxial {
            Grid::new("paraxial_summary_grid")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label("EFL (f):");
                    ui.label(format!("{:.3} mm", parax.efl_mm));
                    ui.end_row();

                    ui.label("BFL:");
                    ui.label(format!("{:.3} mm", parax.bfl_mm));
                    ui.end_row();

                    ui.label("F-number:");
                    ui.label(format!("F/{:.2}", parax.f_number));
                    ui.end_row();

                    ui.label("Entrance Pupil:");
                    ui.label(format!("{:.2} mm", parax.entrance_pupil_pos_mm));
                    ui.end_row();

                    ui.label("Exit Pupil:");
                    ui.label(format!("{:.2} mm", parax.exit_pupil_pos_mm));
                    ui.end_row();
                });
        } else {
            ui.label(
                egui::RichText::new("No paraxial solution")
                    .italics()
                    .color(tokens.text_dim),
            );
        }
    });

    ui.add_space(8.0);

    // Spot Metrics Box (when available)
    if let Some(metrics) = &sim.spot_metrics {
        ui.group(|ui| {
            ui.label(
                egui::RichText::new("Spot Diagram Metrics")
                    .strong()
                    .color(tokens.text_primary),
            );
            ui.add_space(4.0);

            Grid::new("spot_metrics_sidebar_grid")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Centroid X:");
                    ui.label(format!("{:.2} µm", metrics.centroid_x_mm * 1000.0));
                    ui.end_row();

                    ui.label("Centroid Y:");
                    ui.label(format!("{:.2} µm", metrics.centroid_y_mm * 1000.0));
                    ui.end_row();

                    ui.label("RMS Radius:");
                    ui.label(format!("{:.2} µm", metrics.rms_radius_um));
                    ui.end_row();

                    ui.label("GEO Radius:");
                    ui.label(format!("{:.2} µm", metrics.geo_radius_um));
                    ui.end_row();

                    ui.label("Airy Disk:");
                    ui.label(format!("{:.2} µm", metrics.airy_radius_um));
                    ui.end_row();

                    ui.label("Vignetting:");
                    ui.label(format_percent(metrics.vignetting_factor));
                    ui.end_row();

                    ui.label("Passed Rays:");
                    ui.label(format!(
                        "{}/{}",
                        metrics.passed_rays_count, metrics.total_rays_traced
                    ));
                    ui.end_row();
                });

            ui.add_space(4.0);
            if metrics.is_diffraction_limited {
                ui.colored_label(tokens.accent, "Status: Diffraction Limited");
            } else {
                ui.colored_label(tokens.amber, "Status: Aberration Dominated");
            }
        });
    }

    if let Some(err) = &sim.last_error {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.colored_label(Color32::from_rgb(230, 80, 80), "Calculation Notice:");
            ui.label(egui::RichText::new(err).small().color(tokens.text_dim));
        });
    }
}

/// Renders the 2D cross-section view of lens elements and traced ray paths.
fn render_lens_cross_section_view(
    ui: &mut Ui,
    sim: &LensSimulation,
    view: &mut ViewTransform,
    tokens: &OpticsTokens,
) {
    let available_size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    // Pan & Zoom interaction
    if response.dragged() {
        view.pan_by_screen_delta(response.drag_delta());
    }
    let zoom_delta = ui.input(|i| i.zoom_delta());
    if (zoom_delta - 1.0).abs() > 1e-4 {
        if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if rect.contains(mouse_pos) {
                view.zoom_at(zoom_delta, mouse_pos, rect);
            }
        }
    }

    // Auto-fit initial view if scale is default
    if (view.zoom - 6.0).abs() < 1e-3 && view.center_world == Pos2::new(0.0, 20.0) {
        // Lens axial length ~60 mm, semi-diameter ~15 mm
        let bounds = Rect::from_min_max(Pos2::new(-10.0, -25.0), Pos2::new(70.0, 25.0));
        view.fit_to_rect(bounds, rect, 40.0);
    }

    // Background
    painter.rect_filled(rect, 0.0, tokens.canvas);

    // 1. Draw Optical Axis (z-axis, y = 0)
    let p_axis_start = view.world_to_screen(Pos2::new(-20.0, 0.0), rect);
    let p_axis_end = view.world_to_screen(Pos2::new(90.0, 0.0), rect);
    let axis_stroke = Stroke::new(1.0, tokens.border);
    painter.line_segment([p_axis_start, p_axis_end], axis_stroke);

    // 2. Draw Lens Elements (polygonal cross-sections)
    let element_fill = Color32::from_rgba_unmultiplied(26, 45, 66, 180);
    let element_border = Stroke::new(1.2, Color32::from_rgb(60, 95, 135));

    for elem in &sim.elements {
        if elem.boundary_points.len() >= 3 {
            let screen_pts: Vec<Pos2> = elem
                .boundary_points
                .iter()
                .map(|pt| view.world_to_screen(Pos2::new(pt[0] as f32, pt[1] as f32), rect))
                .collect();

            painter.add(PathShape::convex_polygon(
                screen_pts,
                element_fill,
                element_border,
            ));

            // Element material label centered at element vertex
            if let Some(first_pt) = elem.boundary_points.first() {
                let text_pos =
                    view.world_to_screen(Pos2::new((first_pt[0] + 2.0) as f32, -18.0), rect);
                painter.text(
                    text_pos,
                    Align2::CENTER_CENTER,
                    &elem.material_name,
                    egui::FontId::monospace(10.0),
                    tokens.text_dim,
                );
            }
        }
    }

    // 3. Draw Image Sensor Plane (final surface)
    if let Some(last_surf) = sim.prescription.surfaces.last() {
        let total_z: f64 = sim
            .prescription
            .surfaces
            .iter()
            .map(|s| s.thickness_mm)
            .sum();
        let h_sensor = last_surf.semi_diameter_mm;
        let p_top = view.world_to_screen(Pos2::new(total_z as f32, h_sensor as f32), rect);
        let p_bot = view.world_to_screen(Pos2::new(total_z as f32, -h_sensor as f32), rect);
        painter.line_segment([p_top, p_bot], Stroke::new(2.0, tokens.amber));

        painter.text(
            p_top + Vec2::new(0.0, -10.0),
            Align2::CENTER_BOTTOM,
            "Sensor",
            egui::FontId::proportional(11.0),
            tokens.amber,
        );
    }

    // 4. Draw Traced Cross-Section Rays
    for ray in &sim.cross_section_rays {
        if ray.vertices.len() >= 2 {
            let base_color = ray_color_for_background(ray.wavelength_nm, tokens.canvas);
            let alpha = if ray.passed { 210 } else { 80 };
            let color = Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );
            let stroke = Stroke::new(1.2, color);

            for pair in ray.vertices.windows(2) {
                let p0 =
                    view.world_to_screen(Pos2::new(pair[0][0] as f32, pair[0][1] as f32), rect);
                let p1 =
                    view.world_to_screen(Pos2::new(pair[1][0] as f32, pair[1][1] as f32), rect);
                painter.line_segment([p0, p1], stroke);
            }
        }
    }
}

/// Renders the 2D geometric spot diagram scatter plot.
fn render_spot_diagram_view(ui: &mut Ui, sim: &LensSimulation, tokens: &OpticsTokens) {
    let available_size = ui.available_size();
    let (rect, _response) = ui.allocate_exact_size(available_size, Sense::hover());
    let painter = ui.painter_at(rect);

    // Background & border
    painter.rect_filled(rect, 0.0, tokens.canvas);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    let Some(spot) = &sim.spot_diagram else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No spot diagram data available",
            egui::FontId::proportional(14.0),
            tokens.text_dim,
        );
        return;
    };

    let metrics = &spot.metrics;
    let center = rect.center();

    // Determine plot radius scale in micrometers
    let max_radius_um = metrics
        .geo_radius_um
        .max(metrics.airy_radius_um * 2.0)
        .max(25.0);
    // Round up to nearest nice scale: 25, 50, 100, 200, 500, 1000
    let plot_range_um = if max_radius_um <= 25.0 {
        30.0
    } else if max_radius_um <= 50.0 {
        60.0
    } else if max_radius_um <= 100.0 {
        120.0
    } else if max_radius_um <= 250.0 {
        300.0
    } else {
        (max_radius_um * 1.2).ceil()
    };

    let min_dim = rect.width().min(rect.height()) - 60.0;
    let pixels_per_um = ((min_dim * 0.5) as f64) / plot_range_um;

    // Helper: converts (dx_um, dy_um) relative to centroid into screen Pos2
    let to_screen = |dx_um: f64, dy_um: f64| -> Pos2 {
        Pos2::new(
            center.x + (dx_um * pixels_per_um) as f32,
            center.y - (dy_um * pixels_per_um) as f32, // y inverted in screen space
        )
    };

    // 1. Grid & Axes
    let grid_stroke = Stroke::new(
        0.8,
        Color32::from_rgba_unmultiplied(
            tokens.border.r(),
            tokens.border.g(),
            tokens.border.b(),
            90,
        ),
    );
    let axis_stroke = Stroke::new(1.2, tokens.border);

    // Concentric grid circles
    for fraction in [0.25, 0.5, 0.75, 1.0] {
        let r_um = plot_range_um * fraction;
        let r_px = (r_um * pixels_per_um) as f32;
        painter.circle_stroke(center, r_px, grid_stroke);

        // Tick label
        let label_pos = center + Vec2::new(r_px + 2.0, 0.0);
        painter.text(
            label_pos,
            Align2::LEFT_CENTER,
            format!("{r_um:.0} µm"),
            egui::FontId::monospace(9.0),
            tokens.text_dim,
        );
    }

    // Crosshair axes
    let left = Pos2::new(rect.left() + 20.0, center.y);
    let right = Pos2::new(rect.right() - 20.0, center.y);
    let top = Pos2::new(center.x, rect.top() + 20.0);
    let bottom = Pos2::new(center.x, rect.bottom() - 20.0);
    painter.line_segment([left, right], axis_stroke);
    painter.line_segment([top, bottom], axis_stroke);

    // 2. Airy Disk Circle (cyan dotted)
    if metrics.airy_radius_um > 0.0 {
        let airy_px = (metrics.airy_radius_um * pixels_per_um) as f32;
        painter.circle_stroke(center, airy_px, Stroke::new(1.2, tokens.accent));
    }

    // 3. RMS Radius Circle (amber dashed)
    if metrics.rms_radius_um > 0.0 {
        let rms_px = (metrics.rms_radius_um * pixels_per_um) as f32;
        painter.circle_stroke(
            center,
            rms_px,
            Stroke::new(1.2, Color32::from_rgb(245, 166, 35)),
        );
    }

    // 4. Spot Points
    let dot_color = ray_color_for_background(sim.wavelength_nm, tokens.canvas);
    for pt in &spot.points {
        let dx_um = (pt.x_mm - metrics.centroid_x_mm) * 1000.0;
        let dy_um = (pt.y_mm - metrics.centroid_y_mm) * 1000.0;
        let p_screen = to_screen(dx_um, dy_um);
        painter.circle_filled(p_screen, 1.8, dot_color);
    }

    // 5. Centroid Crosshair Marker (+)
    painter.line_segment(
        [center + Vec2::new(-4.0, 0.0), center + Vec2::new(4.0, 0.0)],
        Stroke::new(1.5, tokens.amber),
    );
    painter.line_segment(
        [center + Vec2::new(0.0, -4.0), center + Vec2::new(0.0, 4.0)],
        Stroke::new(1.5, tokens.amber),
    );

    // 6. Academic Legend Overlay at top-left
    let legend_pos = rect.min + Vec2::new(16.0, 16.0);
    painter.text(
        legend_pos,
        Align2::LEFT_TOP,
        format!(
            "Spot Diagram (Field: {:.1}°, λ: {:.1} nm)\nCentroid: ({:.1}, {:.1}) µm\nRMS: {:.2} µm  |  Airy: {:.2} µm\nPassed: {} / {} rays",
            sim.field_angle_deg,
            sim.wavelength_nm,
            metrics.centroid_x_mm * 1000.0,
            metrics.centroid_y_mm * 1000.0,
            metrics.rms_radius_um,
            metrics.airy_radius_um,
            metrics.passed_rays_count,
            metrics.total_rays_traced
        ),
        egui::FontId::monospace(11.0),
        tokens.text_primary,
    );
}

/// Renders the dual transverse ray aberration fan plots (tangential & sagittal).
fn render_ray_fan_view(ui: &mut Ui, sim: &LensSimulation, tokens: &OpticsTokens) {
    let available_size = ui.available_size();
    let (rect, _response) = ui.allocate_exact_size(available_size, Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, tokens.canvas);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    let Some(fan) = &sim.ray_fan else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No ray fan data available",
            egui::FontId::proportional(14.0),
            tokens.text_dim,
        );
        return;
    };

    // Split horizontally into Tangential Fan (left) and Sagittal Fan (right)
    let margin = 20.0;
    let width_each = (rect.width() - margin * 3.0) * 0.5;
    let height = rect.height() - margin * 2.0;

    let rect_tangential = Rect::from_min_size(
        rect.min + Vec2::new(margin, margin),
        Vec2::new(width_each, height),
    );
    let rect_sagittal = Rect::from_min_size(
        rect.min + Vec2::new(margin * 2.0 + width_each, margin),
        Vec2::new(width_each, height),
    );

    // Find maximum aberration across both fans for uniform scaling
    let max_tan = fan
        .tangential
        .iter()
        .map(|p| p.aberration_um.abs())
        .fold(0.0_f64, f64::max);
    let max_sag = fan
        .sagittal
        .iter()
        .map(|p| p.aberration_um.abs())
        .fold(0.0_f64, f64::max);
    let max_err_um = max_tan.max(max_sag).max(10.0);
    // Round up to nearest nice scale: 10, 25, 50, 100, 200, 500
    let plot_range_um = if max_err_um <= 10.0 {
        12.0
    } else if max_err_um <= 25.0 {
        30.0
    } else if max_err_um <= 50.0 {
        60.0
    } else if max_err_um <= 100.0 {
        120.0
    } else if max_err_um <= 250.0 {
        300.0
    } else {
        (max_err_um * 1.2).ceil()
    };

    // Draw Tangential Fan (Ey vs Py)
    draw_single_ray_fan_plot(
        &painter,
        rect_tangential,
        &fan.tangential,
        "Tangential Fan [Ey vs Py]",
        plot_range_um,
        tokens,
    );

    // Draw Sagittal Fan (Ex vs Px)
    draw_single_ray_fan_plot(
        &painter,
        rect_sagittal,
        &fan.sagittal,
        "Sagittal Fan [Ex vs Px]",
        plot_range_um,
        tokens,
    );
}

/// Draws one single ray aberration fan plot (tangential or sagittal).
fn draw_single_ray_fan_plot(
    painter: &egui::Painter,
    rect: Rect,
    points: &[optics_core::analysis::RayFanPoint],
    title: &str,
    plot_range_um: f64,
    tokens: &OpticsTokens,
) {
    let center = rect.center();
    let pad_x = 40.0;
    let pad_y = 35.0;
    let plot_w = rect.width() - pad_x * 2.0;
    let plot_h = rect.height() - pad_y * 2.0;

    let x_scale = (plot_w * 0.5) as f64 / 1.0; // pupil coord in [-1.0, 1.0]
    let y_scale = (plot_h * 0.5) as f64 / plot_range_um;

    let to_screen = |pupil: f64, aberration_um: f64| -> Pos2 {
        Pos2::new(
            center.x + (pupil * x_scale) as f32,
            center.y - (aberration_um * y_scale) as f32,
        )
    };

    // Plot Border
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    // Axes
    let axis_stroke = Stroke::new(1.2, tokens.border);
    let p_x_min = to_screen(-1.0, 0.0);
    let p_x_max = to_screen(1.0, 0.0);
    let p_y_min = to_screen(0.0, -plot_range_um);
    let p_y_max = to_screen(0.0, plot_range_um);
    painter.line_segment([p_x_min, p_x_max], axis_stroke);
    painter.line_segment([p_y_min, p_y_max], axis_stroke);

    // Grid lines & tick marks at pupil -1.0, -0.5, 0.5, 1.0
    let grid_stroke = Stroke::new(
        0.8,
        Color32::from_rgba_unmultiplied(
            tokens.border.r(),
            tokens.border.g(),
            tokens.border.b(),
            70,
        ),
    );
    for p_val in [-1.0, -0.5, 0.5, 1.0] {
        let p_top = to_screen(p_val, plot_range_um);
        let p_bot = to_screen(p_val, -plot_range_um);
        painter.line_segment([p_top, p_bot], grid_stroke);

        painter.text(
            p_bot + Vec2::new(0.0, 4.0),
            Align2::CENTER_TOP,
            format!("{p_val:.1}"),
            egui::FontId::monospace(9.0),
            tokens.text_dim,
        );
    }

    // Grid lines & tick marks at Y: -plot_range, -plot_range/2, plot_range/2, plot_range
    for y_val in [
        -plot_range_um,
        -plot_range_um * 0.5,
        plot_range_um * 0.5,
        plot_range_um,
    ] {
        let p_l = to_screen(-1.0, y_val);
        let p_r = to_screen(1.0, y_val);
        painter.line_segment([p_l, p_r], grid_stroke);

        painter.text(
            p_l + Vec2::new(-4.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{y_val:.0}"),
            egui::FontId::monospace(9.0),
            tokens.text_dim,
        );
    }

    // Plot Title & Units
    painter.text(
        rect.min + Vec2::new(12.0, 8.0),
        Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(11.0),
        tokens.text_primary,
    );
    painter.text(
        rect.min + Vec2::new(rect.width() - 12.0, 8.0),
        Align2::RIGHT_TOP,
        "Scale: ± µm",
        egui::FontId::monospace(10.0),
        tokens.text_dim,
    );

    // Fan Aberration Curve
    if points.len() >= 2 {
        let curve_stroke = Stroke::new(1.8, tokens.accent);
        let screen_pts: Vec<Pos2> = points
            .iter()
            .map(|p| to_screen(p.pupil_coord, p.aberration_um))
            .collect();

        for pair in screen_pts.windows(2) {
            painter.line_segment([pair[0], pair[1]], curve_stroke);
        }

        // Point markers
        for pt in screen_pts {
            painter.circle_filled(pt, 2.0, tokens.accent);
        }
    }
}

/// Renders the editable numerical lens prescription table.
/// Renders the lens prescription editor table.
pub fn render_prescription_table_view(
    ui: &mut Ui,
    sim: &mut LensSimulation,
    tokens: &OpticsTokens,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Lens Prescription Editor")
                .strong()
                .color(tokens.text_primary),
        );
        ui.add_space(16.0);

        if ui.button("+ Add Surface").clicked() {
            let next_idx = sim.prescription.surfaces.len();
            sim.prescription.surfaces.push(OpticalSurface {
                surface_index: next_idx,
                radius_mm: f64::INFINITY,
                conic_constant: 0.0,
                thickness_mm: 5.0,
                semi_diameter_mm: 10.0,
                material_name: "AIR".to_string(),
                is_stop: false,
            });
            sim.recalculate();
        }

        if sim.prescription.surfaces.len() > 2 && ui.button("- Remove Surface").clicked() {
            sim.prescription.surfaces.pop();
            sim.recalculate();
        }
    });

    ui.add_space(8.0);

    let mut needs_recalc = false;

    ScrollArea::both().show(ui, |ui| {
        Grid::new("prescription_table_grid")
            .striped(true)
            .min_col_width(80.0)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                // Table Header
                ui.label(egui::RichText::new("#").strong());
                ui.label(egui::RichText::new("Role").strong());
                ui.label(egui::RichText::new("Radius (mm)").strong());
                ui.label(egui::RichText::new("Thickness (mm)").strong());
                ui.label(egui::RichText::new("Material").strong());
                ui.label(egui::RichText::new("Semi-Diam (mm)").strong());
                ui.label(egui::RichText::new("Conic (k)").strong());
                ui.label(egui::RichText::new("Stop").strong());
                ui.end_row();

                // Table Rows
                let num_surfaces = sim.prescription.surfaces.len();
                for i in 0..num_surfaces {
                    let s = &mut sim.prescription.surfaces[i];

                    // Index
                    ui.label(format!("{i}"));

                    // Role
                    let role = if i == 0 {
                        "Object/Entry"
                    } else if i == num_surfaces - 1 {
                        "Image Sensor"
                    } else if s.is_stop {
                        "Stop"
                    } else {
                        "Standard"
                    };
                    ui.label(role);

                    // Radius (with Infinity support)
                    let is_flat = s.radius_mm.is_infinite() || s.radius_mm.abs() > 1e10;
                    let mut r_display = if is_flat { 0.0 } else { s.radius_mm };
                    let r_resp = ui.add(
                        DragValue::new(&mut r_display)
                            .speed(0.5)
                            .custom_formatter(|val, _| {
                                if val.abs() < 1e-4 {
                                    "Infinity".to_string()
                                } else {
                                    format!("{val:.3}")
                                }
                            })
                            .custom_parser(|text| {
                                if text.trim().eq_ignore_ascii_case("infinity")
                                    || text.trim() == "0"
                                {
                                    Some(0.0)
                                } else {
                                    text.trim().parse::<f64>().ok()
                                }
                            }),
                    );
                    if r_resp.changed() {
                        s.radius_mm = if r_display.abs() < 1e-4 {
                            f64::INFINITY
                        } else {
                            r_display
                        };
                        needs_recalc = true;
                    }

                    // Thickness
                    let t_resp = ui.add(
                        DragValue::new(&mut s.thickness_mm)
                            .speed(0.1)
                            .range(0.0..=500.0),
                    );
                    if t_resp.changed() {
                        needs_recalc = true;
                    }

                    // Material
                    let m_resp =
                        ui.add(TextEdit::singleline(&mut s.material_name).desired_width(70.0));
                    if m_resp.changed() {
                        needs_recalc = true;
                    }

                    // Semi-Diameter
                    let d_resp = ui.add(
                        DragValue::new(&mut s.semi_diameter_mm)
                            .speed(0.1)
                            .range(0.5..=200.0),
                    );
                    if d_resp.changed() {
                        needs_recalc = true;
                    }

                    // Conic Constant
                    let k_resp = ui.add(DragValue::new(&mut s.conic_constant).speed(0.05));
                    if k_resp.changed() {
                        needs_recalc = true;
                    }

                    // Stop Checkbox
                    let mut is_stop = s.is_stop;
                    if ui.checkbox(&mut is_stop, "").changed() {
                        s.is_stop = is_stop;
                        needs_recalc = true;
                    }

                    ui.end_row();
                }
            });
    });

    if needs_recalc {
        sim.recalculate();
    }
}
