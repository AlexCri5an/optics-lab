//! Academic optical analysis & diagnostics panel.
//!
//! Conforms to classic engineering desktop CAD standards:
//! - **Centered Layout:** Entire instrument panel (plots, tables, tabs) stays centered horizontally on resize.
//! - **Structured 1px Engineering Tables:** High-legibility typography, clear borders, and zero rounded fluff.
//! - **Spot Diagram 2D:** Centered coordinate canvas, Airy diffraction disk, auto-fit framing, and RMS/GEO blur metrics.
//! - **Seidel 3rd-Order Aberrations:** Multi-field evaluations with surface-by-surface contributions matrix.
//! - **Diffraction MTF Curve:** Calibrated frequency grid (0–100 lp/mm) with uniform solid legend lines.
//! - **Paraxial Cardinal Data:** Gaussian first-order constants (EFL, BFL, F/#, pupils, NA).

use std::sync::Arc;

use egui::{
    Button, Color32, CornerRadius, Frame, Grid, Image, Painter, Pos2, Rect, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use optics_core::OpticalPrescription;
use optics_core::analysis::{
    MtfCurve, PupilPattern, SpotDiagram, compute_diffraction_mtf, compute_seidel_coefficients, trace_spot_diagram,
};
use optics_core::prescription::{ParaxialProperties, ResolvedPrescription, calculate_paraxial_properties};
use optics_glass::GlassCatalogDb;

use crate::icons::LucideIcon;
use crate::theme::OpticsTokens;

/// Active diagnostic tab in the right panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisTab {
    /// Geometric spot diagram with Airy disk and RMS metrics.
    #[default]
    SpotDiagram,
    /// 3rd-order Seidel aberration coefficients and chromatic contributions.
    SeidelAberrations,
    /// Diffraction Modulation Transfer Function curve.
    MtfCurve,
    /// First-order Gaussian cardinal constants.
    ParaxialCardinal,
    /// Polarization ray tracing, Stokes vector, DoP, and polarization ellipse.
    Polarization,
    /// Manufacturing tolerancing & Monte Carlo production statistics.
    Tolerancing,
}

/// State of the interactive optical analysis panel.
#[derive(Debug, Clone)]
pub struct AnalysisPanelState {
    /// Currently selected diagnostic tab.
    pub active_tab: AnalysisTab,
    /// Selected field angle in degrees for field-dependent diagnostics (Spot, MTF, Seidel).
    pub selected_field_deg: f64,
    /// Number of concentric hexapolar rings for spot diagram evaluation.
    pub spot_rings: usize,
    /// Visual magnification of spot diagram (pixels per micrometer).
    pub spot_zoom: f32,
    /// Whether to display the theoretical diffraction Airy disk circle.
    pub show_airy_disk: bool,
    /// Input polarization state for polarization tracing: 0 = Unpolarized, 1 = Linear Horizontal (s), 2 = Linear Vertical (p), 3 = Linear 45°, 4 = Circular Right (RCP).
    pub polarization_input_mode: usize,
    /// Request to open the standalone Tolerancing & Monte Carlo modal dialog hub.
    pub open_tolerancing_dialog: bool,
    /// Cached Monte Carlo report for the analysis panel view.
    pub cached_mc_report: Option<optics_tolerance::MonteCarloReport>,
}

impl Default for AnalysisPanelState {
    fn default() -> Self {
        Self {
            active_tab: AnalysisTab::SpotDiagram,
            selected_field_deg: 0.0,
            spot_rings: 5,
            spot_zoom: 3.0,
            show_airy_disk: true,
            polarization_input_mode: 3, // Default Linear 45° to see s/p differential
            open_tolerancing_dialog: false,
            cached_mc_report: None,
        }
    }
}

/// Renders the complete Analysis & Diagnostics panel in classic academic engineering style.
pub fn render_analysis_panel(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    state: &mut AnalysisPanelState,
    tolerancing_state: &mut crate::tolerancing::TolerancingModalState,
    tokens: &OpticsTokens,
) -> Option<crate::tolerancing::TolerancingAction> {
    let mut tolerancing_action = None;

    // 1. Resolve optical prescription against glass catalog
    let resolved = match prescription.resolve(&**catalog) {
        Ok(res) => res,
        Err(err) => {
            render_resolution_error(ui, &err.to_string(), tokens);
            return None;
        }
    };

    let primary_wl = prescription
        .wavelengths_nm
        .get(prescription.primary_wavelength_index)
        .copied()
        .unwrap_or(587.56);

    // Ensure selected_field_deg belongs to prescription's available fields
    let initial_fields = if prescription.field_angles_deg.is_empty() {
        vec![0.0]
    } else {
        let mut f = prescription.field_angles_deg.clone();
        if !f.contains(&0.0) {
            f.insert(0, 0.0);
        }
        f
    };
    if !initial_fields
        .iter()
        .any(|&f| (f - state.selected_field_deg).abs() < 1e-3)
    {
        state.selected_field_deg = *initial_fields.first().unwrap_or(&0.0);
    }

    // 2. Compute first-order cardinal properties
    let paraxial = calculate_paraxial_properties(&resolved, primary_wl).ok();

    // 3. Render Header, Tabs, and Tab Content
    render_cardinal_header(ui, paraxial.as_ref(), prescription, tokens);
    ui.add_space(4.0);

    // Detect if any glass designation used fallback approximation
    let fallback_glasses: Vec<&str> = prescription
        .surfaces
        .iter()
        .filter_map(|s| {
            let g = s.material_name.trim();
            let known = g.is_empty()
                || g.eq_ignore_ascii_case("AIR")
                || catalog.lookup(g).is_some()
                || g.eq_ignore_ascii_case("WATER")
                || g.eq_ignore_ascii_case("H2O")
                || g.eq_ignore_ascii_case("MIRROR")
                || (g.starts_with("N_") && g.contains("_V"));
            if known { None } else { Some(g) }
        })
        .collect();

    if !fallback_glasses.is_empty() {
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Fallback Glass: {} approximated as N-BK7.",
                            fallback_glasses.join(", ")
                        ))
                        .size(10.5)
                        .strong()
                        .color(tokens.text_primary),
                    );
                });
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Material not found in Schott catalog. Geometry and ray tracing work normally, but MTF and chromatic curves reflect the N-BK7 approximation until real glass is set in LDE."
                        )
                        .size(9.5)
                        .color(tokens.text_muted),
                    )
                    .wrap(),
                );
            });
        ui.add_space(4.0);
    }

    render_analysis_tabs(ui, state, tokens);
    ui.add_space(8.0);

    // Active Tab Content
    if state.active_tab == AnalysisTab::Tolerancing {
        tolerancing_action =
            crate::tolerancing::render_tolerancing_panel_view(ui, tolerancing_state, prescription, catalog, tokens);
    } else {
        ScrollArea::vertical()
            .id_salt("analysis_panel_content_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match state.active_tab {
                AnalysisTab::SpotDiagram => {
                    render_spot_diagram_view(ui, &resolved, prescription, primary_wl, state, tokens);
                }
                AnalysisTab::SeidelAberrations => {
                    render_seidel_view(ui, &resolved, prescription, primary_wl, state, tokens);
                }
                AnalysisTab::MtfCurve => {
                    render_mtf_view(ui, &resolved, prescription, primary_wl, state, tokens);
                }
                AnalysisTab::ParaxialCardinal => {
                    render_cardinal_view(ui, paraxial.as_ref(), prescription, primary_wl, tokens);
                }
                AnalysisTab::Polarization => {
                    render_polarization_view(ui, &resolved, prescription, primary_wl, state, tokens);
                }
                AnalysisTab::Tolerancing => unreachable!(),
            });
    }

    tolerancing_action
}

