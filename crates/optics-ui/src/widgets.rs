//! Technical UI widgets: drag-to-edit scrub fields, dispersion curve plots, and Abbe diagrams.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
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

/// Renders a classic button combining an icon and a text label.
pub fn icon_text_button(
    ui: &mut Ui,
    icon: crate::icons::IconId,
    text: &str,
    tokens: &OpticsTokens,
    is_selected: bool,
) -> Response {
    let padding = Vec2::new(8.0, 4.0);
    let icon_size = 14.0;
    let font_id = FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), font_id, tokens.text_primary);
    let desired_width = padding.x * 2.0 + icon_size + 6.0 + galley.rect.width();
    let desired_height = 24.0;

    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(desired_width, desired_height), Sense::click());
    if ui.is_rect_visible(rect) {
        let (bg, border, fg) = if resp.is_pointer_button_down_on() {
            (tokens.active_bg, tokens.border_strong, tokens.text_primary)
        } else if is_selected {
            (tokens.selected, tokens.border_strong, tokens.text_primary)
        } else if resp.hovered() {
            (tokens.hover_bg, tokens.border, tokens.text_primary)
        } else {
            (tokens.panel_bg, tokens.border, tokens.text_secondary)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(2), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );

        if resp.has_focus() {
            painter.rect_stroke(
                rect,
                CornerRadius::same(2),
                Stroke::new(1.0, tokens.focus_ring),
                StrokeKind::Inside,
            );
        }

        let icon_rect = Rect::from_min_size(
            Pos2::new(rect.left() + padding.x, rect.center().y - icon_size * 0.5),
            Vec2::splat(icon_size),
        );
        crate::icons::draw_icon(painter, icon_rect, icon, fg, icon_size);

        let text_pos = Pos2::new(icon_rect.right() + 6.0, rect.center().y);
        painter.text(
            text_pos,
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            fg,
        );
    }

    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), is_selected, text)
    });

    resp
}

/// Renders a classic toggle tool button with active indicator state.
pub fn toggle_tool_button(
    ui: &mut Ui,
    icon: crate::icons::IconId,
    tokens: &OpticsTokens,
    is_active: bool,
    tooltip: &str,
) -> Response {
    crate::icons::toolbar_button(ui, icon, tokens, is_active, tooltip)
}

/// Configuration parameters for a classic numeric slider widget.
#[derive(Debug, Clone, Copy)]
pub struct ClassicSliderConfig<'a> {
    /// Slider label text.
    pub label: &'a str,
    /// Minimum allowed value.
    pub min: f64,
    /// Maximum allowed value.
    pub max: f64,
    /// Default reset value (triggered by double-clicking).
    pub default_val: f64,
    /// Numeric step increment.
    pub step: f64,
    /// Unit suffix appended to readout (e.g. "°", " mm", " nm").
    pub unit_suffix: &'a str,
}

/// Renders a classic slider with numeric value box and unit suffix.
///
/// Supports:
/// - Double-click on slider or value box resets to `default_val`
/// - Keyboard arrow Left/Right steps by `step`
/// - Shift key modifies step to `step * 0.1` for fine precision adjustment
pub fn classic_slider(
    ui: &mut Ui,
    config: ClassicSliderConfig<'_>,
    value: &mut f64,
    tokens: &OpticsTokens,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(config.label).color(tokens.text_secondary));

        let slider_resp = ui.add(
            egui::Slider::new(value, config.min..=config.max)
                .step_by(config.step)
                .suffix(config.unit_suffix)
                .show_value(true),
        );

        if slider_resp.changed() {
            changed = true;
        }

        // Double-click resets to default
        if slider_resp.double_clicked() {
            *value = config.default_val;
            changed = true;
        }

        // Keyboard fine control when focused
        if slider_resp.has_focus() {
            let fine_step = if ui.input(|i| i.modifiers.shift) {
                config.step * 0.1
            } else {
                config.step
            };

            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                *value = (*value - fine_step).clamp(config.min, config.max);
                changed = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                *value = (*value + fine_step).clamp(config.min, config.max);
                changed = true;
            }
        }
    });

    changed
}

/// Classic rectangular tabs component with 1px border and crisp state feedback.
pub fn classic_tabs<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    tabs: &[(T, &str)],
    tokens: &OpticsTokens,
) -> bool {
    let mut changed = false;
    let mut next_val = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

        for (idx, &(val, label)) in tabs.iter().enumerate() {
            let is_selected = *current == val;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(80.0, 24.0), Sense::click());

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                let (bg, border, text_col) = if is_selected {
                    (tokens.selected, tokens.border_strong, tokens.text_primary)
                } else if resp.hovered() {
                    (tokens.hover_bg, tokens.border, tokens.text_primary)
                } else {
                    (tokens.panel_bg, tokens.border, tokens.text_muted)
                };

                painter.rect_filled(rect, CornerRadius::ZERO, bg);
                painter.rect_stroke(
                    rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, border),
                    StrokeKind::Inside,
                );

                if resp.has_focus() {
                    painter.rect_stroke(
                        rect,
                        CornerRadius::ZERO,
                        Stroke::new(1.0, tokens.focus_ring),
                        StrokeKind::Inside,
                    );
                }

                // Accent top stripe for active tab
                if is_selected {
                    let stripe = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 2.0));
                    painter.rect_filled(stripe, CornerRadius::ZERO, tokens.accent);
                }

                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(11.5),
                    text_col,
                );
            }

            resp.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    ui.is_enabled(),
                    is_selected,
                    label,
                )
            });

            // Keyboard navigation between tabs
            if resp.has_focus() {
                if ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowUp)
                }) && idx > 0
                {
                    next_val = Some(tabs[idx - 1].0);
                }
                if ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown)
                }) && idx + 1 < tabs.len()
                {
                    next_val = Some(tabs[idx + 1].0);
                }
            }

            if (resp.clicked()
                || (resp.has_focus()
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                    })))
                && !is_selected
            {
                next_val = Some(val);
            }
        }
    });

    if let Some(nv) = next_val {
        *current = nv;
        changed = true;
    }

    changed
}

