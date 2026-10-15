//! `OpticsLabApp` — the main egui application container for OpticsLab.

use crate::formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
    format_wavelength,
};
use crate::hit_test::find_hovered_ray_index;
use crate::icons::{ToolIcon, draw_tool_icon};
use crate::legend::{draw_wavelength_legend, find_closest_ray_by_x};
use crate::lens_sim::{LensPreset, LensSimulation};
use crate::lens_view::render_lens_system_panel;
use crate::sim::{EdgeState, PrismSimulation, TracedRay2D};
use crate::theme::{OpticsTokens, apply_tokens};
use crate::view_transform::ViewTransform;
use egui::{
    Align, Align2, Color32, Context, CornerRadius, Frame, Key, Layout, MenuBar, Pos2, Rect,
    ScrollArea, Sense, SidePanel, Stroke, StrokeKind, TopBottomPanel, Ui, Vec2, Window,
};

/// Helper to render a dashed line with an egui Painter.
fn draw_dashed_line(
    painter: &egui::Painter,
    p0: Pos2,
    p1: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dir = p1 - p0;
    let total_len = dir.length();
    if total_len < 1e-4 {
        return;
    }
    let unit_dir = dir / total_len;
    let step = dash_len + gap_len;
    let mut dist = 0.0;
    while dist < total_len {
        let start = p0 + unit_dir * dist;
        let end_dist = (dist + dash_len).min(total_len);
        let end = p0 + unit_dir * end_dist;
        painter.line_segment([start, end], stroke);
        dist += step;
    }
}

/// Active user tool in the left toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTool {
    /// Selection tool for rays and geometric elements.
    Select,
    /// Ray inspector probe tool (active by default).
    RayProbe,
}

/// High-level mode of the OpticsLab application workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Interactive 2D prism spectral dispersion experiment (Slice A).
    #[default]
    PrismDispersion,
    /// Canonical lens systems and optical aberration analysis (Slice B).
    LensSystem,
}

/// The top-level OpticsLab application state.
pub struct OpticsLabApp {
    /// Active high-level workbench mode.
    pub app_mode: AppMode,
    /// UI theme design tokens.
    pub tokens: OpticsTokens,
    /// Currently active tool.
    pub active_tool: ActiveTool,
    /// 2D interactive view transform (pan & zoom).
    pub view: ViewTransform,
    /// Optical prism simulation model.
    pub sim: PrismSimulation,
    /// Lens system simulation and aberration analysis model.
    pub lens_sim: Option<LensSimulation>,
    /// Dedicated view transform for lens cross-section layout.
    pub lens_view: ViewTransform,
    /// Index of currently selected ray in `sim.rays`.
    pub selected_ray_index: Option<usize>,
    /// Index of currently hovered ray in `sim.rays`.
    pub hovered_ray_index: Option<usize>,
    /// Whether Fresnel reflection branches are rendered.
    pub show_reflections: bool,
    /// Whether ray trace diagnostic timing is shown in optical readout.
    pub show_timing: bool,
    /// Transient status message when copying table data.
    pub copy_status: Option<String>,
    /// Flag indicating whether the canvas needs an initial auto-fit.
    pub needs_fit: bool,
    /// Whether the About dialog window is open.
    pub show_about: bool,
}

impl Default for OpticsLabApp {
    fn default() -> Self {
        Self {
            app_mode: AppMode::PrismDispersion,
            tokens: OpticsTokens::medium_dark(),
            active_tool: ActiveTool::RayProbe,
            view: ViewTransform::default(),
            sim: PrismSimulation::default(),
            lens_sim: LensSimulation::new(LensPreset::CookeTriplet).ok(),
            lens_view: ViewTransform::default(),
            selected_ray_index: None,
            hovered_ray_index: None,
            show_reflections: true,
            show_timing: false,
            copy_status: None,
            needs_fit: true,
            show_about: false,
        }
    }
}

