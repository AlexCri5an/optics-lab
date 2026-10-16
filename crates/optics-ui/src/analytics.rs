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

    painter.rect_filled(rect, egui::CornerRadius::same(2), tokens.panel_bg);
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // Header bar
    painter.text(
        rect.min + Vec2::new(8.0, 12.0),
        egui::Align2::LEFT_CENTER,
        "Convergence",
        egui::FontId::proportional(12.0),
        tokens.text_primary,
    );

    // Empty state container
    let inner_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 28.0),
        Pos2::new(rect.max.x - 8.0, rect.bottom() - 8.0),
    );
    painter.rect_filled(
        inner_rect,
        egui::CornerRadius::same(2),
        tokens.panel_secondary,
    );
    painter.rect_stroke(
        inner_rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border_subtle),
        egui::StrokeKind::Inside,
    );

    painter.text(
        inner_rect.center() - Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "No data. Render not started.",
        egui::FontId::proportional(11.5),
        tokens.text_secondary,
    );
    painter.text(
        inner_rect.center() + Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "Monte Carlo RMSE tracking activates during path tracing.",
        egui::FontId::proportional(10.0),
        tokens.text_disabled,
    );
}

/// Panel 2: Spectral Power (empty state when no ray is selected).
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

    let inner_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 28.0),
        Pos2::new(rect.max.x - 8.0, rect.bottom() - 8.0),
    );
    painter.rect_filled(
        inner_rect,
        egui::CornerRadius::same(2),
        tokens.panel_secondary,
    );
    painter.rect_stroke(
        inner_rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border_subtle),
        egui::StrokeKind::Inside,
    );

    painter.text(
        inner_rect.center() - Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "Select a ray to inspect spectrum.",
        egui::FontId::proportional(11.5),
        tokens.text_secondary,
    );
    painter.text(
        inner_rect.center() + Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "Wavelength energy readout activates upon ray selection.",
        egui::FontId::proportional(10.0),
        tokens.text_disabled,
    );
}

/// Panel 3: BVH Diagnostics (empty state when no GPU path tracer is active).
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

    let inner_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 28.0),
        Pos2::new(rect.max.x - 8.0, rect.bottom() - 8.0),
    );
    painter.rect_filled(
        inner_rect,
        egui::CornerRadius::same(2),
        tokens.panel_secondary,
    );
    painter.rect_stroke(
        inner_rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border_subtle),
        egui::StrokeKind::Inside,
    );

    painter.text(
        inner_rect.center() - Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "No active BVH traversal.",
        egui::FontId::proportional(11.5),
        tokens.text_secondary,
    );
    painter.text(
        inner_rect.center() + Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "Acceleration structure telemetry activates during GPU ray traversal.",
        egui::FontId::proportional(10.0),
        tokens.text_disabled,
    );
}

/// Panel 4: Material / Glass parameter table & real dispersion preview.
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

    // Physical Glass Material parameters from optics-glass
    let material = match glass_name.as_str() {
        "N-BK7" | "BK7" => optics_glass::GlassMaterial::n_bk7(),
        "SF11" => optics_glass::GlassMaterial::sf11(),
        "F2" => optics_glass::GlassMaterial::f2(),
        "Fused Silica" => optics_glass::GlassMaterial::fused_silica(),
        _ => optics_glass::GlassMaterial::n_bk7(),
    };

    let grid_top = combo_rect.bottom() + 8.0;
    let nd_str = format!("{:.5}", material.nd);
    let vd_str = format!("{:.2}", material.vd);
    let model_str = match material.dispersion {
        optics_glass::material::DispersionModel::Sellmeier1 { .. } => "Sellmeier 1",
        optics_glass::material::DispersionModel::Sellmeier2 { .. } => "Sellmeier 2",
        optics_glass::material::DispersionModel::Cauchy { .. } => "Cauchy",
        optics_glass::material::DispersionModel::Constant { .. } => "Constant",
        optics_glass::material::DispersionModel::Air => "Air",
    };
    let cat_str = material.catalog.name();

    let rows = [
        ("n (587.6 nm)", nd_str.as_str()),
        ("Abbe Vd", vd_str.as_str()),
        ("Dispersion", model_str),
        ("Catalog", cat_str),
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

    // Physical Dispersion curve preview n(λ)
    let spec_top = grid_top + 64.0;
    painter.text(
        Pos2::new(rect.min.x + 10.0, spec_top),
        egui::Align2::LEFT_CENTER,
        "Dispersion n(λ)",
        egui::FontId::proportional(10.0),
        tokens.text_muted,
    );

    let curve_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 10.0, spec_top + 8.0),
        Pos2::new(rect.max.x - 12.0, spec_top + 28.0),
    );

    // Compute refractive index across 380 nm to 780 nm
    let curve_steps = 24;
    let c_step_w = curve_rect.width() / curve_steps as f32;
    let n_380 = material.refractive_index(380.0);
    let n_780 = material.refractive_index(780.0);
    let n_range = (n_380 - n_780).abs().max(0.001);

    for i in 0..curve_steps {
        let u = i as f32 / curve_steps as f32;
        let wl = 380.0 + (u as f64) * 400.0;
        let col = wavelength_color32(wl);
        let n_val = material.refractive_index(wl);
        // Normalize n to [0.2, 1.0] of curve height
        let norm_h = 0.2 + 0.8 * ((n_val - n_780) / n_range).clamp(0.0, 1.0) as f32;
        let h = curve_rect.height() * norm_h;
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
        "588",
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
