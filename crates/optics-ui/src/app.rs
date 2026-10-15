//! `OpticsLabApp` — the main egui application container for OpticsLab.

use crate::formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
    format_wavelength,
};
use crate::hit_test::find_hovered_ray_index;
use crate::sim::PrismSimulation;
use crate::theme::{OpticsTokens, apply_tokens};
use crate::view_transform::ViewTransform;
use egui::{
    Align2, Button, Color32, Context, Frame, Key, MenuBar, Pos2, ScrollArea, Sense, SidePanel,
    Stroke, TopBottomPanel, Ui, Vec2, Window,
};

/// Active user tool in the left toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTool {
    /// Selection tool for rays and geometric elements.
    Select,
    /// Ray inspector probe tool (active by default).
    RayProbe,
}

/// The top-level OpticsLab application state.
pub struct OpticsLabApp {
    /// UI theme design tokens.
    pub tokens: OpticsTokens,
    /// Currently active tool.
    pub active_tool: ActiveTool,
    /// 2D interactive view transform (pan & zoom).
    pub view: ViewTransform,
    /// Optical prism simulation model.
    pub sim: PrismSimulation,
    /// Index of currently selected ray in `sim.rays`.
    pub selected_ray_index: Option<usize>,
    /// Index of currently hovered ray in `sim.rays`.
    pub hovered_ray_index: Option<usize>,
    /// Flag indicating whether the canvas needs an initial auto-fit.
    pub needs_fit: bool,
    /// Whether the About dialog window is open.
    pub show_about: bool,
}

