//! Main desktop application state and root update loop for OpticsLab.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{
    CentralPanel, Context, RichText, SidePanel, TopBottomPanel,
};

use optics_core::OpticalPrescription;
use optics_glass::GlassCatalogDb;

use crate::analysis::{AnalysisPanelState, render_analysis_panel};
use crate::glass_browser::render_materials_panel;
use crate::icons::{LucideIcon, lucide_small_icon_button};
use crate::layout::{Breakpoint, LayoutMetrics};
use crate::lens_editor::{LeftPanelTab, LensEditorState, render_lens_editor};
use crate::nonseq::{render_nonseq_bottom_drawer, NonSeqUiState};
use crate::optimization::{OptimizationModalState, render_optimization_panel};
use crate::theme::OpticsTokens;
use crate::tolerancing::{TolerancingAction, TolerancingModalState};
use crate::topbar::{TopBarAction, TopBarState, render_topbar};
use crate::viewport::{ViewportState, render_optical_viewport, render_optical_viewport_3d};

/// Complete snapshot of the application state for synchronized undo/redo.
#[derive(Clone, Debug, PartialEq)]
pub struct AppUndoSnapshot {
    /// Optical prescription snapshot.
    pub prescription: OpticalPrescription,
    /// Last DLS run summary at this point in time.
    pub opt_summary: Option<crate::optimization::DlsRunSummary>,
    /// Optimization history stack at this point in time.
    pub opt_history: Vec<(OpticalPrescription, Option<crate::optimization::DlsRunSummary>)>,
    /// Optimization baseline prescription at this point in time.
    pub opt_original_prescription: Option<OpticalPrescription>,
}

/// Main application struct representing the OpticsLab desktop workbench.
pub struct OpticsWorkbenchApp {
    /// State of the top bar and its controls.
    pub topbar_state: TopBarState,
    /// Currently loaded optical prescription.
    pub prescription: OpticalPrescription,
    /// Shared bundled glass catalog database.
    pub catalog: Arc<GlassCatalogDb>,
    /// State of the Lens Data Editor table.
    pub lens_editor_state: LensEditorState,
    /// State of the interactive 2D central optical viewport.
    pub viewport_state: ViewportState,
    /// State of the optical analysis and diagnostics panel.
    pub analysis_state: AnalysisPanelState,
    /// State of the automatic DLS optimization dialog hub.
    pub optimization_state: OptimizationModalState,
    /// State of the manufacturing tolerancing and Monte Carlo hub.
    pub tolerancing_state: TolerancingModalState,
    /// State of the non-sequential ray tracing & detector viewer drawer.
    pub nonseq_state: NonSeqUiState,
    /// Last action dispatched from the top bar.
    pub last_action_msg: String,
    /// Current ray count or optical status info.
    pub status_text: String,
    /// File path of currently opened document on disk.
    pub current_file_path: Option<PathBuf>,
    /// Whether the active document has unsaved modifications.
    pub is_dirty: bool,
    /// Undo history stack for synchronized edits.
    pub undo_stack: Vec<AppUndoSnapshot>,
    /// Redo history stack.
    pub redo_stack: Vec<AppUndoSnapshot>,
    /// History of recently opened or saved files.
    pub recent_files: Vec<PathBuf>,
    /// Channel sender for non-blocking native file dialogs.
    file_open_sender: Sender<PathBuf>,
    /// Channel receiver for asynchronously selected files.
    file_open_receiver: Receiver<PathBuf>,
}

impl Default for OpticsWorkbenchApp {
    fn default() -> Self {
        let prescription = optics_zemax::presets::cooke_triplet_f4_5()
            .unwrap_or_else(|_| OpticalPrescription {
                name: "Cooke Triplet F/4.5".to_string(),
                ..Default::default()
            });

        let catalog = Arc::new(GlassCatalogDb::default());
        let surf_count = prescription.surfaces.len();
        let epd = prescription.entrance_pupil_diameter_mm;
        let (file_open_sender, file_open_receiver) = channel();

        Self {
            topbar_state: TopBarState {
                raytrace_running: true,
                current_file_name: "Cooke Triplet F/4.5.zmx".to_string(),
                is_dirty: false,
                recent_files: Vec::new(),
                ..Default::default()
            },
            prescription,
            catalog,
            lens_editor_state: LensEditorState {
                selected_surface: None,
                ..Default::default()
            },
            viewport_state: ViewportState::default(),
            analysis_state: AnalysisPanelState::default(),
            optimization_state: OptimizationModalState::default(),
            tolerancing_state: TolerancingModalState::default(),
            nonseq_state: NonSeqUiState::default(),
            last_action_msg: "Loaded preset: Cooke Triplet F/4.5".to_string(),
            status_text: format!("Surfaces: {} | EPD: {:.1} mm | Status: Ready", surf_count, epd),
            current_file_path: None,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            recent_files: Vec::new(),
            file_open_sender,
            file_open_receiver,
        }
    }
}

impl OpticsWorkbenchApp {
    /// Creates a new instance of the application.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Install SVG and image loaders for Lucide icons
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Self::default()
    }

    /// Returns the active design tokens.
    #[must_use]
    pub fn tokens(&self) -> OpticsTokens {
        self.topbar_state.theme_mode.tokens()
    }
}

