//! Academic Optical Optimization dialog and DLS (Levenberg-Marquardt) execution engine.
//!
//! Provides an interactive control hub for the automatic optimization backend (`optics-optimize`):
//! - Direct binding to variable flags tracked in `LensEditorState` (`[V]`).
//! - Academic operand configuration: RMS Spot Radius minimization & Target Focal Length (EFL).
//! - Step-by-step single cycle execution and multi-iteration runs.
//! - Non-destructive experimentation with instantaneous "Revert" functionality.

use egui::{
    Button, Color32, CornerRadius, DragValue, Frame, Image, Margin, RichText, ScrollArea, Stroke,
    Ui, Vec2,
};
use optics_core::OpticalPrescription;
use optics_core::prescription::calculate_paraxial_properties;
use optics_glass::catalog::GlassCatalogDb;
use optics_optimize::{
    DlsConfig, DlsOptimizer, MeritFunction, MeritOperand, OperandType, VariableSet, VariableType,
};

use crate::icons::LucideIcon;
use crate::lens_editor::LensEditorState;
use crate::theme::OpticsTokens;

/// Summary of an executed DLS optimization session.
#[derive(Debug, Clone, PartialEq)]
pub struct DlsRunSummary {
    /// Initial merit value prior to optimization.
    pub initial_merit: f64,
    /// Final merit value achieved.
    pub final_merit: f64,
    /// Number of cycles executed in this run.
    pub iterations_run: usize,
    /// Whether the solver satisfied tolerance criteria.
    pub converged: bool,
    /// Relative percentage reduction of the merit function (positive = improvement).
    pub percent_improvement: f64,
}

/// State for the Optimization dialog modal.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizationModalState {
    /// Whether the optimization window is visible.
    pub open: bool,
    /// Number of iterations per execution cycle (default: 10).
    pub iterations: usize,
    /// Initial Levenberg-Marquardt damping factor $\lambda_0$ (default: 1.0).
    pub initial_damping: f64,
    /// Whether to minimize RMS spot size across configured field angles.
    pub min_rms_spot: bool,
    /// Optimization weight for RMS spot radius.
    pub min_rms_weight: f64,
    /// Whether to enforce a target Effective Focal Length (EFL).
    pub target_efl_enabled: bool,
    /// Target EFL in millimeters.
    pub target_efl_value: f64,
    /// Optimization weight for target EFL operand.
    pub target_efl_weight: f64,
    /// Snapshot of the prescription before optimization started (for Revert).
    pub original_prescription: Option<OpticalPrescription>,
    /// History stack of (prescription, summary) before each optimization run for multi-level synchronized undo.
    pub history: Vec<(OpticalPrescription, Option<DlsRunSummary>)>,
    /// Summary of the last completed run.
    pub last_summary: Option<DlsRunSummary>,
    /// Status or error message from the last operation.
    pub status_message: Option<String>,
}

impl Default for OptimizationModalState {
    fn default() -> Self {
        Self {
            open: false,
            iterations: 10,
            initial_damping: 1.0,
            min_rms_spot: true,
            min_rms_weight: 1.0,
            target_efl_enabled: true,
            target_efl_value: 50.0,
            target_efl_weight: 100.0,
            original_prescription: None,
            history: Vec::new(),
            last_summary: None,
            status_message: None,
        }
    }
}

impl OptimizationModalState {
    /// Resets all history and baseline snapshots (e.g. when loading a new file or preset).
    pub fn reset_history(&mut self) {
        self.original_prescription = None;
        self.history.clear();
        self.last_summary = None;
        self.status_message = None;
    }

    /// Opens the dialog and captures the baseline prescription for potential revert.
    pub fn open_with_snapshot(&mut self, prescription: &OpticalPrescription, catalog: &GlassCatalogDb) {
        self.open = true;
        self.init_if_needed(prescription, catalog);
    }

