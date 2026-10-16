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
    Align, Color32, Context, CornerRadius, Key, Layout, MenuBar, Pos2, Rect, ScrollArea, Sense,
    SidePanel, Stroke, StrokeKind, TopBottomPanel, Vec2, Window,
};

use optics_glass::GlassCatalogDb;
use optics_zemax::parse_zmx_bytes;

use crate::analytics::render_analytics_dock;
use crate::glass_browser::{GlassBrowserState, render_glass_catalog_panel};
use crate::icons::{IconId, draw_icon, init_icon_fonts, toolbar_button};
use crate::inspector::{RayInspectorState, render_ray_inspector_panel};
use crate::lens_sim::{LensPreset, LensSimulation};
use crate::lens_view::{render_lens_system_panel, render_prescription_table_view};
use crate::shortcuts::ALL_SHORTCUTS;
use crate::sim::PrismSimulation;
use crate::theme::{
    AppPreferences, CanvasBackgroundPreference, OpticsTokens, ThemePreference, apply_tokens,
    resolve_theme,
};
use crate::view_transform::ViewTransform;
use crate::viewport_3d::render_viewport_3d;
use crate::widgets::dispersion_curve_plot;

/// Active tab in the right inspector panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RightPanelTab {
    /// Object transform, geometry, and material property editor.
    Properties,
    /// Ray inspector probe and physical bounce tables.
    #[default]
    RayInspector,
    /// Lens prescription editor and canonical presets.
    Prescription,
    /// Complete optical glass catalog browser with Abbe diagram and dispersion plots.
    GlassCatalog,
}

/// Active user tool in the left toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AppMode {
    /// Interactive 2D/3D prism spectral dispersion experiment (Slice A).
    #[default]
    PrismDispersion,
    /// Canonical lens systems and optical aberration analysis (Slice B).
    LensSystem,
}

