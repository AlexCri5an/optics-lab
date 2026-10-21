//! Optical Tolerancing & Monte Carlo Simulation interactive engineering hub.
//!
//! Provides an academic interactive interface for manufacturing error analysis:
//! - Default tolerance generator by quality grade (Commercial, Precision, High Precision).
//! - Tolerance operands editor supporting physical perturbations: Radius, Thickness, Conic,
//!   Surface Decenter, Surface Tilt, Element Wedge, and Glass Index / Abbe drift.
//! - Statistical distribution selection: Truncated Gaussian (3σ), Uniform, Parabolic, Bimodal, Worst-Case Edge.
//! - Sensitivity Analysis ranking worst offenders with RSS degradation and worst-case totals.
//! - Monte Carlo simulation with percentiles (P10, P50, P90, P99), manufacturing yield calculation,
//!   and interactive production histogram rendering.
//! - Inverse tolerancing budget generator to achieve target production yields.
//! - Worst-case prescription extraction to load directly into the active editor for inspection.

use std::sync::Arc;

use egui::{
    Button, Color32, CornerRadius, Frame, Image, Margin, Pos2, Rect, RichText,
    ScrollArea, Stroke, TextEdit, Ui, Vec2,
};
use optics_core::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;
use optics_tolerance::{
    Compensator, InverseToleranceConfig, InverseToleranceReport, InverseTolerancing,
    MonteCarloConfig, MonteCarloReport, MonteCarloSimulator, SensitivityAnalyzer, SensitivityReport,
    ToleranceCriterion, ToleranceDistribution, ToleranceGrade, ToleranceOperand,
    ToleranceOperandType,
};

use crate::icons::{
    lucide_small_icon_button, lucide_text_button, LucideIcon,
};
use crate::theme::OpticsTokens;

/// Sub-navigation tabs within the Tolerancing dialog hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TolerancingSubtab {
    /// Edit tolerance operands, distributions, and limits.
    #[default]
    Operands,
    /// Sensitivity analysis and worst-offender ranking.
    Sensitivity,
    /// Monte Carlo simulation, statistical percentiles, and production histogram.
    MonteCarlo,
    /// Inverse tolerancing and automated yield budgeting.
    Inverse,
}

/// Criterion type selection for tolerancing evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CriterionChoice {
    /// Geometric RMS spot radius in micrometers.
    #[default]
    RmsSpotRadius,
    /// Wavefront RMS OPD error in waves.
    RmsWavefront,
    /// Paraxial Effective Focal Length in millimeters.
    EffectiveFocalLength,
    /// Paraxial Back Focal Length in millimeters.
    BackFocalLength,
}

impl CriterionChoice {
    /// Display name of the criterion.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::RmsSpotRadius => "RMS Spot Radius (µm)",
            Self::RmsWavefront => "RMS Wavefront Error (waves)",
            Self::EffectiveFocalLength => "Effective Focal Length (EFL, mm)",
            Self::BackFocalLength => "Back Focal Length (BFL, mm)",
        }
    }

    /// Unit string for values.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::RmsSpotRadius => "µm",
            Self::RmsWavefront => "λ",
            Self::EffectiveFocalLength | Self::BackFocalLength => "mm",
        }
    }

    /// Build the core ToleranceCriterion with given parameters.
    pub fn to_criterion(&self, field_angle_deg: f64, wavelength_nm: f64) -> ToleranceCriterion {
        match self {
            Self::RmsSpotRadius => ToleranceCriterion::RmsSpotRadius {
                field_angle_deg,
                wavelength_nm,
            },
            Self::RmsWavefront => ToleranceCriterion::RmsWavefront {
                field_angle_deg,
                wavelength_nm,
            },
            Self::EffectiveFocalLength => ToleranceCriterion::EffectiveFocalLength { wavelength_nm },
            Self::BackFocalLength => ToleranceCriterion::BackFocalLength { wavelength_nm },
        }
    }
}

/// Compensator selection for recovering system performance under perturbations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompensatorChoice {
    /// No compensator applied.
    None,
    /// Adjust back focal distance to maintain paraxial best focus.
    #[default]
    ParaxialRefocus,
    /// Adjust thickness of designated surface.
    VariableThickness,
}

impl CompensatorChoice {
    /// Display name of compensator.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None (Fixed Detector)",
            Self::ParaxialRefocus => "Paraxial Refocus (Adjust BFL)",
            Self::VariableThickness => "Variable Surface Thickness",
        }
    }

    /// Convert to core Compensator type.
    pub fn to_compensator(&self, surface_index: usize, wavelength_nm: f64) -> Compensator {
        match self {
            Self::None => Compensator::None,
            Self::ParaxialRefocus => Compensator::ParaxialRefocus { wavelength_nm },
            Self::VariableThickness => Compensator::VariableThickness { surface_index },
        }
    }
}

/// Action emitted by the tolerancing UI to the host application.
#[derive(Debug, Clone, PartialEq)]
pub enum TolerancingAction {
    /// Active optical prescription was replaced (e.g. worst-case Monte Carlo lens loaded).
    PrescriptionModified(OpticalPrescription),
}

/// State for the Tolerancing & Monte Carlo modal dialog and analysis views.
#[derive(Debug, Clone)]
pub struct TolerancingModalState {
    /// Whether the dialog modal is currently open.
    pub open: bool,
    /// Active subtab in the tolerancing hub.
    pub subtab: TolerancingSubtab,
    /// Standard tolerance quality grade.
    pub grade: ToleranceGrade,
    /// Configured tolerance operands.
    pub operands: Vec<ToleranceOperand>,
    /// Selected evaluation criterion.
    pub criterion_choice: CriterionChoice,
    /// Field angle in degrees for field-dependent criteria.
    pub field_angle_deg: f64,
    /// Evaluation wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Selected optical compensator.
    pub compensator_choice: CompensatorChoice,
    /// Target surface index for variable thickness compensator.
    pub compensator_surface_index: usize,
    /// Number of simulated trials for Monte Carlo.
    pub monte_carlo_trials: usize,
    /// Random seed for deterministic Monte Carlo simulations.
    pub monte_carlo_seed: u64,
    /// Whether a yield threshold limit is enabled.
    pub yield_threshold_enabled: bool,
    /// Performance threshold limit for yield percentage.
    pub yield_threshold_value: f64,
    /// Target production yield for inverse tolerancing (%).
    pub target_yield_percent: f64,
    /// Performance limit threshold for inverse tolerancing budgeting.
    pub inverse_performance_threshold: f64,
    /// Validation trials for inverse tolerancing Monte Carlo confirmation.
    pub inverse_validation_trials: usize,
    /// Cached report of the last sensitivity analysis run.
    pub last_sensitivity_report: Option<SensitivityReport>,
    /// Cached report of the last Monte Carlo simulation run.
    pub last_monte_carlo_report: Option<MonteCarloReport>,
    /// Cached report of the last inverse tolerancing run.
    pub last_inverse_report: Option<InverseToleranceReport>,
    /// Status message for the Operands subtab.
    pub operands_status: Option<String>,
    /// Status message for the Sensitivity subtab.
    pub sensitivity_status: Option<String>,
    /// Status message for the Monte Carlo subtab.
    pub monte_carlo_status: Option<String>,
    /// Status message for the Inverse Budget subtab.
    pub inverse_status: Option<String>,
    /// Global or latest status message.
    pub status_message: Option<String>,
}