impl eframe::App for OpticsWorkbenchApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let tokens = self.tokens();
        let c = tokens.bg_panel;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let tokens = self.tokens();

        // 1. Configure retro engineering typography and visuals
        ctx.style_mut(|style| {
            use egui::{FontFamily, FontId, TextStyle};
            let mut text_styles = std::collections::BTreeMap::new();
            text_styles.insert(TextStyle::Heading, FontId::new(13.0, FontFamily::Monospace));
            text_styles.insert(TextStyle::Body, FontId::new(11.5, FontFamily::Monospace));
            text_styles.insert(TextStyle::Monospace, FontId::new(11.0, FontFamily::Monospace));
            text_styles.insert(TextStyle::Button, FontId::new(11.0, FontFamily::Monospace));
            text_styles.insert(TextStyle::Small, FontId::new(9.5, FontFamily::Monospace));
            style.text_styles = text_styles;
        });

        let mut visuals = if self.topbar_state.theme_mode.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = tokens.bg_panel;
        visuals.window_fill = tokens.bg_panel;
        visuals.window_corner_radius = egui::CornerRadius::ZERO;
        visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        visuals.widgets.noninteractive.bg_fill = tokens.bg_panel;
        visuals.widgets.noninteractive.bg_stroke = tokens.border;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, tokens.text_muted);
        visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::ZERO;
        visuals.widgets.inactive.bg_fill = tokens.bg_control;
        visuals.widgets.inactive.bg_stroke = tokens.border_hovered;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, tokens.text_primary);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::ZERO;

        // Subtle academic hover: fine border, no glowing neon blue
        visuals.widgets.hovered.bg_fill = tokens.bg_control_hovered;
        visuals.widgets.hovered.weak_bg_fill = tokens.bg_control_hovered;
        visuals.widgets.hovered.bg_stroke = tokens.border_hovered;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, tokens.text_primary);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::ZERO;

        // Active / Focused / Pressed input controls: strictly inherits theme active tokens
        visuals.widgets.active.bg_fill = tokens.bg_control_active;
        visuals.widgets.active.weak_bg_fill = tokens.bg_control_active;
        visuals.widgets.active.bg_stroke = tokens.border_hovered;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, tokens.text_primary);
        visuals.widgets.active.corner_radius = egui::CornerRadius::ZERO;

        // Open state (ComboBox, active menus)
        visuals.widgets.open.bg_fill = tokens.bg_control_active;
        visuals.widgets.open.weak_bg_fill = tokens.bg_control_active;
        visuals.widgets.open.bg_stroke = tokens.border_hovered;
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, tokens.text_primary);
        visuals.widgets.open.corner_radius = egui::CornerRadius::ZERO;

        // Selection & focus highlights: strictly inherits theme active control background & border
        visuals.selection.bg_fill = tokens.bg_control_active;
        visuals.selection.stroke = tokens.border_hovered;
        ctx.set_visuals(visuals);

        // 2. Global Keyboard Shortcuts (VectorCraft standard)
        let cmd = ctx.input(|i| i.modifiers.command);
        let shift = ctx.input(|i| i.modifiers.shift);
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.handle_topbar_action(TopBarAction::NewFile);
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.handle_topbar_action(TopBarAction::OpenFile);
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            self.handle_topbar_action(TopBarAction::SaveFile);
        }
        if cmd && shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            self.handle_topbar_action(TopBarAction::SaveFileAs);
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
            self.handle_topbar_action(TopBarAction::Undo);
        }
        if (cmd && shift && ctx.input(|i| i.key_pressed(egui::Key::Z)))
            || (cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::Y)))
        {
            self.handle_topbar_action(TopBarAction::Redo);
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::T)) {
            self.handle_topbar_action(TopBarAction::OpenTolerancing);
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
            self.viewport_state.is_3d = false;
            self.topbar_state.viewport_3d_mode = false;
            self.last_action_msg = "Viewport: 2D Cross-Section CAD mode active.".to_string();
        }
        if cmd && !shift && ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
            self.viewport_state.is_3d = true;
            self.topbar_state.viewport_3d_mode = true;
            self.last_action_msg = "Viewport: 3D Hardware Perspective mode active.".to_string();
        }

        // 2.5. Receive any files selected asynchronously from background file picker
        while let Ok(path) = self.file_open_receiver.try_recv() {
            self.open_prescription_from_path(&path);
        }

        // 3. Measure screen size and calculate responsive layout metrics
        let screen_rect = ctx.screen_rect();
        let window_size = screen_rect.size();
        let metrics = LayoutMetrics::new(window_size.x, window_size.y);

        // -------------------------------------------------------------------
        // 4. Render TopBar (Menubar + Responsive Lucide Toolbar)
        // -------------------------------------------------------------------
        TopBottomPanel::top("top_app_panel")
            .exact_height(metrics.topbar_height)
            .frame(
                egui::Frame::NONE
                    .fill(tokens.bg_panel)
                    .stroke(tokens.border)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ctx, |ui| {
                if let Some(action) = render_topbar(
                    ui,
                    &mut self.topbar_state,
                    &tokens,
                    metrics.breakpoint,
                    window_size,
                ) {
                    self.handle_topbar_action(action);
                }
            });

        // -------------------------------------------------------------------
        // 5. Render Bottom Status Bar
        // -------------------------------------------------------------------
        TopBottomPanel::bottom("bottom_status_panel")
            .exact_height(metrics.statusbar_height)
            .frame(
                egui::Frame::NONE
                    .fill(tokens.bg_panel)
                    .stroke(tokens.border)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.last_action_msg)
                            .size(11.0)
                            .color(tokens.accent),
                    );
                    ui.separator();

                    let dirty_tag = if self.is_dirty { " [Modified *]" } else { " [Saved]" };
                    ui.label(
                        RichText::new(format!("{}{}", &self.status_text, dirty_tag))
                            .size(11.0)
                            .monospace()
                            .color(if self.is_dirty { tokens.warning } else { tokens.text_muted }),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let bp_name = match metrics.breakpoint {
                            Breakpoint::Compact => "Compact (<780px)",
                            Breakpoint::Regular => "Regular (780-1300px)",
                            Breakpoint::Wide => "Wide (>1300px)",
                        };
                        ui.label(
                            RichText::new(format!("Responsive Mode: {}", bp_name))
                                .size(11.0)
                                .monospace()
                                .color(tokens.text_muted),
                        );
                    });
                });
            });

        // -------------------------------------------------------------------
        // 6. Left Panel (Lens Data Editor placeholder - responsive & collapsible)
        // -------------------------------------------------------------------
        if self.topbar_state.left_panel_open {
            SidePanel::left("left_lens_data_editor")
                .resizable(false)
                .exact_width(400.0)
                .min_width(400.0)
                .max_width(400.0)
                .show_separator_line(true)
                .frame(
                    egui::Frame::NONE
                        .fill(tokens.bg_panel)
                        .stroke(tokens.border)
                        .corner_radius(egui::CornerRadius::ZERO)
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    ui.set_max_width(ui.available_width());
                    // Header: Left Panel Segmented Tabs (LENS DATA | OPTIMIZER)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::new(4.0, 0.0);

                        // Tab 1: LENS DATA
                        let is_ld = self.lens_editor_state.active_tab == LeftPanelTab::LensData;
                        let btn_ld = egui::Button::new(
                            RichText::new("LENS DATA")
                                .size(10.5)
                                .monospace()
                                .strong()
                                .color(if is_ld { tokens.accent } else { tokens.text_muted }),
                        )
                        .fill(if is_ld { tokens.bg_control_active } else { tokens.bg_control })
                        .stroke(if is_ld { tokens.border_hovered } else { tokens.border })
                        .corner_radius(egui::CornerRadius::ZERO);

                        if ui.add(btn_ld).clicked() {
                            self.lens_editor_state.active_tab = LeftPanelTab::LensData;
                        }

                        // Tab 2: MATERIALS (GLASS CATALOG 646)
                        let is_mat = self.lens_editor_state.active_tab == LeftPanelTab::Materials;
                        let btn_mat = egui::Button::new(
                            RichText::new("MATERIALS (646)")
                                .size(10.5)
                                .monospace()
                                .strong()
                                .color(if is_mat { tokens.accent } else { tokens.text_muted }),
                        )
                        .fill(if is_mat { tokens.bg_control_active } else { tokens.bg_control })
                        .stroke(if is_mat { tokens.border_hovered } else { tokens.border })
                        .corner_radius(egui::CornerRadius::ZERO);

                        if ui.add(btn_mat).clicked() {
                            self.lens_editor_state.active_tab = LeftPanelTab::Materials;
                        }

                        // Tab 3: OPTIMIZER (with active variable count badge)
                        let is_opt = self.lens_editor_state.active_tab == LeftPanelTab::Optimizer;
                        let var_count = self.lens_editor_state.total_variable_count();
                        let opt_text = if var_count > 0 {
                            format!("OPTIMIZER ({})", var_count)
                        } else {
                            "OPTIMIZER".to_string()
                        };

                        let btn_opt = egui::Button::new(
                            RichText::new(opt_text)
                                .size(10.5)
                                .monospace()
                                .strong()
                                .color(if is_opt { tokens.accent } else { tokens.text_muted }),
                        )
                        .fill(if is_opt { tokens.bg_control_active } else { tokens.bg_control })
                        .stroke(if is_opt { tokens.border_hovered } else { tokens.border })
                        .corner_radius(egui::CornerRadius::ZERO);

                        if ui.add(btn_opt).clicked() {
                            self.lens_editor_state.active_tab = LeftPanelTab::Optimizer;
                            self.optimization_state.init_if_needed(&self.prescription, &self.catalog);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Close Panel", &tokens).clicked() {
                                self.topbar_state.left_panel_open = false;
                            }
                        });
                    });
                    ui.separator();

                    let prev_snapshot = AppUndoSnapshot {
                        prescription: self.prescription.clone(),
                        opt_summary: self.optimization_state.last_summary.clone(),
                        opt_history: self.optimization_state.history.clone(),
                        opt_original_prescription: self.optimization_state.original_prescription.clone(),
                    };
                    let modified = match self.lens_editor_state.active_tab {
                        LeftPanelTab::LensData => {
                            render_lens_editor(
                                ui,
                                &mut self.prescription,
                                &mut self.lens_editor_state,
                                &self.catalog,
                                &tokens,
                            )
                        }
                        LeftPanelTab::Materials => {
                            render_materials_panel(
                                ui,
                                &mut self.prescription,
                                &mut self.lens_editor_state,
                                &self.catalog,
                                &tokens,
                            )
                        }
                        LeftPanelTab::Optimizer => {
                            render_optimization_panel(
                                ui,
                                &mut self.prescription,
                                &self.lens_editor_state,
                                &mut self.optimization_state,
                                &self.catalog,
                                &tokens,
                            )
                        }
                    };

                    if modified {
                        self.undo_stack.push(prev_snapshot);
                        if self.undo_stack.len() > 50 {
                            self.undo_stack.remove(0);
                        }
                        self.redo_stack.clear();
                        self.is_dirty = true;
                        self.topbar_state.is_dirty = true;
                        self.last_action_msg = format!("Prescription '{}' updated.", self.prescription.name);
                        self.status_text = format!(
                            "Surfaces: {} | EPD: {:.1} mm | Modified",
                            self.prescription.surfaces.len(),
                            self.prescription.entrance_pupil_diameter_mm
                        );
                    }
                });
        }

        // -------------------------------------------------------------------
        // 7. Right Panel (Analysis & Diagnostics - fully interactive)
        // -------------------------------------------------------------------
        if self.topbar_state.right_panel_open {
            SidePanel::right("right_diagnostics_panel")
                .resizable(false)
                .exact_width(400.0)
                .min_width(400.0)
                .max_width(400.0)
                .show_separator_line(true)
                .frame(
                    egui::Frame::NONE
                        .fill(tokens.bg_panel)
                        .stroke(tokens.border)
                        .corner_radius(egui::CornerRadius::ZERO)
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("ANALYSIS & DIAGNOSTICS")
                                .strong()
                                .size(11.5)
                                .color(tokens.text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Close Panel", &tokens).clicked() {
                                self.topbar_state.right_panel_open = false;
                            }
                        });
                    });
                    ui.separator();

                    if let Some(action) = render_analysis_panel(
                        ui,
                        &mut self.prescription,
                        &self.catalog,
                        &mut self.analysis_state,
                        &mut self.tolerancing_state,
                        &tokens,
                    ) {
                        match action {
                            TolerancingAction::PrescriptionModified(new_presc) => {
                                self.push_undo_snapshot();
                                self.redo_stack.clear();
                                self.prescription = new_presc;
                                self.is_dirty = true;
                                self.topbar_state.is_dirty = true;
                                self.last_action_msg =
                                    "Loaded worst-case Monte Carlo prescription into editor.".to_string();
                            }
                        }
                    }
                });
        }

        // -------------------------------------------------------------------
        // 7b. Non-Sequential & Stray Light Bottom Drawer (Docked under viewport)
        // -------------------------------------------------------------------
        let drawer_height = if self.nonseq_state.drawer_open {
            248.0_f32.min(window_size.y * 0.38)
        } else {
            28.0
        };

        TopBottomPanel::bottom("nonseq_bottom_drawer")
            .resizable(false)
            .exact_height(drawer_height)
            .show_separator_line(self.nonseq_state.drawer_open)
            .frame(
                egui::Frame::NONE
                    .fill(tokens.bg_panel)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_max_width(ui.available_width());
                ui.set_max_height(drawer_height);
                render_nonseq_bottom_drawer(
                    ui,
                    &mut self.nonseq_state,
                    &self.prescription,
                    &self.catalog,
                    &tokens,
                );
            });

        // -------------------------------------------------------------------
        // 8. Central Viewport (2D Optical Canvas)
        // -------------------------------------------------------------------
        // Synchronize ray aiming mode bidirectionally between System Explorer and Viewport
        self.viewport_state.ray_aiming_mode = self.lens_editor_state.ray_aiming_mode;
        // Synchronize ghost ray trajectories and isolated pair from NonSeq state to Viewport state
        self.viewport_state.show_ghost_rays = self.nonseq_state.show_ghost_rays_in_viewport;
        self.viewport_state.ghost_rays = self.nonseq_state.filtered_ghost_ray_paths();
        self.viewport_state.selected_ghost_pair = self.nonseq_state.selected_ghost_pair;
        // Synchronize drawer toggle and 3D mode with topbar
        self.topbar_state.nonseq_drawer_open = self.nonseq_state.drawer_open;
        self.topbar_state.viewport_3d_mode = self.viewport_state.is_3d;

        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.bg_canvas))
            .show(ctx, |ui| {
                if self.viewport_state.is_3d {
                    render_optical_viewport_3d(
                        ui,
                        &self.prescription,
                        &mut self.viewport_state,
                        &tokens,
                    );
                } else {
                    render_optical_viewport(
                        ui,
                        &self.prescription,
                        &mut self.viewport_state,
                        &tokens,
                    );
                }
            });

        // Propagate any toggle back from Viewport to System Explorer and NonSeq
        self.lens_editor_state.ray_aiming_mode = self.viewport_state.ray_aiming_mode;
        self.nonseq_state.show_ghost_rays_in_viewport = self.viewport_state.show_ghost_rays;
    }
}

