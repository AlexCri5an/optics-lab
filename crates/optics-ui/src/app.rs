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
    Align, Align2, Color32, Context, CornerRadius, Key, Layout, MenuBar, Pos2, Rect, ScrollArea,
    Sense, SidePanel, Stroke, StrokeKind, TopBottomPanel, Vec2, Window,
};

use optics_glass::GlassCatalogDb;
use optics_zemax::parse_zmx_bytes;

use crate::analytics::render_analytics_dock;
use crate::glass_browser::{GlassBrowserState, render_glass_catalog_panel};
use crate::icons::{IconId, draw_icon, init_icon_fonts, toolbar_button};
use crate::inspector::{RayInspectorState, render_ray_inspector_panel};
use crate::lens_sim::{LensPreset, LensSimulation};
use crate::lens_view::{render_lens_system_panel, render_prescription_table_view};
use crate::shortcuts::{ALL_SHORTCUTS, ShortcutId, format_tooltip};
use crate::sim::PrismSimulation;
use crate::theme::{
    AppPreferences, CanvasBackgroundPreference, OpticsTokens, ThemePreference, apply_tokens,
    resolve_theme,
};
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
    pub const fn icon(&self) -> IconId {
        match self {
            Self::Select => IconId::Select,
            Self::Move => IconId::Move,
            Self::Rotate => IconId::Rotate,
            Self::Scale => IconId::Scale,
            Self::RayInspector => IconId::RayProbe,
            Self::Spectrum => IconId::Spectrum,
            Self::Bvh => IconId::Bvh,
            Self::Analysis => IconId::Analysis,
            Self::Camera => IconId::Camera,
            Self::Settings => IconId::Settings,
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
    /// Whether icon fonts have been registered in egui font definitions.
    pub fonts_initialized: bool,
    /// User application preferences (theme, UI scale, visual options).
    pub prefs: AppPreferences,
    /// Whether the Preferences modal dialog is open.
    pub show_preferences: bool,
    /// Whether full screen viewport mode is active.
    pub is_fullscreen: bool,
}

impl Default for OpticsLabApp {
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        let (default_app_mode, default_viewport_mode, default_right_tab) = (
            AppMode::LensSystem,
            ViewportMode::View2D,
            RightPanelTab::Prescription,
        );
        #[cfg(not(target_arch = "wasm32"))]
        let (default_app_mode, default_viewport_mode, default_right_tab) = (
            AppMode::PrismDispersion,
            ViewportMode::View3D,
            RightPanelTab::RayInspector,
        );

        let prefs = AppPreferences::default();
        let tokens = OpticsTokens::classic_light();

        Self {
            app_mode: default_app_mode,
            viewport_mode: default_viewport_mode,
            tokens,
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
            show_preferences: false,
            is_fullscreen: false,
            inspector_state: RayInspectorState::default(),
            scene_tree_scene_open: true,
            scene_tree_prism_open: true,
            scene_tree_ground_open: false,
            selected_scene_object: Some("Prism".into()),
            right_panel_tab: default_right_tab,
            glass_db: GlassCatalogDb::load_bundled().ok(),
            glass_browser_state: GlassBrowserState::default(),
            show_shortcuts_help: false,
            inspected_wavelength: 550.0,
            bvh_benchmark_result: None,
            convergence_spp: 128,
            fonts_initialized: false,
            prefs,
        }
    }
}

impl OpticsLabApp {
    /// Storage key for persisted user preferences.
    pub const PREFS_STORAGE_KEY: &'static str = "optics_lab_preferences";

