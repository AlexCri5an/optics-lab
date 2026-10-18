//! Responsive academic TopBar component for OpticsLab.
//!
//! Includes:
//! 1. Classic academic menubar (File, Edit, Presets, Analysis, View, Help).
//! 2. Toolbar with 100% typed Lucide vector icons.
//! 3. Responsive behavior that collapses button labels and adapts spacing
//!    gracefully when resizing the window.

use egui::{Align, Layout, MenuBar, RichText, Stroke, Ui, Vec2};

use crate::icons::{LucideIcon, lucide_toolbar_button};
use crate::layout::Breakpoint;
use crate::theme::{OpticsTokens, ThemeMode};

/// Action events emitted by the TopBar for the main application to handle.
#[derive(Debug, Clone, PartialEq)]
pub enum TopBarAction {
    /// Create a new blank optical prescription.
    NewFile,
    /// Open file dialog for .zmx / .len / .seq.
    OpenFile,
    /// Save current optical file.
    SaveFile,
    /// Save As... with native file dialog.
    SaveFileAs,
    /// Load a recently opened file by path.
    LoadRecent(std::path::PathBuf),
    /// Export AutoCAD Release 12 DXF 2D layout.
    ExportDxf,
    /// Export Scalable Vector Graphics (SVG) technical diagram.
    ExportSvg,
    /// Export 3D Stereolithography (STL) mesh for 3D printing.
    ExportStl,
    /// Export 3D Wavefront OBJ multi-material model.
    ExportObj,
    /// Export ISO 10110 manufacturing drawing sheet markdown report.
    ExportIso10110,
    /// Load preset: Cooke Triplet F/4.5.
    LoadCookeTriplet,
    /// Load preset: Double Gauss F/2.0.
    LoadDoubleGauss,
    /// Load preset: Plano-Convex Singlet.
    LoadPlanoConvex,
    /// Trigger raytracing simulation.
    RunRaytrace,
    /// Zoom in viewport (+25%).
    ZoomIn,
    /// Zoom out viewport (-20%).
    ZoomOut,
    /// Reset 2D/3D viewport zoom and position.
    ResetView,
    /// Zoom fit scene into viewport.
    ZoomFit,
    /// Toggle visibility of Left Panel (Lens Data Editor).
    ToggleLeftPanel,
    /// Toggle visibility of Right Panel (Aberrations & Diagnostics).
    ToggleRightPanel,
    /// Changes the active theme mode.
    SetTheme(ThemeMode),
    /// Toggle between themes.
    ToggleTheme,
    /// Open About modal dialog.
    OpenAbout,
}

/// State for the TopBar component.
#[derive(Debug, Clone)]
pub struct TopBarState {
    /// Whether the left panel (Lens Data Editor) is open.
    pub left_panel_open: bool,
    /// Whether the right panel (Analysis) is open.
    pub right_panel_open: bool,
    /// Current theme mode.
    pub theme_mode: ThemeMode,
    /// Raytrace active animation or state indicator.
    pub raytrace_running: bool,
    /// Active document display name.
    pub current_file_name: String,
    /// Whether current document has unsaved modifications.
    pub is_dirty: bool,
    /// List of recently opened or saved files.
    pub recent_files: Vec<std::path::PathBuf>,
}

impl Default for TopBarState {
    fn default() -> Self {
        Self {
            left_panel_open: true,
            right_panel_open: true,
            theme_mode: ThemeMode::DarkSlate,
            raytrace_running: false,
            current_file_name: "Cooke Triplet F/4.5.zmx".to_string(),
            is_dirty: false,
            recent_files: Vec::new(),
        }
    }
}