/// Renders a graceful error card when a prescription cannot be resolved.
fn render_resolution_error(ui: &mut Ui, error_msg: &str, tokens: &OpticsTokens) {
    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.label(
                RichText::new("SYSTEM RESOLUTION")
                    .size(11.5)
                    .strong()
                    .monospace()
                    .color(tokens.warning),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Cannot compute diagnostics: {}\nVerify surface radii and glass names.",
                    error_msg
                ))
                .size(11.0)
                .color(tokens.text_muted),
            );
        });
}

/// Renders a classic dense rectangular cardinal header (EFL, BFL, F/#, EPD).
fn render_cardinal_header(
    ui: &mut Ui,
    paraxial: Option<&ParaxialProperties>,
    prescription: &OpticalPrescription,
    tokens: &OpticsTokens,
) {
    let efl_str = paraxial
        .map(|p| {
            if p.efl_mm.is_infinite() {
                "∞ (Afocal)".to_string()
            } else {
                format!("{:.2} mm", p.efl_mm)
            }
        })
        .unwrap_or_else(|| "N/A".to_string());
    let fnum_str = paraxial
        .map(|p| {
            if p.f_number.is_infinite() {
                "∞".to_string()
            } else {
                format!("F/{:.2}", p.f_number)
            }
        })
        .unwrap_or_else(|| "N/A".to_string());
    let epd_str = format!("{:.1} mm", prescription.entrance_pupil_diameter_mm);

    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.columns(3, |cols| {
                cols[0].vertical_centered(|ui| {
                    ui.label(RichText::new("EFL").size(10.0).monospace().color(tokens.text_muted));
                    ui.label(
                        RichText::new(&efl_str)
                            .size(12.5)
                            .strong()
                            .monospace()
                            .color(tokens.accent),
                    );
                });
                cols[1].vertical_centered(|ui| {
                    ui.label(RichText::new("F/#").size(10.0).monospace().color(tokens.text_muted));
                    ui.label(
                        RichText::new(&fnum_str)
                            .size(12.5)
                            .strong()
                            .monospace()
                            .color(tokens.text_primary),
                    );
                });
                cols[2].vertical_centered(|ui| {
                    ui.label(RichText::new("EPD").size(10.0).monospace().color(tokens.text_muted));
                    ui.label(
                        RichText::new(&epd_str)
                            .size(12.5)
                            .strong()
                            .monospace()
                            .color(tokens.text_primary),
                    );
                });
            });
        });
}

/// Renders diagnostic tab buttons across two balanced rows with balanced icons and labels.
fn render_analysis_tabs(ui: &mut Ui, state: &mut AnalysisPanelState, tokens: &OpticsTokens) {
    let rows: [&[(AnalysisTab, &str, LucideIcon)]; 2] = [
        &[
            (AnalysisTab::SpotDiagram, "Spot", LucideIcon::Target),
            (AnalysisTab::SeidelAberrations, "Seidel", LucideIcon::Activity),
            (AnalysisTab::MtfCurve, "MTF", LucideIcon::SlidersHorizontal),
        ],
        &[
            (AnalysisTab::ParaxialCardinal, "Cardinal", LucideIcon::Maximize2),
            (AnalysisTab::Polarization, "Polar", LucideIcon::Compass),
            (AnalysisTab::Tolerancing, "Tol/MC", LucideIcon::BarChart2),
        ],
    ];

    let total_w = ui.available_width();
    let spacing = 3.0;
    let btn_h = 24.0;

    for (row_idx, row_tabs) in rows.iter().enumerate() {
        if row_idx > 0 {
            ui.add_space(3.0);
        }
        let count = row_tabs.len() as f32;
        let btn_w = ((total_w - spacing * (count - 1.0)) / count).floor();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for (tab, label, icon) in *row_tabs {
                let is_selected = state.active_tab == *tab;
                let bg_color = if is_selected {
                    tokens.bg_control_active
                } else {
                    tokens.bg_control
                };
                let text_color = if is_selected { tokens.accent } else { tokens.text_muted };
                let border_stroke = if is_selected {
                    Stroke::new(1.0, tokens.accent)
                } else {
                    tokens.border
                };

                let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
                    .tint(text_color)
                    .fit_to_exact_size(Vec2::splat(12.0));

                let btn = Button::image_and_text(img, RichText::new(*label).size(11.0).strong().color(text_color))
                    .min_size(Vec2::new(btn_w, btn_h))
                    .fill(bg_color)
                    .stroke(border_stroke)
                    .corner_radius(CornerRadius::ZERO);

                if ui.add(btn).clicked() {
                    state.active_tab = *tab;
                }
            }
        });
    }
}

// ===========================================================================
// Tab 1: Spot Diagram View
// ===========================================================================