impl Default for TolerancingModalState {
    fn default() -> Self {
        Self {
            open: false,
            subtab: TolerancingSubtab::Operands,
            grade: ToleranceGrade::Precision,
            operands: Vec::new(),
            criterion_choice: CriterionChoice::RmsSpotRadius,
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
            compensator_choice: CompensatorChoice::ParaxialRefocus,
            compensator_surface_index: 0,
            monte_carlo_trials: 100,
            monte_carlo_seed: 42,
            yield_threshold_enabled: true,
            yield_threshold_value: 50.0,
            target_yield_percent: 95.0,
            inverse_performance_threshold: 50.0,
            inverse_validation_trials: 100,
            last_sensitivity_report: None,
            last_monte_carlo_report: None,
            last_inverse_report: None,
            operands_status: None,
            sensitivity_status: None,
            monte_carlo_status: None,
            inverse_status: None,
            status_message: None,
        }
    }
}

impl TolerancingModalState {
    /// Returns the status message specifically for the active subtab.
    pub fn current_status_message(&self) -> Option<&str> {
        match self.subtab {
            TolerancingSubtab::Operands => self.operands_status.as_deref().or(self.status_message.as_deref()),
            TolerancingSubtab::Sensitivity => self.sensitivity_status.as_deref(),
            TolerancingSubtab::MonteCarlo => self.monte_carlo_status.as_deref(),
            TolerancingSubtab::Inverse => self.inverse_status.as_deref(),
        }
    }

    /// Initialize default tolerance operands if the list is empty.
    pub fn init_if_needed(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        if self.operands.is_empty() {
            self.operands =
                ToleranceOperand::generate_default_operands(prescription, self.grade);
            self.operands_status = Some(format!(
                "Generated {} default {} tolerance operands",
                self.operands.len(),
                self.grade.display_name()
            ));

            // Set reasonable default threshold based on nominal criterion evaluation
            let criterion = self
                .criterion_choice
                .to_criterion(self.field_angle_deg, self.wavelength_nm);
            if let Ok(nom) = criterion.evaluate(prescription, catalog) {
                // If spot is nominal 12 µm, set threshold to 2x nominal
                let default_thresh = (nom * 2.0).max(5.0);
                self.yield_threshold_value = (default_thresh * 10.0).round() / 10.0;
                self.inverse_performance_threshold = self.yield_threshold_value;
            }
        }
    }

    /// Reset operands to the standard defaults for the selected grade.
    pub fn reset_operands(&mut self, prescription: &OpticalPrescription) {
        self.operands =
            ToleranceOperand::generate_default_operands(prescription, self.grade);
        let msg = format!(
            "Generated {} default {} tolerance operands",
            self.operands.len(),
            self.grade.display_name()
        );
        self.operands_status = Some(msg.clone());
        self.status_message = Some(msg);
    }

    /// Execute sensitivity analysis across configured operands.
    pub fn execute_sensitivity(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        if self.operands.is_empty() {
            self.sensitivity_status = Some("No tolerance operands defined".to_string());
            self.status_message = self.sensitivity_status.clone();
            return;
        }

        let criterion = self
            .criterion_choice
            .to_criterion(self.field_angle_deg, self.wavelength_nm);
        let compensator = self
            .compensator_choice
            .to_compensator(self.compensator_surface_index, self.wavelength_nm);

        match SensitivityAnalyzer::analyze(
            prescription,
            &self.operands,
            &criterion,
            &compensator,
            catalog,
        ) {
            Ok(rep) => {
                let msg = format!(
                    "Sensitivity analysis complete: {} operands evaluated. RSS degradation = {:.3} {}",
                    rep.records.len(),
                    rep.estimated_rss_degradation,
                    self.criterion_choice.unit()
                );
                self.sensitivity_status = Some(msg.clone());
                self.status_message = Some(msg);
                self.last_sensitivity_report = Some(rep);
            }
            Err(e) => {
                let msg = format!("Sensitivity analysis failed: {e}");
                self.sensitivity_status = Some(msg.clone());
                self.status_message = Some(msg);
            }
        }
    }

    /// Execute Monte Carlo simulation across configured operands.
    pub fn execute_monte_carlo(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        if self.operands.is_empty() {
            self.monte_carlo_status = Some("No tolerance operands defined".to_string());
            self.status_message = self.monte_carlo_status.clone();
            return;
        }

        let criterion = self
            .criterion_choice
            .to_criterion(self.field_angle_deg, self.wavelength_nm);
        let compensator = self
            .compensator_choice
            .to_compensator(self.compensator_surface_index, self.wavelength_nm);

        let config = MonteCarloConfig {
            num_trials: self.monte_carlo_trials.max(1),
            seed: self.monte_carlo_seed,
            yield_threshold: if self.yield_threshold_enabled {
                Some(self.yield_threshold_value)
            } else {
                None
            },
        };

        let sim = MonteCarloSimulator::new(config);
        match sim.run(
            prescription,
            &self.operands,
            &criterion,
            &compensator,
            catalog,
        ) {
            Ok(rep) => {
                let yield_str = rep
                    .yield_percent
                    .map(|y| format!("Yield: {y:.1}%"))
                    .unwrap_or_else(|| "Yield: N/A".to_string());
                let msg = format!(
                    "Monte Carlo complete: {} trials. Mean = {:.3} {}, Median P50 = {:.3} {}, {}",
                    rep.total_trials,
                    rep.mean,
                    self.criterion_choice.unit(),
                    rep.percentiles.p50,
                    self.criterion_choice.unit(),
                    yield_str
                );
                self.monte_carlo_status = Some(msg.clone());
                self.status_message = Some(msg);
                self.last_monte_carlo_report = Some(rep);
            }
            Err(e) => {
                let msg = format!("Monte Carlo simulation failed: {e}");
                self.monte_carlo_status = Some(msg.clone());
                self.status_message = Some(msg);
            }
        }
    }

    /// Execute inverse tolerancing budget calculation.
    pub fn execute_inverse(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        if self.operands.is_empty() {
            self.inverse_status = Some("No tolerance operands defined".to_string());
            self.status_message = self.inverse_status.clone();
            return;
        }

        let criterion = self
            .criterion_choice
            .to_criterion(self.field_angle_deg, self.wavelength_nm);
        let compensator = self
            .compensator_choice
            .to_compensator(self.compensator_surface_index, self.wavelength_nm);

        let config = InverseToleranceConfig {
            performance_threshold: self.inverse_performance_threshold,
            target_yield_percent: self.target_yield_percent,
            validation_trials: self.inverse_validation_trials.max(10),
        };

        match InverseTolerancing::budget(
            prescription,
            &self.operands,
            &criterion,
            &compensator,
            catalog,
            &config,
        ) {
            Ok(rep) => {
                let msg = format!(
                    "Inverse budgeting complete: target yield {:.1}%, achieved yield {:.1}%.",
                    rep.target_yield_percent, rep.achieved_yield_percent
                );
                self.inverse_status = Some(msg.clone());
                self.status_message = Some(msg);
                self.last_inverse_report = Some(rep);
            }
            Err(e) => {
                let msg = format!("Inverse tolerancing failed: {e}");
                self.inverse_status = Some(msg.clone());
                self.status_message = Some(msg);
            }
        }
    }

