//! `OpticsLabApp` — the main desktop application container for OpticsLab.
//!
//! Provides the exact scientific desktop workstation UI matching the reference image:
//! - Top application bar (File, Edit, View, Render, Diagnostics, Help, 128 spp, window controls)
//! - Left sidebar (Tools: Select, Move, Rotate, Scale, Ray Inspector, Spectrum, BVH, Analysis, Camera, Settings + Scene hierarchy tree)
//! - Center 3D perspective viewport (Realistic glass prism, incident white ray, spectral fan, floor grid, XYZ gizmo)
//! - Bottom analytics dock (Convergence, Spectral Power, BVH Diagnostics, Material/Glass)
//! - Right inspector (Ray Inspector, bounce tables, physical ray-path node timeline)
//! - Bottom status bar (Ready, 128 spp, 1920x1080, GPU: Vulkan, 60 FPS)

use egui::{
    Align, Align2, Color32, Context, CornerRadius, Frame, Key, Layout, MenuBar, Rect, ScrollArea,
    Sense, SidePanel, Stroke, StrokeKind, TopBottomPanel, Vec2, Window,
};

use crate::analytics::render_analytics_dock;
use crate::icons::{ToolIcon, draw_tool_icon};
use crate::inspector::{RayInspectorState, render_ray_inspector_panel};
use crate::lens_sim::{LensPreset, LensSimulation};
use crate::lens_view::render_lens_system_panel;
use crate::sim::PrismSimulation;
use crate::theme::{OpticsTokens, apply_tokens};
use crate::view_transform::ViewTransform;
use crate::viewport_3d::render_viewport_3d;

/// Active user tool in the left toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTool {
    /// Selection tool for rays and geometric elements.
    #[default]
    Select,
    /// 3D translation / move tool.
    Move,
    /// 3D rotation tool.
    Rotate,
    /// 3D scale / resize tool.
    Scale,
    /// Ray inspector probe tool.
    RayInspector,
    /// Spectral analysis tool.
    Spectrum,
    /// BVH acceleration hierarchy tool.
    Bvh,
    /// Statistical convergence and variance tool.
    Analysis,
    /// Viewport camera controls.
    Camera,
    /// System and rendering settings.
    Settings,
}

impl ActiveTool {
    /// Human-readable tool name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::RayInspector => "Ray Inspector",
            Self::Spectrum => "Spectrum",
            Self::Bvh => "BVH",
            Self::Analysis => "Analysis",
            Self::Camera => "Camera",
            Self::Settings => "Settings",
        }
    }

    /// Corresponding vector icon.
    #[must_use]
    pub const fn icon(&self) -> ToolIcon {
        match self {
            Self::Select => ToolIcon::Select,
            Self::Move => ToolIcon::Move,
            Self::Rotate => ToolIcon::Rotate,
            Self::Scale => ToolIcon::Scale,
            Self::RayInspector => ToolIcon::RayInspector,
            Self::Spectrum => ToolIcon::Spectrum,
            Self::Bvh => ToolIcon::Bvh,
            Self::Analysis => ToolIcon::Analysis,
            Self::Camera => ToolIcon::Camera,
            Self::Settings => ToolIcon::Settings,
        }
    }
}

/// Active viewport display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    /// 3D perspective ray-tracing viewport (reference image).
    #[default]
    View3D,
    /// 2D cross-section layout (prism or sequential lens system).
    View2D,
    /// Spectral breakdown and CIE chromaticity view.
    Spectral,
    /// BVH acceleration structure traversal view.
    Bvh,
    /// Convergence & variance tracking view.
    Convergence,
}

/// High-level mode of the OpticsLab application workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Interactive 2D/3D prism spectral dispersion experiment (Slice A).
    #[default]
    PrismDispersion,
    /// Canonical lens systems and optical aberration analysis (Slice B).
    LensSystem,
}