impl Default for OpticsLabApp {
    fn default() -> Self {
        Self {
            tokens: OpticsTokens::medium_dark(),
            active_tool: ActiveTool::RayProbe,
            view: ViewTransform::default(),
            sim: PrismSimulation::default(),
            selected_ray_index: None,
            hovered_ray_index: None,
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
                    if ui.button("Reset view").clicked() {
                        self.needs_fit = true;
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                });
            });
        });
    }

    /// Renders the left tool strip.
    fn render_tool_strip(&mut self, ctx: &Context) {
        SidePanel::left("tool_strip")
            .resizable(false)
            .exact_width(48.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);

                    // Select Tool
                    let sel_btn = Button::new("⇱")
                        .min_size(Vec2::new(34.0, 34.0))
                        .selected(self.active_tool == ActiveTool::Select);
                    if ui.add(sel_btn).on_hover_text("Select tool (S)").clicked() {
                        self.active_tool = ActiveTool::Select;
                    }

                    ui.add_space(4.0);

                    // Ray Probe Tool (Default)
                    let probe_btn = Button::new("⚡")
                        .min_size(Vec2::new(34.0, 34.0))
                        .selected(self.active_tool == ActiveTool::RayProbe);
                    if ui
                        .add(probe_btn)
                        .on_hover_text("Ray probe tool (I)")
                        .clicked()
                    {
                        self.active_tool = ActiveTool::RayProbe;
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Planned future tools shown disabled
                    ui.add_enabled(false, Button::new("⬭").min_size(Vec2::new(34.0, 34.0)))
                        .on_disabled_hover_text("Lens surface tool — Coming later");
                    ui.add_space(4.0);

                    ui.add_enabled(false, Button::new("☀").min_size(Vec2::new(34.0, 34.0)))
                        .on_disabled_hover_text("Light source tool — Coming later");
                    ui.add_space(4.0);

                    ui.add_enabled(false, Button::new("📏").min_size(Vec2::new(34.0, 34.0)))
                        .on_disabled_hover_text("Measure tool — Coming later");
                });
            });
    }

    /// Renders the right inspector and control panel.
    fn render_right_panel(&mut self, ctx: &Context) {
        SidePanel::right("inspector_panel")
            .default_width(320.0)
            .min_width(260.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Prism Controls");
                    ui.add_space(6.0);

                    // Glass selection dropdown
                    let mut retrace_needed = false;
                    ui.horizontal(|ui| {
                        ui.label("Glass material:");
                        let prev_glass = self.sim.glass_name.clone();
                        egui::ComboBox::from_id_salt("glass_select")
                            .selected_text(&self.sim.glass_name)
                            .show_ui(ui, |ui| {
                                for name in &["N-BK7", "SF11", "F2", "Fused Silica"] {
                                    ui.selectable_value(
                                        &mut self.sim.glass_name,
                                        (*name).to_string(),
                                        *name,
                                    );
                                }
                            });
                        if self.sim.glass_name != prev_glass {
                            retrace_needed = true;
                        }
                    });

                    // Apex angle slider
                    ui.add_space(4.0);
                    ui.label("Apex angle:");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.sim.apex_deg, 20.0..=90.0)
                                .suffix("°")
                                .step_by(0.5),
                        )
                        .changed()
                    {
                        retrace_needed = true;
                    }

                    // Incidence angle slider
                    ui.add_space(4.0);
                    ui.label("Incidence angle:");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.sim.incidence_deg, 0.0..=80.0)
                                .suffix("°")
                                .step_by(0.5),
                        )
                        .changed()
                    {
                        retrace_needed = true;
                    }

                    // Sample count slider
                    ui.add_space(4.0);
                    ui.label("Spectral samples:");
                    if ui
                        .add(egui::Slider::new(&mut self.sim.sample_count, 8..=64).text("rays"))
                        .changed()
                    {
                        retrace_needed = true;
                    }

                    if retrace_needed {
                        self.sim.retrace();
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Readout section
                    ui.heading("Optical Readout");
                    ui.add_space(4.0);
                    egui::Grid::new("readout_grid")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Helium d index (nd):");
                            ui.label(format_refractive_index(self.sim.nd));
                            ui.end_row();

                            ui.label("Abbe number (Vd):");
                            ui.label(format!("{:.2}", self.sim.vd));
                            ui.end_row();

                            ui.label("Angular spread (380-780):");
                            if let Some(spread) = self.sim.total_spread_deg {
                                ui.label(format!("{spread:.2}°"));
                            } else {
                                ui.label("N/A (TIR)");
                            }
                            ui.end_row();

                            ui.label("Trace time:");
                            ui.label(format!("{:.2} ms", self.sim.trace_time_ms));
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Single-Ray Inspector Section
                    ui.heading("Single-Ray Inspector");
                    ui.add_space(4.0);

                    if let Some(idx) = self.selected_ray_index {
                        if let Some(ray) = self.sim.rays.get(idx) {
                            Self::render_ray_inspector_static(ui, self.tokens.amber, ray);
                        } else {
                            ui.label("Click a ray to inspect it");
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
    fn render_ray_inspector_static(
        ui: &mut Ui,
        amber_color: Color32,
        ray: &crate::sim::TracedRay2D,
    ) {
        // Ray color swatch and wavelength
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
            ui.painter().rect_filled(
                rect,
                4.0,
                Color32::from_rgb(ray.color_rgb[0], ray.color_rgb[1], ray.color_rgb[2]),
            );
            ui.label(format_wavelength(ray.wavelength_nm));
            ui.label(format!("(n = {})", format_refractive_index(ray.n)));
        });

        ui.add_space(4.0);
        if let Some(dev) = ray.deviation_deg {
            ui.label(format!("Total deviation: {dev:.2}°"));
        } else {
            ui.colored_label(amber_color, "Total internal reflection occurred");
        }

        ui.add_space(8.0);
        ui.label("Per-bounce interaction table:");
        ui.add_space(4.0);

        // Per-bounce table
        ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("bounce_table")
                .striped(true)
                .min_col_width(55.0)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    // Headers
                    ui.strong("#");
                    ui.strong("Event");
                    ui.strong("n1 → n2");
                    ui.strong("θi (°)");
                    ui.strong("θt (°)");
                    ui.strong("Rs (%)");
                    ui.strong("Rp (%)");
                    ui.strong("T (%)");
                    ui.strong("Cum (%)");
                    ui.end_row();

                    let mut cumulative_throughput = 1.0_f64;
                    for (i, bounce) in ray.path_record.bounces.iter().enumerate() {
                        ui.label(format!("{}", i + 1));
                        ui.label(format_bounce_event(bounce.event));
                        ui.label(format!("{:.3} → {:.3}", bounce.n1, bounce.n2));
                        ui.label(format_angle_deg(bounce.incident_angle_rad));
                        if let Some(ang) = bounce.refracted_angle_rad {
                            ui.label(format_angle_deg(ang));
                        } else {
                            ui.label("—");
                        }
                        ui.label(format_percent(bounce.rs));
                        ui.label(format_percent(bounce.rp));
                        ui.label(format_percent(bounce.t_total));

                        cumulative_throughput *= bounce.t_total;
                        ui.label(format_percent(cumulative_throughput));
                        ui.end_row();
                    }
                });
        });
    }

    /// Renders the 2D canvas with egui's Painter.
    fn render_canvas(&mut self, ui: &mut Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        let screen_rect = response.rect;

        // Auto-fit bounds on first frame or on Reset view
        if self.needs_fit && screen_rect.width() > 10.0 && screen_rect.height() > 10.0 {
            self.view
                .fit_to_rect(self.sim.bounds_world, screen_rect, 40.0);
            self.needs_fit = false;
        }

        // Handle double-click for Reset view
        if response.double_clicked() {
            self.view
                .fit_to_rect(self.sim.bounds_world, screen_rect, 40.0);
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
            self.hovered_ray_index = find_hovered_ray_index(cursor, &screen_rays, 8.0);
        } else {
            self.hovered_ray_index = None;
        }

        // Left-click to select ray (if in Select or RayProbe tool and not Alt-panning)
        if response.clicked() && !ui.input(|i| i.modifiers.alt) {
            if let Some(hovered) = self.hovered_ray_index {
                self.selected_ray_index = Some(hovered);
            } else {
                self.selected_ray_index = None;
            }
        }

        // Esc key deselects
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.selected_ray_index = None;
        }

        // --- Painting 2D Scene ---
        // 1. Canvas background
        painter.rect_filled(screen_rect, 0.0, self.tokens.canvas);

        // 2. Prism polygon
        let s_left = self.view.world_to_screen(self.sim.v_left, screen_rect);
        let s_apex = self.view.world_to_screen(self.sim.v_apex, screen_rect);
        let s_right = self.view.world_to_screen(self.sim.v_right, screen_rect);

        let prism_points = vec![s_left, s_apex, s_right];
        painter.add(egui::Shape::convex_polygon(
            prism_points,
            Color32::from_rgba_premultiplied(80, 140, 220, 28),
            Stroke::new(2.0, Color32::from_rgb(0x4a, 0x80, 0xc0)),
        ));

        // 3. Incoming white D65 ray
        if let Some(first_ray) = screen_rays.first() {
            if first_ray.len() >= 2 {
                let p0 = first_ray[0];
                let p1 = first_ray[1];
                painter.line_segment([p0, p1], Stroke::new(2.0, Color32::WHITE));

                // Small label for D65
                let label_pos = Pos2::new(p0.x - 10.0, p0.y - 14.0);
                painter.text(
                    label_pos,
                    Align2::CENTER_CENTER,
                    "D65",
                    egui::FontId::proportional(12.0),
                    self.tokens.text_dim,
                );
            }
        }

        // 4. Spectral fan rays
        for (i, pts) in screen_rays.iter().enumerate() {
            if pts.len() < 2 {
                continue;
            }
            let is_selected = self.selected_ray_index == Some(i);
            let is_hovered = self.hovered_ray_index == Some(i);
            let ray_data = &self.sim.rays[i];

            let base_color = Color32::from_rgb(
                ray_data.color_rgb[0],
                ray_data.color_rgb[1],
                ray_data.color_rgb[2],
            );

            // Visual differentiation: Selected > Hovered > Normal
            let (stroke_width, stroke_color) = if is_selected {
                (3.5, Color32::WHITE)
            } else if is_hovered {
                (2.5, base_color)
            } else {
                (1.5, base_color)
            };

            // Draw ray path segments from first bounce onwards
            for window in pts[1..].windows(2) {
                painter.line_segment(
                    [window[0], window[1]],
                    Stroke::new(stroke_width, stroke_color),
                );
            }

            // Draw dots at interaction vertices for selected ray
            if is_selected {
                for &pt in pts {
                    painter.circle_filled(pt, 4.0, self.tokens.accent);
                    painter.circle_stroke(pt, 4.0, Stroke::new(1.0, Color32::WHITE));
                }
            }
        }

        // 5. Empty / Edge State: No rays escape
        let any_escaped = self.sim.rays.iter().any(|r| r.deviation_deg.is_some());
        if !any_escaped && !self.sim.rays.is_empty() {
            let msg = "No transmitted rays escape the prism (Total Internal Reflection).\nAdjust the apex angle or incidence angle.";
            painter.text(
                screen_rect.center(),
                Align2::CENTER_CENTER,
                msg,
                egui::FontId::proportional(15.0),
                self.tokens.amber,
            );
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

        self.render_top_menu(ctx);
        self.render_tool_strip(ctx);
        self.render_right_panel(ctx);

        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                self.render_canvas(ui);
            });

        self.render_about_dialog(ctx);
    }
}
