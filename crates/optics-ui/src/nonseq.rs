//! Non-sequential ray tracing, 2D radiometric detector viewer, false-color heatmaps, and stray light analysis UI.

use std::sync::Arc;
use egui::{
    Button, Color32, CornerRadius, Frame, Margin, Pos2, Rect, RichText, ScrollArea, Stroke,
    TextEdit, Ui, Vec2,
};
use optics_core::analysis::{compute_two_bounce_ghosts, GhostAnalysis, GhostReflection};
use optics_core::prescription::OpticalPrescription;
use optics_glass::catalog::GlassCatalogDb;
use optics_nonseq::detector::Detector2D;
pub use optics_nonseq::detector::RayPathFilter;
use optics_nonseq::scene::NonSeqScene;
use optics_nonseq::trace::{NonSeqTraceConfig, NonSeqTraceReport, NonSeqTracer, RayPathSegment};

use crate::icons::{lucide_small_icon_button, LucideIcon};
use crate::theme::OpticsTokens;

/// Color palette mapping for false-color detector heatmaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeatmapPalette {
    /// Perceptually uniform Inferno (Black -> Purple -> Red/Orange -> Yellow).
    #[default]
    Inferno,
    /// Standard scientific Viridis (Purple -> Blue -> Green -> Yellow).
    Viridis,
    /// Smooth high-contrast rainbow Turbo.
    Turbo,
    /// Monochrome grayscale (Black to White).
    Greyscale,
}

impl HeatmapPalette {
    /// Human-readable display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Inferno => "Inferno",
            Self::Viridis => "Viridis",
            Self::Turbo => "Turbo",
            Self::Greyscale => "Greyscale",
        }
    }

    /// Map a normalized scalar $t \in [0, 1]$ to an RGB color.
    pub fn map_color(&self, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Inferno => {
                // High-fidelity polynomial approximation of Inferno
                let r = (255.0 * (1.05 * t - 0.05).clamp(0.0, 1.0)) as u8;
                let g = (255.0 * (0.9 * t * t).clamp(0.0, 1.0)) as u8;
                let b = (255.0 * (0.6 * (1.0 - (2.0 * t - 1.0).abs())).clamp(0.0, 1.0)) as u8;
                Color32::from_rgb(r, g, b)
            }
            Self::Viridis => {
                // High-fidelity polynomial approximation of Viridis
                let r = (255.0 * (-0.7 * t * t + 1.6 * t - 0.1).clamp(0.0, 1.0)) as u8;
                let g = (255.0 * (0.1 + 0.9 * t).clamp(0.0, 1.0)) as u8;
                let b = (255.0 * (0.8 - 0.6 * t).clamp(0.0, 1.0)) as u8;
                Color32::from_rgb(r, g, b)
            }
            Self::Turbo => {
                let r = (255.0 * (3.5 * t - 1.2).clamp(0.0, 1.0)) as u8;
                let g = (255.0 * (1.0 - 4.0 * (t - 0.5).powi(2)).clamp(0.0, 1.0)) as u8;
                let b = (255.0 * (1.8 - 3.5 * t).clamp(0.0, 1.0)) as u8;
                Color32::from_rgb(r, g, b)
            }
            Self::Greyscale => {
                let v = (255.0 * t) as u8;
                Color32::from_rgb(v, v, v)
            }
        }
    }
}

/// Color scaling mode for detector heatmaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScale {
    /// Standard linear scaling from zero to peak.
    #[default]
    Linear,
    /// Logarithmic scaling across 4 orders of magnitude (essential for stray light and ghost analysis).
    Logarithmic,
}

impl ColorScale {
    /// Human-readable display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Logarithmic => "Log (4 Decades)",
        }
    }

    /// Scale a normalized value $v / v_{\max} \in [0, 1]$ according to the active scale.
    pub fn scale_normalized(&self, norm: f64) -> f32 {
        let norm = norm.clamp(0.0, 1.0);
        match self {
            Self::Linear => norm as f32,
            Self::Logarithmic => {
                if norm <= 1e-4 {
                    0.0
                } else {
                    let log_val = (norm.log10() + 4.0) / 4.0;
                    log_val.clamp(0.0, 1.0) as f32
                }
            }
        }
    }
}

/// Radiometric or photometric display quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetectorQuantity {
    /// Spatial Irradiance in $\text{W}/\text{m}^2$.
    #[default]
    Irradiance,
    /// Spatial Illuminance in $\text{lux}$ ($\text{lm}/\text{m}^2$).
    Illuminance,
}

impl DetectorQuantity {
    /// Human-readable display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Irradiance => "Irradiance (W/m²)",
            Self::Illuminance => "Illuminance (lux)",
        }
    }

    /// Unit string abbreviation.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::Irradiance => "W/m²",
            Self::Illuminance => "lux",
        }
    }
}

/// Cross-section slice profile orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossSectionAxis {
    /// Horizontal slice through centroid (X axis).
    #[default]
    Horizontal,
    /// Vertical slice through centroid (Y axis).
    Vertical,
    /// Both horizontal and vertical profiles overlaid.
    Both,
}

impl CrossSectionAxis {
    /// Human-readable display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Horizontal => "Slice X (Horizontal)",
            Self::Vertical => "Slice Y (Vertical)",
            Self::Both => "Both (X & Y)",
        }
    }
}

/// Active view tab within the Non-Sequential & Stray Light bottom drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NonSeqDrawerTab {
    /// 2D Radiometric Detector array, false-color heatmaps, and 1D beam cross-sections.
    #[default]
    DetectorViewer,
    /// Systematic two-bounce ghost reflection matrix, focus shifts, and glare hazards.
    GhostFocusMatrix,
}

/// Standard optical anti-reflection (AR) thin-film coating models.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ArCoatingPreset {
    /// Bare, uncoated optical glass ($R \approx 4.0\%$).
    Uncoated,
    /// Single-layer Magnesium Fluoride ($\text{MgF}_2$, $R \approx 1.3\%$).
    SingleLayerMgF2,
    /// High-efficiency Broadband Multi-Layer AR (BMAR, $R \approx 0.5\%$).
    #[default]
    BmarMultiLayer,
    /// High-power laser Narrowband V-Coating ($R \approx 0.1\%$).
    LaserVCoating,
}

impl ArCoatingPreset {
    /// Surface power reflectance fraction $R \in [0, 1]$.
    pub fn reflectance(&self) -> f64 {
        match self {
            Self::Uncoated => 0.040,
            Self::SingleLayerMgF2 => 0.013,
            Self::BmarMultiLayer => 0.005,
            Self::LaserVCoating => 0.001,
        }
    }

    /// Display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Uncoated => "Uncoated Glass (4.0%)",
            Self::SingleLayerMgF2 => "MgF2 Single Layer (1.3%)",
            Self::BmarMultiLayer => "BMAR Multi-Layer (0.5%)",
            Self::LaserVCoating => "Laser V-Coating (0.1%)",
        }
    }
}