fn render_spot_diagram_view(
    ui: &mut Ui,
    resolved: &ResolvedPrescription,
    prescription: &OpticalPrescription,
    primary_wl: f64,
    state: &mut AnalysisPanelState,
    tokens: &OpticsTokens,
) {
    let spot_w = ui.available_width();
    let canvas_h = (spot_w * 0.85).clamp(240.0, 360.0);

    // 1. Trace Spot Diagram
    // 1. Field Selection Bar (Wrapping)
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("FIELD:")
                .size(11.0)
                .strong()
                .monospace()
                .color(tokens.text_muted),
        );
        let available_fields = if prescription.field_angles_deg.is_empty() {
            vec![0.0, 10.0, 14.0]
        } else {
            let mut f = prescription.field_angles_deg.clone();
            if !f.contains(&0.0) {
                f.insert(0, 0.0);
            }
            f
        };

        if !available_fields
            .iter()
            .any(|&f| (f - state.selected_field_deg).abs() < 1e-3)
        {
            state.selected_field_deg = *available_fields.first().unwrap_or(&0.0);
        }

        for &field in &available_fields {
            let is_active = (state.selected_field_deg - field).abs() < 1e-3;
            let btn_txt = format!("{:.1}°", field);
            let (bg, stroke, text_color) = if is_active {
                (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
            } else {
                (tokens.bg_control, tokens.border, tokens.text_muted)
            };
            let btn = Button::new(RichText::new(btn_txt).size(11.0).monospace().strong().color(text_color))
                .fill(bg)
                .stroke(stroke)
                .corner_radius(CornerRadius::ZERO);
            if ui.add(btn).clicked() {
                state.selected_field_deg = field;
            }
        }
    });

    ui.add_space(4.0);

    // 2. Trace Spot Diagram at Selected Field
    let spot_res = trace_spot_diagram(
        resolved,
        (0.0, state.selected_field_deg),
        primary_wl,
        PupilPattern::Hexapolar {
            num_rings: state.spot_rings,
        },
    );

    let spot = match spot_res {
        Ok(s) => s,
        Err(err) => {
            Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("SPOT DIAGNOSTIC")
                            .size(11.0)
                            .strong()
                            .monospace()
                            .color(tokens.warning),
                    );
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(RichText::new(format!("{}", err)).size(10.5).color(tokens.text_muted)).wrap(),
                    );
                });
            return;
        }
    };

    // Zoom & scaling controls
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("Fit Spot").size(11.0).monospace())
            .on_hover_text("Auto-scale canvas to fit spot blur and Airy disk")
            .clicked()
        {
            let target_r = (spot.metrics.geo_radius_um as f32)
                .max(spot.metrics.airy_radius_um as f32)
                .max(4.0);
            state.spot_zoom = (spot_w * 0.38 / target_r).clamp(0.4, 40.0);
        }
        if ui.button(RichText::new("+").size(11.5).monospace()).clicked() {
            state.spot_zoom = (state.spot_zoom * 1.3).clamp(0.2, 80.0);
        }
        if ui.button(RichText::new("-").size(11.5).monospace()).clicked() {
            state.spot_zoom = (state.spot_zoom * 0.77).clamp(0.2, 80.0);
        }
        ui.label(
            RichText::new(format!("{:.1} px/µm", state.spot_zoom))
                .size(10.5)
                .monospace()
                .color(tokens.text_muted),
        );
    });

    ui.add_space(6.0);

    // 3. Spot Diagram Canvas (Matches available width)
    let canvas_size = Vec2::new(spot_w, canvas_h);
    let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());

    // Handle drag zoom
    if response.dragged() {
        let delta = response.drag_delta().y;
        state.spot_zoom = (state.spot_zoom * (1.0 - delta * 0.015)).clamp(0.2, 80.0);
    }

    draw_spot_canvas(&painter, response.rect, &spot, state, tokens);

    ui.add_space(8.0);

    // 4. Regime Indicator Card
    let m = &spot.metrics;
    let is_vignetted = m.passed_rays_count == 0;
    let is_diff_lim = m.is_diffraction_limited;
    let (status_color, status_text) = if is_vignetted {
        (tokens.warning, "100% VIGNETTED / CUTOFF (0 RAYS PASSED)")
    } else if is_diff_lim {
        (tokens.accent, "DIFFRACTION LIMITED (RMS <= AIRY)")
    } else {
        (tokens.warning, "ABERRATION LIMITED (RMS > AIRY)")
    };

    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("REGIME:").size(10.5).monospace().color(tokens.text_muted));
                ui.label(
                    RichText::new(status_text)
                        .size(11.0)
                        .strong()
                        .monospace()
                        .color(status_color),
                );
            });
        });

    ui.add_space(4.0);

    // Diagnostic banner if 100% vignetted
    if is_vignetted {
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "Rays at field {:.1}° are physically blocked by lens apertures (100% mechanical vignetting). This is standard for wide-angle systems at extreme boundary angles.",
                            state.selected_field_deg
                        ))
                        .size(10.0)
                        .color(tokens.text_muted),
                    )
                    .wrap(),
                );
            });
        ui.add_space(4.0);
    }

    // 5. Classic Structured Metrics Table
    let rms_str = if is_vignetted || !m.rms_radius_um.is_finite() {
        "— (Blocked)".to_string()
    } else {
        format!("{:.2} µm", m.rms_radius_um)
    };
    let geo_str = if is_vignetted || !m.geo_radius_um.is_finite() {
        "— (Blocked)".to_string()
    } else {
        format!("{:.2} µm", m.geo_radius_um)
    };

    let rows = [
        (
            "RMS Radius:",
            rms_str,
            if is_vignetted { tokens.text_muted } else { tokens.accent },
        ),
        ("GEO Radius:", geo_str, tokens.text_primary),
        ("Airy Radius:", format!("{:.2} µm", m.airy_radius_um), tokens.warning),
        (
            "Pupil Rays:",
            format!(
                "{}/{} ({:.0}%)",
                m.passed_rays_count,
                m.total_rays_traced,
                m.vignetting_factor * 100.0
            ),
            if is_vignetted {
                tokens.warning
            } else {
                tokens.text_muted
            },
        ),
        (
            "Centroid:",
            format!("({:.3}, {:.3}) mm", m.centroid_x_mm, m.centroid_y_mm),
            tokens.text_muted,
        ),
    ];

    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            Grid::new("spot_metrics_grid")
                .num_columns(2)
                .spacing([18.0, 5.0])
                .show(ui, |ui| {
                    for (label, val, color) in rows {
                        ui.label(RichText::new(label).size(11.5).monospace().color(tokens.text_muted));
                        ui.label(RichText::new(val).size(12.0).strong().monospace().color(color));
                        ui.end_row();
                    }
                });
        });
}