    /// Initializes default baseline snapshot and target EFL from prescription if not set.
    pub fn init_if_needed(&mut self, prescription: &OpticalPrescription, catalog: &GlassCatalogDb) {
        if self.original_prescription.is_none() {
            self.original_prescription = Some(prescription.clone());
            self.last_summary = None;
            self.status_message = None;

            // Auto-detect current EFL as the default target EFL if valid
            let prim_wl = prescription
                .wavelengths_nm
                .get(prescription.primary_wavelength_index)
                .copied()
                .unwrap_or(587.56);
            if let Ok(resolved) = prescription.resolve(catalog) {
                if let Ok(parax) = calculate_paraxial_properties(&resolved, prim_wl) {
                    if parax.efl_mm.is_finite() && parax.efl_mm.abs() > 0.1 {
                        self.target_efl_value = parax.efl_mm;
                    }
                }
            }
        }
    }
}

/// Constructs the `VariableSet` from the active flags in `LensEditorState`.
#[must_use]
pub fn build_variable_set(
    editor_state: &LensEditorState,
    prescription: &OpticalPrescription,
) -> VariableSet {
    let mut vars = VariableSet::new();
    let n_surfaces = prescription.surfaces.len();

    for &idx in &editor_state.variable_radii {
        if idx < n_surfaces {
            vars.add(VariableType::Curvature { surface_index: idx });
        }
    }

    for &idx in &editor_state.variable_thicknesses {
        if idx < n_surfaces {
            vars.add(VariableType::Thickness { surface_index: idx });
        }
    }

    vars
}

/// Constructs the `MeritFunction` based on active user constraints.
#[must_use]
pub fn build_merit_function(
    prescription: &OpticalPrescription,
    opt_state: &OptimizationModalState,
) -> MeritFunction {
    let mut mf = MeritFunction::new();
    let prim_wl = prescription
        .wavelengths_nm
        .get(prescription.primary_wavelength_index)
        .copied()
        .unwrap_or(587.56);

    // 1. RMS Spot Radius operands across fields
    if opt_state.min_rms_spot {
        let fields = if prescription.field_angles_deg.is_empty() {
            vec![0.0]
        } else {
            prescription.field_angles_deg.clone()
        };

        for (i, &f_angle) in fields.iter().enumerate() {
            let op_name = format!("RMS_FIELD_{}", i);
            mf.add_operand(MeritOperand::new(
                op_name,
                OperandType::RmsSpotRadius {
                    field_angle_deg: f_angle,
                    wavelength_nm: prim_wl,
                    hex_rings: 3,
                },
                0.0, // target = zero aberration
                opt_state.min_rms_weight,
            ));
        }
    }

    // 2. Target Effective Focal Length
    if opt_state.target_efl_enabled {
        mf.add_operand(MeritOperand::new(
            "TARGET_EFL",
            OperandType::EffectiveFocalLength {
                wavelength_nm: prim_wl,
            },
            opt_state.target_efl_value,
            opt_state.target_efl_weight,
        ));
    }

    mf
}

/// Runs a DLS optimization cycle on the prescription.
///
/// Returns `Ok(DlsRunSummary)` on success, updating `prescription` in-place.
pub fn execute_dls_cycle(
    cycles: usize,
    prescription: &mut OpticalPrescription,
    editor_state: &LensEditorState,
    opt_state: &OptimizationModalState,
    catalog: &GlassCatalogDb,
) -> Result<DlsRunSummary, String> {
    let vars = build_variable_set(editor_state, prescription);
    if vars.num_active() == 0 {
        return Err("No active variables. Toggle [V] on Radii or Thicknesses in the Lens Data Editor first.".to_string());
    }

    let mf = build_merit_function(prescription, opt_state);
    if mf.len() == 0 {
        return Err("Merit function is empty. Enable at least one target operand (RMS Spot or Target EFL).".to_string());
    }

    let config = DlsConfig {
        max_iterations: cycles,
        initial_damping: opt_state.initial_damping,
        relative_merit_tolerance: 1e-6,
        gradient_tolerance: 1e-7,
        finite_difference_relative_step: 1e-5,
    };

    let optimizer = DlsOptimizer::new(config);
    match optimizer.optimize(prescription, &vars, &mf, catalog, None) {
        Ok(result) => {
            *prescription = result.optimized_prescription;
            let improvement = if result.initial_merit > 1e-12 {
                ((result.initial_merit - result.final_merit) / result.initial_merit) * 100.0
            } else {
                0.0
            };

            Ok(DlsRunSummary {
                initial_merit: result.initial_merit,
                final_merit: result.final_merit,
                iterations_run: result.iterations,
                converged: result.converged,
                percent_improvement: improvement,
            })
        }
        Err(err) => Err(format!("DLS Optimization error: {}", err)),
    }
}