/// Complete interactive state for Non-Sequential Ray Tracing & Detector Viewer.
#[derive(Debug, Clone)]
pub struct NonSeqUiState {
    /// Whether the bottom drawer under the viewport is expanded.
    pub drawer_open: bool,
    /// Active view tab in the drawer.
    pub active_tab: NonSeqDrawerTab,
    /// Number of rays to launch into the non-sequential scene.
    pub num_rays: usize,
    /// Maximum bounce limit per ray.
    pub max_bounces: usize,
    /// Minimum power cutoff threshold in Watts.
    pub min_power_threshold: f64,
    /// Whether Fresnel reflection/refraction ray splitting is enabled.
    pub enable_branching: bool,
    /// Whether to record full ray segment paths for stray light visualization.
    pub record_paths: bool,
    /// Random seed for deterministic reproducible Monte Carlo non-seq ray generation.
    pub ray_seed: u64,
    /// Resolution of the synthesized detector grid (e.g. 50x50).
    pub detector_resolution: usize,
    /// Active false-color heatmap color palette.
    pub palette: HeatmapPalette,
    /// Active intensity scaling (Linear or Logarithmic).
    pub color_scale: ColorScale,
    /// Active radiometric / photometric display quantity.
    pub quantity: DetectorQuantity,
    /// Active ray path filtering criterion for isolating signal, ghosts, and stray light.
    pub ray_filter: RayPathFilter,
    /// Selected AR coating preset for analytical two-bounce ghost evaluation.
    pub ar_coating: ArCoatingPreset,
    /// Cross-section profile axis.
    pub cross_section_axis: CrossSectionAxis,
    /// Toggle overlaying cross-section profile graph.
    pub show_cross_section: bool,
    /// Toggle drawing non-sequential ray trajectories in the 2D optical viewport.
    pub show_ghost_rays_in_viewport: bool,
    /// Cached non-sequential scene synthesized from the active prescription.
    pub cached_scene: Option<NonSeqScene>,
    /// Cached ray tracing and energy conservation report.
    pub last_trace_report: Option<NonSeqTraceReport>,
    /// Cached sequential two-bounce ghost matrix analysis.
    pub ghost_analysis: Option<GhostAnalysis>,
    /// Selected surface pair $(S_i, S_j)$ in the Ghost Matrix for viewport highlighting.
    pub selected_ghost_pair: Option<(usize, usize)>,
    /// Only show high-threat / focused ghosts in table.
    pub filter_focused_ghosts_only: bool,
    /// Status message from the last execution.
    pub status_message: Option<String>,
}

impl Default for NonSeqUiState {
    fn default() -> Self {
        Self {
            drawer_open: false,
            active_tab: NonSeqDrawerTab::DetectorViewer,
            num_rays: 1000,
            max_bounces: 20,
            min_power_threshold: 1e-5,
            enable_branching: true,
            record_paths: true,
            ray_seed: 42,
            detector_resolution: 50,
            palette: HeatmapPalette::Inferno,
            color_scale: ColorScale::Linear,
            quantity: DetectorQuantity::Irradiance,
            ray_filter: RayPathFilter::All,
            ar_coating: ArCoatingPreset::BmarMultiLayer,
            cross_section_axis: CrossSectionAxis::Horizontal,
            show_cross_section: true,
            show_ghost_rays_in_viewport: true,
            cached_scene: None,
            last_trace_report: None,
            ghost_analysis: None,
            selected_ghost_pair: None,
            filter_focused_ghosts_only: false,
            status_message: None,
        }
    }
}

impl NonSeqUiState {
    /// Execute non-sequential ray tracing across the synthesized scene.
    pub fn execute_trace(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        let mut scene = match NonSeqScene::build_from_prescription(
            prescription,
            self.detector_resolution,
        ) {
            Ok(s) => s,
            Err(e) => {
                self.status_message = Some(format!("Failed to build NonSeqScene: {e}"));
                return;
            }
        };

        if scene.sources.is_empty() {
            self.status_message = Some("No optical sources found in scene".to_string());
            return;
        }

        let rays = match scene.sources[0].generate_rays(self.num_rays, self.ray_seed) {
            Ok(r) => r,
            Err(e) => {
                self.status_message = Some(format!("Failed to generate source rays: {e}"));
                return;
            }
        };

        let tracer = NonSeqTracer::new(NonSeqTraceConfig {
            max_bounces: self.max_bounces,
            min_power_threshold: self.min_power_threshold,
            enable_branching: self.enable_branching,
            record_paths: self.record_paths,
        });

        match tracer.trace_rays(&mut scene, &rays, Some(catalog)) {
            Ok(report) => {
                let hit_count = scene.detectors.first().map(|d| d.total_rays_hit).unwrap_or(0);
                let hit_flux = scene.detectors.first().map(|d| d.total_flux_watts).unwrap_or(0.0);
                self.status_message = Some(format!(
                    "Traced {} rays with {} interactions. Detector: {} hits, {:.4} W (Conservation: {:.1}%)",
                    report.total_rays_traced,
                    report.total_interactions,
                    hit_count,
                    hit_flux,
                    report.energy_conservation_ratio() * 100.0
                ));
                self.cached_scene = Some(scene);
                self.last_trace_report = Some(report);
            }
            Err(e) => {
                self.status_message = Some(format!("Non-sequential trace failed: {e}"));
            }
        }
    }

    /// Returns recorded ray path segments for overlaying in the viewport.
    pub fn ghost_ray_paths(&self) -> &[RayPathSegment] {
        if self.show_ghost_rays_in_viewport {
            if let Some(rep) = &self.last_trace_report {
                return &rep.paths;
            }
        }
        &[]
    }

    /// Returns recorded ray path segments filtered by active path criterion.
    pub fn filtered_ghost_ray_paths(&self) -> Vec<RayPathSegment> {
        if !self.show_ghost_rays_in_viewport {
            return Vec::new();
        }
        let Some(rep) = &self.last_trace_report else {
            return Vec::new();
        };

        match self.ray_filter {
            RayPathFilter::All => rep.paths.clone(),
            RayPathFilter::SignalOnly => rep
                .paths
                .iter()
                .filter(|s| s.reflection_count == 0 && s.tir_count == 0)
                .cloned()
                .collect(),
            RayPathFilter::GhostsOnly => rep
                .paths
                .iter()
                .filter(|s| s.reflection_count >= 1)
                .cloned()
                .collect(),
            RayPathFilter::TirOnly => rep
                .paths
                .iter()
                .filter(|s| s.tir_count > 0)
                .cloned()
                .collect(),
            RayPathFilter::StrayLight => rep
                .paths
                .iter()
                .filter(|s| s.reflection_count > 0 || s.tir_count > 0)
                .cloned()
                .collect(),
        }
    }