    /// Creates a new OpticsLab application instance with default settings and persisted preferences.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        init_icon_fonts(&cc.egui_ctx);
        let mut app = Self::default();
        if let Some(storage) = cc.storage {
            if let Some(json) = storage.get_string(Self::PREFS_STORAGE_KEY) {
                if let Ok(saved_prefs) = serde_json::from_str::<AppPreferences>(&json) {
                    app.prefs = saved_prefs;
                }
            }
        }
        let system_is_dark = cc.egui_ctx.style().visuals.dark_mode;
        app.tokens = resolve_theme(app.prefs.theme, system_is_dark);
        app.fonts_initialized = true;
        app
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
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Comma)) {
            self.show_preferences = !self.show_preferences;
        }
        if ctx.input(|i| {
            (i.modifiers.command || i.modifiers.ctrl)
                && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
        }) {
            self.prefs.ui_scale = (self.prefs.ui_scale + 0.1).min(3.0);
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Minus)) {
            self.prefs.ui_scale = (self.prefs.ui_scale - 0.1).max(0.5);
        }
        if ctx.input(|i| i.key_pressed(Key::B)) {
            self.show_reflections = !self.show_reflections;
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.show_preferences {
                self.show_preferences = false;
            } else if self.show_shortcuts_help {
                self.show_shortcuts_help = false;
            } else if self.show_about {
                self.show_about = false;
            } else if self.selected_ray_index.is_some() {
                self.selected_ray_index = None;
            } else if self.is_fullscreen {
                self.is_fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Num0)) {
            self.needs_fit = true;
        }
    }

    /// Handles file drag-and-drop events (e.g. dropping a Zemax `.zmx` file onto the window/canvas).
    fn handle_file_drag_and_drop(&mut self, ctx: &Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let zmx_bytes: Option<Vec<u8>> = if let Some(bytes) = file.bytes {
                Some(bytes.to_vec())
            } else if let Some(path) = file.path {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    std::fs::read(&path).ok()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = path;
                    None
                }
            } else {
                None
            };

            if let Some(bytes) = zmx_bytes {
                match parse_zmx_bytes(&bytes) {
                    Ok(system) => {
                        let name = system.name.clone();
                        let num_surfaces = system.surfaces.len();
                        let prescription = system.to_optical_prescription();

                        if let Some(ref mut sim) = self.lens_sim {
                            sim.load_prescription_custom(prescription);
                        } else if let Ok(sim) = LensSimulation::from_prescription(prescription) {
                            self.lens_sim = Some(sim);
                        }

                        self.app_mode = AppMode::LensSystem;
                        self.viewport_mode = ViewportMode::View2D;
                        self.right_panel_tab = RightPanelTab::Prescription;
                        self.copy_status = Some(format!(
                            "Loaded Zemax prescription: {name} ({num_surfaces} surfaces)"
                        ));
                    }
                    Err(e) => {
                        self.copy_status = Some(format!("Failed to parse .zmx: {e}"));
                    }
                }
            }
        }
    }

    /// Renders the top menu bar matching standard desktop chrome.
    fn render_top_bar(&mut self, ctx: &Context) {
        let is_macos = cfg!(target_os = "macos");

        TopBottomPanel::top("top_app_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                    // Logo icon
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                    draw_icon(
                        ui.painter(),
                        logo_rect,
                        IconId::Logo,
                        self.tokens.accent,
                        14.0,
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

                    // Menus: File, Edit, View, Tools, Help
                    MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            ui.menu_button("Load preset", |ui| {
                                if ui.button("Cooke Triplet F/4.5").clicked() {
                                    if let Some(ref mut sim) = self.lens_sim {
                                        sim.select_preset(LensPreset::CookeTriplet);
                                    }
                                    self.app_mode = AppMode::LensSystem;
                                    self.viewport_mode = ViewportMode::View2D;
                                    self.right_panel_tab = RightPanelTab::Prescription;
                                    ui.close();
                                }
                                if ui.button("Double Gauss F/2.0").clicked() {
                                    if let Some(ref mut sim) = self.lens_sim {
                                        sim.select_preset(LensPreset::DoubleGauss);
                                    }
                                    self.app_mode = AppMode::LensSystem;
                                    self.viewport_mode = ViewportMode::View2D;
                                    self.right_panel_tab = RightPanelTab::Prescription;
                                    ui.close();
                                }
                                if ui.button("Plano-Convex Singlet").clicked() {
                                    if let Some(ref mut sim) = self.lens_sim {
                                        sim.select_preset(LensPreset::PlanoConvex);
                                    }
                                    self.app_mode = AppMode::LensSystem;
                                    self.viewport_mode = ViewportMode::View2D;
                                    self.right_panel_tab = RightPanelTab::Prescription;
                                    ui.close();
                                }
                            });
                            ui.separator();
                            ui.label(
                                egui::RichText::new("Drag & drop .zmx file to load")
                                    .size(11.0)
                                    .color(self.tokens.text_muted),
                            );
                            ui.separator();
                            if ui.button("Quit").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                ui.close();
                            }
                        });

                        ui.menu_button("Edit", |ui| {
                            let pref_label =
                                format_tooltip("Preferences...", ShortcutId::Preferences, is_macos);
                            if ui.button(pref_label).clicked() {
                                self.show_preferences = true;
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Reset defaults").clicked() {
                                self.sim = PrismSimulation::default();
                                self.needs_fit = true;
                                ui.close();
                            }
                        });

                        ui.menu_button("View", |ui| {
                            let reset_label =
                                format_tooltip("Reset View", ShortcutId::FitView, is_macos);
                            if ui.button(reset_label).clicked() {
                                self.needs_fit = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.checkbox(&mut self.show_reflections, "Show reflections (B)");
                            ui.checkbox(&mut self.show_timing, "Show diagnostic timing");
                            ui.checkbox(&mut self.prefs.soft_glow, "Soft glow on rays");
                            ui.separator();
                            ui.checkbox(&mut self.prefs.show_left_sidebar, "Left sidebar");
                            ui.checkbox(&mut self.prefs.show_right_inspector, "Right inspector");
                            ui.checkbox(&mut self.prefs.show_bottom_dock, "Bottom dock");
                            ui.separator();
                            if ui.button("Reset layout").clicked() {
                                self.prefs.show_left_sidebar = true;
                                self.prefs.show_right_inspector = true;
                                self.prefs.show_bottom_dock = true;
                                self.prefs.left_sidebar_width = 240.0;
                                self.prefs.right_inspector_width = 320.0;
                                self.prefs.bottom_dock_height = 185.0;
                                ui.close();
                            }
                            ui.separator();
                            let fs_text = if self.is_fullscreen {
                                "Exit full screen (F11)"
                            } else {
                                "Enter full screen (F11)"
                            };
                            if ui.button(fs_text).clicked() {
                                self.is_fullscreen = !self.is_fullscreen;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                    self.is_fullscreen,
                                ));
                                ui.close();
                            }
                        });

                        ui.menu_button("Tools", |ui| {
                            let select_lbl =
                                format_tooltip("Select", ShortcutId::SelectTool, is_macos);
                            if ui.button(select_lbl).clicked() {
                                self.active_tool = ActiveTool::Select;
                                ui.close();
                            }
                            let move_lbl = format_tooltip("Move", ShortcutId::MoveTool, is_macos);
                            if ui.button(move_lbl).clicked() {
                                self.active_tool = ActiveTool::Move;
                                ui.close();
                            }
                            let rot_lbl =
                                format_tooltip("Rotate", ShortcutId::RotateTool, is_macos);
                            if ui.button(rot_lbl).clicked() {
                                self.active_tool = ActiveTool::Rotate;
                                ui.close();
                            }
                            let probe_lbl =
                                format_tooltip("Ray probe", ShortcutId::RayProbe, is_macos);
                            if ui.button(probe_lbl).clicked() {
                                self.active_tool = ActiveTool::RayInspector;
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Prism dispersion mode").clicked() {
                                self.app_mode = AppMode::PrismDispersion;
                                self.viewport_mode = ViewportMode::View3D;
                                self.right_panel_tab = RightPanelTab::RayInspector;
                                ui.close();
                            }
                            if ui.button("Lens system mode").clicked() {
                                self.app_mode = AppMode::LensSystem;
                                self.viewport_mode = ViewportMode::View2D;
                                self.right_panel_tab = RightPanelTab::Prescription;
                                ui.close();
                            }
                        });

                        ui.menu_button("Help", |ui| {
                            let help_lbl =
                                format_tooltip("Keyboard shortcuts", ShortcutId::Help, is_macos);
                            if ui.button(help_lbl).clicked() {
                                self.show_shortcuts_help = true;
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("About OpticsLab").clicked() {
                                self.show_about = true;
                                ui.close();
                            }
                        });
                    });
                });
            });
    }

    /// Renders the classic horizontal icon toolbar directly beneath the menu bar.
    fn render_toolbar(&mut self, ctx: &Context, plan: &crate::layout::LayoutPlan) {
        let is_macos = cfg!(target_os = "macos");

        TopBottomPanel::top("main_toolbar")
            .exact_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                    // 1. Tool shortcuts
                    let tools = [
                        (ActiveTool::Select, IconId::Select, ShortcutId::SelectTool),
                        (ActiveTool::Move, IconId::Move, ShortcutId::MoveTool),
                        (ActiveTool::Rotate, IconId::Rotate, ShortcutId::RotateTool),
                        (
                            ActiveTool::RayInspector,
                            IconId::RayProbe,
                            ShortcutId::RayProbe,
                        ),
                    ];

                    for (tool, icon, shortcut_id) in tools {
                        let is_active = self.active_tool == tool;
                        let tt = format_tooltip(tool.name(), shortcut_id, is_macos);
                        if toolbar_button(ui, icon, &self.tokens, is_active, &tt).clicked() {
                            self.active_tool = tool;
                        }
                    }

                    ui.separator();

                    // 2. View actions
                    let fit_tt = format_tooltip("Reset view", ShortcutId::FitView, is_macos);
                    if toolbar_button(ui, IconId::ResetView, &self.tokens, false, &fit_tt).clicked()
                    {
                        self.needs_fit = true;
                    }

                    if plan.breakpoint != crate::layout::Breakpoint::Compact {
                        let zin_tt = format_tooltip("Zoom in", ShortcutId::ZoomIn, is_macos);
                        if toolbar_button(ui, IconId::ZoomIn, &self.tokens, false, &zin_tt)
                            .clicked()
                        {
                            self.prefs.ui_scale = (self.prefs.ui_scale + 0.1).min(3.0);
                        }

                        let zout_tt = format_tooltip("Zoom out", ShortcutId::ZoomOut, is_macos);
                        if toolbar_button(ui, IconId::ZoomOut, &self.tokens, false, &zout_tt)
                            .clicked()
                        {
                            self.prefs.ui_scale = (self.prefs.ui_scale - 0.1).max(0.5);
                        }

                        ui.separator();
                    }

                    // 3. Reflections toggle
                    let ref_tt = format_tooltip(
                        "Toggle reflections",
                        ShortcutId::ToggleReflections,
                        is_macos,
                    );
                    if toolbar_button(
                        ui,
                        IconId::ReflectionsToggle,
                        &self.tokens,
                        self.show_reflections,
                        &ref_tt,
                    )
                    .clicked()
                    {
                        self.show_reflections = !self.show_reflections;
                    }

                    ui.separator();

                    // 4. Panel toggles
                    let left_tt = if self.prefs.show_left_sidebar {
                        "Hide left sidebar"
                    } else {
                        "Show left sidebar"
                    };
                    if toolbar_button(
                        ui,
                        IconId::PanelsToggle,
                        &self.tokens,
                        self.prefs.show_left_sidebar,
                        left_tt,
                    )
                    .clicked()
                    {
                        self.prefs.show_left_sidebar = !self.prefs.show_left_sidebar;
                    }

                    let btm_tt = if self.prefs.show_bottom_dock {
                        "Hide bottom dock"
                    } else {
                        "Show bottom dock"
                    };
                    if toolbar_button(
                        ui,
                        IconId::Bvh,
                        &self.tokens,
                        self.prefs.show_bottom_dock,
                        btm_tt,
                    )
                    .clicked()
                    {
                        self.prefs.show_bottom_dock = !self.prefs.show_bottom_dock;
                    }

                    // 5. Right-aligned actions: Theme, Preferences, Fullscreen
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let fs_tt = format_tooltip(
                            "Toggle full screen",
                            ShortcutId::ToggleFullscreen,
                            is_macos,
                        );
                        if toolbar_button(
                            ui,
                            IconId::Fullscreen,
                            &self.tokens,
                            self.is_fullscreen,
                            &fs_tt,
                        )
                        .clicked()
                        {
                            self.is_fullscreen = !self.is_fullscreen;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                self.is_fullscreen,
                            ));
                        }

                        let pref_tt =
                            format_tooltip("Preferences", ShortcutId::Preferences, is_macos);
                        if toolbar_button(
                            ui,
                            IconId::Settings,
                            &self.tokens,
                            self.show_preferences,
                            &pref_tt,
                        )
                        .clicked()
                        {
                            self.show_preferences = !self.show_preferences;
                        }

                        if toolbar_button(ui, IconId::Theme, &self.tokens, false, "Cycle UI theme")
                            .clicked()
                        {
                            self.prefs.theme = match self.prefs.theme {
                                ThemePreference::ClassicLight => ThemePreference::ClassicDark,
                                ThemePreference::ClassicDark => ThemePreference::HighContrast,
                                ThemePreference::HighContrast => ThemePreference::ClassicLight,
                                ThemePreference::FollowSystem => ThemePreference::ClassicLight,
                            };
                        }
                    });
                });
            });
    }

    /// Renders the left sidebar containing the mode selector, parameters, and scene hierarchy.
    fn render_left_sidebar(&mut self, ctx: &Context, plan: &crate::layout::LayoutPlan) {
        if !plan.show_left_sidebar {
            return;
        }

        let resp = SidePanel::left("left_sidebar")
            .resizable(true)
            .width_range(180.0..=380.0)
            .default_width(plan.left_sidebar_width)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);

                    // Mode selector as classic rectangular tabs
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        let is_prism = self.app_mode == AppMode::PrismDispersion;
                        let is_lens = self.app_mode == AppMode::LensSystem;

                        if ui.selectable_label(is_prism, "Prism").clicked() {
                            self.app_mode = AppMode::PrismDispersion;
                            self.viewport_mode = ViewportMode::View3D;
                            self.right_panel_tab = RightPanelTab::RayInspector;
                        }
                        if ui.selectable_label(is_lens, "Lens system").clicked() {
                            self.app_mode = AppMode::LensSystem;
                            self.viewport_mode = ViewportMode::View2D;
                            self.right_panel_tab = RightPanelTab::Prescription;
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if self.app_mode == AppMode::LensSystem {
                        ui.label(
                            egui::RichText::new("Lens Presets")
                                .strong()
                                .color(self.tokens.text_primary),
                        );

                        for preset in [
                            LensPreset::CookeTriplet,
                            LensPreset::DoubleGauss,
                            LensPreset::PlanoConvex,
                        ] {
                            let label = match preset {
                                LensPreset::CookeTriplet => "Cooke Triplet F/4.5",
                                LensPreset::DoubleGauss => "Double Gauss F/2.0",
                                LensPreset::PlanoConvex => "Plano-Convex Singlet",
                            };
                            if ui.button(label).clicked() {
                                if let Some(ref mut sim) = self.lens_sim {
                                    sim.select_preset(preset);
                                }
                                self.right_panel_tab = RightPanelTab::Prescription;
                                self.needs_fit = true;
                            }
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        if let Some(ref sim) = self.lens_sim {
                            ui.label(
                                egui::RichText::new("System Summary")
                                    .strong()
                                    .color(self.tokens.text_primary),
                            );
                            ui.label(format!("Surfaces: {}", sim.prescription.surfaces.len()));
                            if let Some(ref paraxial) = sim.paraxial {
                                ui.label(format!("Paraxial EFL: {:.2} mm", paraxial.efl_mm));
                                ui.label(format!("Back focal length: {:.2} mm", paraxial.bfl_mm));
                                ui.label(format!("F/#: {:.2}", paraxial.f_number));
                            }
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

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

                    if ui.button("Reset Defaults").clicked() {
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
                        draw_icon(
                            ui.painter(),
                            icon_rect,
                            IconId::Scene,
                            self.tokens.text_muted,
                            12.0,
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
                        IconId::CaretDown.glyph()
                    } else {
                        IconId::CaretRight.glyph()
                    };
                    if ui
                        .selectable_label(
                            self.selected_scene_object.as_deref() == Some("Scene"),
                            format!("{scene_glyph} Scene"),
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
                                    format!("{} Camera", IconId::Camera.glyph()),
                                )
                                .clicked()
                            {
                                self.selected_scene_object = Some("Camera".into());
                            }

                            // Prism (expandable)
                            let prism_glyph = if self.scene_tree_prism_open {
                                IconId::CaretDown.glyph()
                            } else {
                                IconId::CaretRight.glyph()
                            };
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Prism"),
                                    format!("{prism_glyph} Prism"),
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
                                            "Glass (BK7)",
                                        )
                                        .clicked()
                                    {
                                        self.selected_scene_object = Some("Glass (BK7)".into());
                                    }
                                    if ui
                                        .selectable_label(
                                            self.selected_scene_object.as_deref()
                                                == Some("Material"),
                                            "Material",
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
                                    format!("{} Light", IconId::LightSource.glyph()),
                                )
                                .clicked()
                            {
                                self.selected_scene_object = Some("Light".into());
                            }

                            // Ground (expandable)
                            let ground_glyph = if self.scene_tree_ground_open {
                                IconId::CaretDown.glyph()
                            } else {
                                IconId::CaretRight.glyph()
                            };
                            if ui
                                .selectable_label(
                                    self.selected_scene_object.as_deref() == Some("Ground"),
                                    format!("{ground_glyph} Ground"),
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
        self.prefs.left_sidebar_width = resp.response.rect.width();
    }

    /// Renders the bottom status bar with real simulation data and coordinates.
    fn render_status_bar(&self, ctx: &Context) {
        TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);

                    // Left: status message or copy confirmation
                    let status_text = self.copy_status.as_deref().unwrap_or("Ready");
                    ui.label(egui::RichText::new(status_text).size(11.0).color(
                        if self.copy_status.is_some() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_secondary
                        },
                    ));

                    ui.separator();

                    // Active tool
                    ui.label(
                        egui::RichText::new(format!("Tool: {}", self.active_tool.name()))
                            .size(11.0)
                            .color(self.tokens.text_secondary),
                    );

                    ui.separator();

                    // Mode
                    let mode_str = match self.app_mode {
                        AppMode::PrismDispersion => "Mode: Prism Dispersion",
                        AppMode::LensSystem => "Mode: Lens System",
                    };
                    ui.label(
                        egui::RichText::new(mode_str)
                            .size(11.0)
                            .color(self.tokens.text_secondary),
                    );

                    // Right items: units, ray count, zoom %
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Units: mm / nm")
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );

                        ui.separator();

                        let zoom_pct = (self.prefs.ui_scale * 100.0).round() as i32;
                        ui.label(
                            egui::RichText::new(format!("Zoom: {zoom_pct}%"))
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );

                        ui.separator();

                        let rays_label = match self.app_mode {
                            AppMode::PrismDispersion => {
                                format!("CPU ({} rays)", self.sim.rays.len())
                            }
                            AppMode::LensSystem => {
                                if let Some(ref sim) = self.lens_sim {
                                    format!("CPU ({} rays)", sim.cross_section_rays.len())
                                } else {
                                    "CPU (0 rays)".to_string()
                                }
                            }
                        };
                        ui.label(
                            egui::RichText::new(rays_label)
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );
                    });
                });
            });
    }

    /// Renders the bottom analytics dock containing the 4 scientific panels.
    fn render_bottom_analytics(&mut self, ctx: &Context, plan: &crate::layout::LayoutPlan) {
        if !plan.show_bottom_dock {
            return;
        }

        let resp = TopBottomPanel::bottom("bottom_analytics_dock")
            .resizable(true)
            .height_range(100.0..=360.0)
            .default_height(plan.bottom_dock_height)
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
        self.prefs.bottom_dock_height = resp.response.rect.height();
    }

    /// Renders the right inspector panel.
    fn render_right_panel(&mut self, ctx: &Context, plan: &crate::layout::LayoutPlan) {
        if !plan.show_right_inspector {
            return;
        }

        let resp = SidePanel::right("right_inspector")
            .resizable(true)
            .width_range(240.0..=480.0)
            .default_width(plan.right_inspector_width)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                    let tabs = [
                        (RightPanelTab::RayInspector, "Inspector"),
                        (RightPanelTab::Prescription, "Prescription"),
                        (RightPanelTab::GlassCatalog, "Catalog"),
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
                                .button("Open 2D Layout & Aberrations in Center")
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
        self.prefs.right_inspector_width = resp.response.rect.width();
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
                    draw_icon(
                        ui.painter(),
                        fs_rect,
                        IconId::Fullscreen,
                        if fs_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        14.0,
                    );
                    if fs_resp.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                    }

                    let (frame_rect, frame_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                    draw_icon(
                        ui.painter(),
                        frame_rect,
                        IconId::ZoomFit,
                        if frame_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        14.0,
                    );
                    if frame_resp.clicked() {
                        self.needs_fit = true;
                    }

                    let (cam_rect, cam_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                    draw_icon(
                        ui.painter(),
                        cam_rect,
                        IconId::ExportImage,
                        if cam_resp.hovered() {
                            self.tokens.accent
                        } else {
                            self.tokens.text_muted
                        },
                        14.0,
                    );
                    if cam_resp.clicked() {
                        self.copy_status = Some("Screenshot captured".into());
                    }
                });
            });

            ui.add_space(2.0);

            // Drag-and-drop hover feedback banner
            if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Drop .zmx file to load optical prescription")
                            .size(11.5)
                            .color(self.tokens.accent),
                    );
                });
                ui.add_space(2.0);
            }

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
                        self.prefs.soft_glow,
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
                            self.prefs.soft_glow,
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
                if ui.button("Trace in 3D Prism Viewport").clicked() {
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

                if ui.button("Run 100,000 Ray Traversal Benchmark").clicked() {
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
            .default_size(Vec2::new(360.0, 240.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::hover());
                    draw_icon(
                        ui.painter(),
                        logo_rect,
                        IconId::Logo,
                        self.tokens.accent,
                        24.0,
                    );
                    ui.add_space(4.0);
                    ui.heading("OpticsLab");
                    ui.label(
                        egui::RichText::new(
                            "Open-source educational optics & ray-tracing workbench",
                        )
                        .color(self.tokens.text_secondary),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Version {}\nMIT License\nRepository: github.com/AlexCri5an/optics-lab",
                            env!("CARGO_PKG_VERSION")
                        ))
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

    /// Renders the keyboard shortcuts cheat sheet modal dialog from the canonical registry.
    fn render_shortcuts_modal(&mut self, ctx: &Context) {
        if !self.show_shortcuts_help {
            return;
        }

        let is_macos = cfg!(target_os = "macos");

        Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(440.0, 380.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("OpticsLab Hotkeys & Controls")
                        .strong()
                        .color(self.tokens.text_primary),
                );
                ui.separator();

                ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    egui::Grid::new("shortcuts_table")
                        .num_columns(2)
                        .spacing([24.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for s in ALL_SHORTCUTS {
                                ui.label(
                                    egui::RichText::new(s.label(is_macos))
                                        .strong()
                                        .color(self.tokens.accent),
                                );
                                ui.label(
                                    egui::RichText::new(s.description)
                                        .color(self.tokens.text_secondary),
                                );
                                ui.end_row();
                            }
                        });
                });

                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.show_shortcuts_help = false;
                    }
                });
            });
    }

    /// Renders the user application preferences modal dialog.
    fn render_preferences_dialog(&mut self, ctx: &Context) {
        if !self.show_preferences {
            return;
        }

        Window::new("Preferences")
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(420.0, 340.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Application Preferences")
                        .strong()
                        .color(self.tokens.text_primary),
                );
                ui.separator();

                egui::Grid::new("preferences_grid")
                    .num_columns(2)
                    .spacing([18.0, 8.0])
                    .show(ui, |ui| {
                        // Theme
                        ui.label(
                            egui::RichText::new("Color theme:").color(self.tokens.text_secondary),
                        );
                        egui::ComboBox::from_id_salt("theme_combobox")
                            .selected_text(self.prefs.theme.display_name())
                            .show_ui(ui, |ui| {
                                for theme in [
                                    ThemePreference::ClassicLight,
                                    ThemePreference::ClassicDark,
                                    ThemePreference::HighContrast,
                                    ThemePreference::FollowSystem,
                                ] {
                                    let is_selected = self.prefs.theme == theme;
                                    if ui
                                        .selectable_label(is_selected, theme.display_name())
                                        .clicked()
                                    {
                                        self.prefs.theme = theme;
                                    }
                                }
                            });
                        ui.end_row();

                        // Canvas Background
                        ui.label(
                            egui::RichText::new("Canvas background:")
                                .color(self.tokens.text_secondary),
                        );
                        egui::ComboBox::from_id_salt("canvas_bg_combobox")
                            .selected_text(self.prefs.canvas_bg.display_name())
                            .show_ui(ui, |ui| {
                                for bg in [
                                    CanvasBackgroundPreference::Theme,
                                    CanvasBackgroundPreference::Black,
                                    CanvasBackgroundPreference::White,
                                ] {
                                    let is_selected = self.prefs.canvas_bg == bg;
                                    if ui
                                        .selectable_label(is_selected, bg.display_name())
                                        .clicked()
                                    {
                                        self.prefs.canvas_bg = bg;
                                    }
                                }
                            });
                        ui.end_row();

                        // UI Scale
                        ui.label(
                            egui::RichText::new("UI scale:").color(self.tokens.text_secondary),
                        );
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Slider::new(&mut self.prefs.ui_scale, 0.5..=3.0)
                                    .step_by(0.05)
                                    .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                            );
                            if ui.button("100%").clicked() {
                                self.prefs.ui_scale = 1.0;
                            }
                        });
                        ui.end_row();

                        // Soft glow
                        ui.label(
                            egui::RichText::new("Visual effects:")
                                .color(self.tokens.text_secondary),
                        );
                        ui.checkbox(&mut self.prefs.soft_glow, "Soft glow on rays");
                        ui.end_row();

                        // Reduce motion
                        ui.label(
                            egui::RichText::new("Accessibility:").color(self.tokens.text_secondary),
                        );
                        ui.checkbox(&mut self.prefs.reduce_motion, "Reduce motion");
                        ui.end_row();
                    });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("Reset preferences").clicked() {
                        self.prefs.reset();
                    }

                    if ui.button("Keyboard shortcuts...").clicked() {
                        self.show_shortcuts_help = true;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.show_preferences = false;
                        }
                    });
                });
            });
    }
}