impl OpticsLabApp {
    /// Creates a new OpticsLab application instance with default settings.
    #[must_use]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Handles keyboard shortcuts across the application.
    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        // S: Select tool
        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.active_tool = ActiveTool::Select;
        }

        // I: Ray probe tool
        if ctx.input(|i| i.key_pressed(Key::I)) {
            self.active_tool = ActiveTool::RayProbe;
        }

        // Esc: Deselect ray
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected_ray_index = None;
        }

        // R or Cmd/Ctrl+0: Reset view
        let cmd_zero =
            ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Num0));
        let key_r = ctx.input(|i| i.key_pressed(Key::R) && !i.modifiers.any());
        if cmd_zero || key_r {
            self.needs_fit = true;
        }
    }

    /// Renders the top menu bar.
    fn render_top_menu(&mut self, ctx: &Context) {
        TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Reset view (R or Cmd+0)").clicked() {
                        self.needs_fit = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_reflections, "Show reflections");
                    ui.checkbox(&mut self.show_timing, "Show timing");
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    let is_lens = self.app_mode == AppMode::LensSystem;
                    if ui
                        .selectable_label(is_lens, "Lens System & Aberrations")
                        .clicked()
                    {
                        self.app_mode = AppMode::LensSystem;
                    }
                    let is_prism = self.app_mode == AppMode::PrismDispersion;
                    if ui.selectable_label(is_prism, "Prism Dispersion").clicked() {
                        self.app_mode = AppMode::PrismDispersion;
                    }
                    ui.label(
                        egui::RichText::new("Workbench:")
                            .small()
                            .color(self.tokens.text_dim),
                    );
                });
            });
        });
    }

    /// Renders the left tool strip with minimalist vector icons.
    fn render_tool_strip(&mut self, ctx: &Context) {
        SidePanel::left("tool_strip")
            .resizable(false)
            .exact_width(48.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);

                    // Select tool (S)
                    let sel_resp = Self::render_tool_button(
                        ui,
                        ToolIcon::Select,
                        self.active_tool == ActiveTool::Select,
                        true,
                        "Select (S)",
                        &self.tokens,
                    );
                    if sel_resp.clicked() {
                        self.active_tool = ActiveTool::Select;
                    }

                    ui.add_space(4.0);

                    // Ray probe tool (I)
                    let probe_resp = Self::render_tool_button(
                        ui,
                        ToolIcon::RayProbe,
                        self.active_tool == ActiveTool::RayProbe,
                        true,
                        "Ray probe (I)",
                        &self.tokens,
                    );
                    if probe_resp.clicked() {
                        self.active_tool = ActiveTool::RayProbe;
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Planned future tools shown disabled
                    Self::render_tool_button(
                        ui,
                        ToolIcon::LensSurface,
                        false,
                        false,
                        "Lens surface tool (Coming later)",
                        &self.tokens,
                    );
                    ui.add_space(4.0);

                    Self::render_tool_button(
                        ui,
                        ToolIcon::LightSource,
                        false,
                        false,
                        "Light source tool (Coming later)",
                        &self.tokens,
                    );
                    ui.add_space(4.0);

                    Self::render_tool_button(
                        ui,
                        ToolIcon::Measure,
                        false,
                        false,
                        "Measure tool (Coming later)",
                        &self.tokens,
                    );
                });
            });
    }

    /// Renders a single tool strip button with custom vector icon and contrast styling.
    fn render_tool_button(
        ui: &mut Ui,
        icon: ToolIcon,
        is_selected: bool,
        is_enabled: bool,
        tooltip: &str,
        tokens: &OpticsTokens,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 34.0), Sense::click());
        let painter = ui.painter_at(rect);

        let (bg_fill, stroke_color, icon_fill) = if !is_enabled {
            (
                Color32::TRANSPARENT,
                Color32::from_rgb(70, 78, 98),
                Color32::TRANSPARENT,
            )
        } else if is_selected {
            (tokens.accent, tokens.canvas, tokens.canvas)
        } else if response.hovered() {
            (
                tokens.hover_bg,
                Color32::WHITE,
                Color32::from_rgba_premultiplied(255, 255, 255, 40),
            )
        } else {
            (Color32::TRANSPARENT, tokens.text_dim, Color32::TRANSPARENT)
        };

        if bg_fill != Color32::TRANSPARENT {
            painter.rect_filled(rect, CornerRadius::same(2), bg_fill);
        }
        if is_selected {
            painter.rect_stroke(
                rect,
                CornerRadius::same(2),
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
        } else if is_enabled && response.hovered() {
            painter.rect_stroke(
                rect,
                CornerRadius::same(2),
                Stroke::new(1.0, tokens.accent),
                StrokeKind::Inside,
            );
        }

        draw_tool_icon(&painter, rect, icon, stroke_color, icon_fill);

        if is_enabled {
            response.on_hover_text(tooltip)
        } else {
            response.on_disabled_hover_text(tooltip)
        }
    }

    /// Renders the right inspector and control panel.
    fn render_right_panel(&mut self, ctx: &Context) {
        SidePanel::right("inspector_panel")
            .default_width(320.0)
            .min_width(280.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Prism controls");
                    ui.add_space(6.0);

                    let mut retrace_needed = false;

                    // Glass selection dropdown with nd readout
                    ui.horizontal(|ui| {
                        ui.label("Glass material");
                        let prev_glass = self.sim.glass_name.clone();
                        let options = [
                            ("N-BK7", "N-BK7 (nd 1.5168)"),
                            ("SF11", "SF11 (nd 1.7847)"),
                            ("F2", "F2 (nd 1.6200)"),
                            ("Fused Silica", "Fused Silica (nd 1.4585)"),
                        ];
                        let current_label = options
                            .iter()
                            .find(|(id, _)| *id == self.sim.glass_name)
                            .map(|(_, label)| *label)
                            .unwrap_or(&self.sim.glass_name);

                        egui::ComboBox::from_id_salt("glass_select")
                            .selected_text(current_label)
                            .show_ui(ui, |ui| {
                                for (id, label) in options {
                                    ui.selectable_value(
                                        &mut self.sim.glass_name,
                                        id.to_string(),
                                        label,
                                    );
                                }
                            });
                        if self.sim.glass_name != prev_glass {
                            retrace_needed = true;
                        }
                    });

                    // Apex angle slider with double-click reset
                    ui.add_space(6.0);
                    ui.label("Apex angle");
                    let apex_resp = ui.add(
                        egui::Slider::new(&mut self.sim.apex_deg, 20.0..=90.0)
                            .suffix("°")
                            .step_by(0.5),
                    );
                    if apex_resp.changed() {
                        retrace_needed = true;
                    }
                    if apex_resp.double_clicked() {
                        self.sim.apex_deg = 60.0;
                        retrace_needed = true;
                    }

                    // Incidence angle slider with double-click reset
                    ui.add_space(6.0);
                    ui.label("Incidence angle");
                    let inc_resp = ui.add(
                        egui::Slider::new(&mut self.sim.incidence_deg, 0.0..=80.0)
                            .suffix("°")
                            .step_by(0.5),
                    );
                    if inc_resp.changed() {
                        retrace_needed = true;
                    }
                    if inc_resp.double_clicked() {
                        self.sim.incidence_deg = 45.0;
                        retrace_needed = true;
                    }

                    // Spectral samples slider with double-click reset
                    ui.add_space(6.0);
                    ui.label("Spectral samples");
                    let sample_resp = ui
                        .add(egui::Slider::new(&mut self.sim.sample_count, 8..=64).suffix(" rays"));
                    if sample_resp.changed() {
                        retrace_needed = true;
                    }
                    if sample_resp.double_clicked() {
                        self.sim.sample_count = 32;
                        retrace_needed = true;
                    }

                    // Reset scene button
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Reset scene").clicked() {
                            self.sim.apex_deg = 60.0;
                            self.sim.incidence_deg = 45.0;
                            self.sim.sample_count = 32;
                            self.sim.glass_name = "N-BK7".to_string();
                            self.selected_ray_index = None;
                            self.needs_fit = true;
                            retrace_needed = true;
                        }
                    });

                    if retrace_needed {
                        self.sim.retrace();
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Readout section in sentence case
                    ui.heading("Optical readout");
                    ui.add_space(4.0);
                    egui::Grid::new("readout_grid")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Helium d index nd");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.monospace(format_refractive_index(self.sim.nd));
                            });
                            ui.end_row();

                            ui.label("Abbe number Vd");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.monospace(format!("{:.2}", self.sim.vd));
                            });
                            ui.end_row();

                            ui.label("Angular spread (380–780 nm)");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if let Some(spread) = self.sim.total_spread_deg {
                                    ui.monospace(format!("{spread:.2}°"));
                                } else {
                                    ui.colored_label(self.tokens.amber, "TIR");
                                }
                            });
                            ui.end_row();

                            if self.show_timing {
                                ui.label("Trace time");
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.monospace(format!("{:.2} ms", self.sim.trace_time_ms));
                                });
                                ui.end_row();
                            }
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Single-ray inspector section
                    ui.heading("Single-ray inspector");
                    ui.add_space(4.0);

                    if let Some(idx) = self.selected_ray_index {
                        if let Some(ray) = self.sim.rays.get(idx) {
                            Self::render_ray_inspector(
                                ui,
                                &self.tokens,
                                ray,
                                &mut self.copy_status,
                            );
                        } else {
                            ui.colored_label(self.tokens.text_dim, "Click a ray to inspect it");
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(16.0);
                            ui.colored_label(self.tokens.text_dim, "Click a ray to inspect it");
                            ui.add_space(16.0);
                        });
                    }
                });
            });
    }

    /// Renders the detailed per-bounce inspector for a selected ray.
    fn render_ray_inspector(
        ui: &mut Ui,
        tokens: &OpticsTokens,
        ray: &TracedRay2D,
        copy_status: &mut Option<String>,
    ) {
        // Ray color swatch and wavelength
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
            ui.painter().rect_filled(
                rect,
                4.0,
                Color32::from_rgb(ray.color_rgb[0], ray.color_rgb[1], ray.color_rgb[2]),
            );
            ui.strong(format_wavelength(ray.wavelength_nm));
            ui.monospace(format!("n = {}", format_refractive_index(ray.n)));
        });

        ui.add_space(4.0);
        if let Some(dev) = ray.deviation_deg {
            ui.horizontal(|ui| {
                ui.label("Total deviation");
                ui.monospace(format!("{dev:.2}°"));
            });
        } else {
            ui.colored_label(tokens.amber, "Total internal reflection occurred");
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Per-bounce interaction table");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Copy table").clicked() {
                    let mut tsv = String::from(
                        "Bounce\tEvent\tn1\tn2\tIncident angle (deg)\tRefracted angle (deg)\tRs (%)\tRp (%)\tTransmission (%)\tCumulative (%)\n",
                    );
                    let mut cum = 1.0_f64;
                    for (i, b) in ray.path_record.bounces.iter().enumerate() {
                        let refr_str = match b.refracted_angle_rad {
                            Some(ang) => format!("{:.2}", ang.to_degrees()),
                            None => "TIR".to_string(),
                        };
                        cum *= b.t_total;
                        tsv.push_str(&format!(
                            "{}\t{}\t{:.4}\t{:.4}\t{:.2}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\n",
                            i + 1,
                            format_bounce_event(b.event),
                            b.n1,
                            b.n2,
                            b.incident_angle_rad.to_degrees(),
                            refr_str,
                            b.rs * 100.0,
                            b.rp * 100.0,
                            b.t_total * 100.0,
                            cum * 100.0,
                        ));
                    }
                    ui.ctx().copy_text(tsv);
                    *copy_status = Some("Table copied to clipboard".to_string());
                }
            });
        });

        if let Some(msg) = copy_status.as_ref() {
            ui.colored_label(tokens.accent, msg);
        }

        ui.add_space(4.0);

        // Readable two-row layout per bounce to fit panel width cleanly
        let mut cumulative_throughput = 1.0_f64;
        for (i, bounce) in ray.path_record.bounces.iter().enumerate() {
            cumulative_throughput *= bounce.t_total;

            Frame::group(ui.style()).show(ui, |ui| {
                // Row 1: Sequence #, Event, Refraction media
                ui.horizontal(|ui| {
                    ui.strong(format!("#{}:", i + 1));
                    ui.colored_label(tokens.accent, format_bounce_event(bounce.event));
                    ui.monospace(format!("{:.3} -> {:.3}", bounce.n1, bounce.n2));
                });

                // Row 2: Angles
                ui.horizontal(|ui| {
                    ui.label("θi:");
                    ui.monospace(format_angle_deg(bounce.incident_angle_rad));
                    ui.label("θt:");
                    if let Some(ang) = bounce.refracted_angle_rad {
                        ui.monospace(format_angle_deg(ang));
                    } else {
                        ui.colored_label(tokens.amber, "TIR");
                    }
                });

                // Row 3: Fresnel efficiencies and cumulative transmission
                ui.horizontal(|ui| {
                    ui.label("Rs:");
                    ui.monospace(format_percent(bounce.rs));
                    ui.label("Rp:");
                    ui.monospace(format_percent(bounce.rp));
                    ui.label("T:");
                    ui.monospace(format_percent(bounce.t_total));
                    ui.label("Cum:");
                    ui.monospace(format_percent(cumulative_throughput));
                });
            });
            ui.add_space(2.0);
        }
    }

    /// Renders the 2D canvas with egui's Painter.
    fn render_canvas(&mut self, ui: &mut Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        let screen_rect = response.rect;

        // Auto-fit bounds on first frame or on Reset view
        if self.needs_fit && screen_rect.width() > 10.0 && screen_rect.height() > 10.0 {
            self.view
                .fit_to_rect(self.sim.bounds_world, screen_rect, 48.0);
            self.needs_fit = false;
        }

        // Handle double-click for Reset view
        if response.double_clicked() {
            self.view
                .fit_to_rect(self.sim.bounds_world, screen_rect, 48.0);
        }

        // Handle Pan: Middle-drag or Alt+Left-drag
        if response.dragged() {
            let (is_middle, is_alt) = ui.input(|i| (i.pointer.middle_down(), i.modifiers.alt));
            if is_middle || is_alt {
                self.view.pan_by_screen_delta(response.drag_delta());
            }
        }

        // Handle Zoom: Cursor-centered scroll wheel
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta.abs() > 0.0 {
            if let Some(cursor) = response.hover_pos() {
                let factor = if scroll_delta > 0.0 { 1.1 } else { 0.909 };
                self.view.zoom_at(factor, cursor, screen_rect);
            }
        }

        // Define legend bar area at the bottom of the canvas
        let legend_width = (screen_rect.width() - 80.0).clamp(200.0, 520.0);
        let legend_rect = Rect::from_min_size(
            Pos2::new(
                screen_rect.center().x - legend_width * 0.5,
                screen_rect.max.y - 38.0,
            ),
            Vec2::new(legend_width, 24.0),
        );

        // Convert ray polylines to screen space for rendering and hit testing
        let mut screen_rays: Vec<Vec<Pos2>> = Vec::with_capacity(self.sim.rays.len());
        for ray in &self.sim.rays {
            let pts: Vec<Pos2> = ray
                .points_world
                .iter()
                .map(|&p| self.view.world_to_screen(p, screen_rect))
                .collect();
            screen_rays.push(pts);
        }

        // Hit-test for hover
        if let Some(cursor) = response.hover_pos() {
            if !legend_rect.contains(cursor) {
                self.hovered_ray_index = find_hovered_ray_index(cursor, &screen_rays, 8.0);
            } else {
                self.hovered_ray_index = None;
            }
        } else {
            self.hovered_ray_index = None;
        }

        // Left-click interaction (selection via canvas or legend)
        if response.clicked() && !ui.input(|i| i.modifiers.alt) {
            if let Some(cursor) = response.hover_pos() {
                if legend_rect.contains(cursor) {
                    if let Some(closest_idx) = find_closest_ray_by_x(
                        cursor.x,
                        legend_rect.min.x,
                        legend_rect.max.x,
                        &self.sim.rays,
                    ) {
                        self.selected_ray_index = Some(closest_idx);
                    }
                } else if let Some(hovered) = self.hovered_ray_index {
                    self.selected_ray_index = Some(hovered);
                } else {
                    self.selected_ray_index = None;
                }
            }
        }

        // --- Painting 2D Scene ---
        // 1. Canvas background
        painter.rect_filled(screen_rect, 0.0, self.tokens.canvas);

        // 2. Translucent Prism Rendering with low-alpha glass tint mapped from nd
        let s_left = self.view.world_to_screen(self.sim.v_left, screen_rect);
        let s_apex = self.view.world_to_screen(self.sim.v_apex, screen_rect);
        let s_right = self.view.world_to_screen(self.sim.v_right, screen_rect);

        let prism_points = vec![s_left, s_apex, s_right];
        let tint_alpha = ((self.sim.nd - 1.4) / 0.5 * 20.0 + 16.0).clamp(14.0, 42.0) as u8;
        let glass_tint = Color32::from_rgba_unmultiplied(32, 44, 64, tint_alpha);
        let glass_outline = Stroke::new(1.2, Color32::from_rgba_unmultiplied(130, 165, 210, 80));

        painter.add(egui::Shape::convex_polygon(
            prism_points,
            glass_tint,
            glass_outline,
        ));

        // Subtle inner edge highlights along refracting faces
        let facet_highlight = Stroke::new(1.0, Color32::from_rgba_unmultiplied(180, 215, 255, 95));
        painter.line_segment([s_left, s_apex], facet_highlight);
        painter.line_segment([s_apex, s_right], facet_highlight);

        // Muted base line
        let base_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(80, 100, 130, 50));
        painter.line_segment([s_left, s_right], base_stroke);

        // 3. Incoming D65 White Light Ray
        let s_origin = self
            .view
            .world_to_screen(self.sim.incoming_origin, screen_rect);
        let s_hit = self
            .view
            .world_to_screen(self.sim.incoming_hit, screen_rect);

        // Crisp white ray segment
        painter.line_segment([s_origin, s_hit], Stroke::new(1.2, Color32::WHITE));

        // Source marker dot at origin
        painter.circle_filled(s_origin, 4.0, Color32::from_rgb(255, 245, 220));
        painter.circle_stroke(s_origin, 4.0, Stroke::new(1.2, Color32::WHITE));

        // Source label
        painter.text(
            Pos2::new(s_origin.x - 8.0, s_origin.y - 12.0),
            Align2::RIGHT_BOTTOM,
            "D65 white light",
            egui::FontId::proportional(11.0),
            self.tokens.text_dim,
        );

        // 4. Fresnel Reflection Branches (if enabled)
        if self.show_reflections {
            for (i, ray) in self.sim.rays.iter().enumerate() {
                let is_selected = self.selected_ray_index == Some(i);
                for branch in &ray.reflected_branches {
                    let b_start = self.view.world_to_screen(branch.start, screen_rect);
                    let b_end = self.view.world_to_screen(branch.end, screen_rect);
                    let alpha_u8 = (branch.alpha * 255.0).clamp(40.0, 255.0) as u8;
                    let branch_color = Color32::from_rgba_unmultiplied(
                        ray.color_rgb[0],
                        ray.color_rgb[1],
                        ray.color_rgb[2],
                        if is_selected { 220 } else { alpha_u8 },
                    );
                    draw_dashed_line(
                        &painter,
                        b_start,
                        b_end,
                        Stroke::new(if is_selected { 1.5 } else { 1.0 }, branch_color),
                        4.0,
                        3.0,
                    );
                }
            }
        }

        // 5. Spectral Fan Rays with soft glow and clear selection states
        for (i, pts) in screen_rays.iter().enumerate() {
            if pts.len() < 2 {
                continue;
            }
            let is_selected = self.selected_ray_index == Some(i);
            let is_hovered = self.hovered_ray_index == Some(i);
            let ray_data = &self.sim.rays[i];

            let r = ray_data.color_rgb[0];
            let g = ray_data.color_rgb[1];
            let b = ray_data.color_rgb[2];
            let core_color = Color32::from_rgb(r, g, b);

            // Layered soft glow underlay + core
            let (glow_width, glow_alpha, core_width) = if is_selected {
                (9.0, 130_u8, 2.6)
            } else if is_hovered {
                (5.5, 80_u8, 2.0)
            } else {
                (3.5, 35_u8, 1.3)
            };

            let glow_color = Color32::from_rgba_unmultiplied(r, g, b, glow_alpha);

            // Draw underlay glow along all segments
            for window in pts.windows(2) {
                painter.line_segment([window[0], window[1]], Stroke::new(glow_width, glow_color));
            }

            // Draw crisp core
            for window in pts.windows(2) {
                painter.line_segment([window[0], window[1]], Stroke::new(core_width, core_color));
            }

            // Selected ray decorations: white-ringed dots at interactions and wavelength badge at end
            if is_selected {
                for &pt in pts {
                    painter.circle_filled(pt, 3.5, core_color);
                    painter.circle_stroke(pt, 3.5, Stroke::new(1.5, Color32::WHITE));
                }

                if let Some(&end_pt) = pts.last() {
                    let badge_text = format!("{:.1} nm", ray_data.wavelength_nm);
                    let font_id = egui::FontId::proportional(11.0);
                    let galley = painter.layout_no_wrap(badge_text, font_id, Color32::WHITE);
                    let badge_rect = Rect::from_min_size(
                        Pos2::new(end_pt.x + 8.0, end_pt.y - galley.size().y * 0.5 - 2.0),
                        Vec2::new(galley.size().x + 8.0, galley.size().y + 4.0),
                    );
                    painter.rect_filled(badge_rect, CornerRadius::same(3), self.tokens.panel_bg);
                    painter.rect_stroke(
                        badge_rect,
                        CornerRadius::same(3),
                        Stroke::new(1.0, self.tokens.accent),
                        StrokeKind::Inside,
                    );
                    painter.galley(
                        Pos2::new(badge_rect.min.x + 4.0, badge_rect.min.y + 2.0),
                        galley,
                        Color32::WHITE,
                    );
                }
            }
        }

        // Hovered ray tooltip near cursor
        if let Some(hovered_idx) = self.hovered_ray_index {
            if let Some(ray) = self.sim.rays.get(hovered_idx) {
                if let Some(cursor) = response.hover_pos() {
                    let tooltip_text = format_wavelength(ray.wavelength_nm);
                    let font_id = egui::FontId::proportional(12.0);
                    let galley = painter.layout_no_wrap(tooltip_text, font_id, Color32::WHITE);
                    let tip_rect = Rect::from_min_size(
                        Pos2::new(cursor.x + 14.0, cursor.y - 18.0),
                        Vec2::new(galley.size().x + 8.0, galley.size().y + 4.0),
                    );
                    painter.rect_filled(tip_rect, CornerRadius::same(4), self.tokens.panel_bg);
                    painter.rect_stroke(
                        tip_rect,
                        CornerRadius::same(4),
                        Stroke::new(1.0, self.tokens.accent),
                        StrokeKind::Inside,
                    );
                    painter.galley(
                        Pos2::new(tip_rect.min.x + 4.0, tip_rect.min.y + 2.0),
                        galley,
                        Color32::WHITE,
                    );
                }
            }
        }

        // 6. Wavelength Legend Bar at the bottom of the canvas
        draw_wavelength_legend(
            &painter,
            legend_rect,
            &self.sim.rays,
            self.selected_ray_index,
            &self.tokens,
        );

        // 7. Edge State Guidance Banner
        match self.sim.detect_edge_state() {
            EdgeState::AllTotalInternalReflection => {
                let banner_rect = Rect::from_center_size(
                    Pos2::new(screen_rect.center().x, screen_rect.min.y + 36.0),
                    Vec2::new(380.0, 36.0),
                );
                painter.rect_filled(banner_rect, CornerRadius::same(6), self.tokens.panel_bg);
                painter.rect_stroke(
                    banner_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.0, self.tokens.amber),
                    StrokeKind::Inside,
                );
                painter.text(
                    banner_rect.center(),
                    Align2::CENTER_CENTER,
                    "Total internal reflection: adjust apex or incidence angle",
                    egui::FontId::proportional(13.0),
                    self.tokens.amber,
                );
            }
            EdgeState::NoRaysTransmitted => {
                let banner_rect = Rect::from_center_size(
                    Pos2::new(screen_rect.center().x, screen_rect.min.y + 36.0),
                    Vec2::new(340.0, 36.0),
                );
                painter.rect_filled(banner_rect, CornerRadius::same(6), self.tokens.panel_bg);
                painter.rect_stroke(
                    banner_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.0, self.tokens.amber),
                    StrokeKind::Inside,
                );
                painter.text(
                    banner_rect.center(),
                    Align2::CENTER_CENTER,
                    "No transmitted rays escape the prism",
                    egui::FontId::proportional(13.0),
                    self.tokens.amber,
                );
            }
            EdgeState::RayMissedPrism => {
                let banner_rect = Rect::from_center_size(
                    Pos2::new(screen_rect.center().x, screen_rect.min.y + 36.0),
                    Vec2::new(320.0, 36.0),
                );
                painter.rect_filled(banner_rect, CornerRadius::same(6), self.tokens.panel_bg);
                painter.rect_stroke(
                    banner_rect,
                    CornerRadius::same(6),
                    Stroke::new(1.0, self.tokens.amber),
                    StrokeKind::Inside,
                );
                painter.text(
                    banner_rect.center(),
                    Align2::CENTER_CENTER,
                    "Ray missed the prism",
                    egui::FontId::proportional(13.0),
                    self.tokens.amber,
                );
            }
            EdgeState::Normal => {}
        }
    }

    /// Renders the About dialog window when opened.
    fn render_about_dialog(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }

        let mut is_open = self.show_about;
        let mut close_clicked = false;
        Window::new("About OpticsLab")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("OpticsLab");
                ui.add_space(4.0);
                ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
                ui.label("License: MIT");
                ui.add_space(6.0);
                ui.label(
                    "An open-source educational optics visualization and analysis tool built with Rust and egui.",
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        close_clicked = true;
                    }
                });
            });
        if close_clicked {
            is_open = false;
        }
        self.show_about = is_open;
    }
}

impl eframe::App for OpticsLabApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        apply_tokens(ctx, &self.tokens);

        self.handle_keyboard_shortcuts(ctx);
        self.render_top_menu(ctx);

        match self.app_mode {
            AppMode::PrismDispersion => {
                self.render_tool_strip(ctx);
                self.render_right_panel(ctx);

                egui::CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ctx, |ui| {
                        self.render_canvas(ui);
                    });
            }
            AppMode::LensSystem => {
                egui::CentralPanel::default()
                    .frame(Frame::NONE)
                    .show(ctx, |ui| {
                        if let Some(sim) = &mut self.lens_sim {
                            render_lens_system_panel(ui, sim, &mut self.lens_view, &self.tokens);
                        } else {
                            ui.label("Lens simulation could not be initialized");
                        }
                    });
            }
        }

        self.render_about_dialog(ctx);
    }
}