/// Classic CAD/scientific fieldset group box with embedded title header.
pub fn classic_group_box<R>(
    ui: &mut Ui,
    title: &str,
    tokens: &OpticsTokens,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let margin = Vec2::new(8.0, 10.0);
    let title_font = FontId::proportional(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(title.into(), title_font, tokens.text_secondary);

    let start_pos = ui.cursor().min;
    let initial_width = ui.available_width();

    let response = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(margin.x as i8, margin.y as i8))
        .show(ui, |ui| {
            ui.add_space(6.0);
            add_contents(ui)
        });

    let frame_rect = Rect::from_min_size(
        start_pos,
        Vec2::new(initial_width, response.response.rect.height()),
    );
    let painter = ui.painter();

    // 1px fieldset border
    painter.rect_stroke(
        frame_rect,
        CornerRadius::same(2),
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    // Title cutout background & text
    let title_rect = Rect::from_min_size(
        Pos2::new(
            frame_rect.left() + 10.0,
            frame_rect.top() - galley.rect.height() * 0.5,
        ),
        galley.rect.size() + Vec2::new(6.0, 0.0),
    );
    painter.rect_filled(title_rect, CornerRadius::ZERO, tokens.panel_bg);
    painter.text(
        title_rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(11.0),
        tokens.text_secondary,
    );

    response.inner
}

/// Message severity level for classic banners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerLevel {
    /// Informational notice.
    Info,
    /// Warning of edge states or parameter limits.
    Warning,
    /// Numerical or physical error.
    Error,
}

/// Renders a flat classic banner notice with 1px border and sentence case text.
pub fn classic_banner(
    ui: &mut Ui,
    level: BannerLevel,
    message: &str,
    tokens: &OpticsTokens,
) -> Response {
    let (icon, col) = match level {
        BannerLevel::Info => (crate::icons::IconId::Info, tokens.accent),
        BannerLevel::Warning => (crate::icons::IconId::Warning, tokens.status_warning),
        BannerLevel::Error => (crate::icons::IconId::Error, tokens.status_error),
    };

    let desired_height = 28.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), desired_height),
        Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = match level {
            BannerLevel::Info => tokens.hover_bg,
            BannerLevel::Warning => Color32::from_rgba_unmultiplied(220, 150, 40, 20),
            BannerLevel::Error => Color32::from_rgba_unmultiplied(220, 50, 50, 20),
        };

        painter.rect_filled(rect, CornerRadius::same(2), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0, col),
            StrokeKind::Inside,
        );

        let icon_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 8.0, rect.center().y - 7.0),
            Vec2::splat(14.0),
        );
        crate::icons::draw_icon(painter, icon_rect, icon, col, 14.0);

        painter.text(
            Pos2::new(icon_rect.right() + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            message,
            FontId::proportional(12.0),
            tokens.text_primary,
        );
    }

    resp
}

/// Renders a table with a fixed header bar and striped data rows.
pub fn classic_table<R>(
    ui: &mut Ui,
    headers: &[&str],
    tokens: &OpticsTokens,
    add_rows: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.vertical(|ui| {
        // Fixed header row
        let header_height = 22.0;
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), header_height),
            Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, tokens.panel_bg);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, tokens.border_strong),
        );

        let col_w = rect.width() / headers.len() as f32;
        for (i, &h) in headers.iter().enumerate() {
            let col_x = rect.left() + i as f32 * col_w + 6.0;
            painter.text(
                Pos2::new(col_x, rect.center().y),
                Align2::LEFT_CENTER,
                h,
                FontId::proportional(11.0),
                tokens.text_primary,
            );
        }

        // Data rows
        add_rows(ui)
    })
    .inner
}

/// Renders an item in the status bar with optional hover hint and tooltip.
pub fn status_bar_item(
    ui: &mut Ui,
    text: &str,
    tokens: &OpticsTokens,
    tooltip: Option<&str>,
) -> Response {
    let font_id = FontId::proportional(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), font_id, tokens.text_secondary);
    let size = Vec2::new(galley.rect.width() + 10.0, 20.0);

    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if resp.hovered() {
            painter.rect_filled(rect, CornerRadius::same(2), tokens.hover_bg);
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(11.0),
            if resp.hovered() {
                tokens.text_primary
            } else {
                tokens.text_secondary
            },
        );
    }

    if let Some(tt) = tooltip {
        resp.on_hover_text(tt)
    } else {
        resp
    }
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

    #[test]
    fn test_banner_level_properties() {
        assert_ne!(BannerLevel::Info, BannerLevel::Error);
        assert_ne!(BannerLevel::Warning, BannerLevel::Error);
    }

    #[test]
    fn test_tabs_selection() {
        let ctx = egui::Context::default();
        let tokens =
            crate::theme::resolve_theme(crate::theme::ThemePreference::ClassicLight, false);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut current = 1;
                let tabs = [(1, "Tab 1"), (2, "Tab 2")];
                classic_tabs(ui, &mut current, &tabs, &tokens);
                assert_eq!(current, 1);
            });
        });
    }
}