/// The top-level OpticsLab application state.
pub struct OpticsLabApp {
    /// High-level workbench mode.
    pub app_mode: AppMode,
    /// Active viewport display mode.
    pub viewport_mode: ViewportMode,
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
    /// Whether Fresnel reflection branches are rendered in 2D view.
    pub show_reflections: bool,
    /// Whether ray trace diagnostic timing is shown in optical readout.
    pub show_timing: bool,
    /// Transient status message when copying table data.
    pub copy_status: Option<String>,
    /// Flag indicating whether the canvas needs an initial auto-fit.
    pub needs_fit: bool,
    /// Whether the About dialog window is open.
    pub show_about: bool,
    /// Ray inspector collapse/expand state.
    pub inspector_state: RayInspectorState,
    /// Scene tree root node expansion state.
    pub scene_tree_scene_open: bool,
    /// Prism sub-node expansion state.
    pub scene_tree_prism_open: bool,
    /// Ground sub-node expansion state.
    pub scene_tree_ground_open: bool,
    /// Selected object in scene tree.
    pub selected_scene_object: Option<String>,
}

impl Default for OpticsLabApp {
    fn default() -> Self {
        Self {
            app_mode: AppMode::PrismDispersion,
            viewport_mode: ViewportMode::View3D,
            tokens: OpticsTokens::dark_scientific(),
            active_tool: ActiveTool::Select,
            view: ViewTransform::default(),
            sim: PrismSimulation::default(),
            lens_sim: LensSimulation::new(LensPreset::CookeTriplet).ok(),
            lens_view: ViewTransform::default(),
            selected_ray_index: Some(0),
            hovered_ray_index: None,
            show_reflections: true,
            show_timing: false,
            copy_status: None,
            needs_fit: true,
            show_about: false,
            inspector_state: RayInspectorState::default(),
            scene_tree_scene_open: true,
            scene_tree_prism_open: true,
            scene_tree_ground_open: false,
            selected_scene_object: Some("Prism".into()),
        }
    }
}