    /// Apply recommendations from the inverse report back to the active operands.
    pub fn apply_budgeted_tolerances(&mut self) {
        let Some(rep) = &self.last_inverse_report else {
            return;
        };

        let mut count = 0;
        for budgeted in &rep.budgeted_operands {
            if let Some(op) = self.operands.iter_mut().find(|o| o.id == budgeted.id) {
                let rec = budgeted.recommended_half_width;
                match &mut op.operand_type {
                    ToleranceOperandType::Radius { delta_radius_mm, .. } => *delta_radius_mm = rec,
                    ToleranceOperandType::Thickness { delta_thickness_mm, .. } => *delta_thickness_mm = rec,
                    ToleranceOperandType::Conic { delta_conic, .. } => *delta_conic = rec,
                    ToleranceOperandType::SurfaceDecenter { max_decenter_mm, .. } => *max_decenter_mm = rec,
                    ToleranceOperandType::SurfaceTilt { max_tilt_deg, .. } => *max_tilt_deg = rec,
                    ToleranceOperandType::ElementWedge { max_wedge_arcmin, .. } => *max_wedge_arcmin = rec,
                    ToleranceOperandType::GlassIndexAbbe { delta_nd, .. } => *delta_nd = rec,
                }
                count += 1;
            }
        }

        let msg = format!("Updated {count} operands with budgeted tolerances.");
        self.inverse_status = Some(msg.clone());
        self.status_message = Some(msg);
    }
}

/// Renders the complete academic Tolerancing and Monte Carlo dialog window.
pub fn render_tolerancing_dialog(
    ctx: &egui::Context,
    state: &mut TolerancingModalState,
    prescription: &mut OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) -> Option<TolerancingAction> {
    if !state.open {
        return None;
    }

    state.init_if_needed(prescription, catalog);

    let mut action = None;
    let mut is_open = state.open;

    egui::Window::new(
        RichText::new("Tolerancing & Monte Carlo Simulation")
            .monospace()
            .strong()
            .size(13.0)
            .color(tokens.text_primary),
    )
    .open(&mut is_open)
    .collapsible(false)
    .resizable(true)
    .default_size(Vec2::new(820.0, 560.0))
    .min_size(Vec2::new(650.0, 420.0))
    .frame(
        Frame::window(&ctx.style())
            .fill(tokens.bg_panel)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO),
    )
    .show(ctx, |ui| {
        action = render_tolerancing_panel_view(ui, state, prescription, catalog, tokens);
    });

    state.open = is_open;
    action
}

/// Renders the complete Tolerancing & Monte Carlo interface directly inside a panel (e.g. Right Diagnostics Panel).
pub fn render_tolerancing_panel_view(
    ui: &mut Ui,
    state: &mut TolerancingModalState,
    prescription: &mut OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) -> Option<TolerancingAction> {
    state.init_if_needed(prescription, catalog);
    let mut action = None;

    // 1. Subtabs bar: full-width proportional layout aligned flush with right margin
    let total_w = ui.available_width();
    let spacing = 3.0;
    let net_w = (total_w - spacing * 3.0).max(100.0);
    let w1 = (net_w * 0.21).floor();
    let w2 = (net_w * 0.25).floor();
    let w3 = (net_w * 0.25).floor();
    let w4 = net_w - w1 - w2 - w3;

    let tabs: [(TolerancingSubtab, &str, LucideIcon, f32); 4] = [
        (TolerancingSubtab::Operands, "Operands", LucideIcon::Layers, w1),
        (TolerancingSubtab::Sensitivity, "Sensitivity", LucideIcon::TrendingUp, w2),
        (TolerancingSubtab::MonteCarlo, "Monte Carlo", LucideIcon::BarChart2, w3),
        (TolerancingSubtab::Inverse, "Inverse Budget", LucideIcon::Target, w4),
    ];

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = spacing;
        for (tab, title, icon, btn_w) in tabs {
            let selected = state.subtab == tab;
            let bg_color = if selected { tokens.accent } else { tokens.bg_control };
            let text_color = if selected { Color32::WHITE } else { tokens.text_primary };

            let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
                .tint(text_color)
                .fit_to_exact_size(Vec2::splat(12.0));

            let btn = Button::image_and_text(
                img,
                RichText::new(title)
                    .size(10.5)
                    .strong()
                    .color(text_color),
            )
            .min_size(Vec2::new(btn_w, 24.0))
            .fill(bg_color)
            .stroke(if selected { Stroke::new(1.0, tokens.accent) } else { tokens.border })
            .corner_radius(CornerRadius::ZERO);

            if ui.add(btn).clicked() {
                state.subtab = tab;
            }
        }
    });

    ui.add_space(4.0);

    // Status banner if present for active subtab
    if let Some(msg) = state.current_status_message() {
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(msg).monospace().size(10.0).color(tokens.text_muted),
                        )
                        .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                });
            });
        ui.add_space(4.0);
    }

    ui.separator();

    // 2. Active Tab Content
    match state.subtab {
        TolerancingSubtab::Operands => {
            render_operands_tab(ui, state, prescription, tokens);
        }
        TolerancingSubtab::Sensitivity => {
            render_sensitivity_tab(ui, state, prescription, catalog, tokens);
        }
        TolerancingSubtab::MonteCarlo => {
            if let Some(act) = render_monte_carlo_tab(ui, state, prescription, catalog, tokens) {
                action = Some(act);
            }
        }
        TolerancingSubtab::Inverse => {
            render_inverse_tab(ui, state, prescription, catalog, tokens);
        }
    }

    action
}

/// Strict numerical text input box (zero dragging, purely typed input with exact clipped width, identical to LDE).
fn float_input(ui: &mut Ui, id: egui::Id, value: &mut f64, width: f32, decimals: usize) -> bool {
    let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| {
        if value.is_infinite() {
            "Infinity".to_string()
        } else {
            format!("{:.prec$}", *value, prec = decimals)
        }
    });

    let resp = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(width)
            .clip_text(true)
            .font(egui::TextStyle::Monospace),
    );

    let mut changed = false;
    if resp.changed() {
        if text.trim().eq_ignore_ascii_case("infinity") {
            *value = f64::INFINITY;
            changed = true;
        } else if let Ok(parsed) = text.trim().parse::<f64>() {
            if parsed >= 0.0 {
                *value = parsed;
                changed = true;
            }
        }
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }

    if resp.lost_focus() {
        ui.data_mut(|d| d.remove_temp::<String>(id));
    }

    changed
}

/// Strict integer text input box (zero dragging, purely typed input with exact clipped width).
fn uint_input<T>(ui: &mut Ui, id: egui::Id, value: &mut T, width: f32) -> bool
where
    T: std::fmt::Display + std::str::FromStr + Copy,
{
    let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| value.to_string());

    let resp = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(width)
            .clip_text(true)
            .font(egui::TextStyle::Monospace),
    );

    let mut changed = false;
    if resp.changed() {
        if let Ok(parsed) = text.trim().parse::<T>() {
            *value = parsed;
            changed = true;
        }
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }

    if resp.lost_focus() {
        ui.data_mut(|d| d.remove_temp::<String>(id));
    }

    changed
}

