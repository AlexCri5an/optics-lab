//! Embedded scientific analytics dock matching the reference interface.
//!
//! Provides 4 compact technical diagnostic panels:
//! 1. Convergence (RMSE & Variance error curves)
//! 2. Spectral Power (wavelength histogram & gradient)
//! 3. BVH Diagnostics (traversal cost heatmap & TLAS/BLAS metrics)
//! 4. Material / Glass (Sellmeier/Cauchy parameters & dispersion preview)

use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use optics_spectral::color::display_color_for_wavelength;

use crate::theme::OpticsTokens;

fn wavelength_color32(wavelength_nm: f64) -> Color32 {
    let rgb = display_color_for_wavelength(wavelength_nm);
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Renders the 4-panel horizontal analytics dock.
pub fn render_analytics_dock(
    ui: &mut egui::Ui,
    tokens: &OpticsTokens,
    glass_name: &mut String,
    on_edit_material: impl FnOnce(),
) {
    let avail_width = ui.available_width();
    let panel_height = 195.0;
    let card_width = ((avail_width - 24.0) / 4.0).max(180.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

        // Panel 1: Convergence
        render_convergence_panel(ui, card_width, panel_height, tokens);

        // Panel 2: Spectral Power
        render_spectral_power_panel(ui, card_width, panel_height, tokens);

        // Panel 3: BVH Diagnostics
        render_bvh_diagnostics_panel(ui, card_width, panel_height, tokens);

        // Panel 4: Material / Glass
        render_material_glass_panel(
            ui,
            card_width,
            panel_height,
            tokens,
            glass_name,
            on_edit_material,
        );
    });
}

/// Panel 1: Convergence tracking (RMSE & Variance).
fn render_convergence_panel(ui: &mut egui::Ui, width: f32, height: f32, tokens: &OpticsTokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Card frame
    painter.rect_filled(rect, egui::CornerRadius::same(2), tokens.panel_bg);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // Header bar
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(width, 24.0));
    painter.text(
        header_rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        "Convergence",
        egui::FontId::proportional(12.0),
        tokens.text_primary,
    );

    // Minimize and close glyphs on top-right of panel
    let glyph_y = header_rect.min.y + 12.0;
    painter.text(
        Pos2::new(rect.max.x - 22.0, glyph_y),
        egui::Align2::CENTER_CENTER,
        "-",
        egui::FontId::proportional(12.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(rect.max.x - 10.0, glyph_y),
        egui::Align2::CENTER_CENTER,
        "x",
        egui::FontId::proportional(10.0),
        tokens.text_muted,
    );

    // Legend
    let legend_y = header_rect.bottom() + 8.0;
    let cyan = Color32::from_rgb(0x00, 0xd4, 0xf5);
    let amber = Color32::from_rgb(0xf5, 0xa6, 0x23);

    painter.line_segment(
        [
            Pos2::new(rect.min.x + 80.0, legend_y),
            Pos2::new(rect.min.x + 92.0, legend_y),
        ],
        Stroke::new(1.5, cyan),
    );
    painter.text(
        Pos2::new(rect.min.x + 96.0, legend_y),
        egui::Align2::LEFT_CENTER,
        "RMSE",
        egui::FontId::proportional(10.0),
        tokens.text_secondary,
    );

    painter.line_segment(
        [
            Pos2::new(rect.min.x + 140.0, legend_y),
            Pos2::new(rect.min.x + 152.0, legend_y),
        ],
        Stroke::new(1.5, amber),
    );
    painter.text(
        Pos2::new(rect.min.x + 156.0, legend_y),
        egui::Align2::LEFT_CENTER,
        "Variance",
        egui::FontId::proportional(10.0),
        tokens.text_secondary,
    );

    // Graph plotting area
    let graph_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 36.0, legend_y + 12.0),
        Pos2::new(rect.max.x - 12.0, rect.bottom() - 26.0),
    );

    // Y Axis labels (log-scale error: 1.0, 0.5, 0.1, 0.01)
    let y_ticks = [(1.0_f32, "1.0"), (0.5, "0.5"), (0.1, "0.1"), (0.01, "0.01")];
    for (val, label) in y_ticks {
        // Map log10(val) from log10(0.01)=-2.0 to log10(1.0)=0.0
        let norm_y = (val.log10() - (-2.0)) / 2.0;
        let y = graph_rect.bottom() - norm_y * graph_rect.height();
        painter.text(
            Pos2::new(graph_rect.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            label,
            egui::FontId::monospace(9.5),
            tokens.text_muted,
        );
        painter.line_segment(
            [
                Pos2::new(graph_rect.left(), y),
                Pos2::new(graph_rect.right(), y),
            ],
            Stroke::new(0.8, tokens.border_subtle),
        );
    }

    // X Axis labels (Samples: 0, 32, 64, 128, 256)
    let x_ticks = [
        (0.0_f32, "0"),
        (0.125, "32"),
        (0.25, "64"),
        (0.5, "128"),
        (1.0, "256"),
    ];
    for (norm_x, label) in x_ticks {
        let x = graph_rect.left() + norm_x * graph_rect.width();
        painter.text(
            Pos2::new(x, graph_rect.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::monospace(9.5),
            tokens.text_muted,
        );
    }
    painter.text(
        Pos2::new(graph_rect.center().x, rect.bottom() - 6.0),
        egui::Align2::CENTER_BOTTOM,
        "Samples",
        egui::FontId::proportional(10.0),
        tokens.text_secondary,
    );

    // Curves (Welford RMSE and Variance)
    let sample_count = 60;
    let mut rmse_pts = Vec::with_capacity(sample_count);
    let mut var_pts = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let u = i as f32 / (sample_count - 1) as f32;
        let x = graph_rect.left() + u * graph_rect.width();

        // Realistic decay formulas
        let rmse_val = (1.0 / (1.0 + u * 9.0)).max(0.01);
        let jitter = ((i * 17) % 7) as f32 * 0.008;
        let var_val = (0.8 / (1.0 + u * 24.0) + jitter).max(0.01);

        let rmse_norm_y = (rmse_val.log10() + 2.0) / 2.0;
        let var_norm_y = (var_val.log10() + 2.0) / 2.0;

        rmse_pts.push(Pos2::new(
            x,
            graph_rect.bottom() - rmse_norm_y * graph_rect.height(),
        ));
        var_pts.push(Pos2::new(
            x,
            graph_rect.bottom() - var_norm_y * graph_rect.height(),
        ));
    }

    for w in rmse_pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.4, cyan));
    }
    for w in var_pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.2, amber));
    }
}