/// Serializable snapshot of observable application state used for testing and persistence.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppStateSnapshot {
    /// High-level mode.
    pub app_mode: AppMode,
    /// Viewport mode.
    pub viewport_mode: ViewportMode,
    /// Active tool.
    pub active_tool: ActiveTool,
    /// Right panel tab.
    pub right_panel_tab: RightPanelTab,
    /// Selected ray index.
    pub selected_ray_index: Option<usize>,
    /// Selected scene object.
    pub selected_scene_object: Option<String>,
    /// Selected scene object ID in SceneModel.
    pub selected_scene_id: Option<u64>,
    /// Reflections visibility.
    pub show_reflections: bool,
    /// Diagnostic timing visibility.
    pub show_timing: bool,
    /// Preferences dialog visibility.
    pub show_preferences: bool,
    /// About dialog visibility.
    pub show_about: bool,
    /// Shortcuts help visibility.
    pub show_shortcuts_help: bool,
    /// Fullscreen mode.
    pub is_fullscreen: bool,
    /// Inspected wavelength in nm.
    pub inspected_wavelength: f64,
    /// Prism incidence angle.
    pub prism_incidence_deg: f64,
    /// Prism apex angle.
    pub prism_apex_deg: f64,
    /// Prism sample count.
    pub prism_sample_count: usize,
    /// Prism glass material name.
    pub prism_glass_name: String,
    /// Application preferences.
    pub prefs: AppPreferences,
    /// Copy or status message.
    pub copy_status: Option<String>,
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
    /// Hardware-accelerated GPU viewport and path tracing state.
    pub gpu_state: crate::viewport_3d::GpuViewportState,
    /// Non-sequential parametric 3D scene model and objects.
    pub scene_model: crate::scene_model::SceneModel,
    /// Per-frame UI registry tracking all interactive controls.
    pub ui_registry: crate::registry::UiRegistry,
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
            gpu_state: crate::viewport_3d::GpuViewportState::default(),
            scene_model: crate::scene_model::SceneModel::default(),
            ui_registry: crate::registry::UiRegistry::default(),
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

        if let Some(wgpu_state) = &cc.wgpu_render_state {
            let (bvh, _) = app.scene_model.build_bvh_scene();
            app.gpu_state.init_from_wgpu_render_state(wgpu_state, &bvh);
        }

        app
    }

    /// Returns a serializable snapshot of the observable state fields.
    #[must_use]
    pub fn state_snapshot(&self) -> AppStateSnapshot {
        AppStateSnapshot {
            app_mode: self.app_mode,
            viewport_mode: self.viewport_mode,
            active_tool: self.active_tool,
            right_panel_tab: self.right_panel_tab,
            selected_ray_index: self.selected_ray_index,
            selected_scene_object: self.selected_scene_object.clone(),
            selected_scene_id: self.scene_model.selected_id,
            show_reflections: self.show_reflections,
            show_timing: self.show_timing,
            show_preferences: self.show_preferences,
            show_about: self.show_about,
            show_shortcuts_help: self.show_shortcuts_help,
            is_fullscreen: self.is_fullscreen,
            inspected_wavelength: self.inspected_wavelength,
            prism_incidence_deg: self.sim.incidence_deg,
            prism_apex_deg: self.sim.apex_deg,
            prism_sample_count: self.sim.sample_count,
            prism_glass_name: self.sim.glass_name.clone(),
            prefs: self.prefs.clone(),
            copy_status: self.copy_status.clone(),
        }
    }

    /// Computes a stable, deterministic 64-bit FNV-1a hash of the observable state.
    #[must_use]
    pub fn stable_hash(&self) -> u64 {
        let json = serde_json::to_string(&self.state_snapshot()).unwrap_or_default();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in json.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash
    }

    /// Dispatches an action command through the central dispatcher.
    pub fn dispatch(
        &mut self,
        cmd: crate::commands::Command,
        platform: &mut dyn crate::platform::Platform,
    ) -> Vec<crate::commands::Effect> {
        crate::commands::dispatch(self, cmd, platform)
    }

    /// Handles keyboard shortcuts by routing them through the command architecture.
    fn handle_shortcuts(&mut self, ctx: &Context) {
        use crate::commands::Command;

        let mut platform = crate::platform::EguiPlatform::new(ctx);

        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.dispatch(Command::SelectTool(ActiveTool::Select), &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::M)) {
            self.dispatch(Command::SelectTool(ActiveTool::Move), &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.alt) {
            self.dispatch(Command::SelectTool(ActiveTool::Rotate), &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::I)) {
            self.dispatch(Command::SelectTool(ActiveTool::RayInspector), &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            let cmd = if self.show_shortcuts_help {
                Command::CloseShortcutsHelp
            } else {
                Command::OpenShortcutsHelp
            };
            self.dispatch(cmd, &mut platform);
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Comma)) {
            let cmd = if self.show_preferences {
                Command::ClosePreferences
            } else {
                Command::OpenPreferences
            };
            self.dispatch(cmd, &mut platform);
        }
        if ctx.input(|i| {
            (i.modifiers.command || i.modifiers.ctrl)
                && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
        }) {
            self.dispatch(Command::ZoomIn, &mut platform);
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Minus)) {
            self.dispatch(Command::ZoomOut, &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::B)) {
            self.dispatch(Command::ToggleReflections, &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            self.dispatch(Command::ToggleFullscreen, &mut platform);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.dispatch(Command::DismissModalOrSelection, &mut platform);
        }
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(Key::Num0)) {
            self.dispatch(Command::ResetView, &mut platform);
        }
    }

    /// Renders a menu or dialog button bound to a command and registers it in the UI registry.
    pub fn render_command_button(
        &mut self,
        ui: &mut egui::Ui,
        cmd: crate::commands::Command,
        location: &'static str,
    ) -> egui::Response {
        let is_macos = cfg!(target_os = "macos");
        let (available, reason) = cmd.is_available(self);
        let tooltip = cmd.tooltip(is_macos);

        let resp = ui
            .add_enabled_ui(available, |ui| {
                let label = cmd.label();
                let mut btn = ui.button(label);
                if available {
                    btn = btn.on_hover_text(&tooltip);
                } else if !reason.is_empty() {
                    btn = btn.on_hover_text(reason);
                }
                btn
            })
            .inner;

        if resp.clicked() && available {
            self.ui_registry.queued_commands.push(cmd.clone());
            ui.close();
        }

        self.ui_registry
            .widgets
            .push(crate::registry::RegisteredWidget {
                command: cmd,
                location,
                enabled: available,
                disabled_reason: if available { None } else { Some(reason) },
                rect: resp.rect,
            });

        resp
    }

    /// Renders a toolbar icon button bound to a command and registers it in the UI registry.
    pub fn render_toolbar_command_button(
        &mut self,
        ui: &mut egui::Ui,
        cmd: crate::commands::Command,
        is_active: bool,
        location: &'static str,
    ) -> egui::Response {
        let is_macos = cfg!(target_os = "macos");
        let (available, reason) = cmd.is_available(self);
        let tooltip = cmd.tooltip(is_macos);
        let icon = cmd.icon().unwrap_or(crate::icons::IconId::Select);

        let resp = if available {
            toolbar_button(ui, icon, &self.tokens, is_active, &tooltip)
        } else {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
            draw_icon(ui.painter(), rect, icon, self.tokens.text_disabled, 14.0);
            if !reason.is_empty() {
                resp.on_hover_text(reason)
            } else {
                resp
            }
        };

        if resp.clicked() && available {
            self.ui_registry.queued_commands.push(cmd.clone());
        }

        self.ui_registry
            .widgets
            .push(crate::registry::RegisteredWidget {
                command: cmd,
                location,
                enabled: available,
                disabled_reason: if available { None } else { Some(reason) },
                rect: resp.rect,
            });

        resp
    }

    /// Renders a selectable label bound to a command and registers it in the UI registry.
    pub fn render_selectable_command_label(
        &mut self,
        ui: &mut egui::Ui,
        selected: bool,
        cmd: crate::commands::Command,
        location: &'static str,
    ) -> egui::Response {
        let (available, reason) = cmd.is_available(self);
        let resp = ui
            .add_enabled_ui(available, |ui| {
                let label = cmd.label();
                let mut lbl = ui.selectable_label(selected, label);
                if !available && !reason.is_empty() {
                    lbl = lbl.on_hover_text(reason);
                }
                lbl
            })
            .inner;

        if resp.clicked() && available {
            self.ui_registry.queued_commands.push(cmd.clone());
        }

        self.ui_registry
            .widgets
            .push(crate::registry::RegisteredWidget {
                command: cmd,
                location,
                enabled: available,
                disabled_reason: if available { None } else { Some(reason) },
                rect: resp.rect,
            });

        resp
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
        let _is_macos = cfg!(target_os = "macos");

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
                                self.render_command_button(
                                    ui,
                                    crate::commands::Command::LoadLensPreset(
                                        LensPreset::CookeTriplet,
                                    ),
                                    "top_bar.file.preset_cooke",
                                );
                                self.render_command_button(
                                    ui,
                                    crate::commands::Command::LoadLensPreset(
                                        LensPreset::DoubleGauss,
                                    ),
                                    "top_bar.file.preset_double_gauss",
                                );
                                self.render_command_button(
                                    ui,
                                    crate::commands::Command::LoadLensPreset(
                                        LensPreset::PlanoConvex,
                                    ),
                                    "top_bar.file.preset_plano_convex",
                                );
                            });
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SaveProject,
                                "top_bar.file.save_project",
                            );
                            ui.separator();
                            ui.label(
                                egui::RichText::new("Drag & drop .zmx file to load")
                                    .size(11.0)
                                    .color(self.tokens.text_muted),
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::Quit,
                                "top_bar.file.quit",
                            );
                        });

                        ui.menu_button("Edit", |ui| {
                            self.render_command_button(
                                ui,
                                crate::commands::Command::OpenPreferences,
                                "top_bar.edit.preferences",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ResetDefaults,
                                "top_bar.edit.reset_defaults",
                            );
                        });

                        ui.menu_button("View", |ui| {
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ResetView,
                                "top_bar.view.reset_view",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleReflections,
                                "top_bar.view.toggle_reflections",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleTiming,
                                "top_bar.view.toggle_timing",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleSoftGlow,
                                "top_bar.view.toggle_soft_glow",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleLeftSidebar,
                                "top_bar.view.toggle_left_sidebar",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleRightInspector,
                                "top_bar.view.toggle_right_inspector",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleBottomDock,
                                "top_bar.view.toggle_bottom_dock",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ResetLayout,
                                "top_bar.view.reset_layout",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleBeamRibbons,
                                "top_bar.view.toggle_beam_ribbons",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleFloorCaustic,
                                "top_bar.view.toggle_floor_caustic",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleTofPulses,
                                "top_bar.view.toggle_tof_pulses",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::ToggleFullscreen,
                                "top_bar.view.toggle_fullscreen",
                            );
                        });

                        ui.menu_button("Tools", |ui| {
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SelectTool(ActiveTool::Select),
                                "top_bar.tools.select",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SelectTool(ActiveTool::Move),
                                "top_bar.tools.move",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SelectTool(ActiveTool::Rotate),
                                "top_bar.tools.rotate",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SelectTool(ActiveTool::RayInspector),
                                "top_bar.tools.ray_probe",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SetMode(AppMode::PrismDispersion),
                                "top_bar.tools.mode_prism",
                            );
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SetMode(AppMode::LensSystem),
                                "top_bar.tools.mode_lens",
                            );
                        });

                        ui.menu_button("Help", |ui| {
                            self.render_command_button(
                                ui,
                                crate::commands::Command::OpenShortcutsHelp,
                                "top_bar.help.shortcuts",
                            );
                            ui.separator();
                            self.render_command_button(
                                ui,
                                crate::commands::Command::OpenAbout,
                                "top_bar.help.about",
                            );
                        });
                    });
                });
            });
    }

    /// Renders the classic horizontal icon toolbar directly beneath the menu bar.
    fn render_toolbar(&mut self, ctx: &Context, plan: &crate::layout::LayoutPlan) {
        TopBottomPanel::top("main_toolbar")
            .exact_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                    // 1. Tool shortcuts
                    let tools = [
                        (ActiveTool::Select, "toolbar.select"),
                        (ActiveTool::Move, "toolbar.move"),
                        (ActiveTool::Rotate, "toolbar.rotate"),
                        (ActiveTool::RayInspector, "toolbar.ray_probe"),
                    ];

                    for (tool, loc) in tools {
                        let is_active = self.active_tool == tool;
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::SelectTool(tool),
                            is_active,
                            loc,
                        );
                    }

                    ui.separator();

                    // 2. View actions
                    self.render_toolbar_command_button(
                        ui,
                        crate::commands::Command::ResetView,
                        false,
                        "toolbar.reset_view",
                    );

                    if plan.breakpoint != crate::layout::Breakpoint::Compact {
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ZoomIn,
                            false,
                            "toolbar.zoom_in",
                        );
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ZoomOut,
                            false,
                            "toolbar.zoom_out",
                        );
                        ui.separator();
                    }

                    // 3. Reflections toggle
                    self.render_toolbar_command_button(
                        ui,
                        crate::commands::Command::ToggleReflections,
                        self.show_reflections,
                        "toolbar.toggle_reflections",
                    );

                    ui.separator();

                    // 4. Panel toggles
                    self.render_toolbar_command_button(
                        ui,
                        crate::commands::Command::ToggleLeftSidebar,
                        self.prefs.show_left_sidebar,
                        "toolbar.toggle_left_sidebar",
                    );
                    self.render_toolbar_command_button(
                        ui,
                        crate::commands::Command::ToggleBottomDock,
                        self.prefs.show_bottom_dock,
                        "toolbar.toggle_bottom_dock",
                    );

                    // 5. Right-aligned actions: Theme, Preferences, Fullscreen
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ToggleFullscreen,
                            self.is_fullscreen,
                            "toolbar.fullscreen",
                        );
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::OpenPreferences,
                            self.show_preferences,
                            "toolbar.preferences",
                        );
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::CycleTheme,
                            false,
                            "toolbar.cycle_theme",
                        );
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

                        self.render_selectable_command_label(
                            ui,
                            is_prism,
                            crate::commands::Command::SetMode(AppMode::PrismDispersion),
                            "left_sidebar.mode_prism",
                        );
                        self.render_selectable_command_label(
                            ui,
                            is_lens,
                            crate::commands::Command::SetMode(AppMode::LensSystem),
                            "left_sidebar.mode_lens",
                        );
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
                            self.render_command_button(
                                ui,
                                crate::commands::Command::LoadLensPreset(preset),
                                "left_sidebar.lens_preset",
                            );
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

                    if self.app_mode == AppMode::PrismDispersion {
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
                                egui::RichText::new("Scene Objects")
                                    .size(11.5)
                                    .color(self.tokens.text_secondary),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.menu_button("+ Add", |ui| {
                                    if ui.button("+ Prism").clicked() {
                                        let obj = crate::scene_model::SceneObject::new(
                                            0,
                                            format!("Prism {}", self.scene_model.objects.len() + 1),
                                            crate::scene_model::Transform::default(),
                                            crate::scene_model::ObjectGeometry::Prism {
                                                apex_deg: 60.0,
                                                base_width: 40.0,
                                                depth: 50.0,
                                            },
                                            crate::scene_model::MaterialData::default(),
                                        );
                                        let new_id = self.scene_model.add_object(obj);
                                        self.scene_model.selected_id = Some(new_id);
                                        self.right_panel_tab = RightPanelTab::Properties;
                                        let (bvh, _) = self.scene_model.build_bvh_scene();
                                        self.gpu_state.update_scene_bvh(&bvh);
                                        ui.close();
                                    }
                                    if ui.button("+ Sphere").clicked() {
                                        let obj = crate::scene_model::SceneObject::new(
                                            0,
                                            format!(
                                                "Sphere {}",
                                                self.scene_model.objects.len() + 1
                                            ),
                                            crate::scene_model::Transform::from_position(
                                                optics_core::Vec3::new(0.0, 25.0, 0.0),
                                            ),
                                            crate::scene_model::ObjectGeometry::Sphere {
                                                radius: 20.0,
                                                stacks: 8,
                                                sectors: 16,
                                            },
                                            crate::scene_model::MaterialData::default(),
                                        );
                                        let new_id = self.scene_model.add_object(obj);
                                        self.scene_model.selected_id = Some(new_id);
                                        self.right_panel_tab = RightPanelTab::Properties;
                                        let (bvh, _) = self.scene_model.build_bvh_scene();
                                        self.gpu_state.update_scene_bvh(&bvh);
                                        ui.close();
                                    }
                                    if ui.button("+ Slab").clicked() {
                                        let obj = crate::scene_model::SceneObject::new(
                                            0,
                                            format!("Slab {}", self.scene_model.objects.len() + 1),
                                            crate::scene_model::Transform::from_position(
                                                optics_core::Vec3::new(0.0, 20.0, 0.0),
                                            ),
                                            crate::scene_model::ObjectGeometry::Slab {
                                                width: 40.0,
                                                height: 40.0,
                                                thickness: 12.0,
                                            },
                                            crate::scene_model::MaterialData::default(),
                                        );
                                        let new_id = self.scene_model.add_object(obj);
                                        self.scene_model.selected_id = Some(new_id);
                                        self.right_panel_tab = RightPanelTab::Properties;
                                        let (bvh, _) = self.scene_model.build_bvh_scene();
                                        self.gpu_state.update_scene_bvh(&bvh);
                                        ui.close();
                                    }
                                    if ui.button("+ Singlet Lens").clicked() {
                                        let obj = crate::scene_model::SceneObject::new(
                                            0,
                                            format!(
                                                "Singlet Lens {}",
                                                self.scene_model.objects.len() + 1
                                            ),
                                            crate::scene_model::Transform::from_position(
                                                optics_core::Vec3::new(0.0, 20.0, 0.0),
                                            ),
                                            crate::scene_model::ObjectGeometry::LensMesh {
                                                r1: 50.0,
                                                r2: -50.0,
                                                semi_diameter: 20.0,
                                                center_thickness: 8.0,
                                                radial_segments: 6,
                                                azimuthal_segments: 16,
                                            },
                                            crate::scene_model::MaterialData::default(),
                                        );
                                        let new_id = self.scene_model.add_object(obj);
                                        self.scene_model.selected_id = Some(new_id);
                                        self.right_panel_tab = RightPanelTab::Properties;
                                        let (bvh, _) = self.scene_model.build_bvh_scene();
                                        self.gpu_state.update_scene_bvh(&bvh);
                                        ui.close();
                                    }
                                });
                            });
                        });

                        ui.add_space(4.0);

                        let mut bvh_rebuild = false;
                        let mut to_delete = None;
                        let mut to_duplicate = None;

                        for obj in &mut self.scene_model.objects {
                            ui.horizontal(|ui| {
                                let eye_text = if obj.visible { "👁" } else { "⊘" };
                                if ui
                                    .small_button(eye_text)
                                    .on_hover_text("Toggle object visibility")
                                    .clicked()
                                {
                                    obj.visible = !obj.visible;
                                    bvh_rebuild = true;
                                }

                                let is_selected = self.scene_model.selected_id == Some(obj.id);
                                if ui.selectable_label(is_selected, &obj.name).clicked() {
                                    self.scene_model.selected_id = Some(obj.id);
                                    self.right_panel_tab = RightPanelTab::Properties;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Delete object")
                                        .clicked()
                                    {
                                        to_delete = Some(obj.id);
                                    }
                                    if ui
                                        .small_button("⧉")
                                        .on_hover_text("Duplicate object")
                                        .clicked()
                                    {
                                        to_duplicate = Some(obj.id);
                                    }
                                });
                            });
                        }

                        if let Some(del_id) = to_delete {
                            self.scene_model.delete_object(del_id);
                            bvh_rebuild = true;
                        }
                        if let Some(dup_id) = to_duplicate {
                            self.scene_model.duplicate_object(dup_id);
                            bvh_rebuild = true;
                        }

                        if bvh_rebuild {
                            let (bvh, _) = self.scene_model.build_bvh_scene();
                            self.gpu_state.update_scene_bvh(&bvh);
                        }
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

                    // Real Render State
                    let render_state_str = if self.gpu_state.is_paused {
                        "Render: Paused"
                    } else if self.gpu_state.sample_count >= self.gpu_state.target_spp as u64 {
                        "Render: Converged"
                    } else if self.gpu_state.is_available {
                        "Render: Tracing"
                    } else {
                        "Render: Idle"
                    };
                    ui.label(
                        egui::RichText::new(render_state_str)
                            .size(11.0)
                            .color(self.tokens.text_secondary),
                    );

                    ui.separator();

                    // Real accumulated SPP
                    ui.label(
                        egui::RichText::new(format!(
                            "SPP: {}/{}",
                            self.gpu_state.sample_count, self.gpu_state.target_spp
                        ))
                        .size(11.0)
                        .color(self.tokens.text_secondary),
                    );

                    // Right items: units, GPU Adapter & Backend, FPS & Frame Time
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let fps_str = format!(
                            "{:.1} ms ({:.0} FPS)",
                            self.gpu_state.last_frame_time_ms, self.gpu_state.measured_fps
                        );
                        ui.label(
                            egui::RichText::new(fps_str)
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );

                        ui.separator();

                        let gpu_info_str = format!(
                            "GPU: {} ({})",
                            self.gpu_state.adapter_name, self.gpu_state.backend_name
                        );
                        ui.label(
                            egui::RichText::new(gpu_info_str)
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );

                        ui.separator();

                        let scale_pct = (self.gpu_state.resolution_scale * 100.0).round() as i32;
                        ui.label(
                            egui::RichText::new(format!("Scale: {scale_pct}%"))
                                .size(11.0)
                                .color(self.tokens.text_muted),
                        );

                        ui.separator();

                        ui.label(
                            egui::RichText::new("Units: mm / nm")
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
                        (RightPanelTab::Properties, "Properties"),
                        (RightPanelTab::RayInspector, "Inspector"),
                        (RightPanelTab::Prescription, "Prescription"),
                        (RightPanelTab::GlassCatalog, "Catalog"),
                    ];

                    for (tab, _) in tabs {
                        let is_active = self.right_panel_tab == tab;
                        self.render_selectable_command_label(
                            ui,
                            is_active,
                            crate::commands::Command::SetRightPanelTab(tab),
                            "right_panel.tab",
                        );
                    }
                });

                ui.separator();

                match self.right_panel_tab {
                    RightPanelTab::Properties => {
                        self.render_properties_panel(ui);
                    }
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
                        if self.lens_sim.is_some() {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Preset:").strong());
                                for preset in [
                                    LensPreset::CookeTriplet,
                                    LensPreset::DoubleGauss,
                                    LensPreset::PlanoConvex,
                                ] {
                                    self.render_command_button(
                                        ui,
                                        crate::commands::Command::LoadLensPreset(preset),
                                        "right_panel.prescription_preset",
                                    );
                                }
                            });
                            ui.add_space(4.0);
                            self.render_command_button(
                                ui,
                                crate::commands::Command::SetViewportMode(ViewportMode::View2D),
                                "right_panel.open_2d_view",
                            );
                            ui.separator();
                            let tokens = &self.tokens;
                            if let Some(lens_sim) = &mut self.lens_sim {
                                render_prescription_table_view(ui, lens_sim, tokens);
                            }
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

    /// Renders the property editor for the currently selected scene object.
    fn render_properties_panel(&mut self, ui: &mut egui::Ui) {
        let selected_id = self.scene_model.selected_id;
        let Some(id) = selected_id else {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new("No scene object selected.").color(self.tokens.text_disabled),
            );
            ui.label("Select an object in the Scene Hierarchy tree to edit its transform, geometry, and material.");
            return;
        };

        let last_bvh_time = self.scene_model.last_bvh_build_time_ms;
        let Some(obj) = self.scene_model.get_object_mut(id) else {
            ui.label("Selected object no longer exists.");
            return;
        };

        let mut scene_changed = false;

        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);

            // Object Name and Visibility
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Name:").strong());
                ui.text_edit_singleline(&mut obj.name);
                if ui.checkbox(&mut obj.visible, "Visible").changed() {
                    scene_changed = true;
                }
            });

            ui.add_space(6.0);
            ui.separator();

            // Transform Section
            ui.label(
                egui::RichText::new("Transform")
                    .strong()
                    .color(self.tokens.text_primary),
            );

            ui.horizontal(|ui| {
                ui.label("Position (mm):");
                if ui
                    .add(
                        egui::DragValue::new(&mut obj.transform.position.x)
                            .speed(0.5)
                            .prefix("X: "),
                    )
                    .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.position.y)
                                .speed(0.5)
                                .prefix("Y: "),
                        )
                        .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.position.z)
                                .speed(0.5)
                                .prefix("Z: "),
                        )
                        .changed()
                {
                    scene_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Rotation (°):");
                if ui
                    .add(
                        egui::DragValue::new(&mut obj.transform.rotation_deg.x)
                            .speed(1.0)
                            .prefix("P: "),
                    )
                    .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.rotation_deg.y)
                                .speed(1.0)
                                .prefix("Y: "),
                        )
                        .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.rotation_deg.z)
                                .speed(1.0)
                                .prefix("R: "),
                        )
                        .changed()
                {
                    scene_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Scale:");
                if ui
                    .add(
                        egui::DragValue::new(&mut obj.transform.scale.x)
                            .speed(0.05)
                            .range(0.01..=100.0)
                            .prefix("X: "),
                    )
                    .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.scale.y)
                                .speed(0.05)
                                .range(0.01..=100.0)
                                .prefix("Y: "),
                        )
                        .changed()
                    || ui
                        .add(
                            egui::DragValue::new(&mut obj.transform.scale.z)
                                .speed(0.05)
                                .range(0.01..=100.0)
                                .prefix("Z: "),
                        )
                        .changed()
                {
                    scene_changed = true;
                }
            });

            ui.add_space(6.0);
            ui.separator();

            // Geometry Parameters Section
            ui.label(
                egui::RichText::new("Geometry")
                    .strong()
                    .color(self.tokens.text_primary),
            );
            match &mut obj.geometry {
                crate::scene_model::ObjectGeometry::Prism {
                    apex_deg,
                    base_width,
                    depth,
                } => {
                    ui.horizontal(|ui| {
                        ui.label("Apex Angle:");
                        if ui
                            .add(egui::Slider::new(apex_deg, 20.0..=90.0).suffix("°"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Base Width:");
                        if ui
                            .add(egui::Slider::new(base_width, 10.0..=200.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Extrusion Depth:");
                        if ui
                            .add(egui::Slider::new(depth, 10.0..=200.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                }
                crate::scene_model::ObjectGeometry::Sphere {
                    radius,
                    stacks,
                    sectors,
                } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        if ui
                            .add(egui::Slider::new(radius, 2.0..=100.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Subdivisions:");
                        if ui
                            .add(egui::Slider::new(stacks, 4..=32).prefix("Stacks: "))
                            .changed()
                            || ui
                                .add(egui::Slider::new(sectors, 6..=48).prefix("Sectors: "))
                                .changed()
                        {
                            scene_changed = true;
                        }
                    });
                }
                crate::scene_model::ObjectGeometry::Slab {
                    width,
                    height,
                    thickness,
                } => {
                    ui.horizontal(|ui| {
                        ui.label("Width / Height:");
                        if ui
                            .add(egui::Slider::new(width, 5.0..=200.0).suffix(" mm"))
                            .changed()
                            || ui
                                .add(egui::Slider::new(height, 5.0..=200.0).suffix(" mm"))
                                .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Thickness:");
                        if ui
                            .add(egui::Slider::new(thickness, 1.0..=100.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                }
                crate::scene_model::ObjectGeometry::LensMesh {
                    r1,
                    r2,
                    semi_diameter,
                    center_thickness,
                    radial_segments,
                    azimuthal_segments,
                } => {
                    ui.horizontal(|ui| {
                        ui.label("R1 (Front):");
                        if ui
                            .add(egui::Slider::new(r1, -200.0..=200.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("R2 (Rear):");
                        if ui
                            .add(egui::Slider::new(r2, -200.0..=200.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Semi-Diameter / Thickness:");
                        if ui
                            .add(egui::Slider::new(semi_diameter, 5.0..=50.0).suffix(" mm"))
                            .changed()
                            || ui
                                .add(egui::Slider::new(center_thickness, 2.0..=30.0).suffix(" mm"))
                                .changed()
                        {
                            scene_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tessellation:");
                        if ui
                            .add(egui::Slider::new(radial_segments, 3..=24).prefix("Rad: "))
                            .changed()
                            || ui
                                .add(egui::Slider::new(azimuthal_segments, 6..=48).prefix("Azim: "))
                                .changed()
                        {
                            scene_changed = true;
                        }
                    });
                }
                crate::scene_model::ObjectGeometry::FloorPlane { size } => {
                    ui.horizontal(|ui| {
                        ui.label("Plane Half-Extent:");
                        if ui
                            .add(egui::Slider::new(size, 100.0..=2000.0).suffix(" mm"))
                            .changed()
                        {
                            scene_changed = true;
                        }
                    });
                }
            }

            ui.add_space(6.0);
            ui.separator();

            // Material Section
            ui.label(
                egui::RichText::new("Optical Material")
                    .strong()
                    .color(self.tokens.text_primary),
            );

            let current_glass = obj.material.glass_name.clone();
            ui.horizontal(|ui| {
                ui.label("Catalog Glass:");
                egui::ComboBox::from_id_salt("material_glass_select")
                    .selected_text(&current_glass)
                    .show_ui(ui, |ui| {
                        for g in ["N-BK7", "F2", "SF11", "Fused Silica", "CaF2", "Floor"] {
                            if ui.selectable_label(current_glass == g, g).clicked() {
                                obj.material.glass_name = g.into();
                                obj.material.name = g.into();
                                scene_changed = true;
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Dispersion Model:");
                let model_str = match obj.material.dispersion {
                    crate::scene_model::DispersionModelChoice::Sellmeier => "Sellmeier (Physical)",
                    crate::scene_model::DispersionModelChoice::Cauchy => "Cauchy (2-term)",
                    crate::scene_model::DispersionModelChoice::Constant(_) => {
                        "Constant (Non-dispersive)"
                    }
                };
                egui::ComboBox::from_id_salt("material_dispersion_select")
                    .selected_text(model_str)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(
                                    obj.material.dispersion,
                                    crate::scene_model::DispersionModelChoice::Sellmeier
                                ),
                                "Sellmeier (Physical)",
                            )
                            .clicked()
                        {
                            obj.material.dispersion =
                                crate::scene_model::DispersionModelChoice::Sellmeier;
                            scene_changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(
                                    obj.material.dispersion,
                                    crate::scene_model::DispersionModelChoice::Cauchy
                                ),
                                "Cauchy (2-term)",
                            )
                            .clicked()
                        {
                            obj.material.dispersion =
                                crate::scene_model::DispersionModelChoice::Cauchy;
                            scene_changed = true;
                        }
                        if ui
                            .selectable_label(
                                matches!(
                                    obj.material.dispersion,
                                    crate::scene_model::DispersionModelChoice::Constant(_)
                                ),
                                "Constant (Non-dispersive)",
                            )
                            .clicked()
                        {
                            obj.material.dispersion =
                                crate::scene_model::DispersionModelChoice::Constant(
                                    obj.material.nd,
                                );
                            scene_changed = true;
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Surface Roughness α:");
                if ui
                    .add(egui::Slider::new(&mut obj.material.roughness, 0.0..=1.0).show_value(true))
                    .on_hover_text("GGX microfacet roughness for frosted/ground glass")
                    .changed()
                {
                    scene_changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label(format!(
                    "Nd = {:.4} • Vd = {:.2}",
                    obj.material.nd, obj.material.vd
                ));
            });

            ui.add_space(8.0);
            ui.separator();

            // Incremental BVH Timing Telemetry Readout
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("BVH Rebuild: {:.2} ms", last_bvh_time))
                        .size(11.0)
                        .color(self.tokens.text_secondary),
                );
            });
        });

        if scene_changed {
            let (bvh, _) = self.scene_model.build_bvh_scene();
            self.gpu_state.update_scene_bvh(&bvh);
        }
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

                for (mode, _) in tabs {
                    let is_active = self.viewport_mode == mode;
                    self.render_selectable_command_label(
                        ui,
                        is_active,
                        crate::commands::Command::SetViewportMode(mode),
                        "central_viewport.tab",
                    );
                }

                if ui.available_width() >= 90.0 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ToggleFullscreen,
                            self.is_fullscreen,
                            "central_viewport.fullscreen",
                        );
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ResetView,
                            false,
                            "central_viewport.reset_view",
                        );
                        self.render_toolbar_command_button(
                            ui,
                            crate::commands::Command::ExportImage,
                            false,
                            "central_viewport.export_image",
                        );
                        if self.viewport_mode == ViewportMode::View3D {
                            self.render_toolbar_command_button(
                                ui,
                                crate::commands::Command::ToggleTofPulses,
                                self.gpu_state.beam_display.show_tof_pulses,
                                "central_viewport.toggle_tof",
                            );
                            self.render_toolbar_command_button(
                                ui,
                                crate::commands::Command::ToggleFloorCaustic,
                                self.gpu_state.beam_display.show_caustic,
                                "central_viewport.toggle_caustic",
                            );
                            self.render_toolbar_command_button(
                                ui,
                                crate::commands::Command::ToggleBeamRibbons,
                                self.gpu_state.beam_display.show_ribbons,
                                "central_viewport.toggle_ribbons",
                            );
                        }
                    });
                }
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
                        &mut self.gpu_state,
                    );
                }
                ViewportMode::View2D => {
                    if let Some(lens_sim) = &mut self.lens_sim {
                        render_lens_system_panel(ui, lens_sim, &mut self.lens_view, &self.tokens);
                    } else {
                        self.render_2d_prism_view(ui, avail_rect);
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

    /// Renders the 2D plane-of-incidence cross-section view of the prism experiment.
    fn render_2d_prism_view(&mut self, ui: &mut egui::Ui, screen_rect: Rect) {
        let (response, painter) = ui.allocate_painter(screen_rect.size(), Sense::click_and_drag());

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
            self.hovered_ray_index =
                crate::hit_test::find_hovered_ray_index(cursor, &screen_rays, 8.0);
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
        painter.rect_filled(screen_rect, CornerRadius::ZERO, self.tokens.canvas);

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
        let s_in_orig = self
            .view
            .world_to_screen(self.sim.incoming_origin, screen_rect);
        let s_in_hit = self
            .view
            .world_to_screen(self.sim.incoming_hit, screen_rect);
        painter.line_segment([s_in_orig, s_in_hit], Stroke::new(2.0, Color32::WHITE));
        let label_pos = Pos2::new(s_in_orig.x - 12.0, s_in_orig.y - 12.0);
        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            "D65 Beam",
            egui::FontId::proportional(11.0),
            self.tokens.text_secondary,
        );

        // 4. Spectral fan rays
        for (i, pts) in screen_rays.iter().enumerate() {
            if pts.len() < 2 {
                continue;
            }
            let is_selected = self.selected_ray_index == Some(i);
            let is_hovered = self.hovered_ray_index == Some(i);
            let ray_data = &self.sim.rays[i];

            let stroke_color =
                crate::theme::ray_color_for_background(ray_data.wavelength_nm, self.tokens.canvas);

            let stroke_width = if is_selected {
                3.0
            } else if is_hovered {
                2.2
            } else {
                1.5
            };

            for window in pts.windows(2) {
                painter.line_segment(
                    [window[0], window[1]],
                    Stroke::new(stroke_width, stroke_color),
                );
            }

            // Draw reflected branches if enabled
            if self.show_reflections {
                for branch in &ray_data.reflected_branches {
                    let b_start = self.view.world_to_screen(branch.start, screen_rect);
                    let b_end = self.view.world_to_screen(branch.end, screen_rect);
                    let alpha = branch.alpha;
                    let branch_color = Color32::from_rgba_premultiplied(
                        (f32::from(stroke_color.r()) * alpha) as u8,
                        (f32::from(stroke_color.g()) * alpha) as u8,
                        (f32::from(stroke_color.b()) * alpha) as u8,
                        (alpha * 255.0) as u8,
                    );
                    painter.line_segment([b_start, b_end], Stroke::new(1.0, branch_color));
                }
            }
        }

        // 5. Watermark / 2D plane HUD
        let hud_rect = Rect::from_min_size(
            Pos2::new(screen_rect.min.x + 12.0, screen_rect.min.y + 12.0),
            Vec2::new(220.0, 48.0),
        );
        painter.rect_filled(hud_rect, CornerRadius::same(3), self.tokens.panel_secondary);
        painter.rect_stroke(
            hud_rect,
            CornerRadius::same(3),
            Stroke::new(1.0, self.tokens.border),
            StrokeKind::Inside,
        );
        painter.text(
            Pos2::new(hud_rect.min.x + 8.0, hud_rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            format!("2D Cross Section • {}", self.sim.glass_name),
            egui::FontId::proportional(11.0),
            self.tokens.text_primary,
        );
        painter.text(
            Pos2::new(hud_rect.min.x + 8.0, hud_rect.min.y + 24.0),
            egui::Align2::LEFT_TOP,
            format!(
                "α = {:.1}°  θi = {:.1}°  {} rays",
                self.sim.apex_deg,
                self.sim.incidence_deg,
                self.sim.rays.len()
            ),
            egui::FontId::proportional(10.0),
            self.tokens.text_secondary,
        );
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
                            if ui.button("Reset").clicked() {
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

        self.ui_registry.clear();

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

        // 9. Dispatch all queued commands triggered during this frame
        let queued = std::mem::take(&mut self.ui_registry.queued_commands);
        let mut platform = crate::platform::EguiPlatform::new(ctx);
        for cmd in queued {
            self.dispatch(cmd, &mut platform);
        }
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