/// Renders the Tolerance Operands setup subtab.
fn render_operands_tab(
    ui: &mut Ui,
    state: &mut TolerancingModalState,
    prescription: &OpticalPrescription,
    tokens: &OpticsTokens,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Quality Preset:")
                .monospace()
                .size(11.0)
                .color(tokens.text_muted),
        );

        for grade in [
            ToleranceGrade::Commercial,
            ToleranceGrade::Precision,
            ToleranceGrade::HighPrecision,
        ] {
            let selected = state.grade == grade;
            if ui
                .selectable_label(
                    selected,
                    RichText::new(grade.display_name()).monospace().size(10.5),
                )
                .clicked()
            {
                state.grade = grade;
                state.reset_operands(prescription);
            }
        }

        if lucide_text_button(ui, LucideIcon::Plus, "Add Operand", tokens).clicked() {
            let new_idx = state.operands.len() + 1;
            state.operands.push(ToleranceOperand {
                id: format!("T_NEW_{}", new_idx),
                operand_type: ToleranceOperandType::Thickness {
                    surface_index: 0,
                    delta_thickness_mm: 0.05,
                },
                distribution: ToleranceDistribution::Uniform,
                cost_weight: 1.0,
            });
            state.operands_status = Some("Added new thickness tolerance operand".to_string());
            state.status_message = state.operands_status.clone();
        }

        if lucide_text_button(ui, LucideIcon::RotateCcw, "Reset to Defaults", tokens).clicked() {
            state.reset_operands(prescription);
        }

        if lucide_text_button(ui, LucideIcon::Trash2, "Clear All", tokens).clicked() {
            state.operands.clear();
            state.operands_status = Some("Cleared all tolerance operands".to_string());
            state.status_message = state.operands_status.clone();
        }
    });

    ui.add_space(6.0);

    // Operand summary stats
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "Active Tolerances: {} operands | Prescribed Surfaces: {}",
                state.operands.len(),
                prescription.surfaces.len()
            ))
            .monospace()
            .size(10.0)
            .color(tokens.text_muted),
        );
    });

    ui.add_space(4.0);

    let mut to_remove = None;

    // Operands Cards List (Inspector Property-Card Stack - Varianta A)
    // Scroll area placed exclusively on the operands list; top presets and actions remain pinned.
    ScrollArea::vertical()
        .id_salt("tolerance_operands_card_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;

            for (idx, op) in state.operands.iter_mut().enumerate() {
                Frame::NONE
                    .fill(tokens.bg_control)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        // Line 1: Header (ID + Target Descriptor on left, Delete button on right - NO EMOJIS/ICONS)
                        ui.horizontal(|ui| {
                            let target_desc = match &op.operand_type {
                                ToleranceOperandType::Radius { surface_index, .. } => {
                                    format!("Surf {} Radius (TRAD)", surface_index + 1)
                                }
                                ToleranceOperandType::Thickness { surface_index, .. } => {
                                    format!("Surf {} Thickness (TTHI)", surface_index + 1)
                                }
                                ToleranceOperandType::Conic { surface_index, .. } => {
                                    format!("Surf {} Conic (TCON)", surface_index + 1)
                                }
                                ToleranceOperandType::SurfaceDecenter { surface_index, .. } => {
                                    format!("Surf {} Decenter (TSDC)", surface_index + 1)
                                }
                                ToleranceOperandType::SurfaceTilt { surface_index, .. } => {
                                    format!("Surf {} Tilt (TSTI)", surface_index + 1)
                                }
                                ToleranceOperandType::ElementWedge { front_surface, back_surface, .. } => {
                                    format!("El {}-{} Wedge (TWED)", front_surface + 1, back_surface + 1)
                                }
                                ToleranceOperandType::GlassIndexAbbe { surface_index, .. } => {
                                    format!("Surf {} Glass n/V (TGLA)", surface_index + 1)
                                }
                            };

                            ui.label(
                                RichText::new(&op.id)
                                    .monospace()
                                    .strong()
                                    .size(11.0)
                                    .color(tokens.accent),
                            );

                            ui.label(
                                RichText::new(target_desc)
                                    .monospace()
                                    .size(10.5)
                                    .color(tokens.text_primary),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if lucide_small_icon_button(ui, LucideIcon::Trash2, "Remove operand", tokens).clicked() {
                                    to_remove = Some(idx);
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // Line 2: Parameters (Limit ±T, Distribution, Cost Weight)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;

                            // 1. Limit ±T with exact LDE float_input (strict width, clip_text)
                            ui.label(RichText::new("±T:").monospace().size(10.0).color(tokens.text_muted));
                            let id_t = egui::Id::new(("op_tol_limit", idx));
                            let unit = match &mut op.operand_type {
                                ToleranceOperandType::Radius { delta_radius_mm, .. } => {
                                    float_input(ui, id_t, delta_radius_mm, 52.0, 3);
                                    "mm"
                                }
                                ToleranceOperandType::Thickness { delta_thickness_mm, .. } => {
                                    float_input(ui, id_t, delta_thickness_mm, 52.0, 3);
                                    "mm"
                                }
                                ToleranceOperandType::Conic { delta_conic, .. } => {
                                    float_input(ui, id_t, delta_conic, 52.0, 4);
                                    ""
                                }
                                ToleranceOperandType::SurfaceDecenter { max_decenter_mm, .. } => {
                                    float_input(ui, id_t, max_decenter_mm, 52.0, 3);
                                    "mm"
                                }
                                ToleranceOperandType::SurfaceTilt { max_tilt_deg, .. } => {
                                    float_input(ui, id_t, max_tilt_deg, 52.0, 3);
                                    "°"
                                }
                                ToleranceOperandType::ElementWedge { max_wedge_arcmin, .. } => {
                                    float_input(ui, id_t, max_wedge_arcmin, 52.0, 2);
                                    "'"
                                }
                                ToleranceOperandType::GlassIndexAbbe { delta_nd, .. } => {
                                    float_input(ui, id_t, delta_nd, 52.0, 4);
                                    "nd"
                                }
                            };
                            if !unit.is_empty() {
                                ui.label(RichText::new(unit).monospace().size(10.0).color(tokens.text_muted));
                            }

                            ui.add_space(2.0);

                            // 2. Distribution
                            ui.label(RichText::new("Dist:").monospace().size(10.0).color(tokens.text_muted));
                            let dist_name = match op.distribution {
                                ToleranceDistribution::Uniform => "Uniform",
                                ToleranceDistribution::NormalTruncated { .. } => "Gaussian (3σ)",
                                ToleranceDistribution::Parabolic => "Parabolic",
                                ToleranceDistribution::Bimodal => "Bimodal",
                                ToleranceDistribution::WorstCaseEdge => "Worst-Case Edge",
                            };

                            egui::ComboBox::from_id_salt(format!("dist_combo_{idx}"))
                                .selected_text(RichText::new(dist_name).monospace().size(10.0))
                                .width(95.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(matches!(op.distribution, ToleranceDistribution::NormalTruncated { .. }), "Gaussian (3σ)").clicked() {
                                        op.distribution = ToleranceDistribution::NormalTruncated { sigma_cutoff: 3.0 };
                                    }
                                    if ui.selectable_label(matches!(op.distribution, ToleranceDistribution::Uniform), "Uniform").clicked() {
                                        op.distribution = ToleranceDistribution::Uniform;
                                    }
                                    if ui.selectable_label(matches!(op.distribution, ToleranceDistribution::Parabolic), "Parabolic").clicked() {
                                        op.distribution = ToleranceDistribution::Parabolic;
                                    }
                                    if ui.selectable_label(matches!(op.distribution, ToleranceDistribution::Bimodal), "Bimodal").clicked() {
                                        op.distribution = ToleranceDistribution::Bimodal;
                                    }
                                    if ui.selectable_label(matches!(op.distribution, ToleranceDistribution::WorstCaseEdge), "Worst-Case Edge").clicked() {
                                        op.distribution = ToleranceDistribution::WorstCaseEdge;
                                    }
                                });

                            ui.add_space(2.0);

                            // 3. Cost Weight with exact LDE float_input (strict width, clip_text)
                            ui.label(RichText::new("Cost:").monospace().size(10.0).color(tokens.text_muted));
                            let id_cost = egui::Id::new(("op_cost_weight", idx));
                            float_input(ui, id_cost, &mut op.cost_weight, 36.0, 1);
                        });
                    });
            }
        });

    if let Some(i) = to_remove {
        if i < state.operands.len() {
            state.operands.remove(i);
        }
    }
}