    /// Evaluates or updates the two-bounce ghost focus matrix for the active prescription.
    pub fn compute_ghost_matrix(
        &mut self,
        prescription: &OpticalPrescription,
        catalog: &GlassCatalogDb,
    ) {
        let primary_wl = prescription
            .wavelengths_nm
            .get(prescription.primary_wavelength_index)
            .copied()
            .unwrap_or(587.56);
        let reflectance = self.ar_coating.reflectance();

        if let Ok(resolved) = prescription.resolve(catalog) {
            match compute_two_bounce_ghosts(&resolved, primary_wl, reflectance) {
                Ok(ga) => {
                    self.ghost_analysis = Some(ga);
                }
                Err(e) => {
                    self.status_message = Some(format!("Ghost analysis failed: {e}"));
                }
            }
        }
    }
}

/// Strict integer input box helper.
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

/// Strict floating-point input box helper.
fn float_input(ui: &mut Ui, id: egui::Id, value: &mut f64, width: f32) -> bool {
    let mut text: String = ui
        .data_mut(|d| d.get_temp(id))
        .unwrap_or_else(|| format!("{:.1e}", value));

    let resp = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(width)
            .clip_text(true)
            .font(egui::TextStyle::Monospace),
    );

    let mut changed = false;
    if resp.changed() {
        if let Ok(parsed) = text.trim().parse::<f64>() {
            if parsed > 0.0 {
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

/// Renders the complete collapsible Non-Sequential & Detector Viewer bottom drawer.
pub fn render_nonseq_bottom_drawer(
    ui: &mut Ui,
    state: &mut NonSeqUiState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) {
    let header_bg = tokens.bg_control;
    let border_stroke = tokens.border;
    let avail_w = ui.available_width();
    let is_compact = avail_w < 820.0;

    ui.set_max_width(avail_w);

    // Header bar (always visible, toggles drawer expansion)
    Frame::NONE
        .fill(header_bg)
        .stroke(border_stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // Right-aligned checkbox placed first to guarantee full visibility
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ghost_label_color = if state.show_ghost_rays_in_viewport {
                        tokens.warning
                    } else {
                        tokens.text_primary
                    };
                    ui.checkbox(
                        &mut state.show_ghost_rays_in_viewport,
                        RichText::new(if is_compact { "Ghost Rays" } else { "Show Ghost Rays" })
                            .monospace()
                            .strong()
                            .size(10.0)
                            .color(ghost_label_color),
                    )
                    .on_hover_text("Toggle non-sequential ghost reflections overlay in 2D CAD & 3D viewports");

                    // Left-to-right contents
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let toggle_icon = if state.drawer_open {
                            LucideIcon::Minus
                        } else {
                            LucideIcon::Plus
                        };
                        if lucide_small_icon_button(ui, toggle_icon, "Toggle Non-Sequential Viewer", tokens).clicked() {
                            state.drawer_open = !state.drawer_open;
                        }

                        let title = if is_compact {
                            "NON-SEQ & GHOSTS"
                        } else {
                            "NON-SEQUENTIAL DETECTOR VIEWER & STRAY LIGHT"
                        };
                        ui.label(
                            RichText::new(title)
                                .monospace()
                                .strong()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );

                        ui.separator();

                        let det_active = state.active_tab == NonSeqDrawerTab::DetectorViewer;
                        let det_label = if is_compact { "2D Detector" } else { "2D Detector Map" };
                        let btn_det = ui.add(
                            Button::new(
                                RichText::new(det_label)
                                    .monospace()
                                    .size(10.0)
                                    .color(if det_active { tokens.text_primary } else { tokens.text_muted }),
                            )
                            .fill(if det_active { tokens.bg_control_active } else { tokens.bg_control })
                            .stroke(if det_active { tokens.border_hovered } else { tokens.border })
                            .corner_radius(CornerRadius::ZERO),
                        );
                        if btn_det.clicked() {
                            state.active_tab = NonSeqDrawerTab::DetectorViewer;
                        }

                        let ghost_active = state.active_tab == NonSeqDrawerTab::GhostFocusMatrix;
                        let ghost_label = if is_compact { "Ghost Matrix" } else { "Ghost Focus Matrix (2-Bounce)" };
                        let btn_ghost = ui.add(
                            Button::new(
                                RichText::new(ghost_label)
                                    .monospace()
                                    .size(10.0)
                                    .color(if ghost_active { tokens.text_primary } else { tokens.text_muted }),
                            )
                            .fill(if ghost_active { tokens.bg_control_active } else { tokens.bg_control })
                            .stroke(if ghost_active { tokens.border_hovered } else { tokens.border })
                            .corner_radius(CornerRadius::ZERO),
                        );
                        if btn_ghost.clicked() {
                            state.active_tab = NonSeqDrawerTab::GhostFocusMatrix;
                            if state.ghost_analysis.is_none() {
                                state.compute_ghost_matrix(prescription, catalog);
                            }
                        }

                        if avail_w > 920.0 {
                            if let Some(msg) = &state.status_message {
                                ui.label(RichText::new("|").size(9.0).color(tokens.border.color));
                                let short_msg = if msg.len() > 36 { &msg[..36] } else { msg.as_str() };
                                ui.label(
                                    RichText::new(short_msg)
                                        .monospace()
                                        .size(9.5)
                                        .color(tokens.text_muted),
                                );
                            }
                        }
                    });
                });
            });
        });

    if !state.drawer_open {
        return;
    }

    ui.add_space(2.0);

    // Main Drawer Body
    Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            match state.active_tab {
                NonSeqDrawerTab::DetectorViewer => {
                    render_detector_viewer_tab(ui, state, prescription, catalog, tokens);
                }
                NonSeqDrawerTab::GhostFocusMatrix => {
                    render_ghost_matrix_table(ui, state, prescription, catalog, tokens);
                }
            }
        });
}