/// Paints the 2D spot hit coordinates, crosshairs, concentric rings, and Airy disk.
fn draw_spot_canvas(
    painter: &Painter,
    rect: Rect,
    spot: &SpotDiagram,
    state: &AnalysisPanelState,
    tokens: &OpticsTokens,
) {
    painter.rect_filled(rect, CornerRadius::ZERO, tokens.bg_canvas);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.4)),
        egui::StrokeKind::Inside,
    );

    let center = rect.center();
    let zoom = state.spot_zoom;

    // Crosshairs
    let axis_stroke = Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.5));
    painter.line_segment(
        [Pos2::new(rect.min.x, center.y), Pos2::new(rect.max.x, center.y)],
        axis_stroke,
    );
    painter.line_segment(
        [Pos2::new(center.x, rect.min.y), Pos2::new(center.x, rect.max.y)],
        axis_stroke,
    );

    // Calibration concentric rings (25 µm, 50 µm, 100 µm)
    for &r_um in &[25.0f32, 50.0f32, 100.0f32] {
        let r_px = r_um * zoom;
        if r_px < rect.width() * 0.48 {
            painter.circle_stroke(
                center,
                r_px,
                Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.28)),
            );
        }
    }

    // Theoretical Airy Disk
    if state.show_airy_disk {
        let airy_px = (spot.metrics.airy_radius_um as f32 * zoom).max(1.0);
        if airy_px < rect.width() * 0.48 {
            painter.circle_stroke(center, airy_px, Stroke::new(1.2, tokens.warning));
            painter.text(
                Pos2::new(center.x + airy_px * 0.7, center.y - airy_px * 0.7),
                egui::Align2::LEFT_BOTTOM,
                "Airy",
                egui::FontId::monospace(9.0),
                tokens.warning,
            );
        }
    }

    // Ray hit dots plotted relative to centroid with guaranteed-contrast theme tokens
    let cx_mm = spot.metrics.centroid_x_mm;
    let cy_mm = spot.metrics.centroid_y_mm;

    let dot_color = if state.selected_field_deg.abs() < 1e-3 {
        tokens.plot_axial // High contrast on both dark and light canvas
    } else if state.selected_field_deg < 12.0 {
        tokens.plot_sagittal
    } else {
        tokens.warning
    };

    for pt in &spot.points {
        let dx_um = ((pt.x_mm - cx_mm) * 1000.0) as f32;
        let dy_um = ((pt.y_mm - cy_mm) * 1000.0) as f32;

        let screen_pt = Pos2::new(center.x + dx_um * zoom, center.y - dy_um * zoom);
        if rect.contains(screen_pt) {
            painter.circle_filled(screen_pt, 1.4, dot_color);
        }
    }

    // Scale bar in bottom-left corner
    let bar_um = if zoom > 4.0 {
        10.0f32
    } else if zoom > 1.5 {
        25.0f32
    } else {
        50.0f32
    };
    let bar_px = bar_um * zoom;
    if bar_px > 12.0 && bar_px < rect.width() * 0.45 {
        let bar_start = Pos2::new(rect.min.x + 8.0, rect.max.y - 12.0);
        let bar_end = Pos2::new(bar_start.x + bar_px, bar_start.y);
        let bar_stroke = Stroke::new(1.5, tokens.canvas_fg);
        painter.line_segment([bar_start, bar_end], bar_stroke);
        painter.line_segment(
            [
                Pos2::new(bar_start.x, bar_start.y - 3.0),
                Pos2::new(bar_start.x, bar_start.y + 3.0),
            ],
            bar_stroke,
        );
        painter.line_segment(
            [
                Pos2::new(bar_end.x, bar_end.y - 3.0),
                Pos2::new(bar_end.x, bar_end.y + 3.0),
            ],
            bar_stroke,
        );

        painter.text(
            Pos2::new(bar_start.x + bar_px * 0.5, bar_start.y - 4.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{} µm", bar_um),
            egui::FontId::monospace(9.5),
            tokens.canvas_fg_muted,
        );
    }
}

// ===========================================================================
// Tab 2: Seidel Aberration Matrix View
// ===========================================================================

