//! Ray Inspector side panel matching classic desktop standards.
//!
//! Provides comprehensive physical ray-path diagnostics:
//! - Selected ray index and physical wavelength
//! - Spectral sRGB color swatch
//! - Expandable per-bounce physical quantities (n1, n2, angles, Fresnel reflectances)
//! - Vertical path node graph derived directly from the active ray's bounce record

use egui::{Align2, Color32, Pos2, ScrollArea, Stroke, Ui, Vec2};
use optics_core::RayBounce;
use optics_spectral::color::display_color_for_wavelength;

use crate::formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
};
use crate::icons::{IconId, draw_icon};
use crate::sim::{PrismSimulation, TracedRay2D};
use crate::theme::OpticsTokens;

/// State for the Ray Inspector panel.
#[derive(Debug, Clone)]
pub struct RayInspectorState {
    /// Whether Bounce 0 section is expanded.
    pub bounce_0_open: bool,
    /// Whether Bounce 1 section is expanded.
    pub bounce_1_open: bool,
    /// Whether Bounce 2 section is expanded.
    pub bounce_2_open: bool,
    /// Whether the Ray Path timeline is expanded.
    pub path_open: bool,
}

impl Default for RayInspectorState {
    fn default() -> Self {
        Self {
            bounce_0_open: true,
            bounce_1_open: false,
            bounce_2_open: false,
            path_open: true,
        }
    }
}

/// Renders the Right Ray Inspector panel.
pub fn render_ray_inspector_panel(
    ui: &mut Ui,
    sim: &PrismSimulation,
    selected_ray: Option<usize>,
    state: &mut RayInspectorState,
    tokens: &OpticsTokens,
) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);

        // Header: Target reticle icon + Ray Inspector title
        ui.horizontal(|ui| {
            let (icon_rect, _) =
                ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::hover());
            draw_icon(
                ui.painter(),
                icon_rect,
                IconId::RayProbe,
                tokens.text_primary,
                14.0,
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Ray Inspector")
                    .strong()
                    .size(13.0)
                    .color(tokens.text_primary),
            );
        });

        ui.add_space(8.0);

        // Selected ray header
        let ray_idx = selected_ray
            .unwrap_or(0)
            .min(sim.rays.len().saturating_sub(1));
        let active_ray = sim.rays.get(ray_idx);
        let wavelength = active_ray.map_or(587.56, |r| r.wavelength_nm);

        ui.label(
            egui::RichText::new(format!("Ray #{} of {}", ray_idx + 1, sim.rays.len()))
                .size(11.5)
                .color(tokens.text_secondary),
        );

        let color_name = if wavelength < 430.0 {
            "violet"
        } else if wavelength < 480.0 {
            "blue"
        } else if wavelength < 520.0 {
            "cyan"
        } else if wavelength < 570.0 {
            "green"
        } else if wavelength < 600.0 {
            "yellow"
        } else if wavelength < 640.0 {
            "orange"
        } else {
            "red"
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Wavelength = {wavelength:.2} nm ({color_name})"))
                    .size(11.5)
                    .color(tokens.text_primary),
            );
            let rgb = display_color_for_wavelength(wavelength);
            let color_swatch = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let (swatch_rect, _) =
                ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(swatch_rect.center(), 4.0, color_swatch);
            ui.painter()
                .circle_stroke(swatch_rect.center(), 4.5, Stroke::new(1.0, tokens.border));
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        // Section: Bounce 0
        let b0_glyph = if state.bounce_0_open {
            IconId::CaretDown.glyph()
        } else {
            IconId::CaretRight.glyph()
        };
        if ui
            .selectable_label(
                state.bounce_0_open,
                egui::RichText::new(format!("{b0_glyph} Bounce 0 (Entrance)"))
                    .strong()
                    .size(12.0)
                    .color(tokens.text_primary),
            )
            .clicked()
        {
            state.bounce_0_open = !state.bounce_0_open;
        }

        if state.bounce_0_open {
            ui.indent("bounce_0_body", |ui| {
                ui.add_space(4.0);
                let first_bounce = active_ray.and_then(|r| r.path_record.bounces.first());
                render_bounce_table(ui, first_bounce, tokens);
            });
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // Section: Bounce 1
        let b1_glyph = if state.bounce_1_open {
            IconId::CaretDown.glyph()
        } else {
            IconId::CaretRight.glyph()
        };
        if ui
            .selectable_label(
                state.bounce_1_open,
                egui::RichText::new(format!("{b1_glyph} Bounce 1 (Exit)"))
                    .strong()
                    .size(12.0)
                    .color(tokens.text_secondary),
            )
            .clicked()
        {
            state.bounce_1_open = !state.bounce_1_open;
        }

        if state.bounce_1_open {
            ui.indent("bounce_1_body", |ui| {
                ui.add_space(4.0);
                let second_bounce = active_ray.and_then(|r| r.path_record.bounces.get(1));
                render_bounce_table(ui, second_bounce, tokens);
            });
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // Section: Bounce 2
        let b2_glyph = if state.bounce_2_open {
            IconId::CaretDown.glyph()
        } else {
            IconId::CaretRight.glyph()
        };
        if ui
            .selectable_label(
                state.bounce_2_open,
                egui::RichText::new(format!("{b2_glyph} Bounce 2 (Secondary)"))
                    .strong()
                    .size(12.0)
                    .color(tokens.text_secondary),
            )
            .clicked()
        {
            state.bounce_2_open = !state.bounce_2_open;
        }

        if state.bounce_2_open {
            ui.indent("bounce_2_body", |ui| {
                ui.add_space(4.0);
                let third_bounce = active_ray.and_then(|r| r.path_record.bounces.get(2));
                render_bounce_table(ui, third_bounce, tokens);
            });
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // Section: Path (Vertical timeline)
        let path_glyph = if state.path_open {
            IconId::CaretDown.glyph()
        } else {
            IconId::CaretRight.glyph()
        };
        if ui
            .selectable_label(
                state.path_open,
                egui::RichText::new(format!("{path_glyph} Ray Path Timeline"))
                    .strong()
                    .size(12.0)
                    .color(tokens.text_primary),
            )
            .clicked()
        {
            state.path_open = !state.path_open;
        }

        if state.path_open {
            ui.indent("path_body", |ui| {
                ui.add_space(6.0);
                render_vertical_path(ui, active_ray, tokens);
            });
        }
    });
}