/// Renders the Sensitivity Analysis subtab.
fn render_sensitivity_tab(
    ui: &mut Ui,
    state: &mut TolerancingModalState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) {
    // 1. Configuration Controls (Pinned at top, left-aligned with matching compact width)
    let ctrl_w: f32 = 240.0;
    let label_w: f32 = 80.0;
    let combo_w: f32 = (ctrl_w - label_w - 4.0).max(60.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.allocate_ui_with_layout(
            Vec2::new(label_w, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new("Criterion:").monospace().size(10.5).color(tokens.text_muted));
            },
        );
        egui::ComboBox::from_id_salt("sens_criterion")
            .selected_text(RichText::new(state.criterion_choice.display_name()).monospace().size(10.0))
            .width(combo_w)
            .show_ui(ui, |ui| {
                for choice in [
                    CriterionChoice::RmsSpotRadius,
                    CriterionChoice::RmsWavefront,
                    CriterionChoice::EffectiveFocalLength,
                    CriterionChoice::BackFocalLength,
                ] {
                    if ui.selectable_label(state.criterion_choice == choice, choice.display_name()).clicked() {
                        state.criterion_choice = choice;
                    }
                }
            });
    });

    ui.add_space(3.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.allocate_ui_with_layout(
            Vec2::new(label_w, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new("Compensator:").monospace().size(10.5).color(tokens.text_muted));
            },
        );
        egui::ComboBox::from_id_salt("sens_compensator")
            .selected_text(RichText::new(state.compensator_choice.display_name()).monospace().size(10.0))
            .width(combo_w)
            .show_ui(ui, |ui| {
                for comp in [
                    CompensatorChoice::None,
                    CompensatorChoice::ParaxialRefocus,
                    CompensatorChoice::VariableThickness,
                ] {
                    if ui.selectable_label(state.compensator_choice == comp, comp.display_name()).clicked() {
                        state.compensator_choice = comp;
                    }
                }
            });
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        let icon = LucideIcon::TrendingUp;
        let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
            .tint(tokens.accent)
            .fit_to_exact_size(Vec2::splat(13.0));
        let btn = egui::Button::image_and_text(
            img,
            RichText::new("Run Sensitivity Analysis").monospace().size(10.5),
        )
        .min_size(Vec2::new(ctrl_w, 22.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

        if ui.add(btn).clicked() {
            state.execute_sensitivity(prescription, catalog);
        }
    });

    ui.add_space(6.0);

    // 2. Results View
    if let Some(rep) = &state.last_sensitivity_report {
        // Summary KPI Metrics (Single unified full-width card with 3 equal columns)
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(3, |cols| {
                    cols[0].vertical_centered(|ui| {
                        ui.label(RichText::new("NOMINAL").monospace().size(9.0).color(tokens.text_muted));
                        ui.label(
                            RichText::new(format!("{:.3} {}", rep.nominal_value, state.criterion_choice.unit()))
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );
                    });
                    cols[1].vertical_centered(|ui| {
                        ui.label(RichText::new("EST. RSS").monospace().size(9.0).color(tokens.text_muted));
                        ui.label(
                            RichText::new(format!("+{:.3}", rep.estimated_rss_degradation))
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.warning),
                        );
                    });
                    cols[2].vertical_centered(|ui| {
                        ui.label(RichText::new("WORST SUM").monospace().size(9.0).color(tokens.text_muted));
                        ui.label(
                            RichText::new(format!("{:.3}", rep.worst_case_degradation))
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.warning),
                        );
                    });
                });
            });

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Worst Offenders: {} operands | Sorted by RSS Impact",
                    rep.records.len()
                ))
                .monospace()
                .size(10.0)
                .color(tokens.text_muted),
            );
        });

        ui.add_space(4.0);

        // Sensitivity Ranked Property Cards List (Inspector Property-Card Stack)
        // Scroll area placed exclusively on the ranked list; top controls & KPIs remain pinned.
        ScrollArea::vertical()
            .id_salt("tolerance_sensitivity_ranking_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                let max_deg_overall = rep.records.first().map(|r| r.max_degradation).unwrap_or(1.0).max(1e-6);

                for (rank, rec) in rep.records.iter().enumerate() {
                    Frame::NONE
                        .fill(tokens.bg_control)
                        .stroke(tokens.border)
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Row 1: Rank pill + ID + Max Degradation value
                            ui.horizontal(|ui| {
                                let badge_color = if rank == 0 { tokens.warning } else { tokens.text_muted };
                                ui.label(
                                    RichText::new(format!("#{:<2}", rank + 1))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(badge_color),
                                );

                                ui.label(
                                    RichText::new(&rec.id)
                                        .monospace()
                                        .strong()
                                        .size(11.0)
                                        .color(tokens.accent),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{:.4} {}", rec.max_degradation, state.criterion_choice.unit()))
                                            .monospace()
                                            .strong()
                                            .size(10.5)
                                            .color(tokens.warning),
                                    );
                                    ui.label(
                                        RichText::new("Impact:")
                                            .monospace()
                                            .size(9.5)
                                            .color(tokens.text_muted),
                                    );
                                });
                            });

                            ui.add_space(4.0);

                            // Row 2: Visual Impact Bar (fills relative proportion)
                            let bar_ratio = (rec.max_degradation / max_deg_overall).clamp(0.0, 1.0) as f32;
                            let full_w = ui.available_width();
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(full_w, 5.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, CornerRadius::ZERO, tokens.bg_control_hovered);
                            let fill_rect = Rect::from_min_size(rect.min, Vec2::new(full_w * bar_ratio, 5.0));
                            ui.painter().rect_filled(fill_rect, CornerRadius::ZERO, tokens.accent);

                            ui.add_space(4.0);

                            // Row 3: Nominal, +T, -T details
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("Nom: {:.3}", rec.nominal_value))
                                        .monospace()
                                        .size(9.5)
                                        .color(tokens.text_muted),
                                );
                                ui.label(RichText::new("|").size(9.0).color(tokens.border.color));
                                ui.label(
                                    RichText::new(format!("+T: {:+.3}", rec.delta_plus))
                                        .monospace()
                                        .size(9.5)
                                        .color(if rec.delta_plus > 0.0 { tokens.warning } else { tokens.success }),
                                );
                                ui.label(RichText::new("|").size(9.0).color(tokens.border.color));
                                ui.label(
                                    RichText::new(format!("-T: {:+.3}", rec.delta_minus))
                                        .monospace()
                                        .size(9.5)
                                        .color(if rec.delta_minus > 0.0 { tokens.warning } else { tokens.success }),
                                );
                            });
                        });
                }
            });
    } else {
        ui.add_space(20.0);
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Click 'Run Sensitivity Analysis' to compute tolerance ranking")
                    .monospace()
                    .size(11.0)
                    .color(tokens.text_muted),
            );
        });
    }
}