fn render_seidel_view(
    ui: &mut Ui,
    resolved: &ResolvedPrescription,
    prescription: &OpticalPrescription,
    primary_wl: f64,
    state: &mut AnalysisPanelState,
    tokens: &OpticsTokens,
) {
    // 1. Field Selection for Seidel Evaluation
    let available_fields = if prescription.field_angles_deg.is_empty() {
        vec![0.0, 10.0, 14.0]
    } else {
        let mut f = prescription.field_angles_deg.clone();
        if !f.contains(&0.0) {
            f.insert(0, 0.0);
        }
        f
    };

    if !available_fields
        .iter()
        .any(|&f| (f - state.selected_field_deg).abs() < 1e-3)
    {
        state.selected_field_deg = *available_fields.first().unwrap_or(&0.0);
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("EVAL FIELD:")
                .size(11.0)
                .strong()
                .monospace()
                .color(tokens.text_muted),
        );
        for &f in &available_fields {
            let active = (state.selected_field_deg - f).abs() < 1e-3;
            let (bg, stroke, text_color) = if active {
                (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
            } else {
                (tokens.bg_control, tokens.border, tokens.text_muted)
            };
            let btn = Button::new(
                RichText::new(format!("{:.1}°", f))
                    .size(11.0)
                    .monospace()
                    .strong()
                    .color(text_color),
            )
            .fill(bg)
            .stroke(stroke)
            .corner_radius(CornerRadius::ZERO);
            if ui.add(btn).clicked() {
                state.selected_field_deg = f;
            }
        }
    });

    if state.selected_field_deg.abs() < 1e-3 {
        let has_off_axis = available_fields.iter().any(|&f| f.abs() > 1e-3);
        let note_text = if has_off_axis {
            "Note: On-axis (0.0°), off-axis Coma, Astigmatism, and Distortion are zero by rotational symmetry. Select a non-zero field angle above to view off-axis field aberrations."
        } else {
            "Note: On-axis (0.0°), off-axis Coma, Astigmatism, and Distortion are zero by rotational symmetry. Configure non-zero field angles in System Setup to inspect off-axis aberrations."
        };
        ui.add(egui::Label::new(RichText::new(note_text).size(10.5).color(tokens.text_muted)).wrap());
    }

    ui.add_space(4.0);

    // Compute Seidel at the selected field angle
    let seidel_res = compute_seidel_coefficients(resolved, state.selected_field_deg, primary_wl);
    let seidel = match seidel_res {
        Ok(s) => s,
        Err(err) => {
            Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("SEIDEL DIAGNOSTIC")
                            .size(11.0)
                            .strong()
                            .monospace()
                            .color(tokens.warning),
                    );
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(RichText::new(format!("{}", err)).size(10.5).color(tokens.text_muted)).wrap(),
                    );
                });
            return;
        }
    };

    // 2. Classic Engineering Table: SYSTEM TOTAL SUMS
    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("SEIDEL 3RD-ORDER COEFFICIENTS (SYSTEM TOTALS)")
                    .size(11.0)
                    .strong()
                    .monospace()
                    .color(tokens.text_primary),
            );
        });

    let sums = [
        ("Spherical", "S_I", "W040", seidel.s1_spherical),
        ("Coma", "S_II", "W131", seidel.s2_coma),
        ("Astigmatism", "S_III", "W222", seidel.s3_astigmatism),
        ("Field Curv", "S_IV", "W220", seidel.s4_field_curvature),
        ("Distortion", "S_V", "W311", seidel.s5_distortion),
        ("Axial Color", "C_L", "AXCL", seidel.c1_axial_color),
        ("Lateral Color", "C_T", "TRCL", seidel.c2_lateral_color),
    ];

    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            Grid::new("seidel_sums_grid")
                .num_columns(3)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    // Header
                    ui.label(
                        RichText::new("ABERRATION")
                            .size(10.0)
                            .strong()
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.label(
                        RichText::new("TERM")
                            .size(10.0)
                            .strong()
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.label(
                        RichText::new("VALUE (MM)")
                            .size(10.0)
                            .strong()
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.end_row();

                    for (name, symbol, wave_term, val) in sums {
                        ui.label(RichText::new(name).size(11.0).color(tokens.text_primary));
                        ui.label(
                            RichText::new(format!("{} ({})", symbol, wave_term))
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        let color = if val.abs() < 1e-4 {
                            tokens.accent
                        } else if val.abs() < 0.05 {
                            tokens.text_primary
                        } else {
                            tokens.warning
                        };
                        ui.label(
                            RichText::new(format!("{:+.5} mm", val))
                                .size(11.0)
                                .strong()
                                .monospace()
                                .color(color),
                        );
                        ui.end_row();
                    }
                });
        });

    ui.add_space(8.0);

    // 3. Surface-by-surface contribution table
    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("SURFACE-BY-SURFACE CONTRIBUTIONS (MM)")
                    .size(11.0)
                    .strong()
                    .monospace()
                    .color(tokens.text_primary),
            );
        });

    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(6, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ScrollArea::horizontal()
                .id_salt("seidel_surfaces_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Grid::new("seidel_surfaces_grid")
                        .num_columns(6)
                        .min_col_width(52.0)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("SURF")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            );
                            ui.label(
                                RichText::new("S_I (Sph)")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            )
                            .on_hover_text("Spherical Aberration W040");
                            ui.label(
                                RichText::new("S_II (Coma)")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            )
                            .on_hover_text("Coma W131");
                            ui.label(
                                RichText::new("S_III (Ast)")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            )
                            .on_hover_text("Astigmatism W222");
                            ui.label(
                                RichText::new("S_IV (Curv)")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            )
                            .on_hover_text("Petzval Field Curvature W220");
                            ui.label(
                                RichText::new("S_V (Dist)")
                                    .size(10.0)
                                    .strong()
                                    .monospace()
                                    .color(tokens.text_muted),
                            )
                            .on_hover_text("Distortion W311");
                            ui.end_row();

                            for s in &seidel.surfaces {
                                ui.label(
                                    RichText::new(format!("S{}", s.surface_index))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.accent),
                                );
                                ui.label(
                                    RichText::new(format!("{:+.4}", s.s1_spherical))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.text_primary),
                                );
                                ui.label(
                                    RichText::new(format!("{:+.4}", s.s2_coma))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.text_muted),
                                );
                                ui.label(
                                    RichText::new(format!("{:+.4}", s.s3_astigmatism))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.text_muted),
                                );
                                ui.label(
                                    RichText::new(format!("{:+.4}", s.s4_field_curvature))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.text_muted),
                                );
                                ui.label(
                                    RichText::new(format!("{:+.4}", s.s5_distortion))
                                        .size(10.5)
                                        .monospace()
                                        .color(tokens.text_muted),
                                );
                                ui.end_row();
                            }
                        });
                });
        });
}

// ===========================================================================
// Tab 3: Modulation Transfer Function (MTF) View
// ===========================================================================

