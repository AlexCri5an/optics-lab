//! Technical UI widgets: drag-to-edit scrub fields, dispersion curve plots, and Abbe diagrams.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use optics_glass::{GlassCatalogDb, GlassMaterial};
use optics_spectral::{LAMBDA_C_NM, LAMBDA_D_NM, LAMBDA_F_NM};

use crate::theme::OpticsTokens;

/// A drag-to-edit numeric field with hover feedback and drag sensitivity.
///
/// Returns `true` if the value was modified during this frame.
pub fn scrub_field(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    min: f64,
    max: f64,
    precision: usize,
) -> bool {
    let mut changed = false;
    let text = format!("{:.prec$}", *value, prec = precision);

    ui.horizontal(|ui| {
        ui.label(label);

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(70.0, 18.0), Sense::click_and_drag());

        let is_hovered = response.hovered();
        let is_dragged = response.dragged();

        if is_dragged {
            let delta = response.drag_delta().x as f64;
            let step = (max - min) / 500.0;
            let new_val = (*value + delta * step).clamp(min, max);
            if (new_val - *value).abs() > 1e-12 {
                *value = new_val;
                changed = true;
            }
        }

        let bg_color = if is_dragged {
            Color32::from_rgb(32, 43, 54)
        } else if is_hovered {
            Color32::from_rgb(22, 31, 40)
        } else {
            Color32::from_rgb(16, 24, 32)
        };

        let border_stroke = if is_dragged || is_hovered {
            Stroke::new(1.0, Color32::from_rgb(169, 184, 200))
        } else {
            Stroke::new(1.0, Color32::from_rgb(38, 49, 59))
        };

        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), bg_color);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(2),
            border_stroke,
            StrokeKind::Inside,
        );

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            &text,
            FontId::monospace(11.0),
            Color32::from_rgb(216, 222, 230),
        );
    });

    changed
}

/// Renders a continuous dispersion curve plot $n(\lambda)$ over $380 \dots 780\,\text{nm}$.
pub fn dispersion_curve_plot(
    ui: &mut Ui,
    glass: &GlassMaterial,
    tokens: &OpticsTokens,
    height: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), tokens.panel_bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    let plot_rect = rect.shrink2(Vec2::new(30.0, 16.0));
    if plot_rect.width() <= 10.0 || plot_rect.height() <= 10.0 {
        return response;
    }

    // Evaluate n over 380..780 nm
    let n_min = glass.refractive_index(780.0);
    let n_max = glass.refractive_index(380.0);
    let n_range = (n_max - n_min).max(1e-4);

    let n_to_y = |n: f64| -> f32 {
        let frac = ((n - n_min) / n_range) as f32;
        plot_rect.bottom() - frac * plot_rect.height()
    };

    let lambda_to_x = |lambda: f64| -> f32 {
        let frac = ((lambda - 380.0) / 400.0) as f32;
        plot_rect.left() + frac * plot_rect.width()
    };

    // Grid lines
    let grid_stroke = Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
    for wl in [400.0, 500.0, 600.0, 700.0] {
        let x = lambda_to_x(wl);
        ui.painter().line_segment(
            [
                Pos2::new(x, plot_rect.top()),
                Pos2::new(x, plot_rect.bottom()),
            ],
            grid_stroke,
        );
    }

    // Fraunhofer lines
    let markers = [
        (LAMBDA_F_NM, "F", Color32::from_rgb(70, 130, 240)),
        (LAMBDA_D_NM, "d", Color32::from_rgb(240, 200, 50)),
        (LAMBDA_C_NM, "C", Color32::from_rgb(230, 60, 60)),
    ];

    for (wl, label, col) in markers {
        let x = lambda_to_x(wl);
        let n = glass.refractive_index(wl);
        let y = n_to_y(n);

        ui.painter().line_segment(
            [
                Pos2::new(x, plot_rect.top()),
                Pos2::new(x, plot_rect.bottom()),
            ],
            Stroke::new(1.0, col.gamma_multiply(0.5)),
        );

        ui.painter().circle_filled(Pos2::new(x, y), 3.0, col);
        ui.painter().text(
            Pos2::new(x, plot_rect.top() - 4.0),
            Align2::CENTER_BOTTOM,
            label,
            FontId::proportional(10.0),
            col,
        );
    }

    // Continuous n(lambda) curve
    let steps = 64;
    let mut points = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = i as f64 / (steps - 1) as f64;
        let wl = 380.0 + t * 400.0;
        let n = glass.refractive_index(wl);
        points.push(Pos2::new(lambda_to_x(wl), n_to_y(n)));
    }

    ui.painter().add(egui::Shape::line(
        points,
        Stroke::new(1.5, Color32::from_rgb(0, 212, 245)),
    ));

    // Y-axis labels
    ui.painter().text(
        Pos2::new(plot_rect.left() - 4.0, plot_rect.top()),
        Align2::RIGHT_CENTER,
        format!("{:.3}", n_max),
        FontId::monospace(9.0),
        tokens.text_dim,
    );
    ui.painter().text(
        Pos2::new(plot_rect.left() - 4.0, plot_rect.bottom()),
        Align2::RIGHT_CENTER,
        format!("{:.3}", n_min),
        FontId::monospace(9.0),
        tokens.text_dim,
    );

    response
}

