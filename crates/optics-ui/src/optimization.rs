//! Academic Optical Optimization dialog and DLS (Levenberg-Marquardt) execution engine.
//!
//! Provides an interactive control hub for the automatic optimization backend (`optics-optimize`):
//! - Direct binding to variable flags tracked in `LensEditorState` (`[V]`).
//! - Academic operand configuration: RMS Spot Radius minimization & Target Focal Length (EFL).
//! - Step-by-step single cycle execution and multi-iteration runs.
//! - Non-destructive experimentation with instantaneous "Revert" functionality.

use egui::{
    Align, Button, Color32, CornerRadius, DragValue, Frame, Layout, Margin, RichText,
    ScrollArea, Slider, Ui, Vec2,
};
use optics_core::OpticalPrescription;
use optics_core::prescription::calculate_paraxial_properties;
use optics_glass::catalog::GlassCatalogDb;
use optics_optimize::{
    DlsConfig, DlsOptimizer, MeritFunction, MeritOperand, OperandType, VariableSet, VariableType,
};

use crate::icons::{LucideIcon, lucide_text_button, render_lucide_icon};
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
            last_summary: None,
            status_message: None,
        }
    }
}

impl OptimizationModalState {
    /// Opens the dialog and captures the baseline prescription for potential revert.
    pub fn open_with_snapshot(&mut self, prescription: &OpticalPrescription, catalog: &GlassCatalogDb) {
        self.open = true;
        self.init_if_needed(prescription, catalog);
    }