fn render_mtf_view(
    ui: &mut Ui,
    resolved: &ResolvedPrescription,
    prescription: &OpticalPrescription,
    primary_wl: f64,
    state: &mut AnalysisPanelState,
    tokens: &OpticsTokens,
) {
    let avail_w = ui.available_width();
    let plot_h = (avail_w * 0.58).clamp(160.0, 260.0);

    // 1. Field Selection Bar for MTF (Wrapping)
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("FIELD:")
                .size(11.0)
                .strong()
                .monospace()
                .color(tokens.text_muted),
        );
        let available_fields = if prescription.field_angles_deg.is_empty() {
            vec![0.0, 10.0, 14.0]
        } else {
            let mut f = prescription.field_angles_deg.clone();
            if !f.contains(&0.0) {
                f.insert(0, 0.0);
            }
            f
        };

        if !available_fields
            .iter()
            .any(|&f| (f - state.selected_field_deg).abs() < 1e-3)
        {
            state.selected_field_deg = *available_fields.first().unwrap_or(&0.0);
        }
        for &field in &available_fields {
            let is_active = (state.selected_field_deg - field).abs() < 1e-3;
            let btn_txt = format!("{:.1}°", field);
            let (bg, stroke, text_color) = if is_active {
                (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
            } else {
                (tokens.bg_control, tokens.border, tokens.text_muted)
            };
            let btn = Button::new(RichText::new(btn_txt).size(11.0).monospace().strong().color(text_color))
                .fill(bg)
                .stroke(stroke)
                .corner_radius(CornerRadius::ZERO);
            if ui.add(btn).clicked() {
                state.selected_field_deg = field;
            }
        }
    });

    ui.add_space(4.0);

    let mtf_res = compute_diffraction_mtf(resolved, 32, state.selected_field_deg, primary_wl);
    let mtf = match mtf_res {
        Ok(m) => m,
        Err(err) => {
            Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("MTF DIAGNOSTIC")
                            .size(11.0)
                            .strong()
                            .monospace()
                            .color(tokens.warning),
                    );
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(RichText::new(format!("{}", err)).size(10.5).color(tokens.text_muted)).wrap(),
                    );
                });
            return;
        }
    };

    // Header bar
    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("DIFFRACTION MTF")
                        .size(11.0)
                        .strong()
                        .monospace()
                        .color(tokens.text_primary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.1}° Field", state.selected_field_deg))
                            .size(10.5)
                            .monospace()
                            .color(tokens.accent),
                    );
                });
            });
        });

    ui.add_space(4.0);

    // MTF Plot
    let (response, painter) = ui.allocate_painter(Vec2::new(avail_w, plot_h), egui::Sense::hover());
    draw_mtf_plot(&painter, response.rect, &mtf, tokens);

    ui.add_space(8.0);

    // Classic Legend & Benchmark Table
    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Uniform solid line segment legend
            ui.horizontal_wrapped(|ui| {
                draw_legend_line(ui, tokens.text_muted);
                ui.label(
                    RichText::new("Diff Limit")
                        .size(10.0)
                        .monospace()
                        .color(tokens.text_muted),
                );

                ui.add_space(6.0);

                draw_legend_line(ui, tokens.plot_tangential);
                ui.label(
                    RichText::new("T (Tang)")
                        .size(10.0)
                        .monospace()
                        .color(tokens.plot_tangential),
                );

                ui.add_space(6.0);

                draw_legend_line(ui, tokens.plot_sagittal);
                ui.label(
                    RichText::new("S (Sagit)")
                        .size(10.0)
                        .monospace()
                        .color(tokens.plot_sagittal),
                );
            });

            ui.separator();

            let mtf_20 = sample_mtf_at_freq(&mtf, 20.0);
            let mtf_50 = sample_mtf_at_freq(&mtf, 50.0);

            Grid::new("mtf_benchmarks_grid")
                .num_columns(2)
                .spacing([18.0, 5.0])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Contrast @ 20 lp/mm:")
                            .size(11.0)
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.label(
                        RichText::new(format!("{:.1}%", mtf_20 * 100.0))
                            .size(12.5)
                            .strong()
                            .monospace()
                            .color(tokens.accent),
                    );
                    ui.end_row();

                    ui.label(
                        RichText::new("Contrast @ 50 lp/mm:")
                            .size(11.0)
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.label(
                        RichText::new(format!("{:.1}%", mtf_50 * 100.0))
                            .size(12.5)
                            .strong()
                            .monospace()
                            .color(tokens.text_primary),
                    );
                    ui.end_row();

                    ui.label(
                        RichText::new("Cutoff Frequency:")
                            .size(11.0)
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.label(
                        RichText::new(format!("{:.1} lp/mm", mtf.cutoff_frequency_lpm))
                            .size(11.5)
                            .monospace()
                            .color(tokens.text_muted),
                    );
                    ui.end_row();
                });
        });
}

/// Paints a uniform solid 14px horizontal line sample for the MTF legend.
fn draw_legend_line(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 10.0), egui::Sense::hover());
    let y = rect.center().y;
    let painter = ui.painter();
    painter.line_segment(
        [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
        Stroke::new(2.0, color),
    );
}

fn sample_mtf_at_freq(mtf: &MtfCurve, target_freq: f64) -> f64 {
    for pt in &mtf.points {
        if pt.spatial_frequency_lpm >= target_freq {
            return (pt.tangential_mtf + pt.sagittal_mtf) * 0.5;
        }
    }
    mtf.points
        .last()
        .map(|p| (p.tangential_mtf + p.sagittal_mtf) * 0.5)
        .unwrap_or(0.0)
}

fn draw_mtf_plot(painter: &Painter, rect: Rect, mtf: &MtfCurve, tokens: &OpticsTokens) {
    painter.rect_filled(rect, CornerRadius::ZERO, tokens.bg_canvas);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.4)),
        egui::StrokeKind::Inside,
    );

    let max_freq = mtf.cutoff_frequency_lpm.clamp(20.0, 120.0);

    let to_plot = |freq: f64, modulation: f64| -> Pos2 {
        let x_norm = (freq / max_freq).clamp(0.0, 1.0) as f32;
        let y_norm = modulation.clamp(0.0, 1.0) as f32;
        Pos2::new(
            rect.min.x + 24.0 + x_norm * (rect.width() - 32.0),
            rect.max.y - 16.0 - y_norm * (rect.height() - 24.0),
        )
    };

    // Horizontal grid lines and labels (0.25, 0.5, 0.75, 1.0)
    let grid_stroke = Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.25));
    for &mod_val in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let p_start = to_plot(0.0, mod_val);
        let p_end = to_plot(max_freq, mod_val);
        painter.line_segment([p_start, p_end], grid_stroke);

        painter.text(
            Pos2::new(rect.min.x + 2.0, p_start.y),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", mod_val),
            egui::FontId::monospace(8.5),
            tokens.canvas_fg_muted,
        );
    }

    // Vertical frequency ticks (0, 25, 50, 75, 100)
    let f_step = (max_freq / 4.0).round().max(10.0);
    let mut f = 0.0;
    while f <= max_freq + 1.0 {
        let p_bot = to_plot(f, 0.0);
        let p_top = to_plot(f, 1.0);
        painter.line_segment([p_bot, p_top], grid_stroke);

        painter.text(
            Pos2::new(p_bot.x, rect.max.y - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{:.0}", f),
            egui::FontId::monospace(8.5),
            tokens.canvas_fg_muted,
        );
        f += f_step;
    }

    // Diffraction Limit Curve (dashed muted)
    let diff_stroke = Stroke::new(1.2, tokens.canvas_fg_muted.gamma_multiply(0.7));
    let mut prev_diff = to_plot(0.0, 1.0);
    for pt in &mtf.points {
        let curr = to_plot(pt.spatial_frequency_lpm, pt.diffraction_limit_mtf);
        painter.line_segment([prev_diff, curr], diff_stroke);
        prev_diff = curr;
    }

    // Tangential Curve (Guaranteed high contrast against all 12 themes)
    let tan_stroke = Stroke::new(1.8, tokens.plot_tangential);
    let mut prev_tan = to_plot(0.0, 1.0);
    for pt in &mtf.points {
        let curr = to_plot(pt.spatial_frequency_lpm, pt.tangential_mtf);
        painter.line_segment([prev_tan, curr], tan_stroke);
        prev_tan = curr;
    }

    // Sagittal Curve (Guaranteed high contrast against all 12 themes)
    let sag_stroke = Stroke::new(1.8, tokens.plot_sagittal);
    let mut prev_sag = to_plot(0.0, 1.0);
    for pt in &mtf.points {
        let curr = to_plot(pt.spatial_frequency_lpm, pt.sagittal_mtf);
        painter.line_segment([prev_sag, curr], sag_stroke);
        prev_sag = curr;
    }
}