/// Renders the Monte Carlo Simulation and Production Histogram subtab.
fn render_monte_carlo_tab(
    ui: &mut Ui,
    state: &mut TolerancingModalState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) -> Option<TolerancingAction> {
    let mut action = None;

    // 1. Simulation Controls Bar
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Trials:").monospace().size(10.5).color(tokens.text_muted));
        uint_input(ui, egui::Id::new("mc_trials_inp"), &mut state.monte_carlo_trials, 45.0);

        ui.label(RichText::new("Seed:").monospace().size(10.5).color(tokens.text_muted));
        uint_input(ui, egui::Id::new("mc_seed_inp"), &mut state.monte_carlo_seed, 36.0);

        ui.checkbox(
            &mut state.yield_threshold_enabled,
            RichText::new("Spec Limit:").monospace().size(10.5),
        );
        if state.yield_threshold_enabled {
            let unit = state.criterion_choice.unit();
            float_input(ui, egui::Id::new("mc_spec_limit_inp"), &mut state.yield_threshold_value, 46.0, 2);
            ui.label(RichText::new(unit).monospace().size(10.0).color(tokens.text_muted));
        }
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        let icon = LucideIcon::Play;
        let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
            .tint(tokens.accent)
            .fit_to_exact_size(Vec2::splat(13.0));
        let btn = egui::Button::image_and_text(
            img,
            RichText::new("Run Monte Carlo Simulation").monospace().size(10.5),
        )
        .min_size(Vec2::new(240.0, 22.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

        if ui.add(btn).clicked() {
            state.execute_monte_carlo(prescription, catalog);
        }
    });

    ui.add_space(6.0);

    // 2. Results Dashboard & Histogram
    let report_opt = state.last_monte_carlo_report.clone();
    if let Some(rep) = &report_opt {
        ScrollArea::vertical()
            .id_salt("tolerance_monte_carlo_dashboard_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // KPI Metrics Unified Card (2 columns, perfectly aligned left and right)
                Frame::NONE
                    .fill(tokens.bg_control)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.columns(2, |cols| {
                            cols[0].vertical(|ui| {
                                ui.label(RichText::new("NOMINAL VALUE").monospace().size(9.0).color(tokens.text_muted));
                                ui.label(
                                    RichText::new(format!("{:.3} {}", rep.nominal_value, state.criterion_choice.unit()))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(tokens.text_primary),
                                );
                                ui.add_space(4.0);
                                ui.label(RichText::new("MEDIAN P50").monospace().size(9.0).color(tokens.text_muted));
                                ui.label(
                                    RichText::new(format!("{:.3} {}", rep.percentiles.p50, state.criterion_choice.unit()))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(tokens.accent),
                                );
                            });
                            cols[1].vertical(|ui| {
                                ui.label(RichText::new("MEAN ± 1σ").monospace().size(9.0).color(tokens.text_muted));
                                ui.label(
                                    RichText::new(format!("{:.3} ± {:.3}", rep.mean, rep.std_dev))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(tokens.text_primary),
                                );
                                ui.add_space(4.0);
                                ui.label(RichText::new("P90 / P99").monospace().size(9.0).color(tokens.text_muted));
                                ui.label(
                                    RichText::new(format!("{:.2} / {:.2}", rep.percentiles.p90, rep.percentiles.p99))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(tokens.warning),
                                );
                            });
                        });
                    });

                if let Some(y) = rep.yield_percent {
                    ui.add_space(4.0);
                    let y_color = if y >= 95.0 {
                        tokens.success
                    } else if y >= 80.0 {
                        tokens.warning
                    } else {
                        Color32::from_rgb(220, 50, 50)
                    };
                    Frame::NONE
                        .fill(tokens.bg_control)
                        .stroke(tokens.border)
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("ESTIMATED PRODUCTION YIELD:").monospace().size(9.5).color(tokens.text_muted));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{:.1}% (Spec: {:.2} {})", y, state.yield_threshold_value, state.criterion_choice.unit()))
                                            .monospace()
                                            .strong()
                                            .size(10.5)
                                            .color(y_color),
                                    );
                                });
                            });
                        });
                }

                ui.add_space(8.0);

                // Production Histogram Chart
                ui.label(
                    RichText::new("Statistical Production Histogram:")
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(tokens.text_primary),
                );

                ui.add_space(2.0);

                // Histogram Legend
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    // Nominal (Cyan)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 3.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(0, 210, 255));
                        ui.label(RichText::new("Nominal").monospace().size(9.0).color(tokens.text_muted));
                    });

                    // Median P50 (Green)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 3.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, tokens.success);
                        ui.label(RichText::new("P50").monospace().size(9.0).color(tokens.text_muted));
                    });

                    // P90 (Amber)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 3.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, tokens.warning);
                        ui.label(RichText::new("P90").monospace().size(9.0).color(tokens.text_muted));
                    });

                    // Spec Limit (Red) if enabled
                    if state.yield_threshold_enabled {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 3.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(220, 50, 50));
                            ui.label(RichText::new("Limit").monospace().size(9.0).color(Color32::from_rgb(220, 50, 50)));
                        });
                    }
                });

                ui.add_space(4.0);
                render_production_histogram(ui, rep, state, tokens);

                ui.add_space(8.0);

                // Action: Load Worst-Case Perturbed Prescription into Editor
                if let Some(worst_presc) = &rep.worst_case_prescription {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Worst observed trial degradation: {:.3} {}",
                                rep.max_value,
                                state.criterion_choice.unit()
                            ))
                            .monospace()
                            .size(10.5)
                            .color(tokens.warning),
                        );

                        let icon = LucideIcon::Layers;
                        let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
                            .tint(tokens.accent)
                            .fit_to_exact_size(Vec2::splat(13.0));
                        let load_btn = egui::Button::image_and_text(
                            img,
                            RichText::new("Load Worst-Case Prescription into Lens Editor")
                                .monospace()
                                .size(10.5)
                                .color(tokens.text_primary),
                        )
                        .fill(tokens.bg_control)
                        .stroke(tokens.border)
                        .corner_radius(CornerRadius::ZERO);

                        if ui.add(load_btn).clicked() {
                            action = Some(TolerancingAction::PrescriptionModified(worst_presc.clone()));
                            state.monte_carlo_status = Some("Loaded worst-case Monte Carlo prescription into editor".to_string());
                            state.status_message = state.monte_carlo_status.clone();
                        }
                    });
                }
            });
    } else {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Click 'Run Monte Carlo Simulation' to evaluate production yield")
                    .monospace()
                    .size(11.0)
                    .color(tokens.text_muted),
            );
        });
    }

    action
}