/// Renders a 2D Abbe Diagram scatter plot ($V_d$ vs. $n_d$).
pub fn abbe_diagram(
    ui: &mut Ui,
    catalog: &GlassCatalogDb,
    selected_name: Option<&str>,
    tokens: &OpticsTokens,
    height: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), tokens.panel_bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    let plot_rect = rect.shrink2(Vec2::new(30.0, 20.0));
    if plot_rect.width() <= 10.0 || plot_rect.height() <= 10.0 {
        return response;
    }

    // Inverted Abbe convention: High Vd (Crowns) on left (85..20)
    let vd_to_x = |vd: f64| -> f32 {
        let frac = ((85.0 - vd) / (85.0 - 20.0)) as f32;
        plot_rect.left() + frac.clamp(0.0, 1.0) * plot_rect.width()
    };

    let nd_to_y = |nd: f64| -> f32 {
        let frac = ((nd - 1.45) / (2.05 - 1.45)) as f32;
        plot_rect.bottom() - frac.clamp(0.0, 1.0) * plot_rect.height()
    };

    // Crown / Flint boundary (Vd = 50 or 55)
    let boundary_x = vd_to_x(55.0);
    ui.painter().line_segment(
        [
            Pos2::new(boundary_x, plot_rect.top()),
            Pos2::new(boundary_x, plot_rect.bottom()),
        ],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
    );

    ui.painter().text(
        Pos2::new(boundary_x - 4.0, plot_rect.top() + 8.0),
        Align2::RIGHT_TOP,
        "Crowns",
        FontId::proportional(10.0),
        tokens.text_dim,
    );
    ui.painter().text(
        Pos2::new(boundary_x + 4.0, plot_rect.top() + 8.0),
        Align2::LEFT_TOP,
        "Flints",
        FontId::proportional(10.0),
        tokens.text_dim,
    );

    // Plot catalog glasses as points
    for glass in catalog.all_glasses() {
        if glass.vd < 15.0 || glass.vd > 95.0 || glass.nd < 1.4 || glass.nd > 2.1 {
            continue;
        }

        let is_selected = selected_name.is_some_and(|name| name == glass.name);
        let pt = Pos2::new(vd_to_x(glass.vd), nd_to_y(glass.nd));

        if is_selected {
            // Draw highlight ring
            ui.painter()
                .circle_stroke(pt, 5.0, Stroke::new(1.5, Color32::from_rgb(0, 212, 245)));
            ui.painter()
                .circle_filled(pt, 3.0, Color32::from_rgb(255, 255, 255));
            ui.painter().text(
                Pos2::new(pt.x, pt.y - 8.0),
                Align2::CENTER_BOTTOM,
                &glass.name,
                FontId::proportional(10.0),
                Color32::from_rgb(0, 212, 245),
            );
        } else {
            let col = if glass.vd >= 55.0 {
                Color32::from_rgba_unmultiplied(100, 180, 255, 120) // Crown: soft blue
            } else {
                Color32::from_rgba_unmultiplied(255, 160, 60, 120) // Flint: amber
            };
            ui.painter().circle_filled(pt, 1.5, col);
        }
    }

    // Axis labels
    ui.painter().text(
        Pos2::new(plot_rect.left(), plot_rect.bottom() + 4.0),
        Align2::LEFT_TOP,
        "Vd = 85 (Low Dispersion)",
        FontId::proportional(9.0),
        tokens.text_dim,
    );
    ui.painter().text(
        Pos2::new(plot_rect.right(), plot_rect.bottom() + 4.0),
        Align2::RIGHT_TOP,
        "Vd = 20 (High Dispersion)",
        FontId::proportional(9.0),
        tokens.text_dim,
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_dispersion_curve_evaluates_n_range() -> TestResult {
        let glass = GlassMaterial::n_bk7();
        let n_blue = glass.refractive_index(400.0);
        let n_red = glass.refractive_index(700.0);
        assert!(
            n_blue > n_red,
            "Normal dispersion requires n(blue) > n(red), got n_blue={n_blue}, n_red={n_red}"
        );
        Ok(())
    }
}