// ===========================================================================
// Tab 4: Paraxial Cardinal View
// ===========================================================================

fn render_cardinal_view(
    ui: &mut Ui,
    paraxial: Option<&ParaxialProperties>,
    prescription: &OpticalPrescription,
    primary_wl: f64,
    tokens: &OpticsTokens,
) {
    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("GAUSSIAN PARAXIAL CARDINAL DATA")
                    .size(11.0)
                    .strong()
                    .monospace()
                    .color(tokens.text_primary),
            );
        });

    ui.add_space(4.0);

    let total_track_mm: f64 = prescription.surfaces.iter().map(|s| s.thickness_mm).sum();

    let items = if let Some(p) = paraxial {
        let efl_str = if p.efl_mm.is_infinite() {
            "∞ (Collimated / Afocal)".to_string()
        } else {
            format!("{:.3} mm", p.efl_mm)
        };
        let bfl_str = if p.bfl_mm.is_infinite() {
            "∞ (Image at Infinity)".to_string()
        } else {
            format!("{:.3} mm", p.bfl_mm)
        };
        let fnum_str = if p.f_number.is_infinite() {
            "∞ (Collimated)".to_string()
        } else {
            format!("F/{:.3}", p.f_number)
        };
        let na_str = if p.f_number.is_infinite() {
            "0.0000 (Afocal)".to_string()
        } else {
            format!("{:.4}", (0.5 / p.f_number).min(1.0))
        };
        vec![
            ("Effective Focal Length (EFL):", efl_str),
            ("Back Focal Length (BFL):", bfl_str),
            ("F-Number (F/#):", fnum_str),
            ("Numerical Aperture (NA):", na_str),
            (
                "Entrance Pupil Diameter (EPD):",
                format!("{:.2} mm", prescription.entrance_pupil_diameter_mm),
            ),
            (
                "Entrance Pupil Position (z_ENP):",
                format!("{:+.3} mm", p.entrance_pupil_pos_mm),
            ),
            (
                "Exit Pupil Position (z_EXP):",
                format!("{:+.3} mm", p.exit_pupil_pos_mm),
            ),
            ("Total Track Length:", format!("{:.2} mm", total_track_mm)),
            ("Primary Wavelength:", format!("{:.2} nm", primary_wl)),
            ("Total Surfaces:", format!("{}", prescription.surfaces.len())),
        ]
    } else {
        vec![("Status:", "Paraxial properties unresolvable".to_string())]
    };

    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            Grid::new("cardinal_data_grid")
                .num_columns(2)
                .spacing([18.0, 5.0])
                .show(ui, |ui| {
                    for (label, val) in items {
                        ui.label(RichText::new(label).size(11.0).monospace().color(tokens.text_muted));
                        ui.label(
                            RichText::new(val)
                                .size(11.5)
                                .strong()
                                .monospace()
                                .color(tokens.text_primary),
                        );
                        ui.end_row();
                    }
                });
        });
}