    /// Initializes default baseline snapshot and target EFL from prescription if not set.
    pub fn init_if_needed(&mut self, prescription: &OpticalPrescription, catalog: &GlassCatalogDb) {
        if self.original_prescription.is_none() {
            self.original_prescription = Some(prescription.clone());
        }
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
            // 1. Header: Active Variables Badge & Status
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
                        render_lucide_icon(ui, LucideIcon::SlidersHorizontal, 13.0, tokens.accent);
                        ui.label(
                            RichText::new("OPTIMIZATION VARIABLES")
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let badge_color = if var_count > 0 { tokens.accent } else { tokens.warning };
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
            // 2. Merit Function Operands Card
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
                    ui.add_space(6.0);

                    // Operand 1: Minimize RMS Spot Size
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.checkbox(&mut opt_state.min_rms_spot, "");
                        ui.label(
                            RichText::new("Minimize RMS Spot Size")
                                .size(11.0)
                                .color(tokens.text_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut opt_state.min_rms_weight)
                                    .speed(0.1)
                                    .range(0.1..=1000.0)
                                    .prefix("Weight: "),
                            );
                        });
                    });

                    ui.add_space(4.0);

                    // Operand 2: Target Effective Focal Length
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.checkbox(&mut opt_state.target_efl_enabled, "");
                        ui.label(
                            RichText::new("Hold Target EFL:")
                                .size(11.0)
                                .color(tokens.text_primary),
                        );
                        ui.add(
                            DragValue::new(&mut opt_state.target_efl_value)
                                .speed(0.5)
                                .range(0.5..=5000.0)
                                .suffix(" mm"),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut opt_state.target_efl_weight)
                                    .speed(1.0)
                                    .range(0.1..=5000.0)
                                    .prefix("Weight: "),
                            );
                        });
                    });
                });
            });

            // -------------------------------------------------------------------
            // 3. Algorithm Controls (Iterations & Damping)
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
                    ui.add_space(6.0);

                    // Iterations Slider
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Cycle Iterations:").size(11.0).color(tokens.text_muted));
                        ui.add(Slider::new(&mut opt_state.iterations, 1..=50).text("cycles"));
                    });

                    ui.add_space(3.0);

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
            // 4. Convergence & Results Card
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
                        ui.horizontal(|ui| {
                            render_lucide_icon(ui, LucideIcon::Check, 13.0, tokens.accent);
                            ui.label(
                                RichText::new(format!(
                                    "Completed {} cycles | {}",
                                    summary.iterations_run,
                                    if summary.converged { "Converged" } else { "Step limit reached" }
                                ))
                                .strong()
                                .monospace()
                                .size(10.5)
                                .color(tokens.accent),
                            );
                        });

                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Merit: {:.4} → {:.4}",
                                    summary.initial_merit, summary.final_merit
                                ))
                                .monospace()
                                .size(11.0)
                                .color(tokens.text_primary),
                            );

                            let imp_color = if summary.percent_improvement > 0.0 {
                                tokens.accent
                            } else {
                                tokens.warning
                            };

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("Δ {:.1}%", summary.percent_improvement))
                                        .monospace()
                                        .strong()
                                        .size(11.0)
                                        .color(imp_color),
                                );
                            });
                        });
                    });
                });
            } else if let Some(ref err) = opt_state.status_message {
                let err_frame = Frame::NONE
                    .fill(tokens.bg_control)
                    .stroke(egui::Stroke::new(1.0, tokens.warning))
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::same(8));

                err_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        render_lucide_icon(ui, LucideIcon::X, 12.0, tokens.warning);
                        ui.label(RichText::new(err).size(10.5).color(tokens.warning));
                    });
                });
            }

            // -------------------------------------------------------------------
            // 5. Action Buttons (Run DLS, Step, Revert)
            // -------------------------------------------------------------------
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                let can_run = var_count > 0;

                // Run DLS Button (Primary Action)
                let run_btn = Button::new(
                    RichText::new(format!("Run DLS ({})", opt_state.iterations))
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(Color32::WHITE),
                )
                .fill(if can_run { tokens.accent } else { tokens.bg_control })
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO);

                if ui.add_enabled(can_run, run_btn).on_hover_text("Execute full DLS optimization cycles").clicked() {
                    if opt_state.original_prescription.is_none() {
                        opt_state.original_prescription = Some(prescription.clone());
                    }
                    match execute_dls_cycle(opt_state.iterations, prescription, editor_state, opt_state, catalog) {
                        Ok(summary) => {
                            opt_state.last_summary = Some(summary);
                            opt_state.status_message = None;
                            modified = true;
                        }
                        Err(e) => {
                            opt_state.status_message = Some(e);
                        }
                    }
                }

                // Step 1 Cycle
                let step_btn = Button::new(
                    RichText::new("Step (1)")
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(if can_run { tokens.text_primary } else { tokens.text_muted }),
                )
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO);

                if ui.add_enabled(can_run, step_btn).on_hover_text("Execute a single DLS step for incremental analysis").clicked() {
                    if opt_state.original_prescription.is_none() {
                        opt_state.original_prescription = Some(prescription.clone());
                    }
                    match execute_dls_cycle(1, prescription, editor_state, opt_state, catalog) {
                        Ok(summary) => {
                            opt_state.last_summary = Some(summary);
                            opt_state.status_message = None;
                            modified = true;
                        }
                        Err(e) => {
                            opt_state.status_message = Some(e);
                        }
                    }
                }

                // Revert button
                let can_revert = opt_state.original_prescription.is_some() && opt_state.last_summary.is_some();
                ui.add_enabled_ui(can_revert, |ui| {
                    if lucide_text_button(ui, LucideIcon::RotateCcw, "Revert", tokens)
                        .on_hover_text("Restore baseline system prior to optimization")
                        .clicked()
                    {
                        if let Some(ref orig) = opt_state.original_prescription {
                            *prescription = orig.clone();
                            opt_state.last_summary = None;
                            opt_state.status_message = Some("Reverted to original prescription.".to_string());
                            modified = true;
                        }
                    }
                });
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
                },
                OpticalSurface {
                    surface_index: 1,
                    radius_mm: -100.0,
                    conic_constant: 0.0,
                    thickness_mm: 95.0,
                    semi_diameter_mm: 12.0,
                    material_name: "AIR".to_string(),
                    is_stop: false,
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
}