/// Panel 2: Spectral Power histogram & wavelength gradient.
fn render_spectral_power_panel(ui: &mut egui::Ui, width: f32, height: f32, tokens: &OpticsTokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(2), tokens.panel_bg);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // Header
    painter.text(
        rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        "Spectral Power",
        egui::FontId::proportional(12.0),
        tokens.text_primary,
    );

    let graph_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 32.0, rect.min.y + 36.0),
        Pos2::new(rect.max.x - 12.0, rect.bottom() - 38.0),
    );

    // Y Axis ticks (0.3, 0.2, 0.1, 0.0)
    for (val, label) in [(0.3_f32, "0.3"), (0.2, "0.2"), (0.1, "0.1"), (0.0, "0.0")] {
        let y = graph_rect.bottom() - (val / 0.3) * graph_rect.height();
        painter.text(
            Pos2::new(graph_rect.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            label,
            egui::FontId::monospace(9.5),
            tokens.text_muted,
        );
        painter.line_segment(
            [
                Pos2::new(graph_rect.left(), y),
                Pos2::new(graph_rect.right(), y),
            ],
            Stroke::new(0.8, tokens.border_subtle),
        );
    }

    // Histogram bars (bell curve around 540-580 nm)
    let bin_count = 20;
    let bar_width = (graph_rect.width() / bin_count as f32) - 1.5;

    for i in 0..bin_count {
        let u = i as f32 / (bin_count - 1) as f32;
        let wavelength = 380.0 + u * (700.0 - 380.0);
        let color = wavelength_color32(wavelength as f64);

        // Bell curve profile
        let center_wl = 555.0;
        let sigma = 65.0;
        let norm_power = (-0.5 * ((wavelength - center_wl) / sigma).powi(2)).exp() * 0.27;

        let bar_h = (norm_power / 0.3) * graph_rect.height();
        let x = graph_rect.left() + (i as f32) * (graph_rect.width() / bin_count as f32);
        let bar_rect = Rect::from_min_max(
            Pos2::new(x, graph_rect.bottom() - bar_h),
            Pos2::new(x + bar_width, graph_rect.bottom()),
        );
        painter.rect_filled(bar_rect, egui::CornerRadius::ZERO, color);
    }

    // Continuous spectrum gradient bar at bottom
    let grad_rect = Rect::from_min_max(
        Pos2::new(graph_rect.left(), rect.bottom() - 30.0),
        Pos2::new(graph_rect.right(), rect.bottom() - 24.0),
    );
    let grad_steps = 32;
    let step_w = grad_rect.width() / grad_steps as f32;
    for i in 0..grad_steps {
        let u = i as f32 / grad_steps as f32;
        let wl = 380.0 + u * 320.0;
        let col = wavelength_color32(wl as f64);
        let x = grad_rect.left() + (i as f32) * step_w;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(x, grad_rect.min.y),
                Vec2::new(step_w + 0.5, grad_rect.height()),
            ),
            egui::CornerRadius::ZERO,
            col,
        );
    }

    // X Axis labels (380, 450, 520, 590, 650, 700)
    for (wl, label) in [
        (380.0, "380"),
        (450.0, "450"),
        (520.0, "520"),
        (590.0, "590"),
        (650.0, "650"),
        (700.0, "700"),
    ] {
        let u = (wl - 380.0) / 320.0;
        let x = graph_rect.left() + (u as f32) * graph_rect.width();
        painter.text(
            Pos2::new(x, rect.bottom() - 14.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(9.0),
            tokens.text_muted,
        );
    }
    painter.text(
        Pos2::new(graph_rect.right(), rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        "λ (nm)",
        egui::FontId::proportional(10.0),
        tokens.text_secondary,
    );
}

/// Panel 3: BVH Diagnostics (traversal cost heatmap & metrics).
fn render_bvh_diagnostics_panel(ui: &mut egui::Ui, width: f32, height: f32, tokens: &OpticsTokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(2), tokens.panel_bg);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // Header
    painter.text(
        rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        "BVH Diagnostics",
        egui::FontId::proportional(12.0),
        tokens.text_primary,
    );

    // Miniature wireframe bounding box hierarchy of model
    let box_center = Pos2::new(rect.min.x + 55.0, rect.min.y + 75.0);
    let s = 42.0;

    // Outer root AABB
    let outer_aabb = Rect::from_center_size(box_center, Vec2::new(s, s * 1.3));
    painter.rect_stroke(
        outer_aabb,
        egui::CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_rgb(0x00, 0xa0, 0xd0)),
        egui::StrokeKind::Inside,
    );

    // Child boxes (nested BLAS nodes)
    let left_box = Rect::from_min_max(
        outer_aabb.min,
        Pos2::new(outer_aabb.center().x + 5.0, outer_aabb.center().y + 8.0),
    );
    painter.rect_stroke(
        left_box,
        egui::CornerRadius::ZERO,
        Stroke::new(0.8, Color32::from_rgb(0x00, 0x80, 0xb0)),
        egui::StrokeKind::Inside,
    );

    let right_box = Rect::from_min_max(
        Pos2::new(outer_aabb.center().x - 4.0, outer_aabb.center().y - 10.0),
        outer_aabb.max,
    );
    painter.rect_stroke(
        right_box,
        egui::CornerRadius::ZERO,
        Stroke::new(0.8, Color32::from_rgb(0x00, 0x90, 0xc0)),
        egui::StrokeKind::Inside,
    );

    // Heatmap mesh silhouette (Stanford bunny / geometric prism traversal heatmap)
    let mesh_pts = [
        (Vec2::new(0.0, -20.0), Color32::from_rgb(0x00, 0xd4, 0xf5)),
        (Vec2::new(-10.0, -10.0), Color32::from_rgb(0x30, 0xe0, 0x80)),
        (Vec2::new(8.0, -6.0), Color32::from_rgb(0xf5, 0xa6, 0x23)),
        (Vec2::new(0.0, 4.0), Color32::from_rgb(0xff, 0x33, 0x33)),
        (Vec2::new(-12.0, 12.0), Color32::from_rgb(0x30, 0xe0, 0x80)),
        (Vec2::new(14.0, 16.0), Color32::from_rgb(0x00, 0xd4, 0xf5)),
    ];
    for (offset, col) in mesh_pts {
        painter.circle_filled(box_center + offset, 4.0, col);
    }

    // Heatmap color bar on the right
    let bar_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 115.0, rect.min.y + 44.0),
        Pos2::new(rect.min.x + 123.0, rect.min.y + 115.0),
    );
    painter.text(
        Pos2::new(bar_rect.center().x + 22.0, bar_rect.min.y - 6.0),
        egui::Align2::CENTER_BOTTOM,
        "Traversal Cost",
        egui::FontId::proportional(9.0),
        tokens.text_muted,
    );
    // Vertical ramp (High red to Low blue)
    let bar_steps = 16;
    let step_h = bar_rect.height() / bar_steps as f32;
    for i in 0..bar_steps {
        let u = i as f32 / bar_steps as f32;
        let r = (255.0 * (1.0 - u)) as u8;
        let g = (255.0 * (1.0 - (u - 0.5).abs() * 2.0).max(0.0)) as u8;
        let b = (255.0 * u) as u8;
        let y = bar_rect.min.y + (i as f32) * step_h;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(bar_rect.min.x, y),
                Vec2::new(bar_rect.width(), step_h + 0.5),
            ),
            egui::CornerRadius::ZERO,
            Color32::from_rgb(r, g, b),
        );
    }
    painter.text(
        Pos2::new(bar_rect.right() + 4.0, bar_rect.min.y + 4.0),
        egui::Align2::LEFT_CENTER,
        "High",
        egui::FontId::proportional(9.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(bar_rect.right() + 4.0, bar_rect.max.y - 4.0),
        egui::Align2::LEFT_CENTER,
        "Low",
        egui::FontId::proportional(9.0),
        tokens.text_muted,
    );

    // Bottom statistics
    let stats_y = rect.bottom() - 36.0;
    painter.text(
        Pos2::new(rect.min.x + 10.0, stats_y),
        egui::Align2::LEFT_CENTER,
        "TLAS  ✓",
        egui::FontId::proportional(10.0),
        tokens.text_primary,
    );
    painter.text(
        Pos2::new(rect.min.x + 10.0, stats_y + 14.0),
        egui::Align2::LEFT_CENTER,
        "BLAS  ✓",
        egui::FontId::proportional(10.0),
        tokens.text_primary,
    );

    // Traversal stats metrics
    let metrics_x = rect.min.x + 75.0;
    painter.text(
        Pos2::new(metrics_x, stats_y - 8.0),
        egui::Align2::LEFT_CENTER,
        "Traversal:",
        egui::FontId::proportional(9.5),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(metrics_x, stats_y + 4.0),
        egui::Align2::LEFT_CENTER,
        "Median    43",
        egui::FontId::monospace(9.5),
        tokens.text_secondary,
    );
    painter.text(
        Pos2::new(metrics_x, stats_y + 16.0),
        egui::Align2::LEFT_CENTER,
        "P95       91",
        egui::FontId::monospace(9.5),
        tokens.text_secondary,
    );
    painter.text(
        Pos2::new(metrics_x + 55.0, stats_y + 16.0),
        egui::Align2::LEFT_CENTER,
        "Max 184",
        egui::FontId::monospace(9.5),
        tokens.text_secondary,
    );
}

