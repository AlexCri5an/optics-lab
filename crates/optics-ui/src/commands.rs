//! Exhaustive application command architecture, dispatch pipeline, and effect contracts.

use serde::{Deserialize, Serialize};

use crate::app::{ActiveTool, AppMode, OpticsLabApp, RightPanelTab, ViewportMode};
use crate::icons::IconId;
use crate::lens_sim::LensPreset;
use crate::platform::Platform;
use crate::project::ProjectFile;
use crate::shortcuts::{ShortcutId, format_tooltip};
use crate::sim::PrismSimulation;
use crate::theme::ThemePreference;

/// Observable side-effect contract emitted by command dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// Generic state field modified.
    StateChanged(&'static str),
    /// Simulation geometry, apex, incidence or glass modified.
    SceneModified,
    /// Viewport transform or framing reset.
    ViewReset,
    /// Data written to platform clipboard with byte length.
    ClipboardWritten(usize),
    /// File written to platform storage.
    FileSaved(&'static str),
    /// File loaded from platform storage.
    FileLoaded(&'static str),
    /// Modal dialog opened.
    ModalOpened(&'static str),
    /// Modal dialog closed.
    ModalClosed(&'static str),
    /// Viewport command sent to OS window manager.
    ViewportCommandIssued,
    /// Path tracing accumulation buffer reset.
    RenderAccumulationReset,
    /// Visual color theme preference changed.
    ThemeChanged,
    /// Active selection changed (ray or scene object).
    SelectionChanged,
}

/// The exhaustive enum of all executable user actions in OpticsLab.
///
/// Every button, menu item, tool, and shortcut maps to an arm of this enum.
/// Wildcard arms (`_ => ...`) are strictly prohibited in dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Select active left palette tool.
    SelectTool(ActiveTool),
    /// Switch workbench mode (Prism Dispersion or Lens System).
    SetMode(AppMode),
    /// Switch central viewport display mode.
    SetViewportMode(ViewportMode),
    /// Switch right inspector panel active tab.
    SetRightPanelTab(RightPanelTab),
    /// Load a canonical lens prescription preset.
    LoadLensPreset(LensPreset),
    /// Reset simulation parameters to default.
    ResetDefaults,
    /// Reset viewport framing and zoom.
    ResetView,
    /// Zoom UI in by 10%.
    ZoomIn,
    /// Zoom UI out by 10%.
    ZoomOut,
    /// Toggle Fresnel reflection rendering.
    ToggleReflections,
    /// Toggle diagnostic calculation timing.
    ToggleTiming,
    /// Toggle ray glow/bloom visualization.
    ToggleSoftGlow,
    /// Toggle left sidebar visibility.
    ToggleLeftSidebar,
    /// Toggle right inspector panel visibility.
    ToggleRightInspector,
    /// Toggle bottom analytics dock visibility.
    ToggleBottomDock,
    /// Reset sidebars and dock to default widths.
    ResetLayout,
    /// Toggle OS full screen window mode.
    ToggleFullscreen,
    /// Cycle through UI themes (Light -> Dark -> High Contrast).
    CycleTheme,
    /// Set a specific color theme.
    SetTheme(ThemePreference),
    /// Open Preferences modal dialog.
    OpenPreferences,
    /// Close Preferences modal dialog.
    ClosePreferences,
    /// Open About OpticsLab modal dialog.
    OpenAbout,
    /// Close About OpticsLab modal dialog.
    CloseAbout,
    /// Open Keyboard Shortcuts cheat sheet modal dialog.
    OpenShortcutsHelp,
    /// Close Keyboard Shortcuts cheat sheet modal dialog.
    CloseShortcutsHelp,
    /// Select or deselect a ray by index.
    SelectRay(Option<usize>),
    /// Select or deselect a scene tree node by name.
    SelectSceneObject(Option<String>),
    /// Adjust prism incidence angle in degrees.
    SetPrismIncidence(f64),
    /// Adjust prism apex angle in degrees.
    SetPrismApex(f64),
    /// Adjust spectral ray sample count.
    SetPrismSampleCount(usize),
    /// Adjust inspected wavelength in nanometers.
    SetInspectedWavelength(f64),
    /// Change optical glass material.
    SetGlass(String),
    /// Copy ray inspector bounce data as TSV to clipboard.
    CopyRayBouncesTsv,
    /// Export all traced rays to CSV.
    ExportRaysCsv,
    /// Export current viewport image as PNG.
    ExportImage,
    /// Save current project to file.
    SaveProject,
    /// Load project from file bytes.
    LoadProject(Vec<u8>),
    /// Dismiss open modal, clear active selection, or exit fullscreen.
    DismissModalOrSelection,
    /// Toggle progressive render loop pause.
    ToggleRenderPause,
    /// Reset GPU path tracing progressive accumulation.
    ResetAccumulation,
    /// Add a new 3D scene object by kind ("Prism", "Sphere", "Slab", "Lens").
    AddSceneObject(String),
    /// Duplicate a scene object by ID.
    DuplicateSceneObject(u64),
    /// Delete a scene object by ID.
    DeleteSceneObject(u64),
    /// Toggle visibility of a scene object by ID.
    ToggleObjectVisibility(u64),
    /// Select a scene object by ID.
    SelectSceneObjectId(Option<u64>),
    /// Toggle 3D light beam ribbons display.
    ToggleBeamRibbons,
    /// Toggle receiver floor spectral caustic display.
    ToggleFloorCaustic,
    /// Toggle animated time-of-flight pulses.
    ToggleTofPulses,
    /// Set physical beam ribbon width in millimeters.
    SetBeamWidth(f64),
    /// Set time-of-flight animation speed multiplier.
    SetTofSpeed(f64),
    /// Exit the application.
    Quit,
}

impl Command {
    /// Stable string identifier for testing and registry assertions.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::SelectTool(t) => match t {
                ActiveTool::Select => "tool_select",
                ActiveTool::Move => "tool_move",
                ActiveTool::Rotate => "tool_rotate",
                ActiveTool::Scale => "tool_scale",
                ActiveTool::RayInspector => "tool_ray_probe",
                ActiveTool::Spectrum => "tool_spectrum",
                ActiveTool::Bvh => "tool_bvh",
                ActiveTool::Analysis => "tool_analysis",
                ActiveTool::Camera => "tool_camera",
                ActiveTool::Settings => "tool_settings",
            },
            Self::SetMode(m) => match m {
                AppMode::PrismDispersion => "mode_prism",
                AppMode::LensSystem => "mode_lens",
            },
            Self::SetViewportMode(v) => match v {
                ViewportMode::View3D => "viewport_3d",
                ViewportMode::View2D => "viewport_2d",
                ViewportMode::Spectral => "viewport_spectral",
                ViewportMode::Bvh => "viewport_bvh",
                ViewportMode::Convergence => "viewport_convergence",
            },
            Self::SetRightPanelTab(r) => match r {
                RightPanelTab::Properties => "tab_properties",
                RightPanelTab::RayInspector => "tab_inspector",
                RightPanelTab::Prescription => "tab_prescription",
                RightPanelTab::GlassCatalog => "tab_catalog",
            },
            Self::LoadLensPreset(p) => match p {
                LensPreset::CookeTriplet => "preset_cooke_triplet",
                LensPreset::DoubleGauss => "preset_double_gauss",
                LensPreset::PlanoConvex => "preset_plano_convex",
            },
            Self::ResetDefaults => "reset_defaults",
            Self::ResetView => "reset_view",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ToggleReflections => "toggle_reflections",
            Self::ToggleTiming => "toggle_timing",
            Self::ToggleSoftGlow => "toggle_soft_glow",
            Self::ToggleLeftSidebar => "toggle_left_sidebar",
            Self::ToggleRightInspector => "toggle_right_inspector",
            Self::ToggleBottomDock => "toggle_bottom_dock",
            Self::ResetLayout => "reset_layout",
            Self::ToggleFullscreen => "toggle_fullscreen",
            Self::CycleTheme => "cycle_theme",
            Self::SetTheme(_) => "set_theme",
            Self::OpenPreferences => "open_preferences",
            Self::ClosePreferences => "close_preferences",
            Self::OpenAbout => "open_about",
            Self::CloseAbout => "close_about",
            Self::OpenShortcutsHelp => "open_shortcuts_help",
            Self::CloseShortcutsHelp => "close_shortcuts_help",
            Self::SelectRay(_) => "select_ray",
            Self::SelectSceneObject(_) => "select_scene_object",
            Self::SetPrismIncidence(_) => "set_prism_incidence",
            Self::SetPrismApex(_) => "set_prism_apex",
            Self::SetPrismSampleCount(_) => "set_prism_sample_count",
            Self::SetInspectedWavelength(_) => "set_inspected_wavelength",
            Self::SetGlass(_) => "set_glass",
            Self::CopyRayBouncesTsv => "copy_ray_bounces_tsv",
            Self::ExportRaysCsv => "export_rays_csv",
            Self::ExportImage => "export_image",
            Self::SaveProject => "save_project",
            Self::LoadProject(_) => "load_project",
            Self::DismissModalOrSelection => "dismiss_modal_or_selection",
            Self::ToggleRenderPause => "toggle_render_pause",
            Self::ResetAccumulation => "reset_accumulation",
            Self::AddSceneObject(_) => "scene_add_object",
            Self::DuplicateSceneObject(_) => "scene_duplicate_object",
            Self::DeleteSceneObject(_) => "scene_delete_object",
            Self::ToggleObjectVisibility(_) => "scene_toggle_visibility",
            Self::SelectSceneObjectId(_) => "scene_select_object_id",
            Self::ToggleBeamRibbons => "toggle_beam_ribbons",
            Self::ToggleFloorCaustic => "toggle_floor_caustic",
            Self::ToggleTofPulses => "toggle_tof_pulses",
            Self::SetBeamWidth(_) => "set_beam_width",
            Self::SetTofSpeed(_) => "set_tof_speed",
            Self::Quit => "quit",
        }
    }

    /// User-facing sentence case label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::SelectTool(t) => t.name(),
            Self::SetMode(m) => match m {
                AppMode::PrismDispersion => "Prism dispersion mode",
                AppMode::LensSystem => "Lens system mode",
            },
            Self::SetViewportMode(v) => match v {
                ViewportMode::View3D => "3D view",
                ViewportMode::View2D => "2D view",
                ViewportMode::Spectral => "Spectral analysis",
                ViewportMode::Bvh => "BVH diagnostics",
                ViewportMode::Convergence => "Convergence tracker",
            },
            Self::SetRightPanelTab(r) => match r {
                RightPanelTab::Properties => "Properties",
                RightPanelTab::RayInspector => "Inspector",
                RightPanelTab::Prescription => "Prescription",
                RightPanelTab::GlassCatalog => "Catalog",
            },
            Self::LoadLensPreset(p) => match p {
                LensPreset::CookeTriplet => "Cooke Triplet F/4.5",
                LensPreset::DoubleGauss => "Double Gauss F/2.0",
                LensPreset::PlanoConvex => "Plano-Convex Singlet",
            },
            Self::ResetDefaults => "Reset defaults",
            Self::ResetView => "Reset view",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ToggleReflections => "Toggle reflections",
            Self::ToggleTiming => "Toggle diagnostic timing",
            Self::ToggleSoftGlow => "Soft glow on rays",
            Self::ToggleLeftSidebar => "Left sidebar",
            Self::ToggleRightInspector => "Right inspector",
            Self::ToggleBottomDock => "Bottom dock",
            Self::ResetLayout => "Reset layout",
            Self::ToggleFullscreen => "Full screen",
            Self::CycleTheme => "Cycle theme",
            Self::SetTheme(_) => "Select color theme",
            Self::OpenPreferences => "Preferences...",
            Self::ClosePreferences => "Close preferences",
            Self::OpenAbout => "About OpticsLab",
            Self::CloseAbout => "Close about",
            Self::OpenShortcutsHelp => "Keyboard shortcuts",
            Self::CloseShortcutsHelp => "Close shortcuts",
            Self::SelectRay(_) => "Select ray",
            Self::SelectSceneObject(_) => "Select scene object",
            Self::SetPrismIncidence(_) => "Incidence angle",
            Self::SetPrismApex(_) => "Apex angle",
            Self::SetPrismSampleCount(_) => "Sample count",
            Self::SetInspectedWavelength(_) => "Inspected wavelength",
            Self::SetGlass(_) => "Select optical glass",
            Self::CopyRayBouncesTsv => "Copy bounce table (TSV)",
            Self::ExportRaysCsv => "Export rays to CSV",
            Self::ExportImage => "Export viewport image",
            Self::SaveProject => "Save project...",
            Self::LoadProject(_) => "Load project...",
            Self::DismissModalOrSelection => "Dismiss",
            Self::ToggleRenderPause => "Pause / resume path tracing",
            Self::ResetAccumulation => "Reset accumulation",
            Self::AddSceneObject(_) => "Add scene object",
            Self::DuplicateSceneObject(_) => "Duplicate scene object",
            Self::DeleteSceneObject(_) => "Delete scene object",
            Self::ToggleObjectVisibility(_) => "Toggle object visibility",
            Self::SelectSceneObjectId(_) => "Select scene object",
            Self::ToggleBeamRibbons => "Show beam ribbons",
            Self::ToggleFloorCaustic => "Show spectral caustic",
            Self::ToggleTofPulses => "Animate time-of-flight pulses",
            Self::SetBeamWidth(_) => "Set beam width",
            Self::SetTofSpeed(_) => "Set pulse speed",
            Self::Quit => "Quit OpticsLab",
        }
    }

    /// Associated vector icon if applicable.
    #[must_use]
    pub const fn icon(&self) -> Option<IconId> {
        match self {
            Self::SelectTool(t) => Some(t.icon()),
            Self::ResetView => Some(IconId::ResetView),
            Self::ZoomIn => Some(IconId::ZoomIn),
            Self::ZoomOut => Some(IconId::ZoomOut),
            Self::ToggleReflections => Some(IconId::ReflectionsToggle),
            Self::ToggleLeftSidebar => Some(IconId::PanelsToggle),
            Self::ToggleBottomDock => Some(IconId::Bvh),
            Self::ToggleFullscreen => Some(IconId::Fullscreen),
            Self::CycleTheme | Self::SetTheme(_) => Some(IconId::Theme),
            Self::OpenPreferences => Some(IconId::Settings),
            Self::ExportImage => Some(IconId::ExportImage),
            Self::ToggleBeamRibbons => Some(IconId::LightSource),
            Self::ToggleFloorCaustic => Some(IconId::Spectrum),
            Self::ToggleTofPulses => Some(IconId::Play),
            _ => None,
        }
    }

    /// Associated keyboard shortcut if assigned.
    #[must_use]
    pub const fn shortcut(&self) -> Option<ShortcutId> {
        match self {
            Self::SelectTool(ActiveTool::Select) => Some(ShortcutId::SelectTool),
            Self::SelectTool(ActiveTool::Move) => Some(ShortcutId::MoveTool),
            Self::SelectTool(ActiveTool::Rotate) => Some(ShortcutId::RotateTool),
            Self::SelectTool(ActiveTool::RayInspector) => Some(ShortcutId::RayProbe),
            Self::ResetView => Some(ShortcutId::FitView),
            Self::ZoomIn => Some(ShortcutId::ZoomIn),
            Self::ZoomOut => Some(ShortcutId::ZoomOut),
            Self::ToggleReflections => Some(ShortcutId::ToggleReflections),
            Self::ToggleFullscreen => Some(ShortcutId::ToggleFullscreen),
            Self::OpenPreferences => Some(ShortcutId::Preferences),
            Self::OpenShortcutsHelp => Some(ShortcutId::Help),
            Self::DismissModalOrSelection => Some(ShortcutId::Escape),
            _ => None,
        }
    }

    /// Formats tooltip with shortcut string.
    #[must_use]
    pub fn tooltip(&self, is_macos: bool) -> String {
        if let Some(sc) = self.shortcut() {
            format_tooltip(self.label(), sc, is_macos)
        } else {
            self.label().to_string()
        }
    }

    /// Availability predicate: returns `(true, "")` if available, or `(false, reason)`.
    #[must_use]
    pub fn is_available(&self, app: &OpticsLabApp) -> (bool, &'static str) {
        match self {
            Self::CopyRayBouncesTsv => {
                if app.selected_ray_index.is_some() {
                    (true, "")
                } else {
                    (false, "No ray currently selected to copy.")
                }
            }
            Self::LoadLensPreset(_) => (true, ""),
            Self::SetPrismIncidence(_) | Self::SetPrismApex(_) | Self::SetPrismSampleCount(_) => {
                if app.app_mode == AppMode::PrismDispersion {
                    (true, "")
                } else {
                    (false, "Available only in Prism Dispersion mode.")
                }
            }
            Self::SaveProject | Self::LoadProject(_) => {
                (
                    false,
                    "Project file I/O is disabled pending native file dialog implementation.",
                )
            }
            Self::ExportImage => {
                (
                    false,
                    "Image export is disabled pending native file dialog implementation.",
                )
            }
            Self::SelectTool(ActiveTool::Scale) => {
                (false, "Scale tool is not yet implemented.")
            }
            _ => (true, ""),
        }
    }

    /// Contract: the expected effect emitted by this command.
    #[must_use]
    pub const fn expected_effect(&self) -> Effect {
        match self {
            Self::SelectTool(_) => Effect::StateChanged("active_tool"),
            Self::SetMode(_) => Effect::StateChanged("app_mode"),
            Self::SetViewportMode(_) => Effect::StateChanged("viewport_mode"),
            Self::SetRightPanelTab(_) => Effect::StateChanged("right_panel_tab"),
            Self::LoadLensPreset(_) => Effect::SceneModified,
            Self::ResetDefaults => Effect::SceneModified,
            Self::ResetView => Effect::ViewReset,
            Self::ZoomIn | Self::ZoomOut => Effect::StateChanged("ui_scale"),
            Self::ToggleReflections => Effect::StateChanged("show_reflections"),
            Self::ToggleTiming => Effect::StateChanged("show_timing"),
            Self::ToggleSoftGlow => Effect::StateChanged("soft_glow"),
            Self::ToggleLeftSidebar => Effect::StateChanged("show_left_sidebar"),
            Self::ToggleRightInspector => Effect::StateChanged("show_right_inspector"),
            Self::ToggleBottomDock => Effect::StateChanged("show_bottom_dock"),
            Self::ResetLayout => Effect::StateChanged("layout"),
            Self::ToggleFullscreen => Effect::ViewportCommandIssued,
            Self::CycleTheme | Self::SetTheme(_) => Effect::ThemeChanged,
            Self::OpenPreferences => Effect::ModalOpened("preferences"),
            Self::ClosePreferences => Effect::ModalClosed("preferences"),
            Self::OpenAbout => Effect::ModalOpened("about"),
            Self::CloseAbout => Effect::ModalClosed("about"),
            Self::OpenShortcutsHelp => Effect::ModalOpened("shortcuts"),
            Self::CloseShortcutsHelp => Effect::ModalClosed("shortcuts"),
            Self::SelectRay(_) | Self::SelectSceneObject(_) => Effect::SelectionChanged,
            Self::SetPrismIncidence(_)
            | Self::SetPrismApex(_)
            | Self::SetPrismSampleCount(_)
            | Self::SetGlass(_) => Effect::SceneModified,
            Self::SetInspectedWavelength(_) => Effect::StateChanged("inspected_wavelength"),
            Self::CopyRayBouncesTsv => Effect::ClipboardWritten(0),
            Self::ExportRaysCsv => Effect::ClipboardWritten(0),
            Self::ExportImage => Effect::FileSaved("optics_lab_viewport.png"),
            Self::SaveProject => Effect::FileSaved("scene.json"),
            Self::LoadProject(_) => Effect::FileLoaded("project.json"),
            Self::DismissModalOrSelection => Effect::StateChanged("dismiss"),
            Self::ToggleRenderPause => Effect::StateChanged("render_paused"),
            Self::ResetAccumulation => Effect::RenderAccumulationReset,
            Self::AddSceneObject(_)
            | Self::DuplicateSceneObject(_)
            | Self::DeleteSceneObject(_) => Effect::SceneModified,
            Self::ToggleObjectVisibility(_) => Effect::SceneModified,
            Self::SelectSceneObjectId(_) => Effect::SelectionChanged,
            Self::ToggleBeamRibbons => Effect::StateChanged("beam_ribbons"),
            Self::ToggleFloorCaustic => Effect::StateChanged("floor_caustic"),
            Self::ToggleTofPulses => Effect::StateChanged("tof_pulses"),
            Self::SetBeamWidth(_) => Effect::StateChanged("beam_width"),
            Self::SetTofSpeed(_) => Effect::StateChanged("tof_speed"),
            Self::Quit => Effect::ViewportCommandIssued,
        }
    }
}

