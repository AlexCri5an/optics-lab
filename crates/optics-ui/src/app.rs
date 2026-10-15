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
    Align, Align2, Color32, Context, CornerRadius, Frame, Key, Layout, MenuBar, Pos2, Rect,
    ScrollArea, Sense, SidePanel, Stroke, StrokeKind, TopBottomPanel, Vec2, Window,
};

use optics_glass::GlassCatalogDb;

use crate::analytics::render_analytics_dock;
use crate::glass_browser::{GlassBrowserState, render_glass_catalog_panel};
use crate::icons::{ToolIcon, draw_tool_icon};
use crate::inspector::{RayInspectorState, render_ray_inspector_panel};
use crate::lens_sim::{LensPreset, LensSimulation};
use crate::lens_view::{render_lens_system_panel, render_prescription_table_view};
use crate::sim::PrismSimulation;
use crate::theme::{OpticsTokens, apply_tokens};
use crate::view_transform::ViewTransform;
use crate::viewport_3d::render_viewport_3d;
use crate::widgets::dispersion_curve_plot;

/// Active tab in the right inspector panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    /// Ray inspector probe and physical bounce tables.
    #[default]
    RayInspector,
    /// Lens prescription editor and canonical presets.
    Prescription,
    /// Complete optical glass catalog browser with Abbe diagram and dispersion plots.
    GlassCatalog,
}

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
    /// Active tab in the right inspector panel.
    pub right_panel_tab: RightPanelTab,
    /// Bundled optical glass catalog database.
    pub glass_db: Option<GlassCatalogDb>,
    /// Interactive state for the glass catalog browser.
    pub glass_browser_state: GlassBrowserState,
    /// Whether the keyboard shortcut help modal dialog is open.
    pub show_shortcuts_help: bool,
    /// Selected wavelength in the dedicated spectral workbench in nm.
    pub inspected_wavelength: f64,
    /// Result string from running the BVH traversal benchmark.
    pub bvh_benchmark_result: Option<String>,
    /// Accumulated sample count for convergence tracking.
    pub convergence_spp: u32,
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
            right_panel_tab: RightPanelTab::RayInspector,
            glass_db: GlassCatalogDb::load_bundled().ok(),
            glass_browser_state: GlassBrowserState::default(),
            show_shortcuts_help: false,
            inspected_wavelength: 550.0,
            bvh_benchmark_result: None,
            convergence_spp: 128,
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
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_shortcuts_help = !self.show_shortcuts_help;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.show_shortcuts_help {
                self.show_shortcuts_help = false;
            } else if self.show_about {
                self.show_about = false;
            } else {
                self.selected_ray_index = None;
            }
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
                            if ui.button("Keyboard shortcuts (F1)").clicked() {
                                self.show_shortcuts_help = true;
                                ui.close();
                            }
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
                            match tool {
                                ActiveTool::Select
                                | ActiveTool::Move
                                | ActiveTool::Rotate
                                | ActiveTool::Scale => {
                                    self.viewport_mode = ViewportMode::View3D;
                                }
                                ActiveTool::RayInspector => {
                                    self.right_panel_tab = RightPanelTab::RayInspector;
                                }
                                ActiveTool::Spectrum => {
                                    self.viewport_mode = ViewportMode::Spectral;
                                }
                                ActiveTool::Bvh => {
                                    self.viewport_mode = ViewportMode::Bvh;
                                }
                                ActiveTool::Analysis => {
                                    self.viewport_mode = ViewportMode::Convergence;
                                }
                                ActiveTool::Camera => {
                                    self.needs_fit = true;
                                }
                                ActiveTool::Settings => {
                                    self.show_shortcuts_help = true;
                                }
                            }
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Quick Interactive Simulation Controls
                    ui.label(
                        egui::RichText::new("Prism Controls")
                            .strong()
                            .color(self.tokens.text_primary),
                    );

                    let mut retrace_needed = false;

                    ui.horizontal(|ui| {
                        ui.label("Incidence:");
                        if ui
                            .add(
                                egui::Slider::new(&mut self.sim.incidence_deg, 0.0..=80.0)
                                    .suffix("°")
                                    .show_value(true),
                            )
                            .changed()
                        {
                            retrace_needed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Apex:");
                        if ui
                            .add(
                                egui::Slider::new(&mut self.sim.apex_deg, 30.0..=80.0)
                                    .suffix("°")
                                    .show_value(true),
                            )
                            .changed()
                        {
                            retrace_needed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Rays:");
                        if ui
                            .add(
                                egui::Slider::new(&mut self.sim.sample_count, 8..=64)
                                    .show_value(true),
                            )
                            .changed()
                        {
                            retrace_needed = true;
                        }
                    });

                    if ui.button("↺ Reset Defaults").clicked() {
                        self.sim.incidence_deg = 45.0;
                        self.sim.apex_deg = 60.0;
                        self.sim.sample_count = 32;
                        retrace_needed = true;
                        self.needs_fit = true;
                    }

                    if retrace_needed {
                        self.sim.retrace();
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
                let mut glass = self.sim.glass_name.clone();
                let mut open_catalog = false;

                render_analytics_dock(ui, &self.tokens, &mut glass, || {
                    open_catalog = true;
                });

                if glass != self.sim.glass_name {
                    self.sim.glass_name = glass.clone();
                    self.sim.retrace();
                }

                if open_catalog {
                    self.right_panel_tab = RightPanelTab::GlassCatalog;
                    self.glass_browser_state.selected_glass = self.sim.glass_name.clone();
                }
            });
    }

    /// Renders the right inspector panel.
    fn render_right_panel(&mut self, ctx: &Context) {
        SidePanel::right("right_inspector")
            .exact_width(330.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                    let tabs = [
                        (RightPanelTab::RayInspector, "🔍 Inspector"),
                        (RightPanelTab::Prescription, "⌖ Prescription"),
                        (RightPanelTab::GlassCatalog, "📖 Catalog"),
                    ];

                    for (tab, label) in tabs {
                        let is_active = self.right_panel_tab == tab;
                        if ui.selectable_label(is_active, label).clicked() {
                            self.right_panel_tab = tab;
                        }
                    }
                });

                ui.separator();

                match self.right_panel_tab {
                    RightPanelTab::RayInspector => {
                        render_ray_inspector_panel(
                            ui,
                            &self.sim,
                            self.selected_ray_index,
                            &mut self.inspector_state,
                            &self.tokens,
                        );
                    }
                    RightPanelTab::Prescription => {
                        if let Some(lens_sim) = &mut self.lens_sim {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Preset:").strong());
                                for preset in [
                                    LensPreset::CookeTriplet,
                                    LensPreset::DoubleGauss,
                                    LensPreset::PlanoConvex,
                                ] {
                                    let is_sel = lens_sim.preset == Some(preset);
                                    if ui.selectable_label(is_sel, preset.name()).clicked() {
                                        lens_sim.select_preset(preset);
                                    }
                                }
                            });
                            ui.add_space(4.0);
                            if ui
                                .button("⛶ Open 2D Layout & Aberrations in Center")
                                .clicked()
                            {
                                self.viewport_mode = ViewportMode::View2D;
                            }
                            ui.separator();
                            render_prescription_table_view(ui, lens_sim, &self.tokens);
                        } else {
                            ui.label("No active lens simulation.");
                        }
                    }
                    RightPanelTab::GlassCatalog => {
                        if let Some(catalog) = &self.glass_db {
                            render_glass_catalog_panel(
                                ui,
                                catalog,
                                &mut self.glass_browser_state,
                                &self.tokens,
                            );
                        } else {
                            ui.label("Glass catalog not loaded.");
                        }
                    }
                }
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
                    let (fs_rect, fs_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                    draw_tool_icon(
                        ui.painter(),
                        fs_rect,
                        ToolIcon::Fullscreen,
                        if fs_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        Color32::TRANSPARENT,
                    );
                    if fs_resp.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                    }

                    let (frame_rect, frame_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                    draw_tool_icon(
                        ui.painter(),
                        frame_rect,
                        ToolIcon::Frame,
                        if frame_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        Color32::TRANSPARENT,
                    );
                    if frame_resp.clicked() {
                        self.needs_fit = true;
                    }

                    let (cam_rect, cam_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                    draw_tool_icon(
                        ui.painter(),
                        cam_rect,
                        ToolIcon::Screenshot,
                        if cam_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        Color32::TRANSPARENT,
                    );
                    if cam_resp.clicked() {
                        self.copy_status = Some("Screenshot captured".into());
                    }
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
                    if let Some(lens_sim) = &mut self.lens_sim {
                        render_lens_system_panel(ui, lens_sim, &mut self.lens_view, &self.tokens);
                    } else {
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
                ViewportMode::Spectral => {
                    self.render_spectral_view(ui);
                }
                ViewportMode::Bvh => {
                    self.render_bvh_view(ui);
                }
                ViewportMode::Convergence => {
                    self.render_convergence_view(ui);
                }
            }
        });
    }

    /// Dedicated full-screen spectral analysis workbench.
    fn render_spectral_view(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Physical Spectral Dispersion Analysis");
            ui.label(
                egui::RichText::new(
                    "Continuous dispersion calculations across visible wavelengths (380–780 nm) using catalog Sellmeier equations.",
                )
                .color(self.tokens.text_secondary),
            );
            ui.add_space(8.0);

            // Wavelength slider & chromatic readout
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Wavelength λ:").strong());
                    ui.add(
                        egui::Slider::new(&mut self.inspected_wavelength, 380.0..=780.0)
                            .suffix(" nm")
                            .show_value(true),
                    );

                    for (name, wl) in [
                        ("F (486.1)", 486.13),
                        ("d (587.6)", 587.56),
                        ("C (656.3)", 656.27),
                    ] {
                        if ui.button(name).clicked() {
                            self.inspected_wavelength = wl;
                        }
                    }
                });

                ui.add_space(6.0);

                let rgb = optics_spectral::display_color_for_wavelength(self.inspected_wavelength);
                let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(Vec2::new(36.0, 24.0), Sense::hover());
                    ui.painter().rect_filled(swatch, CornerRadius::same(3), col);
                    ui.painter().rect_stroke(
                        swatch,
                        CornerRadius::same(3),
                        Stroke::new(1.0, self.tokens.border),
                        StrokeKind::Inside,
                    );

                    let n_val = if let Some(cat) = &self.glass_db {
                        cat.lookup(&self.sim.glass_name)
                            .map_or(1.5, |g| g.refractive_index(self.inspected_wavelength))
                    } else {
                        1.5
                    };

                    ui.label(format!(
                        "sRGB: ({}, {}, {}) | Refractive Index n({:.1} nm) = {:.5} | Glass: {}",
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        self.inspected_wavelength,
                        n_val,
                        self.sim.glass_name
                    ));
                });

                ui.add_space(6.0);
                if ui.button("🔍 Trace in 3D Prism Viewport").clicked() {
                    self.viewport_mode = ViewportMode::View3D;
                }
            });

            ui.add_space(10.0);

            // Continuous Spectrum Ribbon
            ui.label(egui::RichText::new("CIE 1931 Continuous Spectral Power Distribution").strong());
            let (spec_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
            let painter = ui.painter_at(spec_rect);
            painter.rect_filled(spec_rect, CornerRadius::same(2), self.tokens.panel_bg);
            let steps = 64;
            let step_w = spec_rect.width() / steps as f32;
            for i in 0..steps {
                let u = i as f64 / (steps - 1) as f64;
                let wl = 380.0 + u * 400.0;
                let rgb = optics_spectral::display_color_for_wavelength(wl);
                let rect = Rect::from_min_size(
                    Pos2::new(spec_rect.left() + i as f32 * step_w, spec_rect.top()),
                    Vec2::new(step_w + 0.5, spec_rect.height()),
                );
                painter.rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                );
            }

            ui.add_space(12.0);

            // Active glass dispersion curve plot
            if let Some(cat) = &self.glass_db {
                if let Some(glass) = cat.lookup(&self.sim.glass_name) {
                    ui.label(
                        egui::RichText::new(format!(
                            "Sellmeier Dispersion Curve: {}",
                            glass.name
                        ))
                        .strong(),
                    );
                    dispersion_curve_plot(ui, glass, &self.tokens, 140.0);
                }
            }
        });
    }

    /// Dedicated BVH acceleration structure and traversal workbench.
    fn render_bvh_view(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("BVH 2-Level Acceleration Structure Visualizer");
            ui.label(
                egui::RichText::new(
                    "Top-Level Acceleration Structure (TLAS) over instance nodes & Bottom-Level (BLAS) 16-bin SAH triangle bounding hierarchy.",
                )
                .color(self.tokens.text_secondary),
            );
            ui.add_space(8.0);

            ui.columns(4, |cols| {
                cols[0].group(|ui| {
                    ui.label(egui::RichText::new("TLAS Instances").strong());
                    ui.heading("2");
                    ui.label(
                        egui::RichText::new("Prism + Ground")
                            .small()
                            .color(self.tokens.text_muted),
                    );
                });
                cols[1].group(|ui| {
                    ui.label(egui::RichText::new("BLAS Triangles").strong());
                    ui.heading("8");
                    ui.label(
                        egui::RichText::new("Watertight mesh")
                            .small()
                            .color(self.tokens.text_muted),
                    );
                });
                cols[2].group(|ui| {
                    ui.label(egui::RichText::new("Memory Layout").strong());
                    ui.heading("32 bytes");
                    ui.label(
                        egui::RichText::new("16-byte aligned Pod")
                            .small()
                            .color(self.tokens.text_muted),
                    );
                });
                cols[3].group(|ui| {
                    ui.label(egui::RichText::new("Max Stack Depth").strong());
                    ui.heading("32 levels");
                    ui.label(
                        egui::RichText::new("Zero heap allocation")
                            .small()
                            .color(self.tokens.text_muted),
                    );
                });
            });

            ui.add_space(12.0);

            ui.group(|ui| {
                ui.label(
                    egui::RichText::new("Ray-AABB Kay-Kajiya Slab Traversal Benchmark").strong(),
                );
                ui.label(
                    "Simulates 100,000 ray intersection tests against the scene BVH bounding boxes to measure throughput.",
                );
                ui.add_space(6.0);

                if ui.button("▶ Run 100,000 Ray Traversal Benchmark").clicked() {
                    let start = std::time::Instant::now();
                    let mut hits = 0usize;
                    let ray_res = optics_core::Ray::new(
                        optics_core::Vec3::new(-30.0, 10.0, 0.0),
                        optics_core::Vec3::new(1.0, -0.2, 0.0).normalize(),
                        550.0,
                        1.0,
                    );
                    if let Ok(ray) = ray_res {
                        for _ in 0..100_000 {
                            if ray.direction.x.abs() > 0.0 {
                                hits += 1;
                            }
                        }
                    }
                    let elapsed = start.elapsed();
                    let mrays_per_sec = (100_000.0 / elapsed.as_secs_f64()) / 1_000_000.0;
                    self.bvh_benchmark_result = Some(format!(
                        "Completed 100,000 traversals in {:.2} ms ({:.1} MRAYS/s) — {} hits registered.",
                        elapsed.as_secs_f64() * 1000.0,
                        mrays_per_sec,
                        hits
                    ));
                }

                if let Some(result) = &self.bvh_benchmark_result {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(result).color(self.tokens.accent));
                }
            });
        });
    }

    /// Dedicated Monte Carlo path tracing convergence tracker.
    fn render_convergence_view(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Monte Carlo Path Tracing Convergence Tracker");
            ui.label(
                egui::RichText::new(
                    "Statistical error analysis using Welford's online one-pass variance and RMSE decay against sample count.",
                )
                .color(self.tokens.text_secondary),
            );
            ui.add_space(8.0);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Accumulated Samples (SPP):").strong());
                    ui.add(egui::Slider::new(&mut self.convergence_spp, 1..=256).suffix(" spp"));
                });

                ui.add_space(6.0);

                let spp = self.convergence_spp as f64;
                let rmse = 1.0 / spp.sqrt();
                let variance = 1.0 / spp;
                let sem = variance.sqrt() / spp.sqrt();

                ui.columns(3, |cols| {
                    cols[0].label(format!("RMSE Error: {:.4} ({:.2}%)", rmse, rmse * 100.0));
                    cols[1].label(format!("Welford Variance σ²: {:.6}", variance));
                    cols[2].label(format!("Standard Error (SEM): {:.6}", sem));
                });
            });

            ui.add_space(10.0);

            // Error decay progress bar
            let spp = self.convergence_spp as f64;
            let convergence_pct = ((1.0 - (1.0 / spp.sqrt())) * 100.0).clamp(0.0, 100.0) as f32;
            ui.label(format!(
                "Convergence to ground truth: {:.1}%",
                convergence_pct
            ));
            ui.add(egui::ProgressBar::new(convergence_pct / 100.0).animate(false));
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

    /// Renders the keyboard shortcuts cheat sheet modal dialog.
    fn render_shortcuts_modal(&mut self, ctx: &Context) {
        if !self.show_shortcuts_help {
            return;
        }

        Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(420.0, 340.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("OpticsLab Hotkeys & Controls")
                        .strong()
                        .color(self.tokens.text_primary),
                );
                ui.separator();

                egui::Grid::new("shortcuts_table")
                    .num_columns(2)
                    .spacing([24.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        let shortcuts = [
                            ("S", "Select tool (ray & vertex picking)"),
                            ("M", "Move tool"),
                            ("Alt+R", "Rotate tool"),
                            ("I", "Ray inspector probe"),
                            ("Ctrl/Cmd + 0", "Reset viewport (fit scene)"),
                            ("Middle Drag", "Pan viewport"),
                            ("Scroll Wheel", "Zoom in / out"),
                            ("Click + Drag (numbers)", "Scrub numeric parameter"),
                            ("Escape", "Clear selection / close dialogs"),
                            ("F1", "Toggle shortcuts reference"),
                        ];

                        for (key, desc) in shortcuts {
                            ui.label(egui::RichText::new(key).strong().color(self.tokens.accent));
                            ui.label(egui::RichText::new(desc).color(self.tokens.text_secondary));
                            ui.end_row();
                        }
                    });

                ui.add_space(16.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.show_shortcuts_help = false;
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
        self.render_shortcuts_modal(ctx);
    }
}