/// Renders the 2D Detector Viewer tab (controls strip, heatmap, cross section, KPIs).
fn render_detector_viewer_tab(
    ui: &mut Ui,
    state: &mut NonSeqUiState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) {
    let avail_w = ui.available_width();
    ui.set_max_width(avail_w);

    // 1. Controls Strip: Two neat, mathematically sized rows (< 560px per row)
    // Row 1: Execution & Source Parameters
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        let icon = LucideIcon::Play;
        let img = egui::Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
            .tint(tokens.accent)
            .fit_to_exact_size(Vec2::splat(12.0));
        let btn = Button::image_and_text(
            img,
            RichText::new("Trace Non-Seq").monospace().size(10.0),
        )
        .min_size(Vec2::new(115.0, 20.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

        if ui.add(btn).clicked() {
            state.execute_trace(prescription, catalog);
        }

        ui.label(RichText::new("Rays:").monospace().size(9.5).color(tokens.text_muted));
        uint_input(ui, egui::Id::new("ns_rays_inp"), &mut state.num_rays, 38.0);

        ui.label(RichText::new("Bounces:").monospace().size(9.5).color(tokens.text_muted));
        uint_input(ui, egui::Id::new("ns_bounces_inp"), &mut state.max_bounces, 24.0);

        ui.label(RichText::new("Cutoff:").monospace().size(9.5).color(tokens.text_muted));
        float_input(ui, egui::Id::new("ns_cutoff_inp"), &mut state.min_power_threshold, 44.0);

        ui.checkbox(
            &mut state.enable_branching,
            RichText::new("Branching").monospace().size(9.5),
        );
    });

    ui.add_space(2.0);

    // Row 2: Filtering & Physical Metric (Radiometric vs Photometric)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Ray Filter Dropdown
        ui.label(RichText::new("Filter:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_ray_filter_cb")
            .selected_text(RichText::new(state.ray_filter.display_name()).monospace().size(9.5))
            .width(110.0)
            .show_ui(ui, |ui| {
                for f in [
                    RayPathFilter::All,
                    RayPathFilter::SignalOnly,
                    RayPathFilter::GhostsOnly,
                    RayPathFilter::TirOnly,
                    RayPathFilter::StrayLight,
                ] {
                    if ui.selectable_label(state.ray_filter == f, f.display_name()).clicked() {
                        state.ray_filter = f;
                    }
                }
            });

        ui.separator();

        // Radiometric vs Photometric
        ui.label(RichText::new("Metric:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_quantity_cb")
            .selected_text(RichText::new(state.quantity.display_name()).monospace().size(9.5))
            .width(105.0)
            .show_ui(ui, |ui| {
                for q in [DetectorQuantity::Irradiance, DetectorQuantity::Illuminance] {
                    if ui.selectable_label(state.quantity == q, q.display_name()).clicked() {
                        state.quantity = q;
                    }
                }
            });
    });

    ui.add_space(2.0);

    // Row 3: Color Palette, Scale, and 1D Profile Slice Axis
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Palette dropdown
        ui.label(RichText::new("Palette:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_palette_cb")
            .selected_text(RichText::new(state.palette.display_name()).monospace().size(9.5))
            .width(72.0)
            .show_ui(ui, |ui| {
                for p in [
                    HeatmapPalette::Inferno,
                    HeatmapPalette::Viridis,
                    HeatmapPalette::Turbo,
                    HeatmapPalette::Greyscale,
                ] {
                    if ui.selectable_label(state.palette == p, p.display_name()).clicked() {
                        state.palette = p;
                    }
                }
            });

        // Scale dropdown (Linear vs Log)
        ui.label(RichText::new("Scale:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_scale_cb")
            .selected_text(RichText::new(state.color_scale.display_name()).monospace().size(9.5))
            .width(75.0)
            .show_ui(ui, |ui| {
                for s in [ColorScale::Linear, ColorScale::Logarithmic] {
                    if ui.selectable_label(state.color_scale == s, s.display_name()).clicked() {
                        state.color_scale = s;
                    }
                }
            });

        // 1D Slice Axis
        ui.label(RichText::new("Slice:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_cs_axis_cb")
            .selected_text(RichText::new(state.cross_section_axis.display_name()).monospace().size(9.5))
            .width(88.0)
            .show_ui(ui, |ui| {
                for axis in [CrossSectionAxis::Horizontal, CrossSectionAxis::Vertical, CrossSectionAxis::Both] {
                    if ui.selectable_label(state.cross_section_axis == axis, axis.display_name()).clicked() {
                        state.cross_section_axis = axis;
                    }
                }
            });
    });

    ui.add_space(4.0);

    // 2. Visualization Area: Mathematically adapts to available width
    let det_opt = state.cached_scene.as_ref().and_then(|s| s.detectors.first().cloned());
    if let Some(detector) = det_opt {
        let avail_w = ui.available_width().max(380.0);

        if avail_w >= 860.0 {
            // WIDE MODE: 3 Columns side-by-side
            let left_w = 200.0;
            let rem_w = avail_w - left_w - 20.0;
            let col_w = (rem_w * 0.5).max(180.0);

            ui.horizontal(|ui| {
                ui.allocate_ui(Vec2::new(left_w, 150.0), |ui| {
                    render_detector_heatmap_widget(ui, &detector, state, tokens);
                });

                ui.separator();

                ui.allocate_ui(Vec2::new(col_w, 150.0), |ui| {
                    ui.label(
                        RichText::new("1D Cross-Section Beam Profile:")
                            .monospace()
                            .strong()
                            .size(10.0)
                            .color(tokens.text_primary),
                    );
                    ui.add_space(2.0);
                    render_cross_section_plot(ui, &detector, state, 120.0, tokens);
                });

                ui.separator();

                ui.allocate_ui(Vec2::new(col_w, 150.0), |ui| {
                    ui.label(
                        RichText::new("Detector Radiometric Metrics:")
                            .monospace()
                            .strong()
                            .size(10.0)
                            .color(tokens.text_primary),
                    );
                    ui.add_space(2.0);
                    render_radiometric_kpis_side(ui, &detector, state, tokens);
                });
            });
        } else {
            // COMPACT MODE: 2 Columns (Left: Heatmap, Right: Stacked Plot & Compact KPIs)
            let left_w = 190.0_f32.min(avail_w * 0.38);
            let right_w = (avail_w - left_w - 14.0).max(180.0);

            ui.horizontal(|ui| {
                ui.allocate_ui(Vec2::new(left_w, 150.0), |ui| {
                    render_detector_heatmap_widget(ui, &detector, state, tokens);
                });

                ui.separator();

                ui.allocate_ui(Vec2::new(right_w, 150.0), |ui| {
                    ui.vertical(|ui| {
                        render_cross_section_plot(ui, &detector, state, 65.0, tokens);
                        ui.add_space(2.0);
                        render_radiometric_kpis_compact(ui, &detector, state, tokens);
                    });
                });
            });
        }
    } else {
        ui.allocate_ui(Vec2::new(avail_w, 140.0), |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Click 'Trace Non-Seq' to trace physical 3D rays and generate detector maps.")
                        .monospace()
                        .size(10.5)
                        .color(tokens.text_muted),
                );
            });
        });
    }
}