/// Renders a crisp, technical square checkbox with an exact 1px academic border and label.
///
/// Distinct from rounded consumer checkboxes:
/// - Unchecked: clear 14x14px square with visible academic border and background.
/// - Checked: filled with accent color and crisp checkmark vector lines.
/// - Fully clickable across both the square box and the label text.
pub fn academic_square_checkbox_with_label(
    ui: &mut Ui,
    checked: &mut bool,
    label: &str,
    tokens: &OpticsTokens,
) -> egui::Response {
    let size = Vec2::splat(14.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(7.0, 0.0);
        let (rect, mut box_resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let label_resp = ui.add(
            egui::Label::new(
                RichText::new(label)
                    .size(11.0)
                    .strong()
                    .color(if *checked { tokens.text_primary } else { tokens.text_muted }),
            )
            .sense(egui::Sense::click()),
        );

        let clicked = box_resp.clicked() || label_resp.clicked();
        let hovered = box_resp.hovered() || label_resp.hovered();

        if clicked {
            *checked = !*checked;
            box_resp.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let border_color = if hovered {
                tokens.text_primary
            } else if *checked {
                tokens.accent
            } else {
                tokens.border_hovered.color
            };

            let bg_color = if *checked {
                tokens.accent
            } else if hovered {
                tokens.bg_control_active
            } else {
                tokens.bg_canvas
            };

            painter.rect_filled(rect, CornerRadius::ZERO, bg_color);
            painter.rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(1.0, border_color),
                egui::StrokeKind::Inside,
            );

            if *checked {
                let p1 = egui::pos2(rect.left() + 2.5, rect.top() + 7.0);
                let p2 = egui::pos2(rect.left() + 5.5, rect.top() + 10.5);
                let p3 = egui::pos2(rect.left() + 11.5, rect.top() + 3.5);

                painter.line_segment([p1, p2], Stroke::new(1.8, Color32::WHITE));
                painter.line_segment([p2, p3], Stroke::new(1.8, Color32::WHITE));
            }
        }

        box_resp.union(label_resp)
    })
    .inner
}