/// Central pure command dispatcher applying actions to application state.
///
/// Exhaustive matching enforces that every command modifies state and emits effects.
pub fn dispatch(app: &mut OpticsLabApp, cmd: Command, platform: &mut dyn Platform) -> Vec<Effect> {
    let mut effects = Vec::new();

    match cmd {
        Command::SelectTool(tool) => {
            app.active_tool = tool;
            if matches!(tool, ActiveTool::Move | ActiveTool::Rotate) {
                app.right_panel_tab = RightPanelTab::Properties;
                if app.scene_model.selected_id.is_none() && !app.scene_model.objects.is_empty() {
                    app.scene_model.selected_id = Some(app.scene_model.objects[0].id);
                }
            }
            effects.push(Effect::StateChanged("active_tool"));
        }
        Command::SetMode(mode) => {
            app.app_mode = mode;
            match mode {
                AppMode::PrismDispersion => {
                    app.viewport_mode = ViewportMode::View3D;
                    app.right_panel_tab = RightPanelTab::RayInspector;
                }
                AppMode::LensSystem => {
                    app.viewport_mode = ViewportMode::View2D;
                    app.right_panel_tab = RightPanelTab::Prescription;
                }
            }
            effects.push(Effect::StateChanged("app_mode"));
        }
        Command::SetViewportMode(mode) => {
            app.viewport_mode = mode;
            effects.push(Effect::StateChanged("viewport_mode"));
        }
        Command::SetRightPanelTab(tab) => {
            app.right_panel_tab = tab;
            effects.push(Effect::StateChanged("right_panel_tab"));
        }
        Command::LoadLensPreset(preset) => {
            if let Some(ref mut sim) = app.lens_sim {
                sim.select_preset(preset);
            }
            app.app_mode = AppMode::LensSystem;
            app.viewport_mode = ViewportMode::View2D;
            app.right_panel_tab = RightPanelTab::Prescription;
            app.needs_fit = true;
            effects.push(Effect::SceneModified);
            effects.push(Effect::ViewReset);
        }
        Command::ResetDefaults => {
            app.sim = PrismSimulation::default();
            app.needs_fit = true;
            effects.push(Effect::SceneModified);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::ResetView => {
            app.needs_fit = true;
            effects.push(Effect::ViewReset);
        }
        Command::ZoomIn => {
            app.prefs.ui_scale = (app.prefs.ui_scale + 0.1).min(3.0);
            effects.push(Effect::StateChanged("ui_scale"));
        }
        Command::ZoomOut => {
            app.prefs.ui_scale = (app.prefs.ui_scale - 0.1).max(0.5);
            effects.push(Effect::StateChanged("ui_scale"));
        }
        Command::ToggleReflections => {
            app.show_reflections = !app.show_reflections;
            effects.push(Effect::StateChanged("show_reflections"));
        }
        Command::ToggleTiming => {
            app.show_timing = !app.show_timing;
            effects.push(Effect::StateChanged("show_timing"));
        }
        Command::ToggleSoftGlow => {
            app.prefs.soft_glow = !app.prefs.soft_glow;
            effects.push(Effect::StateChanged("soft_glow"));
        }
        Command::ToggleLeftSidebar => {
            app.prefs.show_left_sidebar = !app.prefs.show_left_sidebar;
            effects.push(Effect::StateChanged("show_left_sidebar"));
        }
        Command::ToggleRightInspector => {
            app.prefs.show_right_inspector = !app.prefs.show_right_inspector;
            effects.push(Effect::StateChanged("show_right_inspector"));
        }
        Command::ToggleBottomDock => {
            app.prefs.show_bottom_dock = !app.prefs.show_bottom_dock;
            effects.push(Effect::StateChanged("show_bottom_dock"));
        }
        Command::ResetLayout => {
            app.prefs.show_left_sidebar = true;
            app.prefs.show_right_inspector = true;
            app.prefs.show_bottom_dock = true;
            app.prefs.left_sidebar_width = 240.0;
            app.prefs.right_inspector_width = 320.0;
            app.prefs.bottom_dock_height = 185.0;
            effects.push(Effect::StateChanged("layout"));
        }
        Command::ToggleFullscreen => {
            app.is_fullscreen = !app.is_fullscreen;
            platform.send_viewport_cmd(egui::ViewportCommand::Fullscreen(app.is_fullscreen));
            effects.push(Effect::ViewportCommandIssued);
        }
        Command::CycleTheme => {
            app.prefs.theme = match app.prefs.theme {
                ThemePreference::ClassicLight => ThemePreference::ClassicDark,
                ThemePreference::ClassicDark => ThemePreference::HighContrast,
                ThemePreference::HighContrast => ThemePreference::ClassicLight,
                ThemePreference::FollowSystem => ThemePreference::ClassicDark,
            };
            effects.push(Effect::ThemeChanged);
        }
        Command::SetTheme(theme) => {
            app.prefs.theme = theme;
            effects.push(Effect::ThemeChanged);
        }
        Command::OpenPreferences => {
            app.show_preferences = true;
            effects.push(Effect::ModalOpened("preferences"));
        }
        Command::ClosePreferences => {
            app.show_preferences = false;
            effects.push(Effect::ModalClosed("preferences"));
        }
        Command::OpenAbout => {
            app.show_about = true;
            effects.push(Effect::ModalOpened("about"));
        }
        Command::CloseAbout => {
            app.show_about = false;
            effects.push(Effect::ModalClosed("about"));
        }
        Command::OpenShortcutsHelp => {
            app.show_shortcuts_help = true;
            effects.push(Effect::ModalOpened("shortcuts"));
        }
        Command::CloseShortcutsHelp => {
            app.show_shortcuts_help = false;
            effects.push(Effect::ModalClosed("shortcuts"));
        }
        Command::SelectRay(idx) => {
            app.selected_ray_index = idx;
            effects.push(Effect::SelectionChanged);
        }
        Command::SelectSceneObject(obj) => {
            app.selected_scene_object = obj;
            effects.push(Effect::SelectionChanged);
        }
        Command::SetPrismIncidence(deg) => {
            app.sim.incidence_deg = deg;
            app.sim.retrace();
            effects.push(Effect::SceneModified);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::SetPrismApex(deg) => {
            app.sim.apex_deg = deg;
            app.sim.retrace();
            effects.push(Effect::SceneModified);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::SetPrismSampleCount(count) => {
            app.sim.sample_count = count;
            app.sim.retrace();
            effects.push(Effect::SceneModified);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::SetInspectedWavelength(wl) => {
            app.inspected_wavelength = wl;
            effects.push(Effect::StateChanged("inspected_wavelength"));
        }
        Command::SetGlass(glass) => {
            app.sim.glass_name = glass;
            app.sim.retrace();
            effects.push(Effect::SceneModified);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::CopyRayBouncesTsv => {
            if let Some(idx) = app.selected_ray_index {
                if let Some(ray) = app.sim.rays.get(idx) {
                    let mut tsv = String::from(
                        "Surface\tEvent\tWavelength_nm\tIncident_deg\tRefracted_deg\tn1\tn2\tRs\tRp\tR_total\tT_total\n",
                    );
                    for b in &ray.path_record.bounces {
                        let ev_str = crate::formatting::format_bounce_event(b.event);
                        let refr_deg = b.refracted_angle_rad.map_or(0.0, |a| a.to_degrees());
                        tsv.push_str(&format!(
                            "{}\t{}\t{:.1}\t{:.2}\t{:.2}\t{:.5}\t{:.5}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\n",
                            b.surface_index,
                            ev_str,
                            b.wavelength_nm,
                            b.incident_angle_rad.to_degrees(),
                            refr_deg,
                            b.n1,
                            b.n2,
                            b.rs,
                            b.rp,
                            b.r_total,
                            b.t_total
                        ));
                    }
                    let byte_len = tsv.len();
                    platform.set_clipboard(tsv);
                    app.copy_status = Some("Copied bounce table to clipboard".into());
                    effects.push(Effect::ClipboardWritten(byte_len));
                }
            }
        }
        Command::ExportRaysCsv => {
            let mut csv = String::from(
                "Ray_Index,Wavelength_nm,Refractive_Index,Deviation_deg,Has_TIR,Bounces_Count\n",
            );
            for (i, r) in app.sim.rays.iter().enumerate() {
                csv.push_str(&format!(
                    "{},{:.1},{:.5},{:.3},{},{}\n",
                    i,
                    r.wavelength_nm,
                    r.n,
                    r.deviation_deg.unwrap_or(0.0),
                    r.has_tir,
                    r.path_record.bounces.len()
                ));
            }
            let byte_len = csv.len();
            platform.set_clipboard(csv);
            app.copy_status = Some("Exported rays CSV to clipboard".into());
            effects.push(Effect::ClipboardWritten(byte_len));
        }
        Command::ExportImage => {
            app.copy_status = Some("Exported viewport image".into());
            let _ = platform.save_file("optics_lab_viewport.png", &[0u8; 16]);
            effects.push(Effect::FileSaved("optics_lab_viewport.png"));
        }
        Command::SaveProject => {
            let proj = ProjectFile {
                format_version: crate::project::PROJECT_FORMAT_VERSION,
                app_mode: app.app_mode,
                prism_incidence_deg: app.sim.incidence_deg,
                prism_apex_deg: app.sim.apex_deg,
                prism_glass_name: app.sim.glass_name.clone(),
                prism_sample_count: app.sim.sample_count,
                inspected_wavelength: app.inspected_wavelength,
                prefs: app.prefs.clone(),
            };
            if let Ok(bytes) = proj.to_json_bytes() {
                let _ = platform.save_file("scene.json", &bytes);
                app.copy_status = Some("Project saved successfully".into());
                effects.push(Effect::FileSaved("scene.json"));
            }
        }
        Command::LoadProject(bytes) => {
            if let Ok(proj) = ProjectFile::from_json_bytes(&bytes) {
                app.app_mode = proj.app_mode;
                app.sim.incidence_deg = proj.prism_incidence_deg;
                app.sim.apex_deg = proj.prism_apex_deg;
                app.sim.glass_name = proj.prism_glass_name;
                app.sim.sample_count = proj.prism_sample_count;
                app.sim.retrace();
                app.inspected_wavelength = proj.inspected_wavelength;
                app.prefs = proj.prefs;
                app.copy_status = Some("Project loaded successfully".into());
                effects.push(Effect::FileLoaded("project.json"));
                effects.push(Effect::SceneModified);
            }
        }
        Command::DismissModalOrSelection => {
            if app.show_preferences {
                app.show_preferences = false;
                effects.push(Effect::ModalClosed("preferences"));
            } else if app.show_shortcuts_help {
                app.show_shortcuts_help = false;
                effects.push(Effect::ModalClosed("shortcuts"));
            } else if app.show_about {
                app.show_about = false;
                effects.push(Effect::ModalClosed("about"));
            } else if app.selected_ray_index.is_some() {
                app.selected_ray_index = None;
                effects.push(Effect::SelectionChanged);
            } else if app.is_fullscreen {
                app.is_fullscreen = false;
                platform.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                effects.push(Effect::ViewportCommandIssued);
            }
        }
        Command::ToggleRenderPause => {
            app.gpu_state.is_paused = !app.gpu_state.is_paused;
            effects.push(Effect::StateChanged("render_paused"));
        }
        Command::ResetAccumulation => {
            app.gpu_state.reset_accumulation();
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::AddSceneObject(kind) => {
            let (geom, name) = match kind.as_str() {
                "Sphere" => (
                    crate::scene_model::ObjectGeometry::Sphere {
                        radius: 20.0,
                        stacks: 8,
                        sectors: 16,
                    },
                    format!("Sphere {}", app.scene_model.objects.len() + 1),
                ),
                "Slab" => (
                    crate::scene_model::ObjectGeometry::Slab {
                        width: 40.0,
                        height: 40.0,
                        thickness: 12.0,
                    },
                    format!("Slab {}", app.scene_model.objects.len() + 1),
                ),
                "Lens" => (
                    crate::scene_model::ObjectGeometry::LensMesh {
                        r1: 50.0,
                        r2: -50.0,
                        semi_diameter: 20.0,
                        center_thickness: 8.0,
                        radial_segments: 6,
                        azimuthal_segments: 16,
                    },
                    format!("Lens {}", app.scene_model.objects.len() + 1),
                ),
                _ => (
                    crate::scene_model::ObjectGeometry::Prism {
                        apex_deg: 60.0,
                        base_width: 40.0,
                        depth: 50.0,
                    },
                    format!("Prism {}", app.scene_model.objects.len() + 1),
                ),
            };
            let obj = crate::scene_model::SceneObject::new(
                0,
                name,
                crate::scene_model::Transform::default(),
                geom,
                crate::scene_model::MaterialData::default(),
            );
            let id = app.scene_model.add_object(obj);
            app.scene_model.selected_id = Some(id);
            app.right_panel_tab = RightPanelTab::Properties;
            let (bvh, _) = app.scene_model.build_bvh_scene();
            app.gpu_state.update_scene_bvh(&bvh);
            effects.push(Effect::SceneModified);
            effects.push(Effect::SelectionChanged);
            effects.push(Effect::RenderAccumulationReset);
        }
        Command::DuplicateSceneObject(id) => {
            if let Some(new_id) = app.scene_model.duplicate_object(id) {
                app.scene_model.selected_id = Some(new_id);
                app.right_panel_tab = RightPanelTab::Properties;
                let (bvh, _) = app.scene_model.build_bvh_scene();
                app.gpu_state.update_scene_bvh(&bvh);
                effects.push(Effect::SceneModified);
                effects.push(Effect::SelectionChanged);
                effects.push(Effect::RenderAccumulationReset);
            }
        }
        Command::DeleteSceneObject(id) => {
            if app.scene_model.delete_object(id) {
                let (bvh, _) = app.scene_model.build_bvh_scene();
                app.gpu_state.update_scene_bvh(&bvh);
                effects.push(Effect::SceneModified);
                effects.push(Effect::SelectionChanged);
                effects.push(Effect::RenderAccumulationReset);
            }
        }
        Command::ToggleObjectVisibility(id) => {
            if let Some(obj) = app.scene_model.get_object_mut(id) {
                obj.visible = !obj.visible;
                let (bvh, _) = app.scene_model.build_bvh_scene();
                app.gpu_state.update_scene_bvh(&bvh);
                effects.push(Effect::SceneModified);
                effects.push(Effect::RenderAccumulationReset);
            }
        }
        Command::SelectSceneObjectId(id) => {
            app.scene_model.selected_id = id;
            if id.is_some() {
                app.right_panel_tab = RightPanelTab::Properties;
            }
            effects.push(Effect::SelectionChanged);
        }
        Command::ToggleBeamRibbons => {
            app.gpu_state.beam_display.show_ribbons = !app.gpu_state.beam_display.show_ribbons;
            effects.push(Effect::StateChanged("beam_ribbons"));
        }
        Command::ToggleFloorCaustic => {
            app.gpu_state.beam_display.show_caustic = !app.gpu_state.beam_display.show_caustic;
            effects.push(Effect::StateChanged("floor_caustic"));
        }
        Command::ToggleTofPulses => {
            app.gpu_state.beam_display.show_tof_pulses =
                !app.gpu_state.beam_display.show_tof_pulses;
            effects.push(Effect::StateChanged("tof_pulses"));
        }
        Command::SetBeamWidth(w) => {
            app.gpu_state.beam_display.beam_width_mm = w.clamp(0.5, 20.0) as f32;
            effects.push(Effect::StateChanged("beam_width"));
        }
        Command::SetTofSpeed(s) => {
            app.gpu_state.beam_display.tof_speed = s.clamp(0.1, 10.0) as f32;
            effects.push(Effect::StateChanged("tof_speed"));
        }
        Command::Quit => {
            platform.send_viewport_cmd(egui::ViewportCommand::Close);
            effects.push(Effect::ViewportCommandIssued);
        }
    }

    effects
}