/// Renders a two-column physical parameter table for a ray bounce.
fn render_bounce_table(ui: &mut Ui, bounce: Option<&RayBounce>, tokens: &OpticsTokens) {
    if let Some(b) = bounce {
        let rows = [
            ("Surface", format!("{} (prism)", b.surface_index)),
            ("n1", format_refractive_index(b.n1)),
            ("n2", format_refractive_index(b.n2)),
            ("theta_i", format_angle_deg(b.incident_angle_rad)),
            (
                "theta_t",
                b.refracted_angle_rad
                    .map_or_else(|| "TIR".into(), format_angle_deg),
            ),
            ("Rs", format_percent(b.rs)),
            ("Rp", format_percent(b.rp)),
            ("R (total)", format_percent(b.r_total)),
            ("T (total)", format_percent(b.t_total)),
            ("Event", format_bounce_event(b.event).to_string()),
        ];

        egui::Grid::new("bounce_table_grid")
            .num_columns(2)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for (key, val) in rows {
                    ui.label(
                        egui::RichText::new(key)
                            .size(11.0)
                            .color(tokens.text_secondary),
                    );
                    ui.label(
                        egui::RichText::new(val)
                            .monospace()
                            .size(11.0)
                            .color(tokens.text_primary),
                    );
                    ui.end_row();
                }
            });
    } else {
        ui.label(
            egui::RichText::new("No bounce data. Ray did not encounter this interface.")
                .size(11.0)
                .color(tokens.text_dim),
        );
    }
}

/// Renders the vertical ray path graph derived dynamically from the active ray's bounce history.
fn render_vertical_path(ui: &mut Ui, active_ray: Option<&TracedRay2D>, tokens: &OpticsTokens) {
    let mut steps: Vec<String> = Vec::new();

    if let Some(ray) = active_ray {
        steps.push(format!("Emitter (λ = {:.1} nm)", ray.wavelength_nm));
        for (i, bounce) in ray.path_record.bounces.iter().enumerate() {
            steps.push(format!(
                "Surface {} ({})",
                bounce.surface_index + 1,
                format_bounce_event(bounce.event)
            ));
            let _ = i;
        }
        if ray.path_record.terminated {
            steps.push("Terminated / Absorbed".to_string());
        } else {
            steps.push("Transmitted to Observer".to_string());
        }
    } else {
        ui.label(
            egui::RichText::new("No ray selected.")
                .size(11.0)
                .color(tokens.text_dim),
        );
        return;
    }

    let row_height = 24.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), steps.len() as f32 * row_height + 8.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    let line_x = rect.min.x + 12.0;
    let node_color = tokens.accent;
    let stroke = Stroke::new(1.0, tokens.border);

    // Continuous vertical trunk line
    let p_first = Pos2::new(line_x, rect.min.y + 10.0);
    let p_last = Pos2::new(
        line_x,
        rect.min.y + 10.0 + (steps.len() - 1) as f32 * row_height,
    );
    painter.line_segment([p_first, p_last], stroke);

    // Nodes and labels
    for (i, label) in steps.iter().enumerate() {
        let y = rect.min.y + 10.0 + (i as f32) * row_height;
        let node_pt = Pos2::new(line_x, y);

        // Outer ring & filled circle
        painter.circle_filled(node_pt, 3.5, node_color);
        painter.circle_stroke(node_pt, 4.5, Stroke::new(1.0, tokens.panel_bg));

        // Step text
        let font_id = egui::FontId::proportional(11.0);
        painter.text(
            node_pt + Vec2::new(14.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            font_id,
            tokens.text_primary,
        );
    }
}