/// Renders the complete academic Optimization Panel docked cleanly inside the Left Panel.
///
/// Returns `true` if the optical prescription was modified by optimization.
pub fn render_optimization_panel(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    editor_state: &LensEditorState,
    opt_state: &mut OptimizationModalState,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);

            // -------------------------------------------------------------------
            // 1. Header: Active Variables Badge & Status (No icon, clean typography)
            // -------------------------------------------------------------------
            let var_count = editor_state.total_variable_count();
            let var_frame = Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::same(10));

            var_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("OPTIMIZATION VARIABLES")
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );

                        let badge_color = if var_count > 0 { tokens.accent } else { tokens.warning };
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{} Active", var_count))
                                    .monospace()
                                    .strong()
                                    .size(10.5)
                                    .color(badge_color),
                            );
                        });
                    });

                    ui.add_space(4.0);

                    if var_count == 0 {
                        ui.label(
                            RichText::new("No variables marked. Switch to the LENS DATA tab and toggle [ V ] on any Radius or Thickness to allow DLS to optimize it.")
                                .size(10.0)
                                .color(tokens.text_muted),
                        );
                    } else {
                        let mut var_tags = Vec::new();
                        for &r_idx in &editor_state.variable_radii {
                            var_tags.push(format!("R{} (Curv)", r_idx));
                        }
                        for &t_idx in &editor_state.variable_thicknesses {
                            var_tags.push(format!("T{} (Thick)", t_idx));
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                            ui.label(RichText::new("Varying:").size(10.5).color(tokens.text_muted));
                            for tag in var_tags {
                                ui.label(
                                    RichText::new(tag)
                                        .monospace()
                                        .strong()
                                        .size(10.0)
                                        .color(tokens.accent),
                                );
                            }
                        });
                    }
                });
            });

            // -------------------------------------------------------------------
            // 2. Merit Function Operands Card (Aligned, Square Checkboxes)
            // -------------------------------------------------------------------
            let operands_frame = Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::same(10));

            operands_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("MERIT FUNCTION TARGETS")
                            .monospace()
                            .strong()
                            .size(10.5)
                            .color(tokens.text_primary),
                    );
                    ui.add_space(8.0);

                    // Operand 1: Minimize RMS Spot Size
                    academic_square_checkbox_with_label(
                        ui,
                        &mut opt_state.min_rms_spot,
                        "Minimize RMS Spot Size",
                        tokens,
                    );
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(21.0);
                        ui.label(RichText::new("Weight:").size(10.5).color(tokens.text_muted));
                        ui.add_enabled(
                            opt_state.min_rms_spot,
                            DragValue::new(&mut opt_state.min_rms_weight)
                                .speed(0.1)
                                .range(0.1..=1000.0),
                        );
                    });

                    ui.add_space(8.0);

                    // Operand 2: Target Effective Focal Length
                    academic_square_checkbox_with_label(
                        ui,
                        &mut opt_state.target_efl_enabled,
                        "Hold Target EFL",
                        tokens,
                    );
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(21.0);
                        ui.label(RichText::new("Target:").size(10.5).color(tokens.text_muted));
                        ui.add_enabled(
                            opt_state.target_efl_enabled,
                            DragValue::new(&mut opt_state.target_efl_value)
                                .speed(0.5)
                                .range(0.5..=5000.0)
                                .suffix(" mm"),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new("Weight:").size(10.5).color(tokens.text_muted));
                        ui.add_enabled(
                            opt_state.target_efl_enabled,
                            DragValue::new(&mut opt_state.target_efl_weight)
                                .speed(1.0)
                                .range(0.1..=5000.0),
                        );
                    });
                });
            });

            // -------------------------------------------------------------------
            // 3. Algorithm Controls (Direct Numeric Inputs, No Slider)
            // -------------------------------------------------------------------
            let algo_frame = Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::same(10));

            algo_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("DLS ALGORITHM CONTROLS")
                            .monospace()
                            .strong()
                            .size(10.5)
                            .color(tokens.text_primary),
                    );
                    ui.add_space(8.0);

                    // Cycle Iterations DragValue (Clean numeric input, no slider)
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Cycle Iterations:").size(11.0).color(tokens.text_muted));
                        ui.add(
                            DragValue::new(&mut opt_state.iterations)
                                .range(1..=100)
                                .speed(1)
                                .suffix(" cycles"),
                        );
                    });

                    ui.add_space(4.0);

                    // Damping factor lambda
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Damping Factor λ:").size(11.0).color(tokens.text_muted));
                        ui.add(
                            DragValue::new(&mut opt_state.initial_damping)
                                .speed(0.05)
                                .range(0.001..=100.0),
                        );
                    });
                });
            });

            // -------------------------------------------------------------------
            // 4. Convergence & Results Card (Lucide Vector Icons, No ASCII/Greek)
            // -------------------------------------------------------------------
            if let Some(ref summary) = opt_state.last_summary {
                let res_frame = Frame::NONE
                    .fill(tokens.bg_control_active)
                    .stroke(tokens.border_hovered)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::same(10));

                res_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Completed {} cycles ({})",
                                summary.iterations_run,
                                if summary.converged { "Converged" } else { "Step limit reached" }
                            ))
                            .strong()
                            .monospace()
                            .size(10.5)
                            .color(tokens.accent),
                        );

                        ui.add_space(4.0);

                        // Merit progression row
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                            ui.label(RichText::new("Merit:").size(10.5).color(tokens.text_muted));
                            let init_str = if summary.initial_merit >= 1e5 {
                                format!("{:.2e}", summary.initial_merit)
                            } else {
                                format!("{:.4}", summary.initial_merit)
                            };
                            let final_str = if summary.final_merit >= 1e5 {
                                format!("{:.2e}", summary.final_merit)
                            } else {
                                format!("{:.4}", summary.final_merit)
                            };
                            ui.label(
                                RichText::new(init_str)
                                    .monospace()
                                    .size(11.0)
                                    .color(tokens.text_primary),
                            );
                            ui.label(
                                RichText::new("->")
                                    .monospace()
                                    .size(11.0)
                                    .color(tokens.text_muted),
                            );
                            ui.label(
                                RichText::new(final_str)
                                    .monospace()
                                    .strong()
                                    .size(11.0)
                                    .color(tokens.text_primary),
                            );
                        });

                        ui.add_space(3.0);

                        // Improvement row
                        let (color, text) = if summary.percent_improvement >= 0.0 {
                            (
                                tokens.accent,
                                format!("Reduction: -{:.1}%", summary.percent_improvement),
                            )
                        } else {
                            (
                                tokens.warning,
                                format!("Increased: +{:.1}%", -summary.percent_improvement),
                            )
                        };
                        ui.label(
                            RichText::new(text)
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(color),
                        );
                    });
                });
            } else if let Some(ref msg) = opt_state.status_message {
                let msg_frame = Frame::NONE
                    .fill(tokens.bg_control)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::same(8));

                msg_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(
                        egui::Label::new(
                            RichText::new(msg).size(10.5).color(tokens.text_primary),
                        )
                        .wrap(),
                    );
                });
            }

            // -------------------------------------------------------------------
            // 5. Action Buttons (Run DLS, Step, Revert) - Crisp & Legible When Disabled
            // -------------------------------------------------------------------
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                let has_targets = opt_state.min_rms_spot || opt_state.target_efl_enabled;
                let can_run = var_count > 0 && has_targets;

                let run_tooltip = if var_count == 0 {
                    "Cannot run: select variables with [V] in LENS DATA tab first"
                } else if !has_targets {
                    "Cannot run: check at least one target operand above (RMS Spot or Target EFL)"
                } else {
                    "Execute full DLS optimization cycles"
                };

                let step_tooltip = if var_count == 0 {
                    "Cannot step: select variables with [V] in LENS DATA tab first"
                } else if !has_targets {
                    "Cannot step: check at least one target operand above (RMS Spot or Target EFL)"
                } else {
                    "Execute a single DLS step for incremental analysis"
                };

                // Run DLS Button (Primary Action)
                let (run_bg, run_fg, run_stroke) = if can_run {
                    (tokens.accent, Color32::WHITE, tokens.border)
                } else {
                    (tokens.bg_control, tokens.text_muted, tokens.border)
                };

                let run_btn = Button::new(
                    RichText::new(format!("Run DLS ({})", opt_state.iterations))
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(run_fg),
                )
                .fill(run_bg)
                .stroke(run_stroke)
                .corner_radius(CornerRadius::ZERO);

                let run_resp = ui.add(run_btn);
                if can_run {
                    if run_resp.on_hover_text(run_tooltip).clicked() {
                        if opt_state.original_prescription.is_none() {
                            opt_state.original_prescription = Some(prescription.clone());
                        }
                        opt_state.history.push((prescription.clone(), opt_state.last_summary.clone()));
                        match execute_dls_cycle(opt_state.iterations, prescription, editor_state, opt_state, catalog) {
                            Ok(summary) => {
                                opt_state.last_summary = Some(summary);
                                opt_state.status_message = None;
                                modified = true;
                            }
                            Err(e) => {
                                opt_state.status_message = Some(e);
                                if let Some((_, prev_sum)) = opt_state.history.pop() {
                                    opt_state.last_summary = prev_sum;
                                }
                            }
                        }
                    }
                } else {
                    run_resp.on_hover_text(run_tooltip);
                }

                // Step 1 Cycle
                let (step_bg, step_fg) = if can_run {
                    (tokens.bg_control, tokens.text_primary)
                } else {
                    (tokens.bg_control, tokens.text_muted)
                };

                let step_btn = Button::new(
                    RichText::new("Step (1)")
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(step_fg),
                )
                .fill(step_bg)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO);

                let step_resp = ui.add(step_btn);
                if can_run {
                    if step_resp.on_hover_text(step_tooltip).clicked() {
                        if opt_state.original_prescription.is_none() {
                            opt_state.original_prescription = Some(prescription.clone());
                        }
                        opt_state.history.push((prescription.clone(), opt_state.last_summary.clone()));
                        match execute_dls_cycle(1, prescription, editor_state, opt_state, catalog) {
                            Ok(summary) => {
                                opt_state.last_summary = Some(summary);
                                opt_state.status_message = None;
                                modified = true;
                            }
                            Err(e) => {
                                opt_state.status_message = Some(e);
                                if let Some((_, prev_sum)) = opt_state.history.pop() {
                                    opt_state.last_summary = prev_sum;
                                }
                            }
                        }
                    }
                } else {
                    step_resp.on_hover_text(step_tooltip);
                }

                // Revert button (supports multi-step history undo)
                let can_revert = !opt_state.history.is_empty();
                let revert_icon_color = if can_revert {
                    tokens.accent
                } else {
                    tokens.text_muted
                };
                let revert_text_color = if can_revert {
                    tokens.text_primary
                } else {
                    tokens.text_muted
                };

                let revert_img = Image::from_bytes(
                    LucideIcon::RotateCcw.uri(),
                    LucideIcon::RotateCcw.svg_content().as_bytes(),
                )
                .tint(revert_icon_color)
                .fit_to_exact_size(Vec2::splat(13.0));

                let revert_label = if opt_state.history.len() > 1 {
                    format!("Revert ({})", opt_state.history.len())
                } else {
                    "Revert".to_string()
                };

                let revert_btn = Button::image_and_text(
                    revert_img,
                    RichText::new(revert_label)
                        .size(11.0)
                        .color(revert_text_color),
                )
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO);

                let revert_resp = ui.add(revert_btn);
                if can_revert {
                    let tooltip = if opt_state.history.len() > 1 {
                        format!("Undo last run step ({} steps saved in history)", opt_state.history.len())
                    } else {
                        "Restore baseline system prior to optimization".to_string()
                    };
                    if revert_resp.on_hover_text(tooltip).clicked() {
                        if let Some((prev_presc, prev_summary)) = opt_state.history.pop() {
                            *prescription = prev_presc;
                            opt_state.last_summary = prev_summary;
                            modified = true;
                            if opt_state.history.is_empty() {
                                opt_state.last_summary = None;
                                opt_state.original_prescription = None;
                                opt_state.status_message =
                                    Some("Reverted to original baseline prescription.".to_string());
                            } else {
                                opt_state.status_message = Some(format!(
                                    "Reverted 1 run step ({} remaining in history).",
                                    opt_state.history.len()
                                ));
                            }
                        }
                    }
                } else {
                    revert_resp.on_hover_text("No optimization changes to revert");
                }

                // If multiple runs exist in history, provide a direct "Reset All" button to jump to baseline in 1 click
                if opt_state.history.len() > 1 {
                    let reset_btn = Button::new(
                        RichText::new("Reset All")
                            .size(11.0)
                            .color(tokens.text_primary),
                    )
                    .fill(tokens.bg_control)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO);

                    if ui
                        .add(reset_btn)
                        .on_hover_text("Discard all runs and restore baseline in 1 click")
                        .clicked()
                    {
                        if let Some((first_presc, _)) = opt_state.history.first().cloned() {
                            *prescription = first_presc;
                            opt_state.history.clear();
                            opt_state.original_prescription = None;
                            opt_state.last_summary = None;
                            opt_state.status_message =
                                Some("Reset all optimization runs back to baseline.".to_string());
                            modified = true;
                        }
                    }
                }
            });
        });

    modified
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_core::OpticalSurface;

    fn make_test_singlet() -> OpticalPrescription {
        OpticalPrescription {
            name: "Test Singlet".to_string(),
            entrance_pupil_diameter_mm: 20.0,
            field_angles_deg: vec![0.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                OpticalSurface {
                    surface_index: 0,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 12.0,
                    material_name: "N-BK7".to_string(),
                    is_stop: true,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: -100.0,
                    conic_constant: 0.0,
                    thickness_mm: 95.0,
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                },
            ],
        }
    }

    #[test]
    fn test_optimization_modal_state_default() {
        let state = OptimizationModalState::default();
        assert!(!state.open);
        assert_eq!(state.iterations, 10);
        assert!(state.min_rms_spot);
        assert!(state.target_efl_enabled);
    }

    #[test]
    fn test_variable_set_construction() {
        let presc = make_test_singlet();
        let mut editor_state = LensEditorState::default();
        editor_state.variable_radii.insert(0);
        editor_state.variable_thicknesses.insert(0);

        let vars = build_variable_set(&editor_state, &presc);
        assert_eq!(vars.num_active(), 2);
    }

    #[test]
    fn test_merit_function_construction() {
        let presc = make_test_singlet();
        let mut opt_state = OptimizationModalState::default();
        opt_state.min_rms_spot = true;
        opt_state.target_efl_enabled = true;

        let mf = build_merit_function(&presc, &opt_state);
        // 1 RMS spot operand for field 0.0 + 1 Target EFL operand
        assert_eq!(mf.len(), 2);
    }

    #[test]
    fn test_execute_dls_cycle_runs_successfully() {
        let catalog = GlassCatalogDb::load_bundled().expect("bundled catalog");
        let mut presc = make_test_singlet();
        let mut editor_state = LensEditorState::default();
        editor_state.variable_radii.insert(0);
        editor_state.variable_radii.insert(1);

        let opt_state = OptimizationModalState {
            min_rms_spot: true,
            min_rms_weight: 1.0,
            target_efl_enabled: true,
            target_efl_value: 100.0,
            target_efl_weight: 50.0,
            initial_damping: 1.0,
            ..Default::default()
        };

        let res = execute_dls_cycle(3, &mut presc, &editor_state, &opt_state, &catalog);
        assert!(res.is_ok(), "DLS cycle should execute successfully: {:?}", res.err());
        let summary = res.unwrap();
        assert_eq!(summary.iterations_run, 3);
        assert!(summary.final_merit.is_finite());
    }

    #[test]
    fn test_optimization_history_stack_and_revert() {
        let catalog = GlassCatalogDb::load_bundled().expect("bundled catalog");
        let mut presc = make_test_singlet();
        let initial_r0 = presc.surfaces[0].radius_mm;
        let mut editor_state = LensEditorState::default();
        editor_state.variable_radii.insert(0);
        editor_state.variable_radii.insert(1);

        let mut opt_state = OptimizationModalState::default();
        assert!(opt_state.history.is_empty());

        // Run 1
        opt_state.history.push((presc.clone(), None));
        let summary1 = execute_dls_cycle(2, &mut presc, &editor_state, &opt_state, &catalog).unwrap();
        opt_state.last_summary = Some(summary1);
        let r0_run1 = presc.surfaces[0].radius_mm;
        assert_eq!(opt_state.history.len(), 1);

        // Run 2
        opt_state.history.push((presc.clone(), opt_state.last_summary.clone()));
        let summary2 = execute_dls_cycle(2, &mut presc, &editor_state, &opt_state, &catalog).unwrap();
        opt_state.last_summary = Some(summary2);
        let r0_run2 = presc.surfaces[0].radius_mm;
        assert_ne!(r0_run2, r0_run1);
        assert_eq!(opt_state.history.len(), 2);

        // Revert 1 step: should restore Run 1 state and Run 1 summary
        let (prev, prev_sum) = opt_state.history.pop().expect("pop run 2");
        presc = prev;
        opt_state.last_summary = prev_sum;
        assert_eq!(presc.surfaces[0].radius_mm, r0_run1);
        assert!(opt_state.last_summary.is_some());
        assert_eq!(opt_state.history.len(), 1);

        // Revert 2nd step: should restore initial state and None summary
        let (prev2, prev_sum2) = opt_state.history.pop().expect("pop run 1");
        presc = prev2;
        opt_state.last_summary = prev_sum2;
        assert_eq!(presc.surfaces[0].radius_mm, initial_r0);
        assert!(opt_state.last_summary.is_none());
        assert!(opt_state.history.is_empty());
    }
}