/// Renders an academic production histogram using egui Painter.
fn render_production_histogram(
    ui: &mut Ui,
    report: &MonteCarloReport,
    state: &TolerancingModalState,
    tokens: &OpticsTokens,
) {
    let canvas_w = ui.available_width().max(260.0);
    let desired_size = Vec2::new(canvas_w, 160.0);
    let (_response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    let rect = _response.rect;

    // Background canvas
    painter.rect_filled(rect, CornerRadius::ZERO, tokens.bg_control);
    painter.rect_stroke(rect, CornerRadius::ZERO, tokens.border, egui::StrokeKind::Inside);

    let margin = 32.0;
    let plot_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + margin, rect.min.y + 16.0),
        Pos2::new(rect.max.x - margin, rect.max.y - 24.0),
    );

    let num_bins = 18;
    let bins = report.compute_histogram(num_bins);
    if bins.is_empty() {
        return;
    }

    let max_count = bins.iter().map(|b| b.2).max().unwrap_or(1).max(1) as f32;
    let min_val = report.min_value;
    let max_val = report.max_value;
    let span = (max_val - min_val).max(1e-6);

    let val_to_x = |v: f64| -> f32 {
        let t = ((v - min_val) / span).clamp(0.0, 1.0) as f32;
        plot_rect.min.x + t * plot_rect.width()
    };

    // Draw horizontal grid lines
    for i in 1..=4 {
        let y_t = (i as f32) / 4.0;
        let y = plot_rect.max.y - y_t * plot_rect.height();
        painter.line_segment(
            [Pos2::new(plot_rect.min.x, y), Pos2::new(plot_rect.max.x, y)],
            tokens.border,
        );
    }

    // Draw histogram bars
    let bin_w_px = (plot_rect.width() / (bins.len() as f32)).max(2.0);
    for (i, (b_start, b_end, count)) in bins.iter().enumerate() {
        if *count == 0 {
            continue;
        }

        let bar_h = (*count as f32 / max_count) * plot_rect.height();
        let x_start = plot_rect.min.x + (i as f32) * bin_w_px;
        let x_end = x_start + bin_w_px - 1.0;
        let y_top = plot_rect.max.y - bar_h;

        let bar_rect = Rect::from_min_max(
            Pos2::new(x_start, y_top),
            Pos2::new(x_end, plot_rect.max.y),
        );

        // Color coding: If bin is beyond threshold limit, color amber/red
        let is_failed = state.yield_threshold_enabled && *b_start > state.yield_threshold_value;
        let bar_color = if is_failed {
            Color32::from_rgb(220, 50, 50)
        } else {
            tokens.accent
        };

        painter.rect_filled(bar_rect, CornerRadius::same(1), bar_color);
        painter.rect_stroke(bar_rect, CornerRadius::same(1), tokens.border, egui::StrokeKind::Inside);

        let _ = b_end;
    }

    // 1. Draw Nominal Line (Cyan)
    let x_nom = val_to_x(report.nominal_value);
    painter.line_segment(
        [Pos2::new(x_nom, plot_rect.min.y), Pos2::new(x_nom, plot_rect.max.y)],
        Stroke::new(1.5, Color32::from_rgb(0, 210, 255)),
    );

    // 2. Draw P50 Median Line (Green)
    let x_p50 = val_to_x(report.percentiles.p50);
    painter.line_segment(
        [Pos2::new(x_p50, plot_rect.min.y), Pos2::new(x_p50, plot_rect.max.y)],
        Stroke::new(1.5, tokens.success),
    );

    // 3. Draw P90 Line (Amber)
    let x_p90 = val_to_x(report.percentiles.p90);
    painter.line_segment(
        [Pos2::new(x_p90, plot_rect.min.y), Pos2::new(x_p90, plot_rect.max.y)],
        Stroke::new(1.5, tokens.warning),
    );

    // 4. Draw Specification Threshold Line (Red)
    if state.yield_threshold_enabled {
        let x_thresh = val_to_x(state.yield_threshold_value);
        painter.line_segment(
            [Pos2::new(x_thresh, plot_rect.min.y), Pos2::new(x_thresh, plot_rect.max.y)],
            Stroke::new(2.0, Color32::from_rgb(220, 50, 50)),
        );
        painter.text(
            Pos2::new(x_thresh, plot_rect.min.y - 4.0),
            egui::Align2::CENTER_BOTTOM,
            "LIMIT",
            egui::FontId::monospace(9.0),
            Color32::from_rgb(220, 50, 50),
        );
    }

    // X-Axis Labels
    painter.text(
        Pos2::new(plot_rect.min.x, plot_rect.max.y + 12.0),
        egui::Align2::LEFT_CENTER,
        format!("{:.2} {}", min_val, state.criterion_choice.unit()),
        egui::FontId::monospace(9.5),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(plot_rect.max.x, plot_rect.max.y + 12.0),
        egui::Align2::RIGHT_CENTER,
        format!("{:.2} {}", max_val, state.criterion_choice.unit()),
        egui::FontId::monospace(9.5),
        tokens.text_muted,
    );
}