/// Renders the systematic Two-Bounce Ghost Focus Matrix table (Zemax Ghost Generator standard).
fn render_ghost_matrix_table(
    ui: &mut Ui,
    state: &mut NonSeqUiState,
    prescription: &OpticalPrescription,
    catalog: &Arc<GlassCatalogDb>,
    tokens: &OpticsTokens,
) {
    let avail_w = ui.available_width().max(400.0);
    ui.set_max_width(avail_w);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new("AR Coating:").monospace().size(9.5).color(tokens.text_muted));
        egui::ComboBox::from_id_salt("ns_ar_coating_cb")
            .selected_text(RichText::new(state.ar_coating.display_name()).monospace().size(9.5))
            .width(135.0)
            .show_ui(ui, |ui| {
                for preset in [
                    ArCoatingPreset::Uncoated,
                    ArCoatingPreset::SingleLayerMgF2,
                    ArCoatingPreset::BmarMultiLayer,
                    ArCoatingPreset::LaserVCoating,
                ] {
                    if ui.selectable_label(state.ar_coating == preset, preset.display_name()).clicked() {
                        state.ar_coating = preset;
                        state.compute_ghost_matrix(prescription, catalog);
                    }
                }
            });

        let recompute_btn = ui.add(
            Button::new(RichText::new("Recompute").monospace().size(9.5))
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO),
        );
        if recompute_btn.clicked() {
            state.compute_ghost_matrix(prescription, catalog);
        }

        ui.checkbox(
            &mut state.filter_focused_ghosts_only,
            RichText::new("Focused Only").monospace().size(9.5),
        );

        let ghost_text_color = if state.show_ghost_rays_in_viewport {
            tokens.warning
        } else {
            tokens.text_primary
        };
        ui.checkbox(
            &mut state.show_ghost_rays_in_viewport,
            RichText::new("Show Ghost Rays")
                .monospace()
                .size(9.5)
                .color(ghost_text_color),
        ).on_hover_text("Toggle display of 2-bounce ghost reflections overlay in 2D and 3D viewports");

        if let Some(ga) = &state.ghost_analysis {
            ui.label(RichText::new("|").size(9.0).color(tokens.border.color));
            ui.label(
                RichText::new(format!(
                    "Pairs: {} | Focused: {} | Stray: {:.2e}",
                    ga.ghosts.len(),
                    ga.num_focused_ghosts,
                    ga.total_ghost_power_fraction
                ))
                .monospace()
                .size(9.5)
                .color(tokens.text_muted),
            );
        }
    });

    ui.add_space(4.0);

    let col_pair = 76.0;
    let col_defocus = 76.0;
    let col_height = 76.0;
    let col_power = 82.0;
    let col_irrad = 86.0;
    let col_threat = 110.0;
    let total_table_w = col_pair + col_defocus + col_height + col_power + col_irrad + col_threat + 16.0;

    ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(155.0)
        .show(ui, |ui| {
            ui.set_min_width(total_table_w);

            // Table Header
            Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(Vec2::new(col_pair, 14.0), |ui| {
                            ui.label(RichText::new("Pair (S↔S)").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                        ui.allocate_ui(Vec2::new(col_defocus, 14.0), |ui| {
                            ui.label(RichText::new("Defocus Δz").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                        ui.allocate_ui(Vec2::new(col_height, 14.0), |ui| {
                            ui.label(RichText::new("Marginal y").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                        ui.allocate_ui(Vec2::new(col_power, 14.0), |ui| {
                            ui.label(RichText::new("Power Frac.").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                        ui.allocate_ui(Vec2::new(col_irrad, 14.0), |ui| {
                            ui.label(RichText::new("Rel. Irrad.").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                        ui.allocate_ui(Vec2::new(col_threat, 14.0), |ui| {
                            ui.label(RichText::new("Threat Badge").monospace().strong().size(9.5).color(tokens.text_primary));
                        });
                    });
                });

            if let Some(ga) = &state.ghost_analysis {
                let filtered_ghosts: Vec<&GhostReflection> = ga
                    .ghosts
                    .iter()
                    .filter(|g| !state.filter_focused_ghosts_only || g.is_focused_ghost)
                    .collect();

                if filtered_ghosts.is_empty() {
                    ui.add_space(16.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("No ghost reflections match current filter criteria.")
                                .monospace()
                                .size(10.5)
                                .color(tokens.text_muted),
                        );
                    });
                    ui.add_space(16.0);
                } else {
                    for (row_idx, g) in filtered_ghosts.iter().enumerate() {
                        let is_selected = state.selected_ghost_pair == Some((g.surface_front, g.surface_rear));
                        let bg = if is_selected {
                            tokens.bg_control_active
                        } else if row_idx % 2 == 0 {
                            tokens.bg_panel
                        } else {
                            tokens.bg_control
                        };

                        Frame::NONE
                            .fill(bg)
                            .stroke(if is_selected { tokens.border_hovered } else { Stroke::NONE })
                            .corner_radius(CornerRadius::ZERO)
                            .inner_margin(Margin::symmetric(6, 3))
                            .show(ui, |ui| {
                                let resp = ui.horizontal(|ui| {
                                    // Pair name
                                    let pair_label = format!("S{} ↔ S{}", g.surface_front + 1, g.surface_rear + 1);
                                    ui.allocate_ui(Vec2::new(col_pair, 16.0), |ui| {
                                        ui.label(RichText::new(pair_label).monospace().size(9.5).color(tokens.text_primary));
                                    });

                                    // Defocus
                                    ui.allocate_ui(Vec2::new(col_defocus, 16.0), |ui| {
                                        let sign = if g.ghost_defocus_mm >= 0.0 { "+" } else { "" };
                                        ui.label(RichText::new(format!("{}{:.2} mm", sign, g.ghost_defocus_mm)).monospace().size(9.5).color(tokens.text_muted));
                                    });

                                    // Marginal Ray y
                                    ui.allocate_ui(Vec2::new(col_height, 16.0), |ui| {
                                        ui.label(RichText::new(format!("{:.3} mm", g.ghost_marginal_height_mm)).monospace().size(9.5).color(tokens.text_muted));
                                    });

                                    // Power Fraction
                                    ui.allocate_ui(Vec2::new(col_power, 16.0), |ui| {
                                        ui.label(RichText::new(format!("{:.2e}%", g.ghost_power_fraction * 100.0)).monospace().size(9.5).color(tokens.text_muted));
                                    });

                                    // Peak Irradiance
                                    ui.allocate_ui(Vec2::new(col_irrad, 16.0), |ui| {
                                        ui.label(RichText::new(format!("{:.2e}×", g.relative_irradiance)).monospace().size(9.5).color(tokens.text_muted));
                                    });

                                    // Threat badge
                                    ui.allocate_ui(Vec2::new(col_threat, 16.0), |ui| {
                                        if g.is_focused_ghost {
                                            ui.label(RichText::new("[FOCUSED DANGER]").monospace().strong().size(9.0).color(Color32::from_rgb(220, 50, 40)));
                                        } else if g.ghost_marginal_height_mm.abs() < 1.5 && g.ghost_power_fraction > 1e-6 {
                                            ui.label(RichText::new("[COMPACT GHOST]").monospace().size(9.0).color(tokens.warning));
                                        } else {
                                            ui.label(RichText::new("[DIFFUSE FLARE]").monospace().size(9.0).color(tokens.text_muted));
                                        }
                                    });
                                });

                                if resp.response.interact(egui::Sense::click()).clicked() {
                                    state.selected_ghost_pair = Some((g.surface_front, g.surface_rear));
                                }
                            });
                    }
                }
            } else {
                ui.add_space(20.0);
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Click 'Recompute' to calculate systematic two-bounce ghost matrix.")
                            .monospace()
                            .size(10.5)
                            .color(tokens.text_muted),
                    );
                });
                ui.add_space(20.0);
            }
        });
}

/// Renders the 2D false-color pixel array and vertical colorbar scale.
fn render_detector_heatmap_widget(
    ui: &mut Ui,
    detector: &Detector2D,
    state: &NonSeqUiState,
    tokens: &OpticsTokens,
) {
    // Find max value in detector for normalization
    let area = detector.pixel_area_m2().max(1e-12);
    let conv_factor = if state.quantity == DetectorQuantity::Illuminance {
        // Approximation for photopic lumens / watts conversion
        683.0 * optics_nonseq::detector::photopic_luminous_efficiency(587.56)
    } else {
        1.0
    };

    let (pixel_flux_data, _total_filtered_flux, _filtered_hits) =
        detector.filtered_pixel_flux(state.ray_filter);

    let max_flux = pixel_flux_data.iter().copied().fold(0.0_f64, f64::max);
    let max_val = (max_flux / area * conv_factor).max(1e-12);

    let res_x = detector.res_x;
    let res_y = detector.res_y;

    ui.horizontal(|ui| {
        let map_size = Vec2::splat(135.0);
        let (rect_map, _) = ui.allocate_exact_size(map_size, egui::Sense::hover());
        {
            let painter = ui.painter();
            painter.rect_filled(rect_map, CornerRadius::ZERO, tokens.bg_control);
            painter.rect_stroke(rect_map, CornerRadius::ZERO, tokens.border, egui::StrokeKind::Inside);

            if max_flux <= 0.0 {
                painter.text(
                    rect_map.center(),
                    egui::Align2::CENTER_CENTER,
                    "No hits matching filter",
                    egui::FontId::monospace(9.0),
                    tokens.text_muted,
                );
            } else if res_x > 0 && res_y > 0 {
                let px_w = rect_map.width() / (res_x as f32);
                let px_h = rect_map.height() / (res_y as f32);

                for y in 0..res_y {
                    for x in 0..res_x {
                        let flux = pixel_flux_data[y * res_x + x];
                        if flux <= 0.0 {
                            continue;
                        }
                        let val = flux / area * conv_factor;
                        let norm = (val / max_val).clamp(0.0, 1.0);
                        let scaled_norm = state.color_scale.scale_normalized(norm);
                        let color = state.palette.map_color(scaled_norm);

                        let min_pos = Pos2::new(
                            rect_map.min.x + (x as f32) * px_w,
                            rect_map.max.y - ((y + 1) as f32) * px_h, // Flip Y so +Y is up
                        );
                        let max_pos = Pos2::new(min_pos.x + px_w, min_pos.y + px_h);
                        let pixel_rect = Rect::from_min_max(min_pos, max_pos);
                        painter.rect_filled(pixel_rect, CornerRadius::ZERO, color);
                    }
                }

                // Draw spatial centroid crosshair
                let (cx_mm, cy_mm) = detector.centroid_mm();
                let half_w = detector.width_mm * 0.5;
                let half_h = detector.height_mm * 0.5;
                if half_w > 0.0 && half_h > 0.0 {
                    let u_c = ((cx_mm + half_w) / detector.width_mm).clamp(0.0, 1.0) as f32;
                    let v_c = ((cy_mm + half_h) / detector.height_mm).clamp(0.0, 1.0) as f32;
                    let cx_px = rect_map.min.x + u_c * rect_map.width();
                    let cy_px = rect_map.max.y - v_c * rect_map.height();

                    painter.line_segment(
                        [Pos2::new(cx_px - 4.0, cy_px), Pos2::new(cx_px + 4.0, cy_px)],
                        Stroke::new(1.0, Color32::WHITE),
                    );
                    painter.line_segment(
                        [Pos2::new(cx_px, cy_px - 4.0), Pos2::new(cx_px, cy_px + 4.0)],
                        Stroke::new(1.0, Color32::WHITE),
                    );
                }
            }
        }

        ui.add_space(4.0);

        // Vertical Colorbar
        let bar_size = Vec2::new(10.0, 135.0);
        let (rect_bar, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
        {
            let painter = ui.painter();
            painter.rect_stroke(rect_bar, CornerRadius::ZERO, tokens.border, egui::StrokeKind::Inside);

            let steps = 32;
            let step_h = rect_bar.height() / (steps as f32);
            for s in 0..steps {
                let t = (steps - 1 - s) as f32 / ((steps - 1) as f32);
                let color = state.palette.map_color(t);
                let y_top = rect_bar.min.y + (s as f32) * step_h;
                let cell = Rect::from_min_size(Pos2::new(rect_bar.min.x, y_top), Vec2::new(bar_size.x, step_h));
                painter.rect_filled(cell, CornerRadius::ZERO, color);
            }
        }

        ui.add_space(2.0);

        // Colorbar Labels
        ui.vertical(|ui| {
            ui.label(
                RichText::new(format!("{:.1e}", max_val))
                    .monospace()
                    .size(8.5)
                    .color(tokens.text_primary),
            );
            ui.add_space(44.0);
            ui.label(
                RichText::new(format!("{:.1e}", max_val * 0.5))
                    .monospace()
                    .size(8.0)
                    .color(tokens.text_muted),
            );
            ui.add_space(44.0);
            ui.label(
                RichText::new("0.0")
                    .monospace()
                    .size(8.5)
                    .color(tokens.text_muted),
            );
        });
    });
}

/// Renders a 1D cross-section slice plot across the centroid / peak of the beam.
fn render_cross_section_plot(
    ui: &mut Ui,
    detector: &Detector2D,
    state: &NonSeqUiState,
    plot_height: f32,
    tokens: &OpticsTokens,
) {
    let plot_size = Vec2::new(ui.available_width() - 4.0, plot_height);
    let (rect_plot, _) = ui.allocate_exact_size(plot_size, egui::Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect_plot, CornerRadius::ZERO, tokens.bg_control);
    painter.rect_stroke(rect_plot, CornerRadius::ZERO, tokens.border, egui::StrokeKind::Inside);

    let area = detector.pixel_area_m2().max(1e-12);
    let conv = if state.quantity == DetectorQuantity::Illuminance {
        683.0 * optics_nonseq::detector::photopic_luminous_efficiency(587.56)
    } else {
        1.0
    };

    let res_x = detector.res_x;
    let res_y = detector.res_y;
    if res_x == 0 || res_y == 0 {
        return;
    }

    let margin = 14.0;
    let inner_rect = Rect::from_min_max(
        Pos2::new(rect_plot.min.x + margin, rect_plot.min.y + 6.0),
        Pos2::new(rect_plot.max.x - 8.0, rect_plot.max.y - 14.0),
    );

    // Draw grid lines
    for i in 1..=2 {
        let y_t = (i as f32) / 3.0;
        let y = inner_rect.max.y - y_t * inner_rect.height();
        painter.line_segment(
            [Pos2::new(inner_rect.min.x, y), Pos2::new(inner_rect.max.x, y)],
            tokens.border,
        );
    }

    // Find row/column corresponding to centroid
    let (cx_mm, cy_mm) = detector.centroid_mm();
    let half_w = detector.width_mm * 0.5;
    let half_h = detector.height_mm * 0.5;
    let u_c = ((cx_mm + half_w) / detector.width_mm).clamp(0.0, 0.9999);
    let v_c = ((cy_mm + half_h) / detector.height_mm).clamp(0.0, 0.9999);
    let centroid_px = (u_c * res_x as f64) as usize;
    let centroid_py = (v_c * res_y as f64) as usize;

    let (pixel_flux_data, _, _) = detector.filtered_pixel_flux(state.ray_filter);
    let max_flux = pixel_flux_data.iter().copied().fold(0.0_f64, f64::max);
    let peak_val = (max_flux / area * conv).max(1e-12);

    // 1. Horizontal profile (Slice along X)
    if state.cross_section_axis == CrossSectionAxis::Horizontal || state.cross_section_axis == CrossSectionAxis::Both {
        let mut pts = Vec::with_capacity(res_x);
        for x in 0..res_x {
            let flux = pixel_flux_data[centroid_py * res_x + x];
            let val = flux / area * conv;
            let norm = (val / peak_val).clamp(0.0, 1.0) as f32;
            let px_x = inner_rect.min.x + (x as f32 / (res_x - 1) as f32) * inner_rect.width();
            let px_y = inner_rect.max.y - norm * inner_rect.height();
            pts.push(Pos2::new(px_x, px_y));
        }
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.4, tokens.accent));
        }
    }

    // 2. Vertical profile (Slice along Y)
    if state.cross_section_axis == CrossSectionAxis::Vertical || state.cross_section_axis == CrossSectionAxis::Both {
        let mut pts = Vec::with_capacity(res_y);
        for y in 0..res_y {
            let flux = pixel_flux_data[y * res_x + centroid_px];
            let val = flux / area * conv;
            let norm = (val / peak_val).clamp(0.0, 1.0) as f32;
            let px_x = inner_rect.min.x + (y as f32 / (res_y - 1) as f32) * inner_rect.width();
            let px_y = inner_rect.max.y - norm * inner_rect.height();
            pts.push(Pos2::new(px_x, px_y));
        }
        let stroke_color = if state.cross_section_axis == CrossSectionAxis::Both {
            tokens.warning
        } else {
            tokens.accent
        };
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.4, stroke_color));
        }
    }

    // Axis coordinate labels
    painter.text(
        Pos2::new(inner_rect.min.x, inner_rect.max.y + 6.0),
        egui::Align2::LEFT_CENTER,
        format!("-{:.1} mm", half_w),
        egui::FontId::monospace(8.0),
        tokens.text_muted,
    );
    painter.text(
        Pos2::new(inner_rect.max.x, inner_rect.max.y + 6.0),
        egui::Align2::RIGHT_CENTER,
        format!("+{:.1} mm", half_w),
        egui::FontId::monospace(8.0),
        tokens.text_muted,
    );
}