impl OpticsWorkbenchApp {
    /// Pushes current application state to the undo stack.
    pub fn push_undo_snapshot(&mut self) {
        let snapshot = AppUndoSnapshot {
            prescription: self.prescription.clone(),
            opt_summary: self.optimization_state.last_summary.clone(),
            opt_history: self.optimization_state.history.clone(),
            opt_original_prescription: self.optimization_state.original_prescription.clone(),
        };
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Appends a file path to the recent files history list.
    fn add_recent_file(&mut self, path: PathBuf) {
        if let Some(pos) = self.recent_files.iter().position(|p| p == &path) {
            self.recent_files.remove(pos);
        }
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 8 {
            self.recent_files.truncate(8);
        }
        self.topbar_state.recent_files = self.recent_files.clone();
    }

    /// Loads an optical prescription from disk (.zmx, .len, or .seq).
    fn open_prescription_from_path(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();

                let parse_result = match ext.as_str() {
                    "len" => {
                        let content = String::from_utf8_lossy(&bytes);
                        optics_zemax::parse_len(&content)
                            .map(|sys| sys.to_optical_prescription())
                            .map_err(|e| e.to_string())
                    }
                    "seq" => {
                        let content = String::from_utf8_lossy(&bytes);
                        optics_zemax::parse_seq(&content)
                            .map(|sys| sys.to_optical_prescription())
                            .map_err(|e| e.to_string())
                    }
                    _ => optics_zemax::parse_zmx_bytes(&bytes)
                        .map(|sys| sys.to_optical_prescription())
                        .map_err(|e| e.to_string()),
                };

                match parse_result {
                    Ok(mut rx) => {
                        let file_stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("optical_system");
                        let file_name = path
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("system.zmx")
                            .to_string();

                        if rx.name.is_empty() || rx.name == "Untitled" {
                            rx.name = file_stem.to_string();
                        }
                        self.prescription = rx;
                        self.current_file_path = Some(path.to_path_buf());
                        self.is_dirty = false;
                        self.topbar_state.current_file_name = file_name.clone();
                        self.topbar_state.is_dirty = false;
                        self.add_recent_file(path.to_path_buf());
                        self.undo_stack.clear();
                        self.redo_stack.clear();
                        self.lens_editor_state.clear_all_variables();
                        self.optimization_state.reset_history();
                        self.lens_editor_state.selected_surface =
                            Some(1.min(self.prescription.surfaces.len().saturating_sub(1)));
                        self.analysis_state.selected_field_deg = 0.0;
                        self.viewport_state.initialized = false; // Trigger camera zoom_fit
                        self.last_action_msg = format!("Loaded optical system '{}'", file_name);
                        self.status_text = format!(
                            "Surfaces: {} | EPD: {:.1} mm | File: {}",
                            self.prescription.surfaces.len(),
                            self.prescription.entrance_pupil_diameter_mm,
                            file_name
                        );
                    }
                    Err(e) => {
                        self.last_action_msg = format!("Failed to parse optical file: {}", e);
                    }
                }
            }
            Err(e) => {
                self.last_action_msg = format!("Failed to read file: {}", e);
            }
        }
    }

    /// Serializes and saves the optical prescription to disk in Zemax `.zmx` format.
    fn save_prescription_to_path(&mut self, path: &Path) {
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("optical_system")
            .to_string();
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("system.zmx")
            .to_string();

        // Update prescription and document name to match the saved file immediately
        self.prescription.name = file_stem;
        self.topbar_state.current_file_name = file_name.clone();
        self.current_file_path = Some(path.to_path_buf());
        self.is_dirty = false;
        self.topbar_state.is_dirty = false;
        self.add_recent_file(path.to_path_buf());

        let zmx_str = optics_zemax::write_prescription_zmx(&self.prescription);
        match std::fs::write(path, zmx_str) {
            Ok(()) => {
                self.last_action_msg = format!("Saved prescription to '{}'", file_name);
                self.status_text = format!(
                    "Surfaces: {} | EPD: {:.1} mm | Saved: {}",
                    self.prescription.surfaces.len(),
                    self.prescription.entrance_pupil_diameter_mm,
                    file_name
                );
            }
            Err(e) => {
                self.last_action_msg = format!("Error saving file: {}", e);
            }
        }
    }

    fn handle_topbar_action(&mut self, action: TopBarAction) {
        match action {
            TopBarAction::NewFile => {
                self.prescription = OpticalPrescription {
                    name: "Untitled Singlet".to_string(),
                    entrance_pupil_diameter_mm: 10.0,
                    field_angles_deg: vec![0.0],
                    wavelengths_nm: vec![587.56],
                    primary_wavelength_index: 0,
                    surfaces: vec![
                        optics_core::OpticalSurface {
                            surface_index: 1,
                            radius_mm: 50.0,
                            conic_constant: 0.0,
                            thickness_mm: 5.0,
                            semi_diameter_mm: 10.0,
                            material_name: "N-BK7".to_string(),
                            is_stop: false,
                            asphere_coefficients: Vec::new(),
                            coordinate_break: None,
                            grin_profile: None,
                            profile: None,
                            aperture: None,
                    coating: None,
                        },
                        optics_core::OpticalSurface {
                            surface_index: 2,
                            radius_mm: f64::INFINITY,
                            conic_constant: 0.0,
                            thickness_mm: 10.0,
                            semi_diameter_mm: 10.0,
                            material_name: "AIR".to_string(),
                            is_stop: true,
                            asphere_coefficients: Vec::new(),
                            coordinate_break: None,
                            grin_profile: None,
                            profile: None,
                            aperture: None,
                    coating: None,
                        },
                        optics_core::OpticalSurface {
                            surface_index: 3,
                            radius_mm: f64::INFINITY,
                            conic_constant: 0.0,
                            thickness_mm: 35.0,
                            semi_diameter_mm: 10.0,
                            material_name: "AIR".to_string(),
                            is_stop: false,
                            asphere_coefficients: Vec::new(),
                            coordinate_break: None,
                            grin_profile: None,
                            profile: None,
                            aperture: None,
                    coating: None,
                        },
                    ],
                };
                self.current_file_path = None;
                self.is_dirty = false;
                self.topbar_state.current_file_name = "Untitled Singlet.zmx".to_string();
                self.topbar_state.is_dirty = false;
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.lens_editor_state.clear_all_variables();
                self.optimization_state.reset_history();
                self.lens_editor_state.selected_surface = Some(1);
                self.analysis_state.selected_field_deg = 0.0;
                self.viewport_state.initialized = false;
                self.last_action_msg = "New optical prescription created.".to_string();
                self.status_text = format!(
                    "Surfaces: {} | EPD: {:.1} mm | Custom",
                    self.prescription.surfaces.len(),
                    self.prescription.entrance_pupil_diameter_mm
                );
            }
            TopBarAction::OpenFile => {
                let sender = self.file_open_sender.clone();
                std::thread::spawn(move || {
                    let dialog = rfd::FileDialog::new()
                        .set_title("Open Optical Prescription")
                        .add_filter(
                            "Optical Prescriptions (*.zmx, *.len, *.seq)",
                            &["zmx", "ZMX", "len", "LEN", "seq", "SEQ"],
                        )
                        .add_filter("Zemax Prescription (*.zmx)", &["zmx", "ZMX"])
                        .add_filter("OSLO Lens File (*.len)", &["len", "LEN"])
                        .add_filter("CODE V Sequence (*.seq)", &["seq", "SEQ"])
                        .add_filter("All Files (*.*)", &["*"]);

                    if let Some(path) = dialog.pick_file() {
                        let _ = sender.send(path);
                    }
                });
            }
            TopBarAction::SaveFile => {
                if let Some(path) = self.current_file_path.clone() {
                    self.save_prescription_to_path(&path);
                } else {
                    self.handle_topbar_action(TopBarAction::SaveFileAs);
                }
            }
            TopBarAction::SaveFileAs => {
                let sanitized_name = self
                    .prescription
                    .name
                    .replace(['/', '\\', ' ', ':'], "_");
                let default_file = if sanitized_name.ends_with(".zmx") {
                    sanitized_name
                } else {
                    format!("{}.zmx", sanitized_name)
                };
                let dialog = rfd::FileDialog::new()
                    .set_title("Save Zemax Prescription")
                    .set_file_name(&default_file)
                    .add_filter("Zemax Prescription (*.zmx)", &["zmx"]);

                if let Some(path) = dialog.save_file() {
                    self.save_prescription_to_path(&path);
                }
            }
            TopBarAction::LoadRecent(path) => {
                self.open_prescription_from_path(&path);
            }
            TopBarAction::ExportDxf => {
                let default_name = format!(
                    "{}_2d.dxf",
                    self.prescription.name.replace(['/', '\\', ' ', ':'], "_")
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export AutoCAD DXF 2D Technical Layout")
                    .set_file_name(&default_name)
                    .add_filter("AutoCAD DXF (*.dxf)", &["dxf"])
                    .save_file()
                {
                    match optics_zemax::export_dxf_layout(
                        &self.prescription,
                        None,
                        &optics_zemax::DxfConfig::default(),
                    ) {
                        Ok(content) => match std::fs::write(&path, content) {
                            Ok(()) => {
                                self.last_action_msg =
                                    format!("Exported DXF CAD to '{}'", path.display())
                            }
                            Err(e) => self.last_action_msg = format!("Error saving DXF: {}", e),
                        },
                        Err(e) => self.last_action_msg = format!("Error formatting DXF: {:?}", e),
                    }
                }
            }
            TopBarAction::ExportSvg => {
                let default_name = format!(
                    "{}_diagram.svg",
                    self.prescription.name.replace(['/', '\\', ' ', ':'], "_")
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export Technical SVG Vector Layout")
                    .set_file_name(&default_name)
                    .add_filter("Scalable Vector Graphics (*.svg)", &["svg"])
                    .save_file()
                {
                    match optics_zemax::export_svg_layout(
                        &self.prescription,
                        None,
                        &optics_zemax::SvgConfig::default(),
                    ) {
                        Ok(content) => match std::fs::write(&path, content) {
                            Ok(()) => {
                                self.last_action_msg =
                                    format!("Exported SVG Vector Layout to '{}'", path.display())
                            }
                            Err(e) => self.last_action_msg = format!("Error saving SVG: {}", e),
                        },
                        Err(e) => self.last_action_msg = format!("Error formatting SVG: {:?}", e),
                    }
                }
            }
            TopBarAction::ExportStl => {
                let default_name = format!(
                    "{}_lens.stl",
                    self.prescription.name.replace(['/', '\\', ' ', ':'], "_")
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export 3D STL Precision Mesh (3D Print)")
                    .set_file_name(&default_name)
                    .add_filter("Stereolithography Mesh (*.stl)", &["stl"])
                    .save_file()
                {
                    let stl_bytes =
                        optics_zemax::export_stl_lens(&self.prescription, true, 48, 16);
                    match std::fs::write(&path, stl_bytes) {
                        Ok(()) => {
                            self.last_action_msg =
                                format!("Exported 3D STL to '{}'", path.display())
                        }
                        Err(e) => self.last_action_msg = format!("Error saving STL: {}", e),
                    }
                }
            }
            TopBarAction::ExportObj => {
                let default_name = format!(
                    "{}_mesh.obj",
                    self.prescription.name.replace(['/', '\\', ' ', ':'], "_")
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export Wavefront 3D OBJ & MTL System")
                    .set_file_name(&default_name)
                    .add_filter("Wavefront 3D OBJ (*.obj)", &["obj"])
                    .save_file()
                {
                    let (obj_str, mtl_str) =
                        optics_zemax::export_obj_system(&self.prescription, 32, 12);
                    let mtl_path = path.with_extension("mtl");
                    let mut ok = true;
                    if let Err(e) = std::fs::write(&path, obj_str) {
                        self.last_action_msg = format!("Error saving OBJ: {}", e);
                        ok = false;
                    }
                    if ok {
                        let _ = std::fs::write(&mtl_path, mtl_str);
                        self.last_action_msg =
                            format!("Exported OBJ & MTL to '{}'", path.display());
                    }
                }
            }
            TopBarAction::ExportIso10110 => {
                let default_name = format!(
                    "{}_iso10110.md",
                    self.prescription.name.replace(['/', '\\', ' ', ':'], "_")
                );
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export ISO 10110 Manufacturing Drawing Sheet (Markdown)")
                    .set_file_name(&default_name)
                    .add_filter("ISO 10110 Markdown Sheet (*.md)", &["md", "txt"])
                    .save_file()
                {
                    let sheet = optics_zemax::Iso10110DrawingSheet::from_prescription(
                        &self.prescription,
                        Some(&self.catalog),
                    );
                    let report_md = sheet.to_markdown_table();
                    match std::fs::write(&path, report_md) {
                        Ok(()) => {
                            self.last_action_msg =
                                format!("Exported ISO 10110 Sheet to '{}'", path.display())
                        }
                        Err(e) => {
                            self.last_action_msg = format!("Error saving ISO 10110 sheet: {}", e)
                        }
                    }
                }
            }
            TopBarAction::LoadCookeTriplet => {
                if let Ok(rx) = optics_zemax::presets::cooke_triplet_f4_5() {
                    self.prescription = rx;
                    self.current_file_path = None;
                    self.is_dirty = false;
                    self.topbar_state.current_file_name = "Cooke Triplet F/4.5.zmx".to_string();
                    self.topbar_state.is_dirty = false;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.lens_editor_state.clear_all_variables();
                    self.optimization_state.reset_history();
                    self.lens_editor_state.selected_surface = Some(1);
                    self.analysis_state.selected_field_deg = 0.0;
                    self.viewport_state.initialized = false;
                    self.last_action_msg =
                        "Loaded preset: Cooke Triplet F/4.5 (3 elements).".to_string();
                    self.status_text = format!(
                        "Surfaces: {} | EPD: {:.1} mm | EFL ~50mm F/4.5",
                        self.prescription.surfaces.len(),
                        self.prescription.entrance_pupil_diameter_mm
                    );
                }
            }
            TopBarAction::LoadDoubleGauss => {
                if let Ok(rx) = optics_zemax::presets::double_gauss_50mm() {
                    self.prescription = rx;
                    self.current_file_path = None;
                    self.is_dirty = false;
                    self.topbar_state.current_file_name = "Double Gauss F/2.0.zmx".to_string();
                    self.topbar_state.is_dirty = false;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.lens_editor_state.clear_all_variables();
                    self.optimization_state.reset_history();
                    self.lens_editor_state.selected_surface = Some(1);
                    self.analysis_state.selected_field_deg = 0.0;
                    self.viewport_state.initialized = false;
                    self.last_action_msg =
                        "Loaded preset: Double Gauss F/2.0 (6 elements).".to_string();
                    self.status_text = format!(
                        "Surfaces: {} | EPD: {:.1} mm | EFL ~50mm F/2.0",
                        self.prescription.surfaces.len(),
                        self.prescription.entrance_pupil_diameter_mm
                    );
                }
            }
            TopBarAction::LoadPlanoConvex => {
                if let Ok(rx) = optics_zemax::presets::plano_convex_100mm() {
                    self.prescription = rx;
                    self.current_file_path = None;
                    self.is_dirty = false;
                    self.topbar_state.current_file_name = "Plano-Convex Singlet.zmx".to_string();
                    self.topbar_state.is_dirty = false;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.lens_editor_state.clear_all_variables();
                    self.optimization_state.reset_history();
                    self.lens_editor_state.selected_surface = Some(1);
                    self.analysis_state.selected_field_deg = 0.0;
                    self.viewport_state.initialized = false;
                    self.last_action_msg = "Loaded preset: Plano-Convex Singlet.".to_string();
                    self.status_text = format!(
                        "Surfaces: {} | EPD: {:.1} mm | EFL ~100mm",
                        self.prescription.surfaces.len(),
                        self.prescription.entrance_pupil_diameter_mm
                    );
                }
            }
            TopBarAction::Undo => {
                if let Some(prev) = self.undo_stack.pop() {
                    let current_snapshot = AppUndoSnapshot {
                        prescription: self.prescription.clone(),
                        opt_summary: self.optimization_state.last_summary.clone(),
                        opt_history: self.optimization_state.history.clone(),
                        opt_original_prescription: self.optimization_state.original_prescription.clone(),
                    };
                    self.redo_stack.push(current_snapshot);
                    self.prescription = prev.prescription;
                    self.optimization_state.last_summary = prev.opt_summary;
                    self.optimization_state.history = prev.opt_history;
                    self.optimization_state.original_prescription = prev.opt_original_prescription;
                    self.is_dirty = true;
                    self.topbar_state.is_dirty = true;
                    self.viewport_state.initialized = false;
                    self.last_action_msg = "Undo edit.".to_string();
                } else {
                    self.last_action_msg = "Nothing to undo.".to_string();
                }
            }
            TopBarAction::Redo => {
                if let Some(next) = self.redo_stack.pop() {
                    let current_snapshot = AppUndoSnapshot {
                        prescription: self.prescription.clone(),
                        opt_summary: self.optimization_state.last_summary.clone(),
                        opt_history: self.optimization_state.history.clone(),
                        opt_original_prescription: self.optimization_state.original_prescription.clone(),
                    };
                    self.undo_stack.push(current_snapshot);
                    self.prescription = next.prescription;
                    self.optimization_state.last_summary = next.opt_summary;
                    self.optimization_state.history = next.opt_history;
                    self.optimization_state.original_prescription = next.opt_original_prescription;
                    self.is_dirty = true;
                    self.topbar_state.is_dirty = true;
                    self.viewport_state.initialized = false;
                    self.last_action_msg = "Redo edit.".to_string();
                } else {
                    self.last_action_msg = "Nothing to redo.".to_string();
                }
            }
            TopBarAction::ShowSpotDiagram => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::SpotDiagram;
                self.last_action_msg = "Analysis: Switched to Spot Diagram RMS tab.".to_string();
            }
            TopBarAction::ShowSeidel => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::SeidelAberrations;
                self.last_action_msg = "Analysis: Switched to 3rd-Order Seidel Sums tab.".to_string();
            }
            TopBarAction::ShowMtf => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::MtfCurve;
                self.last_action_msg = "Analysis: Switched to Diffraction MTF Curve tab.".to_string();
            }
            TopBarAction::ShowCardinal => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::ParaxialCardinal;
                self.last_action_msg = "Analysis: Switched to Paraxial Cardinal tab.".to_string();
            }
            TopBarAction::RunRaytrace => {
                self.viewport_state.show_rays = !self.viewport_state.show_rays;
                self.topbar_state.raytrace_running = self.viewport_state.show_rays;
                self.last_action_msg = if self.viewport_state.show_rays {
                    "Raytrace: Tracing chromatic ray fans (lines d, F, C)...".to_string()
                } else {
                    "Raytrace: Rays hidden.".to_string()
                };
            }
            TopBarAction::ZoomIn => {
                self.viewport_state.zoom = (self.viewport_state.zoom * 1.25).clamp(0.2, 500.0);
                self.last_action_msg = format!("Zoom: In ({:.1} px/mm)", self.viewport_state.zoom);
            }
            TopBarAction::ZoomOut => {
                self.viewport_state.zoom = (self.viewport_state.zoom * 0.8).clamp(0.2, 500.0);
                self.last_action_msg =
                    format!("Zoom: Out ({:.1} px/mm)", self.viewport_state.zoom);
            }
            TopBarAction::ZoomFit => {
                self.viewport_state
                    .zoom_fit(&self.prescription, egui::Vec2::new(800.0, 600.0));
                self.last_action_msg = "View: Zoom fit to optical bounding box.".to_string();
            }
            TopBarAction::ResetView => {
                self.viewport_state.reset_view();
                self.last_action_msg = "View: Viewport re-centered on optical axis.".to_string();
            }
            TopBarAction::ToggleLeftPanel => {
                self.topbar_state.left_panel_open = !self.topbar_state.left_panel_open;
                self.last_action_msg = format!(
                    "Left Panel (Lens Data Editor): {}",
                    if self.topbar_state.left_panel_open {
                        "Shown"
                    } else {
                        "Hidden"
                    }
                );
            }
            TopBarAction::ToggleRightPanel => {
                self.topbar_state.right_panel_open = !self.topbar_state.right_panel_open;
                self.last_action_msg = format!(
                    "Right Panel (Diagnostics): {}",
                    if self.topbar_state.right_panel_open {
                        "Shown"
                    } else {
                        "Hidden"
                    }
                );
            }
            TopBarAction::SetTheme(mode) => {
                self.topbar_state.theme_mode = mode;
                self.last_action_msg = format!("Theme set to: {}", mode.display_name());
            }
            TopBarAction::ToggleTheme => {
                self.topbar_state.theme_mode = self.topbar_state.theme_mode.next();
                self.last_action_msg = format!(
                    "Theme switched to: {}",
                    self.topbar_state.theme_mode.display_name()
                );
            }
            TopBarAction::OpenAbout => {
                self.last_action_msg =
                    "OpticsLab v0.0.1 — Academic Optical Engineering Workbench".to_string();
            }
            TopBarAction::OpenOptimization => {
                self.topbar_state.left_panel_open = true;
                self.lens_editor_state.active_tab = LeftPanelTab::Optimizer;
                self.optimization_state.init_if_needed(&self.prescription, &self.catalog);
                self.last_action_msg = "Switched to Optimizer tab.".to_string();
            }
            TopBarAction::OpenOptimizationAlgorithm(algo) => {
                self.topbar_state.left_panel_open = true;
                self.lens_editor_state.active_tab = LeftPanelTab::Optimizer;
                self.optimization_state.init_if_needed(&self.prescription, &self.catalog);
                self.optimization_state.algorithm = algo;
                self.last_action_msg = format!("Switched to Optimizer tab ({:?}).", algo);
            }
            TopBarAction::ShowPolarization => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::Polarization;
                self.last_action_msg = "Analysis: Switched to Polarization Ray Tracing & Stokes tab.".to_string();
            }
            TopBarAction::OpenTolerancing => {
                self.topbar_state.right_panel_open = true;
                self.analysis_state.active_tab = crate::analysis::AnalysisTab::Tolerancing;
                self.tolerancing_state.init_if_needed(&self.prescription, &self.catalog);
                self.last_action_msg = "Analysis: Switched to Tolerancing & Monte Carlo tab.".to_string();
            }
            TopBarAction::ToggleNonSeqDrawer => {
                self.nonseq_state.drawer_open = !self.nonseq_state.drawer_open;
                self.topbar_state.nonseq_drawer_open = self.nonseq_state.drawer_open;
                self.last_action_msg = if self.nonseq_state.drawer_open {
                    "Opened Non-Sequential Detector Drawer.".to_string()
                } else {
                    "Closed Non-Sequential Detector Drawer.".to_string()
                };
            }
            TopBarAction::ToggleViewport3DMode => {
                self.viewport_state.is_3d = !self.viewport_state.is_3d;
                self.topbar_state.viewport_3d_mode = self.viewport_state.is_3d;
                self.last_action_msg = if self.viewport_state.is_3d {
                    "Switched to 3D Hardware Perspective Viewport.".to_string()
                } else {
                    "Switched to 2D Cross-Section CAD Viewport.".to_string()
                };
            }
        }
    }
}