/// Renders the complete TopBar (Menubar + Responsive Lucide Toolbar).
pub fn render_topbar(
    ui: &mut Ui,
    state: &mut TopBarState,
    tokens: &OpticsTokens,
    breakpoint: Breakpoint,
    window_size: Vec2,
) -> Option<TopBarAction> {
    let mut triggered_action: Option<TopBarAction> = None;

    ui.vertical(|ui| {
        // -------------------------------------------------------------------
        // 1. Academic Menubar (File, Edit, Presets, Analysis, View, Help)
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

            // Title & Academic brand badge
            ui.label(
                RichText::new("OPTICSLAB")
                    .strong()
                    .monospace()
                    .size(12.0)
                    .color(tokens.accent),
            );

            // Document status tag (Filename + Dirty indicator)
            let dirty_suffix = if state.is_dirty { " *" } else { "" };
            let doc_title = format!("{}{}", state.current_file_name, dirty_suffix);
            ui.label(
                RichText::new(doc_title)
                    .monospace()
                    .size(11.0)
                    .color(if state.is_dirty { tokens.warning } else { tokens.text_muted }),
            );

            ui.label(
                RichText::new("·")
                    .size(12.0)
                    .color(tokens.text_muted),
            );

            // Classic Menus
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New (Ctrl+N)").clicked() {
                        triggered_action = Some(TopBarAction::NewFile);
                        ui.close();
                    }
                    if ui.button("Open (Ctrl+O)").clicked() {
                        triggered_action = Some(TopBarAction::OpenFile);
                        ui.close();
                    }
                    if ui.button("Save (Ctrl+S)").clicked() {
                        triggered_action = Some(TopBarAction::SaveFile);
                        ui.close();
                    }
                    if ui.button("Save As (Ctrl+Shift+S)").clicked() {
                        triggered_action = Some(TopBarAction::SaveFileAs);
                        ui.close();
                    }

                    ui.separator();

                    ui.menu_button("Export CAD & Drawings", |ui| {
                        if ui.button("AutoCAD DXF 2D").clicked() {
                            triggered_action = Some(TopBarAction::ExportDxf);
                            ui.close();
                        }
                        if ui.button("SVG 2D Layout").clicked() {
                            triggered_action = Some(TopBarAction::ExportSvg);
                            ui.close();
                        }
                        if ui.button("3D STL (Binary)").clicked() {
                            triggered_action = Some(TopBarAction::ExportStl);
                            ui.close();
                        }
                        if ui.button("Wavefront OBJ 3D").clicked() {
                            triggered_action = Some(TopBarAction::ExportObj);
                            ui.close();
                        }
                        if ui.button("ISO 10110 Drawing (MD)").clicked() {
                            triggered_action = Some(TopBarAction::ExportIso10110);
                            ui.close();
                        }
                    });

                    ui.separator();

                    if !state.recent_files.is_empty() {
                        ui.menu_button("Recent Files", |ui| {
                            for path in &state.recent_files {
                                let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("file");
                                if ui.button(name).clicked() {
                                    triggered_action = Some(TopBarAction::LoadRecent(path.clone()));
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                    }

                    ui.menu_button("Standard Presets", |ui| {
                        if ui.button("Cooke Triplet (F/4.5)").clicked() {
                            triggered_action = Some(TopBarAction::LoadCookeTriplet);
                            ui.close();
                        }
                        if ui.button("Double Gauss (F/2.0)").clicked() {
                            triggered_action = Some(TopBarAction::LoadDoubleGauss);
                            ui.close();
                        }
                        if ui.button("Plano-Convex Singlet").clicked() {
                            triggered_action = Some(TopBarAction::LoadPlanoConvex);
                            ui.close();
                        }
                    });
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        ui.close();
                    }
                    if ui.button("Redo").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Analysis", |ui| {
                    if ui.button("Run Raytrace").clicked() {
                        triggered_action = Some(TopBarAction::RunRaytrace);
                        ui.close();
                    }
                    if ui.button("Paraxial 1st Order Solve").clicked() {
                        ui.close();
                    }
                    if ui.button("3rd Order Seidel Sums").clicked() {
                        ui.close();
                    }
                    if ui.button("Spot Diagram RMS").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Reset View").clicked() {
                        triggered_action = Some(TopBarAction::ResetView);
                        ui.close();
                    }
                    if ui.button("Zoom Fit").clicked() {
                        triggered_action = Some(TopBarAction::ZoomFit);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(if state.left_panel_open { "Hide Lens Editor" } else { "Show Lens Editor" }).clicked() {
                        triggered_action = Some(TopBarAction::ToggleLeftPanel);
                        ui.close();
                    }
                    if ui.button(if state.right_panel_open { "Hide Analysis Panel" } else { "Show Analysis Panel" }).clicked() {
                        triggered_action = Some(TopBarAction::ToggleRightPanel);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button("Theme", |ui| {
                        for mode in ThemeMode::ALL {
                            let prefix = if state.theme_mode == mode { "✓ " } else { "  " };
                            if ui.button(format!("{}{}", prefix, mode.display_name())).clicked() {
                                state.theme_mode = mode;
                                triggered_action = Some(TopBarAction::SetTheme(mode));
                                ui.close();
                            }
                        }
                    });
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About OpticsLab").clicked() {
                        triggered_action = Some(TopBarAction::OpenAbout);
                        ui.close();
                    }
                });
            });

            // Right side of menubar: subtle status & breakpoint badge
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Breakpoint indicator
                let bp_tag = match breakpoint {
                    Breakpoint::Compact => "[Compact]",
                    Breakpoint::Regular => "[Regular]",
                    Breakpoint::Wide => "[Wide]",
                };
                ui.label(
                    RichText::new(format!("{} {:.0}x{:.0}", bp_tag, window_size.x, window_size.y))
                        .monospace()
                        .size(10.5)
                        .color(tokens.text_muted),
                );
            });
        });

        // 1px separation stroke between menubar and toolbar
        ui.painter().line_segment(
            [
                ui.cursor().min,
                egui::Pos2::new(ui.cursor().min.x + ui.available_width(), ui.cursor().min.y),
            ],
            tokens.border,
        );
        ui.add_space(2.0);

        // -------------------------------------------------------------------
        // 2. Responsive Lucide Toolbar
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

            // Group 1: File actions
            if lucide_toolbar_button(ui, LucideIcon::FilePlus, "New Prescription (Ctrl+N)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::NewFile);
            }
            if lucide_toolbar_button(ui, LucideIcon::FolderOpen, "Open Prescription (Ctrl+O)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::OpenFile);
            }
            if lucide_toolbar_button(ui, LucideIcon::Save, "Save Prescription (Ctrl+S)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::SaveFile);
            }

            draw_toolbar_separator(ui, tokens.border);

            // Group 2: Presets quick access
            if lucide_toolbar_button(ui, LucideIcon::Sparkles, "Standard Lens Presets (Cooke, Double Gauss)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::LoadCookeTriplet);
            }

            draw_toolbar_separator(ui, tokens.border);

            // Group 3: Simulation
            if lucide_toolbar_button(ui, LucideIcon::Play, "Run Sequential Raytrace (F5)", state.raytrace_running, tokens).clicked() {
                triggered_action = Some(TopBarAction::RunRaytrace);
            }

            draw_toolbar_separator(ui, tokens.border);

            // Group 4: Unified View & Zoom Navigation (Plus, Minus, Fit, Reset)
            if lucide_toolbar_button(ui, LucideIcon::Plus, "Zoom In (+25%)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::ZoomIn);
            }
            if lucide_toolbar_button(ui, LucideIcon::Minus, "Zoom Out (-20%)", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::ZoomOut);
            }
            if lucide_toolbar_button(ui, LucideIcon::Maximize2, "Zoom Fit System to Viewport", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::ZoomFit);
            }
            if lucide_toolbar_button(ui, LucideIcon::RotateCcw, "Reset View & Center Optical Axis", false, tokens).clicked() {
                triggered_action = Some(TopBarAction::ResetView);
            }

            draw_toolbar_separator(ui, tokens.border);

            // Group 5: Panel toggles
            if lucide_toolbar_button(
                ui,
                LucideIcon::PanelLeft,
                if state.left_panel_open { "Hide Lens Data Editor" } else { "Show Lens Data Editor" },
                state.left_panel_open,
                tokens,
            ).clicked() {
                triggered_action = Some(TopBarAction::ToggleLeftPanel);
            }

            if lucide_toolbar_button(
                ui,
                LucideIcon::PanelRight,
                if state.right_panel_open { "Hide Aberration Analysis" } else { "Show Aberration Analysis" },
                state.right_panel_open,
                tokens,
            ).clicked() {
                triggered_action = Some(TopBarAction::ToggleRightPanel);
            }

            // Right side of toolbar: Retro Theme Selector Dropdown + Quick Cycle Button
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let icon = if state.theme_mode.is_dark() {
                    LucideIcon::Moon
                } else {
                    LucideIcon::Sun
                };
                if lucide_toolbar_button(ui, icon, "Cycle Theme", false, tokens).clicked() {
                    triggered_action = Some(TopBarAction::ToggleTheme);
                }

                egui::ComboBox::from_id_salt("topbar_theme_dropdown")
                    .selected_text(
                        RichText::new(state.theme_mode.display_name())
                            .size(10.5)
                            .monospace()
                            .color(tokens.text_primary),
                    )
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for mode in ThemeMode::ALL {
                            let is_selected = state.theme_mode == mode;
                            if ui
                                .selectable_label(
                                    is_selected,
                                    RichText::new(mode.display_name()).size(10.5).monospace(),
                                )
                                .clicked()
                            {
                                state.theme_mode = mode;
                                triggered_action = Some(TopBarAction::SetTheme(mode));
                            }
                        }
                    });
            });
        });
    });

    triggered_action
}

/// Draws a subtle 1px vertical separator in the toolbar.
fn draw_toolbar_separator(ui: &mut Ui, border_stroke: Stroke) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(7.0, 22.0), egui::Sense::hover());
    let center_x = rect.center().x;
    ui.painter().line_segment(
        [
            egui::Pos2::new(center_x, rect.min.y + 2.0),
            egui::Pos2::new(center_x, rect.max.y - 2.0),
        ],
        border_stroke,
    );
}