/// Renders the radiometric and energy balance metrics in a vertical card (Wide mode).
fn render_radiometric_kpis_side(
    ui: &mut Ui,
    detector: &Detector2D,
    state: &NonSeqUiState,
    tokens: &OpticsTokens,
) {
    let area = detector.pixel_area_m2().max(1e-12);
    let conv = if state.quantity == DetectorQuantity::Illuminance {
        683.0 * optics_nonseq::detector::photopic_luminous_efficiency(587.56)
    } else {
        1.0
    };

    let (pixel_flux_data, total_filtered_flux, filtered_hits) =
        detector.filtered_pixel_flux(state.ray_filter);
    let max_flux = pixel_flux_data.iter().copied().fold(0.0_f64, f64::max);
    let peak_val = max_flux / area * conv;
    let (cx, cy) = detector.centroid_mm();
    let rms_spot = detector.rms_radius_mm();
    let filtered_lumens = optics_nonseq::detector::watts_to_lumens(total_filtered_flux, 587.56);

    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;

            ui.horizontal(|ui| {
                ui.label(RichText::new("Peak Signal:").monospace().size(9.5).color(tokens.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.2e} {}", peak_val, state.quantity.unit()))
                            .monospace()
                            .strong()
                            .size(10.0)
                            .color(tokens.text_primary),
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Filtered Flux:").monospace().size(9.5).color(tokens.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.3e} W ({:.2e} lm)", total_filtered_flux, filtered_lumens))
                            .monospace()
                            .strong()
                            .size(9.5)
                            .color(tokens.accent),
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Hits:").monospace().size(9.5).color(tokens.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} hits", filtered_hits))
                            .monospace()
                            .size(9.5)
                            .color(tokens.text_primary),
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Centroid:").monospace().size(9.5).color(tokens.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("({:.2}, {:.2}) mm", cx, cy))
                            .monospace()
                            .size(9.5)
                            .color(tokens.text_primary),
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("RMS Spot:").monospace().size(9.5).color(tokens.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.3} mm", rms_spot))
                            .monospace()
                            .strong()
                            .size(9.5)
                            .color(tokens.warning),
                    );
                });
            });

            if let Some(rep) = &state.last_trace_report {
                let ratio = rep.energy_conservation_ratio() * 100.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Energy:").monospace().size(9.5).color(tokens.text_muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:.1}%", ratio))
                                .monospace()
                                .size(9.5)
                                .color(if (ratio - 100.0).abs() < 1.0 { tokens.success } else { tokens.warning }),
                        );
                    });
                });
            }
        });
}