impl OpticsLabApp {
    /// Creates a new OpticsLab application instance with default settings.
    #[must_use]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Handles keyboard shortcuts.
    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.active_tool = ActiveTool::Select;
        }
        if ctx.input(|i| i.key_pressed(Key::M)) {
            self.active_tool = ActiveTool::Move;
        }
        if ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.alt) {
            self.active_tool = ActiveTool::Rotate;
        }
        if ctx.input(|i| i.key_pressed(Key::I)) {
            self.active_tool = ActiveTool::RayInspector;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected_ray_index = None;
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Num0)) {
            self.needs_fit = true;
        }
    }

    /// Renders the top application bar matching the reference image.
    fn render_top_bar(&mut self, ctx: &Context) {
        TopBottomPanel::top("top_app_bar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                    // Logo icon
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                    draw_tool_icon(
                        ui.painter(),
                        logo_rect,
                        ToolIcon::Logo,
                        self.tokens.accent,
                        Color32::TRANSPARENT,
                    );

                    // App Title
                    ui.label(
                        egui::RichText::new("OpticsLab")
                            .strong()
                            .size(13.0)
                            .color(self.tokens.text_primary),
                    );

                    ui.label(
                        egui::RichText::new("·")
                            .size(13.0)
                            .color(self.tokens.text_muted),
                    );

                    // Menus: File, Edit, View, Render, Diagnostics, Help
                    MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("Quit").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                ui.close();
                            }
                        });

                        ui.menu_button("Edit", |ui| {
                            if ui.button("Reset defaults").clicked() {
                                self.sim = PrismSimulation::default();
                                self.needs_fit = true;
                                ui.close();
                            }
                        });

                        ui.menu_button("View", |ui| {
                            if ui.button("Reset View (Cmd+0)").clicked() {
                                self.needs_fit = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.checkbox(&mut self.show_reflections, "Show reflections");
                            ui.checkbox(&mut self.show_timing, "Show timing");
                        });

                        ui.menu_button("Render", |ui| {
                            if ui.button("Start path tracer").clicked() {
                                ui.close();
                            }
                        });

                        ui.menu_button("Diagnostics", |ui| {
                            if ui.button("BVH slab test").clicked() {
                                ui.close();
                            }
                        });

                        ui.menu_button("Help", |ui| {
                            if ui.button("About OpticsLab").clicked() {
                                self.show_about = true;
                                ui.close();
                            }
                        });
                    });

                    // Right side controls: 128 spp + window controls
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);

                        // Window close
                        let (close_rect, close_resp) =
                            ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::click());
                        let close_col = if close_resp.hovered() {
                            Color32::from_rgb(0xff, 0x55, 0x55)
                        } else {
                            self.tokens.text_muted
                        };
                        draw_tool_icon(
                            ui.painter(),
                            close_rect,
                            ToolIcon::Close,
                            close_col,
                            Color32::TRANSPARENT,
                        );
                        if close_resp.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        // Window maximize
                        let (max_rect, _) =
                            ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
                        draw_tool_icon(
                            ui.painter(),
                            max_rect,
                            ToolIcon::Maximize,
                            self.tokens.text_muted,
                            Color32::TRANSPARENT,
                        );

                        // Window minimize
                        let (min_rect, _) =
                            ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
                        draw_tool_icon(
                            ui.painter(),
                            min_rect,
                            ToolIcon::Minimize,
                            self.tokens.text_muted,
                            Color32::TRANSPARENT,
                        );

                        ui.add_space(10.0);

                        // Sample counter indicator (128 spp)
                        let spp_frame = Frame::NONE
                            .fill(self.tokens.panel_secondary)
                            .stroke(Stroke::new(1.0, self.tokens.border))
                            .corner_radius(CornerRadius::same(2))
                            .inner_margin(egui::Margin::symmetric(6, 2));

                        spp_frame.show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("128 spp")
                                    .size(11.0)
                                    .color(self.tokens.text_secondary),
                            );
                        });
                    });
                });
            });
    }

    /// Renders the left sidebar containing the vertical tool list and scene tree.
    fn render_left_sidebar(&mut self, ctx: &Context) {
        SidePanel::left("left_sidebar")
            .exact_width(240.0)
            .resizable(false)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);

                    // 1. Tool list
                    let tools = [
                        ActiveTool::Select,
                        ActiveTool::Move,
                        ActiveTool::Rotate,
                        ActiveTool::Scale,
                        ActiveTool::RayInspector,
                        ActiveTool::Spectrum,
                        ActiveTool::Bvh,
                        ActiveTool::Analysis,
                        ActiveTool::Camera,
                        ActiveTool::Settings,
                    ];

                    for tool in tools {
                        let is_active = self.active_tool == tool;
                        let bg_col = if is_active {
                            self.tokens.selected
                        } else {
                            Color32::TRANSPARENT
                        };
                        let fg_col = if is_active {
                            self.tokens.text_primary
                        } else {
                            self.tokens.text_secondary
                        };

                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 26.0),
                            Sense::click(),
                        );
                        let painter = ui.painter_at(rect);

                        if is_active {
                            painter.rect_filled(rect, CornerRadius::same(2), bg_col);
                            painter.rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0, self.tokens.border),
                                StrokeKind::Inside,
                            );
                        } else if response.hovered() {
                            painter.rect_filled(rect, CornerRadius::same(2), self.tokens.hover_bg);
                        }

                        // Icon
                        let icon_rect = Rect::from_min_size(
                            rect.min + Vec2::new(8.0, 4.0),
                            Vec2::new(18.0, 18.0),
                        );
                        draw_tool_icon(
                            &painter,
                            icon_rect,
                            tool.icon(),
                            fg_col,
                            Color32::TRANSPARENT,
                        );

                        // Label
                        painter.text(
                            rect.min + Vec2::new(34.0, 13.0),
                            Align2::LEFT_CENTER,
                            tool.name(),
                            egui::FontId::proportional(12.5),
                            fg_col,
                        );

                        if response.clicked() {
                            self.active_tool = tool;
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // 2. Scene hierarchy tree section
                    ui.horizontal(|ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                        draw_tool_icon(
                            ui.painter(),
                            icon_rect,
                            ToolIcon::Scene,
                            self.tokens.text_muted,
                            Color32::TRANSPARENT,
                        );
                        ui.label(
                            egui::RichText::new("Scene")
                                .size(11.5)
                                .color(self.tokens.text_secondary),
                        );
                    });

                    ui.add_space(4.0);

                    // Tree item: Scene
                    let scene_glyph = if self.scene_tree_scene_open {
                        "▼"
                    } else {
                        "▶"
                    };
                    if ui
                        .selectable_label(
                            self.selected_scene_object.as_deref() == Some("Scene"),
                            format!("{scene_glyph} ⛶ Scene"),
                        )
                        .clicked()
                    {
                        self.scene_tree_scene_open = !self.scene_tree_scene_open;
                        self.selected_scene_object = Some("Scene".into());
                    }

                    if self.scene_tree_scene_open {
                        ui.indent("scene_tree_body", |ui| {
                            // Camera
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Camera"),
                                    "📷 Camera",
                                )
                                .clicked()
                            {
                                self.selected_scene_object = Some("Camera".into());
                            }

                            // Prism (expandable)
                            let prism_glyph = if self.scene_tree_prism_open {
                                "▼"
                            } else {
                                "▶"
                            };
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Prism"),
                                    format!("{prism_glyph} ▲ Prism"),
                                )
                                .clicked()
                            {
                                self.scene_tree_prism_open = !self.scene_tree_prism_open;
                                self.selected_scene_object = Some("Prism".into());
                            }

                            if self.scene_tree_prism_open {
                                ui.indent("prism_children", |ui| {
                                    if ui
                                        .selectable_label(
                                            self.selected_scene_object.as_deref()
                                                == Some("Glass (BK7)"),
                                            "● Glass (BK7)",
                                        )
                                        .clicked()
                                    {
                                        self.selected_scene_object = Some("Glass (BK7)".into());
                                    }
                                    if ui
                                        .selectable_label(
                                            self.selected_scene_object.as_deref()
                                                == Some("Material"),
                                            "❖ Material",
                                        )
                                        .clicked()
                                    {
                                        self.selected_scene_object = Some("Material".into());
                                    }
                                });
                            }

                            // Light
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Light"),
                                    "☼ Light",
                                )
                                .clicked()
                            {
                                self.selected_scene_object = Some("Light".into());
                            }

                            // Ground (expandable)
                            let ground_glyph = if self.scene_tree_ground_open {
                                "▼"
                            } else {
                                "▶"
                            };
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Ground"),
                                    format!("{ground_glyph} ▤ Ground"),
                                )
                                .clicked()
                            {
                                self.scene_tree_ground_open = !self.scene_tree_ground_open;
                                self.selected_scene_object = Some("Ground".into());
                            }
                        });
                    }
                });
            });
    }

    /// Renders the bottom status bar matching the reference image.
    fn render_status_bar(&self, ctx: &Context) {
        TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Ready")
                            .size(11.0)
                            .color(self.tokens.text_secondary),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("128 spp  |  1920×1080  |  GPU: Vulkan  |  60 FPS")
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );
                    });
                });
            });
    }

    /// Renders the bottom analytics dock containing the 4 scientific panels.
    fn render_bottom_analytics(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("bottom_analytics_dock")
            .exact_height(205.0)
            .show(ctx, |ui| {
                render_analytics_dock(ui, &self.tokens, &self.sim.glass_name, || {});
            });
    }

    /// Renders the right inspector panel.
    fn render_right_panel(&mut self, ctx: &Context) {
        SidePanel::right("right_inspector")
            .exact_width(330.0)
            .resizable(false)
            .show(ctx, |ui| {
                render_ray_inspector_panel(
                    ui,
                    &self.sim,
                    self.selected_ray_index,
                    &mut self.inspector_state,
                    &self.tokens,
                );
            });
    }

    /// Renders the central viewport area.
    fn render_central_viewport(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Viewport top toolbar with mode tabs and right action buttons
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                let tabs = [
                    (ViewportMode::View3D, "3D View"),
                    (ViewportMode::View2D, "2D View"),
                    (ViewportMode::Spectral, "Spectral"),
                    (ViewportMode::Bvh, "BVH"),
                    (ViewportMode::Convergence, "Convergence"),
                ];

                for (mode, label) in tabs {
                    let is_active = self.viewport_mode == mode;
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(76.0, 24.0), Sense::click());
                    let painter = ui.painter_at(rect);

                    if is_active {
                        painter.rect_filled(rect, CornerRadius::same(2), self.tokens.selected);
                        painter.rect_stroke(
                            rect,
                            CornerRadius::same(2),
                            Stroke::new(1.0, self.tokens.border),
                            StrokeKind::Inside,
                        );
                    } else if resp.hovered() {
                        painter.rect_filled(rect, CornerRadius::same(2), self.tokens.hover_bg);
                    }

                    let text_col = if is_active {
                        self.tokens.text_primary
                    } else {
                        self.tokens.text_muted
                    };
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(11.5),
                        text_col,
                    );

                    if resp.clicked() {
                        self.viewport_mode = mode;
                    }
                }

                // Right buttons: Screenshot, Frame, Fullscreen
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (fs_rect, _) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::hover());
                    draw_tool_icon(
                        ui.painter(),
                        fs_rect,
                        ToolIcon::Fullscreen,
                        self.tokens.text_muted,
                        Color32::TRANSPARENT,
                    );

                    let (frame_rect, _) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::hover());
                    draw_tool_icon(
                        ui.painter(),
                        frame_rect,
                        ToolIcon::Frame,
                        self.tokens.text_muted,
                        Color32::TRANSPARENT,
                    );

                    let (cam_rect, _) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::hover());
                    draw_tool_icon(
                        ui.painter(),
                        cam_rect,
                        ToolIcon::Screenshot,
                        self.tokens.text_muted,
                        Color32::TRANSPARENT,
                    );
                });
            });

            ui.add_space(2.0);

            // Viewport content canvas
            let avail_rect = ui.available_rect_before_wrap();
            match self.viewport_mode {
                ViewportMode::View3D => {
                    render_viewport_3d(
                        ui,
                        avail_rect,
                        &self.sim,
                        &self.tokens,
                        &mut self.selected_ray_index,
                        &mut self.hovered_ray_index,
                    );
                }
                ViewportMode::View2D => {
                    if self.app_mode == AppMode::LensSystem {
                        if let Some(lens_sim) = &mut self.lens_sim {
                            render_lens_system_panel(
                                ui,
                                lens_sim,
                                &mut self.lens_view,
                                &self.tokens,
                            );
                        }
                    } else {
                        // 2D Prism View
                        render_viewport_3d(
                            ui,
                            avail_rect,
                            &self.sim,
                            &self.tokens,
                            &mut self.selected_ray_index,
                            &mut self.hovered_ray_index,
                        );
                    }
                }
                ViewportMode::Spectral | ViewportMode::Bvh | ViewportMode::Convergence => {
                    render_viewport_3d(
                        ui,
                        avail_rect,
                        &self.sim,
                        &self.tokens,
                        &mut self.selected_ray_index,
                        &mut self.hovered_ray_index,
                    );
                }
            }
        });
    }

    /// Renders the About dialog window.
    fn render_about_dialog(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }

        Window::new("About OpticsLab")
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(360.0, 220.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::hover());
                    draw_tool_icon(
                        ui.painter(),
                        logo_rect,
                        ToolIcon::Logo,
                        self.tokens.accent,
                        Color32::TRANSPARENT,
                    );
                    ui.add_space(4.0);
                    ui.heading("OpticsLab");
                    ui.label(
                        egui::RichText::new("Professional Optical Ray-Tracing & Simulation")
                            .color(self.tokens.text_secondary),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "Version 0.0.1\nMIT License\nInspired by RenderDoc, Blender & pbrt-v4",
                        )
                        .small()
                        .color(self.tokens.text_muted),
                    );
                    ui.add_space(16.0);
                    if ui.button("Close").clicked() {
                        self.show_about = false;
                    }
                });
            });
    }
}

impl eframe::App for OpticsLabApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        apply_tokens(ctx, &self.tokens);
        self.handle_shortcuts(ctx);

        // 1. Top Application Bar
        self.render_top_bar(ctx);

        // 2. Status Bar at very bottom
        self.render_status_bar(ctx);

        // 3. Left Sidebar (Tools & Scene)
        self.render_left_sidebar(ctx);

        // 4. Right Inspector (Ray Inspector & Path)
        self.render_right_panel(ctx);

        // 5. Bottom Analytics Dock (Convergence, Spectral Power, BVH, Material)
        self.render_bottom_analytics(ctx);

        // 6. Central Main Viewport (dominated by 3D prism & spectral fan)
        self.render_central_viewport(ctx);

        // 7. Dialogs
        self.render_about_dialog(ctx);
    }
}