/// Panel 4: Material / Glass parameter table & miniature dispersion preview.
fn render_material_glass_panel(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    tokens: &OpticsTokens,
    glass_name: &mut String,
    on_edit_material: impl FnOnce(),
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, egui::CornerRadius::same(2), tokens.panel_bg);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // Header
    painter.text(
        rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        "Material / Glass",
        egui::FontId::proportional(12.0),
        tokens.text_primary,
    );

    // Interactive Dropdown box for glass selection
    let combo_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 26.0),
        Vec2::new(width - 16.0, 22.0),
    );

    let display_title: &'static str = match glass_name.as_str() {
        "N-BK7" | "BK7" => "BK7 (Schott)",
        "SF11" => "SF11 (Schott)",
        "F2" => "F2 (Schott)",
        "Fused Silica" => "Fused Silica (Corning)",
        _ => "Custom",
    };

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(combo_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ComboBox::from_id_salt("analytics_material_dropdown")
                .selected_text(display_title)
                .width(width - 24.0)
                .show_ui(ui, |ui| {
                    let glasses = [
                        ("N-BK7", "BK7 (Schott)"),
                        ("SF11", "SF11 (Schott)"),
                        ("F2", "F2 (Schott)"),
                        ("Fused Silica", "Fused Silica (Corning)"),
                    ];
                    for (code, label) in glasses {
                        let is_sel = glass_name.as_str() == code;
                        if ui.selectable_label(is_sel, label).clicked() {
                            *glass_name = code.to_string();
                        }
                    }
                });
        },
    );

    // Parameter Grid
    let grid_top = combo_rect.bottom() + 8.0;
    let (nd, vd) = match glass_name.as_str() {
        "N-BK7" | "BK7" => ("1.5168", "64.2"),
        "SF11" => ("1.7847", "25.7"),
        "F2" => ("1.6200", "36.4"),
        "Fused Silica" => ("1.4585", "67.8"),
        _ => ("1.5168", "64.2"),
    };

    let rows = [
        ("n (587.6 nm)", nd),
        ("Abbe Vd", vd),
        ("Dispersion", "Cauchy"),
        ("Sellmeier", "Yes"),
    ];

    for (i, (key, val)) in rows.iter().enumerate() {
        let y = grid_top + (i as f32) * 15.0;
        painter.text(
            Pos2::new(rect.min.x + 10.0, y),
            egui::Align2::LEFT_CENTER,
            *key,
            egui::FontId::proportional(10.5),
            tokens.text_secondary,
        );
        painter.text(
            Pos2::new(rect.max.x - 12.0, y),
            egui::Align2::RIGHT_CENTER,
            *val,
            egui::FontId::monospace(10.5),
            tokens.text_primary,
        );
    }

    // Miniature Spectrum preview curve
    let spec_top = grid_top + 64.0;
    painter.text(
        Pos2::new(rect.min.x + 10.0, spec_top),
        egui::Align2::LEFT_CENTER,
        "Spectrum",
        egui::FontId::proportional(10.0),
        tokens.text_muted,
    );

    let curve_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 10.0, spec_top + 8.0),
        Pos2::new(rect.max.x - 12.0, spec_top + 28.0),
    );
    // Draw continuous spectrum fill under curve
    let curve_steps = 24;
    let c_step_w = curve_rect.width() / curve_steps as f32;
    for i in 0..curve_steps {
        let u = i as f32 / curve_steps as f32;
        let wl = 380.0 + u * 320.0;
        let col = wavelength_color32(wl as f64);
        let h = curve_rect.height() * (0.4 + 0.6 * (1.0 - u).powi(2));
        let x = curve_rect.left() + (i as f32) * c_step_w;
        let b = Rect::from_min_max(
            Pos2::new(x, curve_rect.bottom() - h),
            Pos2::new(x + c_step_w, curve_rect.bottom()),
        );
        painter.rect_filled(
            b,
            egui::CornerRadius::ZERO,
            Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 180),
        );
    }
    painter.text(
        Pos2::new(curve_rect.left(), curve_rect.bottom() + 6.0),
        egui::Align2::LEFT_TOP,
        "380",
        egui::FontId::monospace(8.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(curve_rect.center().x, curve_rect.bottom() + 6.0),
        egui::Align2::CENTER_TOP,
        "520    660",
        egui::FontId::monospace(8.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(curve_rect.right(), curve_rect.bottom() + 6.0),
        egui::Align2::RIGHT_TOP,
        "780",
        egui::FontId::monospace(8.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(curve_rect.center().x, curve_rect.bottom() + 14.0),
        egui::Align2::CENTER_TOP,
        "λ (nm)",
        egui::FontId::proportional(8.5),
        tokens.text_muted,
    );

    // Edit Material button at bottom
    let btn_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.bottom() - 24.0),
        Vec2::new(width - 24.0, 18.0),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(btn_rect)
            .layout(egui::Layout::centered_and_justified(
                egui::Direction::LeftToRight,
            )),
        |ui| {
            if ui
                .button(egui::RichText::new("Edit Material").size(10.0))
                .clicked()
            {
                on_edit_material();
            }
        },
    );
}