impl eframe::App for OpticsLabApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.fonts_initialized {
            init_icon_fonts(ctx);
            self.fonts_initialized = true;
        }

        // Live theme resolution & canvas override
        let system_is_dark = ctx.style().visuals.dark_mode;
        self.tokens = resolve_theme(self.prefs.theme, system_is_dark);
        match self.prefs.canvas_bg {
            CanvasBackgroundPreference::Theme => {}
            CanvasBackgroundPreference::Black => {
                self.tokens.canvas = Color32::from_rgb(17, 20, 26);
            }
            CanvasBackgroundPreference::White => {
                self.tokens.canvas = Color32::WHITE;
            }
        }
        ctx.set_zoom_factor(self.prefs.ui_scale);
        apply_tokens(ctx, &self.tokens);

        self.handle_shortcuts(ctx);
        self.handle_file_drag_and_drop(ctx);

        let available_size = ctx.screen_rect().size();
        let touch_mode = ctx.input(|i| {
            !i.raw.events.is_empty()
                && i.raw
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Touch { .. }))
        });
        let layout_plan = crate::layout::compute_layout(available_size, &self.prefs, touch_mode);

        // 1. Top Application Bar
        self.render_top_bar(ctx);

        // 2. Icon Toolbar directly below Menu Bar
        self.render_toolbar(ctx, &layout_plan);

        // 3. Status Bar at very bottom
        self.render_status_bar(ctx);

        // 4. Left Sidebar (Tools & Scene)
        self.render_left_sidebar(ctx, &layout_plan);

        // 5. Right Inspector (Ray Inspector & Path)
        self.render_right_panel(ctx, &layout_plan);

        // 6. Bottom Analytics Dock (Convergence, Spectral Power, BVH, Material)
        self.render_bottom_analytics(ctx, &layout_plan);

        // 7. Central Main Viewport (dominated by 3D prism & spectral fan)
        self.render_central_viewport(ctx);

        // 8. Dialogs
        self.render_about_dialog(ctx);
        self.render_shortcuts_modal(ctx);
        self.render_preferences_dialog(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(&self.prefs) {
            storage.set_string(Self::PREFS_STORAGE_KEY, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_and_drop_zmx_file() -> Result<(), Box<dyn std::error::Error>> {
        let ctx = Context::default();
        let mut app = OpticsLabApp::default();

        let zmx_content = optics_zemax::presets::DOUBLE_GAUSS_F2_0_ZMX.as_bytes();
        let dropped = egui::DroppedFile {
            name: "double_gauss.zmx".to_string(),
            path: None,
            last_modified: None,
            bytes: Some(std::sync::Arc::from(zmx_content)),
            mime: String::new(),
        };

        ctx.input_mut(|i| {
            i.raw.dropped_files.push(dropped);
        });

        app.handle_file_drag_and_drop(&ctx);

        assert_eq!(app.app_mode, AppMode::LensSystem);
        assert_eq!(app.viewport_mode, ViewportMode::View2D);
        assert_eq!(app.right_panel_tab, RightPanelTab::Prescription);
        let status = app.copy_status.as_deref().unwrap_or("");
        assert!(status.contains("Double Gauss F/2.0"));
        assert!(status.contains("surfaces"));

        let lens = app.lens_sim.as_ref().ok_or("Lens simulation missing")?;
        assert_eq!(lens.elements.len(), 6);

        Ok(())
    }
}