/// Renders the Inverse Tolerancing budgeting subtab.
fn render_inverse_tab(
    ui: &mut Ui,
    state: &mut TolerancingModalState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Target Yield:").monospace().size(10.5).color(tokens.text_muted));
        float_input(ui, egui::Id::new("inv_target_yield_inp"), &mut state.target_yield_percent, 46.0, 1);
        ui.label(RichText::new("%").monospace().size(10.0).color(tokens.text_muted));

        ui.label(RichText::new("Spec Limit:").monospace().size(10.5).color(tokens.text_muted));
        let unit = state.criterion_choice.unit();
        float_input(ui, egui::Id::new("inv_spec_limit_inp"), &mut state.inverse_performance_threshold, 52.0, 2);
        ui.label(RichText::new(unit).monospace().size(10.0).color(tokens.text_muted));
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        let icon = LucideIcon::Target;
        let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
            .tint(tokens.accent)
            .fit_to_exact_size(Vec2::splat(13.0));
        let run_btn = Button::image_and_text(
            img,
            RichText::new("Compute Tolerance Budget")
                .monospace()
                .size(10.5)
                .color(tokens.text_primary),
        )
        .min_size(Vec2::new(240.0, 22.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

        if ui.add(run_btn).clicked() {
            state.execute_inverse(prescription, catalog);
        }
    });

    ui.add_space(6.0);

    let mut should_apply_budget = false;

    if let Some(rep) = &state.last_inverse_report {
        // Summary KPI Metrics (Single unified full-width card with 2 equal columns)
        Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(2, |cols| {
                    cols[0].vertical_centered(|ui| {
                        ui.label(RichText::new("TARGET YIELD").monospace().size(9.0).color(tokens.text_muted));
                        ui.label(
                            RichText::new(format!("{:.1}%", rep.target_yield_percent))
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );
                    });
                    cols[1].vertical_centered(|ui| {
                        ui.label(RichText::new("ACHIEVED YIELD").monospace().size(9.0).color(tokens.text_muted));
                        let yield_color = if rep.achieved_yield_percent >= rep.target_yield_percent {
                            tokens.success
                        } else {
                            tokens.warning
                        };
                        ui.label(
                            RichText::new(format!("{:.1}%", rep.achieved_yield_percent))
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(yield_color),
                        );
                    });
                });
            });

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let apply_img = Image::from_bytes(LucideIcon::Check.uri(), LucideIcon::Check.svg_content().as_bytes())
                .tint(tokens.accent)
                .fit_to_exact_size(Vec2::splat(12.0));
            let apply_btn = Button::image_and_text(
                apply_img,
                RichText::new("Apply Budgeted Tolerances to Operands")
                    .monospace()
                    .size(10.5)
                    .color(tokens.text_primary),
            )
            .min_size(Vec2::new(260.0, 22.0))
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO);

            if ui.add(apply_btn).clicked() {
                should_apply_budget = true;
            }
        });

        ui.add_space(8.0);

        ui.label(
            RichText::new("Budget Recommendations (Tolerances scaled for target yield):")
                .monospace()
                .strong()
                .size(10.5)
                .color(tokens.text_primary),
        );

        ui.add_space(4.0);

        // Budgeted Recommendations Cards (Inspector Property-Card Stack)
        // Scroll area placed exclusively on the budgeted list; top controls & KPIs remain pinned.
        ScrollArea::vertical()
            .id_salt("tolerance_inverse_cards_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;

                for item in &rep.budgeted_operands {
                    Frame::NONE
                        .fill(tokens.bg_control)
                        .stroke(tokens.border)
                        .corner_radius(CornerRadius::ZERO)
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Row 1: ID + Recommendation badge
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&item.id)
                                        .monospace()
                                        .strong()
                                        .size(11.0)
                                        .color(tokens.accent),
                                );

                                let (tag, color) = if item.scaling_ratio > 1.05 {
                                    ("RELAX (Save Cost)", tokens.success)
                                } else if item.scaling_ratio < 0.95 {
                                    ("TIGHTEN (Higher Precision)", tokens.warning)
                                } else {
                                    ("MAINTAIN", tokens.text_muted)
                                };

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(tag)
                                            .monospace()
                                            .strong()
                                            .size(10.0)
                                            .color(color),
                                    );
                                });
                            });

                            ui.add_space(4.0);

                            // Row 2: Original vs Recommended + Scaling
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("Orig: ±{:.4}", item.original_half_width))
                                        .monospace()
                                        .size(10.0)
                                        .color(tokens.text_muted),
                                );
                                ui.label(
                                    RichText::new("->")
                                        .monospace()
                                        .size(10.0)
                                        .color(tokens.accent),
                                );
                                ui.label(
                                    RichText::new(format!("Rec: ±{:.4}", item.recommended_half_width))
                                        .monospace()
                                        .strong()
                                        .size(10.5)
                                        .color(tokens.text_primary),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("Ratio: {:.2}x", item.scaling_ratio))
                                            .monospace()
                                            .size(10.0)
                                            .color(tokens.text_muted),
                                    );
                                });
                            });
                        });
                }
            });
    } else {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("Click 'Compute Tolerance Budget' to optimize manufacturing tolerances for yield")
                    .monospace()
                    .size(11.0)
                    .color(tokens.text_muted),
            );
        });
    }

    if should_apply_budget {
        state.apply_budgeted_tolerances();
    }
}





#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_tolerancing_state_defaults() {
        let state = TolerancingModalState::default();
        assert!(!state.open);
        assert_eq!(state.subtab, TolerancingSubtab::Operands);
        assert_eq!(state.grade, ToleranceGrade::Precision);
        assert_eq!(state.monte_carlo_trials, 100);
        assert!(state.yield_threshold_enabled);
    }

    #[test]
    fn test_tolerancing_state_initialization() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let mut state = TolerancingModalState::default();

        state.init_if_needed(&cooke, &catalog);
        assert!(!state.operands.is_empty());
        assert!(state.yield_threshold_value > 0.0);

        Ok(())
    }

    #[test]
    fn test_tolerancing_sensitivity_execution() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let mut state = TolerancingModalState::default();
        state.init_if_needed(&cooke, &catalog);

        state.execute_sensitivity(&cooke, &catalog);
        assert!(state.last_sensitivity_report.is_some());
        let rep = state.last_sensitivity_report.as_ref().ok_or("missing report")?;
        assert!(!rep.records.is_empty());
        assert!(rep.estimated_rss_degradation > 0.0);
        assert!(rep.worst_case_degradation >= rep.nominal_value);

        Ok(())
    }

    #[test]
    fn test_tolerancing_monte_carlo_execution() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let mut state = TolerancingModalState::default();
        state.init_if_needed(&cooke, &catalog);
        state.monte_carlo_trials = 25;

        state.execute_monte_carlo(&cooke, &catalog);
        assert!(state.last_monte_carlo_report.is_some());
        let rep = state.last_monte_carlo_report.as_ref().ok_or("missing report")?;
        assert_eq!(rep.total_trials, 25);
        assert_eq!(rep.trial_values.len(), 25);
        assert!(rep.percentiles.p50 > 0.0);
        assert!(rep.percentiles.p10 <= rep.percentiles.p90);
        assert!(rep.worst_case_prescription.is_some());

        // Test histogram generation
        let bins = rep.compute_histogram(10);
        assert_eq!(bins.len(), 10);

        Ok(())
    }

    #[test]
    fn test_tolerancing_inverse_budgeting_and_application() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;
        let mut state = TolerancingModalState::default();
        state.init_if_needed(&cooke, &catalog);
        state.inverse_validation_trials = 20;

        let orig_w0 = state.operands[0].half_width();
        state.execute_inverse(&cooke, &catalog);
        assert!(state.last_inverse_report.is_some());

        state.apply_budgeted_tolerances();
        let new_w0 = state.operands[0].half_width();
        assert!(new_w0 > 0.0);
        let _ = orig_w0;

        Ok(())
    }

    #[test]
    fn test_tolerancing_grade_reset() -> TestResult {
        let cooke = optics_zemax::presets::cooke_triplet_f4_5()?;
        let mut state = TolerancingModalState::default();
        state.grade = ToleranceGrade::Commercial;
        state.reset_operands(&cooke);
        let w_comm = state.operands[0].half_width();

        state.grade = ToleranceGrade::HighPrecision;
        state.reset_operands(&cooke);
        let w_high = state.operands[0].half_width();

        assert!(w_high < w_comm, "High precision must be tighter than commercial");

        Ok(())
    }
}