/// Renders the complete Polarization Ray Tracing & Polarimetric Analysis view.
fn render_polarization_view(
    ui: &mut Ui,
    resolved: &ResolvedPrescription,
    prescription: &OpticalPrescription,
    primary_wl: f64,
    state: &mut AnalysisPanelState,
    tokens: &OpticsTokens,
) {
    use optics_core::aiming::trace_polarized_ray;
    use optics_core::polarization::JonesVector;
    use optics_core::types::{Field, Pupil, Ray, Vec3, Wavelength};

    ui.vertical(|ui| {
        // -------------------------------------------------------------------
        // 1. Polarization State Control Bar
        // -------------------------------------------------------------------
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 3.0);
                    ui.label(
                        RichText::new("Input Polarization:")
                            .strong()
                            .size(11.0)
                            .color(tokens.text_primary),
                    );

                    let modes = [
                        (1, "Linear Horizontal (s)"),
                        (2, "Linear Vertical (p)"),
                        (3, "Linear +45°"),
                        (4, "Circular Right (RCP)"),
                    ];

                    for (m_idx, label) in modes {
                        let selected = state.polarization_input_mode == m_idx;
                        if ui.selectable_label(selected, label).clicked() {
                            state.polarization_input_mode = m_idx;
                        }
                    }
                });

                ui.add_space(4.0);

                // Field Selector
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    ui.label(RichText::new("Field Angle:").size(10.5).color(tokens.text_muted));
                    for &f in &prescription.field_angles_deg {
                        let is_sel = (state.selected_field_deg - f).abs() < 1e-4;
                        let lbl = if f == 0.0 {
                            "0.0° (On-Axis)".to_string()
                        } else {
                            format!("{:.1}°", f)
                        };
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            state.selected_field_deg = f;
                        }
                    }
                });
            });

        ui.add_space(8.0);

        // -------------------------------------------------------------------
        // 2. Perform Real Polarized Ray Trace
        // -------------------------------------------------------------------
        let input_jones = match state.polarization_input_mode {
            1 => JonesVector::linear_horizontal(),
            2 => JonesVector::linear_vertical(),
            4 => JonesVector::circular_right(),
            _ => JonesVector::linear_45(),
        };

        // Aim marginal / chief ray at selected field
        let chief_ray_res = optics_core::aiming::trace_aimed_ray(
            resolved,
            Field::AngleDegrees {
                x: 0.0,
                y: state.selected_field_deg,
            },
            Pupil::new(0.0, 0.707).unwrap_or_else(|_| Pupil::center()),
            Wavelength::from_nanometers(primary_wl).unwrap_or(Wavelength::D_LINE),
            optics_core::RayAimingMode::RealIterative,
        );

        let chief_ray = match chief_ray_res {
            Ok(r) => r,
            Err(_) => {
                // Fallback straight on-axis ray
                Ray {
                    origin: Vec3::new(0.0, 0.0, -10.0),
                    direction: Vec3::new(0.0, 0.0, 1.0),
                    wavelength_nm: primary_wl,
                    energy: 1.0,
                }
            }
        };

        let pol_result = match trace_polarized_ray(resolved, &chief_ray, input_jones) {
            Ok(res) => res,
            Err(err) => {
                ui.label(RichText::new(format!("Polarized trace error: {}", err)).color(tokens.warning));
                return;
            }
        };

        // -------------------------------------------------------------------
        // 3. Polarimetric KPI Header Grid (Throughput, DoP, Orientation, Ellipticity)
        // -------------------------------------------------------------------
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.columns(4, |cols| {
                    cols[0].vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Throughput")
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        ui.label(
                            RichText::new(format!("{:.1}%", pol_result.throughput * 100.0))
                                .size(13.0)
                                .strong()
                                .monospace()
                                .color(tokens.accent),
                        );
                    });
                    cols[1].vertical_centered(|ui| {
                        ui.label(RichText::new("DoP").size(10.0).monospace().color(tokens.text_muted));
                        ui.label(
                            RichText::new(format!("{:.2}", pol_result.dop))
                                .size(13.0)
                                .strong()
                                .monospace()
                                .color(tokens.text_primary),
                        );
                    });
                    cols[2].vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Azimuth (ψ)")
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        ui.label(
                            RichText::new(format!("{:+.1}°", pol_result.orientation_deg))
                                .size(13.0)
                                .strong()
                                .monospace()
                                .color(tokens.text_primary),
                        );
                    });
                    cols[3].vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Ellipticity (χ)")
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        ui.label(
                            RichText::new(format!("{:+.1}°", pol_result.ellipticity_deg))
                                .size(13.0)
                                .strong()
                                .monospace()
                                .color(tokens.text_primary),
                        );
                    });
                });
            });

        ui.add_space(8.0);

        // -------------------------------------------------------------------
        // 4. Polarization Ellipse 2D Diagram
        // -------------------------------------------------------------------
        Frame::NONE
            .fill(tokens.bg_canvas)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("POLARIZATION ELLIPSE (TRANSVERSE ELECTRIC FIELD)")
                            .size(10.5)
                            .strong()
                            .monospace()
                            .color(tokens.text_primary),
                    );
                });
                ui.add_space(4.0);

                let canvas_size = Vec2::new(ui.available_width(), 160.0);
                let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::hover());
                let center = response.rect.center();
                let scale: f64 = 60.0;

                // Axes
                painter.line_segment(
                    [
                        Pos2::new((center.x as f64 - scale * 1.15) as f32, center.y),
                        Pos2::new((center.x as f64 + scale * 1.15) as f32, center.y),
                    ],
                    tokens.border,
                );
                painter.line_segment(
                    [
                        Pos2::new(center.x, (center.y as f64 - scale * 1.15) as f32),
                        Pos2::new(center.x, (center.y as f64 + scale * 1.15) as f32),
                    ],
                    tokens.border,
                );

                // Draw analytical electric field ellipse trajectory E(t) = [Re(Ex * e^iwt), Re(Ey * e^iwt)]
                let steps = 72;
                let mut pts = Vec::with_capacity(steps + 1);
                let ex = pol_result.final_jones.ex;
                let ey = pol_result.final_jones.ey;

                let max_amp = (ex.norm_sq() + ey.norm_sq()).sqrt().max(1e-12);
                let norm_scale = scale / max_amp.max(1.0);

                for i in 0..=steps {
                    let theta = (i as f64) * std::f64::consts::TAU / (steps as f64);
                    let cos_t = theta.cos();
                    let sin_t = theta.sin();
                    let e_x = ex.re * cos_t - ex.im * sin_t;
                    let e_y = ey.re * cos_t - ey.im * sin_t;

                    let px = center.x as f64 + e_x * norm_scale;
                    let py = center.y as f64 - e_y * norm_scale; // -y for screen coords
                    pts.push(Pos2::new(px as f32, py as f32));
                }

                for window in pts.windows(2) {
                    painter.line_segment([window[0], window[1]], Stroke::new(1.8, tokens.accent));
                }

                // Orientation axis line
                let psi = pol_result.orientation_deg.to_radians();
                let major_r = scale * 0.95;
                let axis_p1 = Pos2::new(
                    (center.x as f64 - major_r * psi.cos()) as f32,
                    (center.y as f64 + major_r * psi.sin()) as f32,
                );
                let axis_p2 = Pos2::new(
                    (center.x as f64 + major_r * psi.cos()) as f32,
                    (center.y as f64 - major_r * psi.sin()) as f32,
                );
                painter.line_segment([axis_p1, axis_p2], Stroke::new(1.0, Color32::from_rgb(180, 140, 50)));
            });

        ui.add_space(8.0);

        // -------------------------------------------------------------------
        // 5. Surface-by-Surface Polarimetric Breakdown Table
        // -------------------------------------------------------------------
        Frame::NONE
            .fill(tokens.bg_panel)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("SURFACE POLARIZATION CONTRIBUTIONS")
                        .strong()
                        .size(11.0)
                        .color(tokens.text_primary),
                );
                ui.add_space(4.0);

                Grid::new("polarization_table_grid")
                    .num_columns(6)
                    .striped(true)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        // Headers
                        ui.label(RichText::new("Surf").size(10.0).monospace().color(tokens.text_muted));
                        ui.label(RichText::new("AOI (°)").size(10.0).monospace().color(tokens.text_muted));
                        ui.label(RichText::new("Ts (%)").size(10.0).monospace().color(tokens.text_muted));
                        ui.label(RichText::new("Tp (%)").size(10.0).monospace().color(tokens.text_muted));
                        ui.label(
                            RichText::new("Diattenuation")
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        ui.label(
                            RichText::new("Phase Shift Δ")
                                .size(10.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                        ui.end_row();

                        for rec in &pol_result.surface_records {
                            ui.label(
                                RichText::new(format!("S{}", rec.surface_index))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.text_primary),
                            );
                            ui.label(
                                RichText::new(format!("{:.1}°", rec.incident_angle_deg))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.text_primary),
                            );
                            ui.label(
                                RichText::new(format!("{:.1}%", rec.ts * 100.0))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.accent),
                            );
                            ui.label(
                                RichText::new(format!("{:.1}%", rec.tp * 100.0))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.text_primary),
                            );
                            ui.label(
                                RichText::new(format!("{:.3}", rec.diattenuation))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.text_primary),
                            );
                            ui.label(
                                RichText::new(format!("{:+.1}°", rec.phase_shift_deg))
                                    .size(10.5)
                                    .monospace()
                                    .color(tokens.text_muted),
                            );
                            ui.end_row();
                        }
                    });
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_tabs_interaction() {
        let ctx = egui::Context::default();
        let mut state = AnalysisPanelState::default();
        let tokens = crate::theme::ThemeMode::ClassicCad.tokens();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_analysis_tabs(ui, &mut state, &tokens);
            });
        });

        assert_eq!(state.active_tab, AnalysisTab::SpotDiagram);
    }
}