/// Renders compact radiometric KPI chips in a 2-column micro-grid (Compact mode).
fn render_radiometric_kpis_compact(
    ui: &mut Ui,
    detector: &Detector2D,
    state: &NonSeqUiState,
    tokens: &OpticsTokens,
) {
    let area = detector.pixel_area_m2().max(1e-12);
    let conv = if state.quantity == DetectorQuantity::Illuminance {
        683.0 * optics_nonseq::detector::photopic_luminous_efficiency(587.56)
    } else {
        1.0
    };

    let (pixel_flux_data, total_filtered_flux, filtered_hits) =
        detector.filtered_pixel_flux(state.ray_filter);
    let max_flux = pixel_flux_data.iter().copied().fold(0.0_f64, f64::max);
    let peak_val = max_flux / area * conv;
    let (cx, cy) = detector.centroid_mm();
    let rms_spot = detector.rms_radius_mm();

    Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(RichText::new(format!("Peak: {:.2e} {}", peak_val, state.quantity.unit())).monospace().strong().size(9.0).color(tokens.text_primary));
                    ui.label(RichText::new(format!("Flux: {:.2e} W", total_filtered_flux)).monospace().size(8.5).color(tokens.accent));
                    ui.label(RichText::new(format!("Hits: {}", filtered_hits)).monospace().size(8.5).color(tokens.text_muted));
                });
                cols[1].vertical(|ui| {
                    ui.label(RichText::new(format!("Centroid: ({:.2}, {:.2}) mm", cx, cy)).monospace().size(9.0).color(tokens.text_primary));
                    ui.label(RichText::new(format!("RMS Spot: {:.3} mm", rms_spot)).monospace().strong().size(8.5).color(tokens.warning));
                    if let Some(rep) = &state.last_trace_report {
                        let ratio = rep.energy_conservation_ratio() * 100.0;
                        ui.label(RichText::new(format!("Energy: {:.1}%", ratio)).monospace().size(8.5).color(if (ratio - 100.0).abs() < 1.0 { tokens.success } else { tokens.warning }));
                    }
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use optics_zemax::presets::cooke_triplet_f4_5;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_nonseq_ui_state_default() {
        let state = NonSeqUiState::default();
        assert!(!state.drawer_open);
        assert_eq!(state.num_rays, 1000);
        assert_eq!(state.max_bounces, 20);
        assert!(state.enable_branching);
        assert!(state.record_paths);
        assert_eq!(state.palette, HeatmapPalette::Inferno);
        assert_eq!(state.color_scale, ColorScale::Linear);
        assert_eq!(state.quantity, DetectorQuantity::Irradiance);
    }

    #[test]
    fn test_heatmap_palette_mapping() {
        for p in [HeatmapPalette::Inferno, HeatmapPalette::Viridis, HeatmapPalette::Turbo, HeatmapPalette::Greyscale] {
            let c0 = p.map_color(0.0);
            let c1 = p.map_color(1.0);
            let c_mid = p.map_color(0.5);
            assert_ne!(c0, c1);
            let _ = c_mid;
        }
    }

    #[test]
    fn test_color_scale_normalization() {
        let lin = ColorScale::Linear;
        assert_eq!(lin.scale_normalized(0.5), 0.5);

        let log = ColorScale::Logarithmic;
        let v_peak = log.scale_normalized(1.0);
        assert!((v_peak - 1.0).abs() < 1e-4);
        let v_low = log.scale_normalized(1e-4);
        assert!(v_low >= 0.0);
    }

    #[test]
    fn test_nonseq_trace_execution() -> TestResult {
        let cooke = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;

        let mut state = NonSeqUiState::default();
        state.num_rays = 60;
        state.detector_resolution = 30;
        state.execute_trace(&cooke, &catalog);

        assert!(state.last_trace_report.is_some());
        assert!(state.cached_scene.is_some());
        assert!(state.status_message.is_some());

        let ghost_paths = state.ghost_ray_paths();
        assert!(!ghost_paths.is_empty(), "Ghost ray paths must be available for viewport overlay");

        Ok(())
    }

    #[test]
    fn test_ghost_matrix_and_filtering() -> TestResult {
        let cooke = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;

        let mut state = NonSeqUiState::default();
        state.compute_ghost_matrix(&cooke, &catalog);

        assert!(state.ghost_analysis.is_some(), "Ghost analysis matrix should be computed");
        if let Some(ga) = &state.ghost_analysis {
            assert!(!ga.ghosts.is_empty(), "Ghost pairs must be populated for Cooke Triplet");
            assert!(ga.total_ghost_power_fraction > 0.0, "Total ghost power fraction must be positive");
        }

        // Verify AR coating presets
        assert!((ArCoatingPreset::Uncoated.reflectance() - 0.040).abs() < 1e-6);
        assert!((ArCoatingPreset::SingleLayerMgF2.reflectance() - 0.013).abs() < 1e-6);
        assert!((ArCoatingPreset::BmarMultiLayer.reflectance() - 0.005).abs() < 1e-6);
        assert!((ArCoatingPreset::LaserVCoating.reflectance() - 0.001).abs() < 1e-6);

        // Run trace and check filtered ghost ray paths
        state.num_rays = 50;
        state.detector_resolution = 20;
        state.execute_trace(&cooke, &catalog);

        // All rays
        state.ray_filter = RayPathFilter::All;
        let all_paths = state.filtered_ghost_ray_paths();
        assert!(!all_paths.is_empty());

        // Ghosts only
        state.ray_filter = RayPathFilter::GhostsOnly;
        let ghost_paths = state.filtered_ghost_ray_paths();
        for p in &ghost_paths {
            assert!(p.reflection_count > 0 || p.interaction == "FresnelReflect");
        }

        // Signal only
        state.ray_filter = RayPathFilter::SignalOnly;
        let signal_paths = state.filtered_ghost_ray_paths();
        for p in &signal_paths {
            assert_eq!(p.reflection_count, 0);
        }

        Ok(())
    }

    #[test]
    fn test_ar_coating_reflectance_attenuation() -> TestResult {
        let cooke = cooke_triplet_f4_5()?;
        let catalog = GlassCatalogDb::load_bundled()?;

        let mut state_uncoated = NonSeqUiState::default();
        state_uncoated.ar_coating = ArCoatingPreset::Uncoated;
        state_uncoated.compute_ghost_matrix(&cooke, &catalog);

        let mut state_bmar = NonSeqUiState::default();
        state_bmar.ar_coating = ArCoatingPreset::BmarMultiLayer;
        state_bmar.compute_ghost_matrix(&cooke, &catalog);

        let p_uncoated = state_uncoated.ghost_analysis.as_ref().unwrap().total_ghost_power_fraction;
        let p_bmar = state_bmar.ghost_analysis.as_ref().unwrap().total_ghost_power_fraction;

        // BMAR (0.5% reflectance) vs Uncoated (4.0% reflectance) drops 2-bounce ghost power by (0.005/0.040)^2 = (1/8)^2 = 1/64 ≈ 64x
        assert!(p_uncoated > 0.0);
        assert!(p_bmar > 0.0);
        let attenuation = p_uncoated / p_bmar;
        assert!(attenuation > 50.0, "BMAR coating must attenuate two-bounce ghost power by > 50x, got {attenuation}x");

        Ok(())
    }

    #[test]
    fn test_detector_quantities_and_cross_section_axes() {
        assert_eq!(CrossSectionAxis::Horizontal.display_name(), "Slice X (Horizontal)");
        assert_eq!(CrossSectionAxis::Vertical.display_name(), "Slice Y (Vertical)");
        assert_eq!(CrossSectionAxis::Both.display_name(), "Both (X & Y)");

        assert_eq!(DetectorQuantity::Irradiance.unit(), "W/m²");
        assert_eq!(DetectorQuantity::Illuminance.unit(), "lux");
        assert_eq!(DetectorQuantity::Irradiance.display_name(), "Irradiance (W/m²)");
        assert_eq!(DetectorQuantity::Illuminance.display_name(), "Illuminance (lux)");
    }
}
