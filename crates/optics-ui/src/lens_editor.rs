//! Academic Lens Data Editor (LDE) component.
//!
//! Provides a dense, high-precision technical surface editor:
//! - Strictly zero ASCII pseudo-icons (uses real Lucide vector icons).
//! - Direct, non-dragging numerical input boxes (pure text edit).
//! - Compact 2-row surface card architecture designed for ~330px width.
//! - Glass catalog dropdown supporting both classic and ISO designations.
//! - Clean, neutral selection with zero permanent hover highlight.

use std::collections::BTreeSet;

use egui::{
    Align, Area, Button, CornerRadius, DragValue, Frame, Id, Image, Layout, Margin, RichText,
    ScrollArea, TextEdit, Ui, Vec2,
};
use optics_core::surface::{
    BiconicSurface, CoordinateBreak, GrinProfile, SurfaceProfile, ToroidalSurface,
};
use optics_core::{OpticalPrescription, OpticalSurface};
use optics_glass::GlassCatalogDb;

use crate::glass_browser::MaterialVendorFilter;
use crate::icons::{LucideIcon, lucide_small_icon_button, lucide_text_button};
use crate::theme::OpticsTokens;

/// Common optical glasses available in standard catalogs (Schott / Ohara).
pub const COMMON_OPTICAL_GLASSES: &[&str] = &[
    "AIR",
    "SK16",
    "N-SK16",
    "F2",
    "N-F2",
    "BK7",
    "N-BK7",
    "SF1",
    "N-SF1",
    "SF11",
    "N-SF11",
    "LAK9",
    "N-LAK9",
    "BAF10",
    "N-BAF10",
    "FUSED_SILICA",
];

/// Tab selection for the left panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftPanelTab {
    /// Surface-by-surface optical prescription editor.
    #[default]
    LensData,
    /// Dedicated 646-glass materials database browser.
    Materials,
    /// DLS automatic optimization engine & merit function.
    Optimizer,
}

/// Persistent UI state for the Lens Data Editor and System Explorer.
#[derive(Debug, Clone)]
pub struct LensEditorState {
    /// Active tab in the left panel.
    pub active_tab: LeftPanelTab,
    /// Currently selected surface index.
    pub selected_surface: Option<usize>,
    /// Target surface index for assigning materials in the Materials tab.
    pub materials_target_surface: usize,
    /// Selected manufacturer filter for the Materials tab.
    pub materials_filter: MaterialVendorFilter,
    /// Whether the System Explorer (Aperture, Fields, Wavelengths) section is expanded.
    pub system_explorer_open: bool,
    /// New field angle input buffer (string for typing, e.g. "20.0").
    pub new_field_buf: String,
    /// New wavelength input buffer (string for typing, e.g. "532.0").
    pub new_wav_buf: String,
    /// Filter query text for glass catalog search.
    pub glass_search_query: String,
    /// Surface indices whose radius of curvature is marked as variable for optimization.
    pub variable_radii: BTreeSet<usize>,
    /// Surface indices whose axial thickness is marked as variable for optimization.
    pub variable_thicknesses: BTreeSet<usize>,
    /// Currently open autocomplete surface index in the Lens Data Editor.
    pub autocomplete_surface: Option<usize>,
}

impl Default for LensEditorState {
    fn default() -> Self {
        Self {
            active_tab: LeftPanelTab::LensData,
            selected_surface: None,
            materials_target_surface: 0,
            materials_filter: MaterialVendorFilter::All,
            system_explorer_open: false,
            new_field_buf: "20.0".to_string(),
            new_wav_buf: "532.0".to_string(),
            glass_search_query: String::new(),
            variable_radii: BTreeSet::new(),
            variable_thicknesses: BTreeSet::new(),
            autocomplete_surface: None,
        }
    }
}

impl LensEditorState {
    /// Count of total active variables (radii + thicknesses).
    #[must_use]
    pub fn total_variable_count(&self) -> usize {
        self.variable_radii.len() + self.variable_thicknesses.len()
    }

    /// Clears all variable flags.
    pub fn clear_all_variables(&mut self) {
        self.variable_radii.clear();
        self.variable_thicknesses.clear();
    }
}

/// Renders the complete academic Lens Data Editor panel.
///
/// Returns `true` if any surface parameter was edited this frame.
pub fn render_lens_editor(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    state: &mut LensEditorState,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        // -------------------------------------------------------------------
        // 1. Technical System Specification Card
        // -------------------------------------------------------------------
        if render_specification_card(ui, prescription, state, tokens) {
            modified = true;
        }

        ui.add_space(6.0);

        // -------------------------------------------------------------------
        // 2. Action Toolbar (Insert Surface, Presets)
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

            // Insert Surface Button with Lucide Plus icon
            if lucide_text_button(ui, LucideIcon::Plus, "Insert Surface", tokens).clicked() {
                let total_surfs = prescription.surfaces.len();
                let insert_at = state.selected_surface.unwrap_or_else(|| {
                    if total_surfs > 1 {
                        total_surfs - 1
                    } else {
                        total_surfs
                    }
                });
                let new_surf = OpticalSurface {
                    surface_index: insert_at,
                    radius_mm: 50.0,
                    conic_constant: 0.0,
                    thickness_mm: 5.0,
                    semi_diameter_mm: 10.0,
                    material_name: "SK16".to_string(),
                    is_stop: false,
                    asphere_coefficients: Vec::new(),
                    coordinate_break: None,
                    grin_profile: None,
                    profile: None,
                };

                if insert_at < prescription.surfaces.len() {
                    prescription.surfaces.insert(insert_at, new_surf);
                } else {
                    prescription.surfaces.push(new_surf);
                }
                reindex_surfaces(prescription);
                // Shift variable indices after insert_at
                state.variable_radii = state
                    .variable_radii
                    .iter()
                    .map(|&i| if i >= insert_at { i + 1 } else { i })
                    .collect();
                state.variable_thicknesses = state
                    .variable_thicknesses
                    .iter()
                    .map(|&i| if i >= insert_at { i + 1 } else { i })
                    .collect();
                state.selected_surface = None;
                modified = true;
            }

            // Surface & Variable counters
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let var_count = state.total_variable_count();
                if var_count > 0 {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    if ui.small_button(RichText::new("Clear Var").size(9.5).color(tokens.text_muted)).on_hover_text("Reset all variable flags back to fixed").clicked() {
                        state.clear_all_variables();
                    }
                    ui.label(
                        RichText::new(format!("{} Var", var_count))
                            .monospace()
                            .strong()
                            .size(10.5)
                            .color(tokens.accent),
                    ).on_hover_text(format!("Active optimization variables: {} radii, {} thicknesses", state.variable_radii.len(), state.variable_thicknesses.len()));
                    ui.separator();
                }

                ui.label(
                    RichText::new(format!("{} Surfaces", prescription.surfaces.len()))
                        .monospace()
                        .size(10.5)
                        .color(tokens.text_muted),
                );
            });
        });

        ui.add_space(4.0);

        // Subtle 1px separation stroke
        ui.painter().line_segment(
            [
                ui.cursor().min,
                egui::Pos2::new(ui.cursor().min.x + ui.available_width(), ui.cursor().min.y),
            ],
            tokens.border,
        );
        ui.add_space(6.0);

        // -------------------------------------------------------------------
        // 3. Compact 2-Row Surface Cards (Zero Horizontal Overflow)
        // -------------------------------------------------------------------
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total_surfaces = prescription.surfaces.len();
                let mut surf_to_remove: Option<usize> = None;
                let mut surf_to_toggle_stop: Option<usize> = None;

                for (idx, surf) in prescription.surfaces.iter_mut().enumerate() {
                    let card_modified = render_surface_card(
                        ui,
                        idx,
                        surf,
                        total_surfaces,
                        state,
                        &mut surf_to_remove,
                        &mut surf_to_toggle_stop,
                        catalog,
                        tokens,
                    );
                    if card_modified {
                        modified = true;
                    }
                    ui.add_space(4.0);
                }

                // Handle surface deletion
                if let Some(del_idx) = surf_to_remove {
                    if prescription.surfaces.len() > 1 && del_idx < prescription.surfaces.len() {
                        prescription.surfaces.remove(del_idx);
                        reindex_surfaces(prescription);
                        state.variable_radii.remove(&del_idx);
                        state.variable_thicknesses.remove(&del_idx);
                        state.variable_radii = state
                            .variable_radii
                            .iter()
                            .map(|&i| if i > del_idx { i - 1 } else { i })
                            .collect();
                        state.variable_thicknesses = state
                            .variable_thicknesses
                            .iter()
                            .map(|&i| if i > del_idx { i - 1 } else { i })
                            .collect();
                        state.selected_surface = None;
                        modified = true;
                    }
                }

                // Handle aperture stop exclusivity
                if let Some(stop_idx) = surf_to_toggle_stop {
                    for (i, s) in prescription.surfaces.iter_mut().enumerate() {
                        s.is_stop = i == stop_idx;
                    }
                    modified = true;
                }
            });
    });

    modified
}

/// Renders a single compact surface card with clean numerical inputs (zero dragging).
fn render_surface_card(
    ui: &mut Ui,
    idx: usize,
    surf: &mut OpticalSurface,
    total_surfaces: usize,
    state: &mut LensEditorState,
    surf_to_remove: &mut Option<usize>,
    surf_to_toggle_stop: &mut Option<usize>,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    // Clean neutral background and 1px border stroke (no blue selection border or hover fill)
    let card_bg = tokens.bg_control;
    let card_stroke = tokens.border;

    let frame = Frame::NONE
        .fill(card_bg)
        .stroke(card_stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(8, 6));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        // -------------------------------------------------------------------
        // Row 1: Identification, Stop designation, Delete button
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

            let role_name = if idx == 0 {
                "Object"
            } else if idx == total_surfaces.saturating_sub(1) {
                "Image"
            } else if surf.is_stop {
                "Stop"
            } else {
                "Standard"
            };

            let title_text = format!("Surface {} : {}", idx, role_name);

            ui.label(
                RichText::new(&title_text)
                    .strong()
                    .size(11.0)
                    .color(tokens.text_primary),
            );

            // Right actions: Stop button & Delete button
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if total_surfaces > 2
                    && lucide_small_icon_button(ui, LucideIcon::Trash2, "Delete Surface", tokens)
                        .clicked()
                {
                    *surf_to_remove = Some(idx);
                }

                if surf.is_stop {
                    ui.label(
                        RichText::new("[STOP]")
                            .strong()
                            .monospace()
                            .size(10.5)
                            .color(tokens.warning),
                    );
                } else if ui
                    .small_button(RichText::new("Set Stop").size(10.0))
                    .clicked()
                {
                    *surf_to_toggle_stop = Some(idx);
                }
            });
        });

        ui.add_space(4.0);

        // -------------------------------------------------------------------
        // Row 2A: Radius & Thickness (Strict Numerical TextEdit Inputs + Variable Toggles)
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

            // Radius input + Variable badge
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                let is_r_var = state.variable_radii.contains(&idx);
                if render_variable_badge(ui, is_r_var, "Radius", tokens) {
                    if is_r_var {
                        state.variable_radii.remove(&idx);
                    } else {
                        state.variable_radii.insert(idx);
                    }
                }
                ui.label(RichText::new("Radius:").size(10.5).color(tokens.text_muted));

                let is_flat = surf.radius_mm.is_infinite() || surf.radius_mm.abs() > 1e12;
                if is_flat {
                    ui.label(
                        RichText::new("Infinity")
                            .monospace()
                            .size(10.5)
                            .color(tokens.text_primary),
                    );
                    if ui.small_button("Curve").on_hover_text("Set finite radius (50 mm)").clicked() {
                        surf.radius_mm = 50.0;
                        modified = true;
                    }
                } else {
                    if float_input(ui, Id::new(("surf_radius", idx)), &mut surf.radius_mm, 42.0) {
                        modified = true;
                    }
                    if ui.small_button("Flat").on_hover_text("Make planar (Infinity)").clicked() {
                        surf.radius_mm = f64::INFINITY;
                        state.variable_radii.remove(&idx);
                        modified = true;
                    }
                }
            });

            // Thickness input + Variable badge
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                let is_t_var = state.variable_thicknesses.contains(&idx);
                if render_variable_badge(ui, is_t_var, "Thickness", tokens) {
                    if is_t_var {
                        state.variable_thicknesses.remove(&idx);
                    } else {
                        state.variable_thicknesses.insert(idx);
                    }
                }
                ui.label(RichText::new("Thick:").size(10.5).color(tokens.text_muted));
                if float_input(ui, Id::new(("surf_thick", idx)), &mut surf.thickness_mm, 42.0) {
                    modified = true;
                }
                ui.label(RichText::new("mm").size(10.0).color(tokens.text_muted));
            });
        });

        ui.add_space(3.0);

        // -------------------------------------------------------------------
        // Row 2B: Glass Dropdown (Fixed compact width) & Clear Aperture
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);

            // Glass Picker with Direct Typing + Searchable 646-glass catalog
            if render_glass_picker(ui, idx, surf, state, catalog, tokens) {
                modified = true;
            }

            // Clear Semi-Aperture (strictly 4 digits + point width)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                ui.label(RichText::new("Aperture:").size(10.5).color(tokens.text_muted));
                if float_input(ui, Id::new(("surf_semi_d", idx)), &mut surf.semi_diameter_mm, 42.0) {
                    modified = true;
                }
                ui.label(RichText::new("mm").size(10.0).color(tokens.text_muted));
            });
        });

        // -------------------------------------------------------------------
        // Row 3: Airy Feature Pills Bar: [ Asph ] [ Bic ] [ Tor ] | [ Grin ] [ CB ]
        // Note: Asph, Bic, and Tor are strictly mutually exclusive (one sag profile per surface).
        // -------------------------------------------------------------------
        let is_biconic = matches!(surf.profile, Some(SurfaceProfile::Biconic(_)));
        let is_toroidal = matches!(surf.profile, Some(SurfaceProfile::Toroidal(_)));
        let has_asph = surf.profile.is_none() && (!surf.asphere_coefficients.is_empty() || surf.conic_constant.abs() > 1e-9);
        let has_grin = surf.grin_profile.is_some();
        let has_cb = surf.coordinate_break.is_some();

        ui.add_space(5.0);

        // Calculate uniform pill width distributed across the available card width (accounting for separator)
        let avail_w = ui.available_width();
        let pill_gap = 4.0;
        let pill_w = ((avail_w - (pill_gap * 4.0) - 8.0) / 5.0).clamp(40.0, 68.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(pill_gap, 0.0);

            // 1. Asph (Exclusive Sag Profile)
            let asph_resp = feature_pill(ui, "Asph", has_asph, tokens.plot_tangential, pill_w, tokens)
                .on_hover_text("Even Asphere Profile\nConic constant (k) and polynomial even coefficients (A4, A6...)\nExclusive with Bic and Tor.");
            if asph_resp.clicked() {
                if has_asph {
                    surf.conic_constant = 0.0;
                    surf.asphere_coefficients.clear();
                    ui.data_mut(|d| {
                        d.remove_temp::<String>(Id::new(("surf_conic_k", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a4", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a6", idx)));
                    });
                } else {
                    surf.asphere_coefficients = vec![0.0, 0.0];
                    surf.profile = None; // Asphere replaces Biconic and Toroidal
                }
                modified = true;
            }

            // 2. Bic (Exclusive Sag Profile)
            let bic_resp = feature_pill(ui, "Bic", is_biconic, tokens.plot_sagittal, pill_w, tokens)
                .on_hover_text("Biconic Anamorphic Surface\nIndependent X and Y curvatures (Rx, Ry) and conics (kx, ky)\nExclusive with Asph and Tor.");
            if bic_resp.clicked() {
                if is_biconic {
                    surf.profile = None;
                } else {
                    surf.profile = Some(SurfaceProfile::Biconic(BiconicSurface::default()));
                    surf.asphere_coefficients.clear();
                    surf.conic_constant = 0.0;
                    ui.data_mut(|d| {
                        d.remove_temp::<String>(Id::new(("surf_conic_k", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a4", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a6", idx)));
                    });
                }
                modified = true;
            }

            // 3. Tor (Exclusive Sag Profile)
            let tor_resp = feature_pill(ui, "Tor", is_toroidal, tokens.accent, pill_w, tokens)
                .on_hover_text("Toroidal Surface\nRevolution of an optical conic about a displaced or orthogonal axis\nExclusive with Asph and Bic.");
            if tor_resp.clicked() {
                if is_toroidal {
                    surf.profile = None;
                } else {
                    surf.profile = Some(SurfaceProfile::Toroidal(ToroidalSurface::default()));
                    surf.asphere_coefficients.clear();
                    surf.conic_constant = 0.0;
                    ui.data_mut(|d| {
                        d.remove_temp::<String>(Id::new(("surf_conic_k", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a4", idx)));
                        d.remove_temp::<String>(Id::new(("surf_asph_a6", idx)));
                    });
                }
                modified = true;
            }

            // Visual separator between surface geometry profiles (exclusive) and volumetric/spatial modifiers
            ui.add(egui::Separator::default().vertical().shrink(3.0));

            // 4. Grin (Volumetric Glass Medium)
            let grin_resp = feature_pill(ui, "Grin", has_grin, tokens.warning, pill_w, tokens)
                .on_hover_text("Gradient Index (GRIN) Glass\nContinuous radial Selfoc, axial, or spherical Luneburg gradient");
            if grin_resp.clicked() {
                if has_grin {
                    surf.grin_profile = None;
                } else {
                    surf.grin_profile = Some(GrinProfile::Radial { n0: 1.6, a: 0.01 });
                }
                modified = true;
            }

            // 5. CB
            let cb_resp = feature_pill(ui, "CB", has_cb, tokens.plot_axial, pill_w, tokens)
                .on_hover_text("Coordinate Break (3D Transform)\nRigid decenters (dx, dy, dz) and 3D Euler rotation tilts (tx, ty, tz)");
            if cb_resp.clicked() {
                if has_cb {
                    surf.coordinate_break = None;
                } else {
                    surf.coordinate_break = Some(CoordinateBreak::default());
                }
                modified = true;
            }
        });

        // -------------------------------------------------------------------
        // Expanded Parameter Sub-Cards for Active Features
        // -------------------------------------------------------------------

        // 1. Expanded Asphere Card
        let mut remove_asph = false;
        if has_asph {
            ui.add_space(4.0);
            Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(egui::Stroke::new(1.0, tokens.plot_tangential.gamma_multiply(0.45)))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Even Asphere Profile").size(10.5).strong().color(tokens.plot_tangential));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Remove Asphere", tokens).clicked() {
                                remove_asph = true;
                                modified = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                    // Single row: Conic (k): [ ... ] A4: [ ... ] A6: [ ... ]
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                        ui.label(RichText::new("Conic (k):").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_conic_k", idx)), &mut surf.conic_constant, 40.0) {
                            modified = true;
                        }
                        if !surf.asphere_coefficients.is_empty() {
                            ui.label(RichText::new("A4:").size(10.0).color(tokens.text_muted));
                            if float_input(ui, Id::new(("surf_asph_a4", idx)), &mut surf.asphere_coefficients[0], 48.0) {
                                modified = true;
                            }
                        }
                        if surf.asphere_coefficients.len() > 1 {
                            ui.label(RichText::new("A6:").size(10.0).color(tokens.text_muted));
                            if float_input(ui, Id::new(("surf_asph_a6", idx)), &mut surf.asphere_coefficients[1], 48.0) {
                                modified = true;
                            }
                        }
                    });
                });
        }
        if remove_asph {
            surf.conic_constant = 0.0;
            surf.asphere_coefficients.clear();
            ui.data_mut(|d| {
                d.remove_temp::<String>(Id::new(("surf_conic_k", idx)));
                d.remove_temp::<String>(Id::new(("surf_asph_a4", idx)));
                d.remove_temp::<String>(Id::new(("surf_asph_a6", idx)));
            });
        }

        // 2. Expanded Biconic Card
        let mut remove_biconic = false;
        if let Some(SurfaceProfile::Biconic(biconic)) = &mut surf.profile {
            ui.add_space(4.0);
            Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(egui::Stroke::new(1.0, tokens.plot_sagittal.gamma_multiply(0.45)))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Biconic Anamorphic Profile").size(10.5).strong().color(tokens.plot_sagittal));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Remove Biconic", tokens).clicked() {
                                remove_biconic = true;
                                modified = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                    // X-Axis
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.label(RichText::new("X-Axis:").size(10.0).strong().color(tokens.plot_sagittal));
                        ui.label(RichText::new("Rx:").size(10.0).color(tokens.text_muted));
                        let mut rx = biconic.rx_mm();
                        if float_input(ui, Id::new(("surf_bic_rx", idx)), &mut rx, 46.0) {
                            biconic.cx = if rx.abs() < 1e-15 || rx.is_infinite() { 0.0 } else { 1.0 / rx };
                            modified = true;
                        }
                        ui.label(RichText::new("kx:").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_bic_kx", idx)), &mut biconic.kx, 40.0) {
                            modified = true;
                        }
                    });
                    ui.add_space(2.0);
                    // Y-Axis
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.label(RichText::new("Y-Axis:").size(10.0).strong().color(tokens.plot_sagittal));
                        ui.label(RichText::new("Ry:").size(10.0).color(tokens.text_muted));
                        let mut ry = biconic.ry_mm();
                        if float_input(ui, Id::new(("surf_bic_ry", idx)), &mut ry, 46.0) {
                            biconic.cy = if ry.abs() < 1e-15 || ry.is_infinite() { 0.0 } else { 1.0 / ry };
                            modified = true;
                        }
                        ui.label(RichText::new("ky:").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_bic_ky", idx)), &mut biconic.ky, 40.0) {
                            modified = true;
                        }
                    });
                });
        }
        if remove_biconic {
            surf.profile = None;
        }

        // 3. Expanded Toroidal Card
        let mut remove_toroid = false;
        if let Some(SurfaceProfile::Toroidal(toroid)) = &mut surf.profile {
            ui.add_space(4.0);
            Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(egui::Stroke::new(1.0, tokens.accent.gamma_multiply(0.45)))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Toroidal Surface Profile").size(10.5).strong().color(tokens.accent));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Remove Toroidal", tokens).clicked() {
                                remove_toroid = true;
                                modified = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.label(RichText::new("Rotation Rx:").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_tor_rx", idx)), &mut toroid.rx_mm, 46.0) {
                            modified = true;
                        }
                        ui.label(RichText::new("mm").size(10.0).color(tokens.text_muted));
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        ui.label(RichText::new("Base Ry:").size(10.0).color(tokens.text_muted));
                        let mut ry = toroid.ry_mm();
                        if float_input(ui, Id::new(("surf_tor_ry", idx)), &mut ry, 46.0) {
                            toroid.cy = if ry.abs() < 1e-15 || ry.is_infinite() { 0.0 } else { 1.0 / ry };
                            modified = true;
                        }
                        ui.label(RichText::new("ky:").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_tor_ky", idx)), &mut toroid.ky, 40.0) {
                            modified = true;
                        }
                    });
                });
        }
        if remove_toroid {
            surf.profile = None;
        }

        // 4. Expanded Coordinate Break Card
        let mut remove_cb = false;
        if let Some(cb) = &mut surf.coordinate_break {
            ui.add_space(4.0);
            Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(egui::Stroke::new(1.0, tokens.plot_axial.gamma_multiply(0.45)))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Coordinate Break (3D Transform)").size(10.5).strong().color(tokens.plot_axial));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Remove Coordinate Break", tokens).clicked() {
                                remove_cb = true;
                                modified = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                    // Decenters
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                        ui.label(RichText::new("Decenter:").size(10.0).color(tokens.text_muted));
                        ui.label(RichText::new("dx").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_dx", idx)), &mut cb.decenter_x_mm, 36.0) {
                            modified = true;
                        }
                        ui.label(RichText::new("dy").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_dy", idx)), &mut cb.decenter_y_mm, 36.0) {
                            modified = true;
                        }
                        ui.label(RichText::new("dz").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_dz", idx)), &mut cb.decenter_z_mm, 36.0) {
                            modified = true;
                        }
                    });
                    ui.add_space(2.0);
                    // Tilts
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                        ui.label(RichText::new("Euler Tilt:").size(10.0).color(tokens.text_muted));
                        ui.label(RichText::new("tx°").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_tx", idx)), &mut cb.tilt_x_deg, 36.0) {
                            modified = true;
                        }
                        ui.label(RichText::new("ty°").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_ty", idx)), &mut cb.tilt_y_deg, 36.0) {
                            modified = true;
                        }
                        ui.label(RichText::new("tz°").size(10.0).color(tokens.text_muted));
                        if float_input(ui, Id::new(("surf_cb_tz", idx)), &mut cb.tilt_z_deg, 36.0) {
                            modified = true;
                        }
                    });
                    ui.add_space(2.0);
                    if ui.checkbox(&mut cb.tilt_first, "Order: Tilt First (rotations before decenters)").changed() {
                        modified = true;
                    }
                });
        }
        if remove_cb {
            surf.coordinate_break = None;
        }

        // 5. Expanded GRIN Profile Card
        let mut remove_grin = false;
        if let Some(grin) = &mut surf.grin_profile {
            ui.add_space(4.0);
            Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(egui::Stroke::new(1.0, tokens.warning.gamma_multiply(0.45)))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    let name = match grin {
                        GrinProfile::Radial { .. } => "Radial Selfoc GRIN",
                        GrinProfile::Axial { .. } => "Axial GRIN Profile",
                        GrinProfile::Luneburg { .. } => "Luneburg Sphere GRIN",
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(10.5).strong().color(tokens.warning));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Remove GRIN", tokens).clicked() {
                                remove_grin = true;
                                modified = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                    match grin {
                        GrinProfile::Radial { n0, a } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                ui.label(RichText::new("n0:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_n0", idx)), n0, 44.0) {
                                    modified = true;
                                }
                                ui.label(RichText::new("a:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_a", idx)), a, 48.0) {
                                    modified = true;
                                }
                            });
                        }
                        GrinProfile::Axial { n0, n1, n2 } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                                ui.label(RichText::new("n0:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_n0", idx)), n0, 38.0) {
                                    modified = true;
                                }
                                ui.label(RichText::new("n1:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_n1", idx)), n1, 38.0) {
                                    modified = true;
                                }
                                ui.label(RichText::new("n2:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_n2", idx)), n2, 38.0) {
                                    modified = true;
                                }
                            });
                        }
                        GrinProfile::Luneburg { n0, radius_mm } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                ui.label(RichText::new("n0:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_n0", idx)), n0, 42.0) {
                                    modified = true;
                                }
                                ui.label(RichText::new("Radius:").size(10.0).color(tokens.text_muted));
                                if float_input(ui, Id::new(("surf_grin_r", idx)), radius_mm, 46.0) {
                                    modified = true;
                                }
                                ui.label(RichText::new("mm").size(10.0).color(tokens.text_muted));
                            });
                        }
                    }
                });
        }
        if remove_grin {
            surf.grin_profile = None;
        }
    });

    modified
}

/// Renders the clean, compact technical glass cell with direct typing and discrete [...] browser button.
fn render_glass_picker(
    ui: &mut Ui,
    idx: usize,
    surf: &mut OpticalSurface,
    state: &mut LensEditorState,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
        ui.label(RichText::new("Glass:").size(10.5).color(tokens.text_muted));

        // 1. Direct editable text box
        let old_mat = surf.material_name.clone();
        let mut mat_buf = surf.material_name.clone();

        let is_known = surf.material_name.to_uppercase() == "AIR"
            || catalog.lookup(&surf.material_name).is_some();
        let text_color = if is_known {
            tokens.text_primary
        } else {
            tokens.warning
        };

        let tooltip = if surf.material_name.to_uppercase() == "AIR" {
            "AIR: Refractive index 1.0 (free space)".to_string()
        } else if let Some(g) = catalog.lookup(&surf.material_name) {
            format!("{}: {} (nd = {:.4}, Vd = {:.1})", g.catalog.name(), g.name, g.nd, g.vd)
        } else {
            format!("Custom material: '{}' (not in standard catalog)", surf.material_name)
        };

        let text_edit = TextEdit::singleline(&mut mat_buf)
            .font(egui::TextStyle::Monospace)
            .text_color(text_color)
            .desired_width(52.0);

        let resp = ui.add(text_edit).on_hover_text(tooltip);
        if resp.gained_focus() {
            state.autocomplete_surface = Some(idx);
        }

        if resp.changed() {
            surf.material_name = mat_buf.trim().to_uppercase();
            state.autocomplete_surface = Some(idx);
            if surf.material_name != old_mat {
                modified = true;
            }
        }

        // Live autocomplete dropdown as user types in LDE
        if state.autocomplete_surface == Some(idx) {
            let query = mat_buf.trim().to_uppercase();
            if !query.is_empty() {
                let mut matches: Vec<&optics_glass::GlassMaterial> = catalog
                    .all_glasses()
                    .filter(|g| g.name.to_uppercase().contains(&query))
                    .collect();

                matches.sort_by_key(|g| (!g.name.to_uppercase().starts_with(&query), g.name.len(), g.name.clone()));
                let air_matches = "AIR".contains(&query) && query != "AIR";

                if !matches.is_empty() || air_matches {
                    let popup_pos = resp.rect.left_bottom() + Vec2::new(0.0, 2.0);
                    let area = Area::new(Id::new(("glass_ac_dropdown", idx)))
                        .order(egui::Order::Foreground)
                        .fixed_pos(popup_pos);

                    let area_resp = area.show(ui.ctx(), |ui| {
                        Frame::NONE
                            .fill(tokens.bg_panel)
                            .stroke(tokens.border_hovered)
                            .corner_radius(CornerRadius::ZERO)
                            .inner_margin(Margin::same(4))
                            .show(ui, |ui| {
                                ui.set_width(215.0);
                                ScrollArea::vertical()
                                    .max_height(134.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            ui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);

                                            if air_matches {
                                                let row_resp = ui.allocate_response(Vec2::new(ui.available_width(), 22.0), egui::Sense::click());
                                                let is_hov = row_resp.hovered();
                                                if is_hov {
                                                    ui.painter().rect_filled(row_resp.rect, 0.0, tokens.bg_control_active);
                                                }
                                                let text_pos = row_resp.rect.left_center() + Vec2::new(6.0, 0.0);
                                                let text_color = if is_hov { tokens.accent } else { tokens.text_primary };
                                                ui.painter().text(
                                                    text_pos,
                                                    egui::Align2::LEFT_CENTER,
                                                    "AIR       nd=1.0000 (Air)",
                                                    egui::FontId::monospace(10.5),
                                                    text_color,
                                                );
                                                if row_resp.clicked() {
                                                    surf.material_name = "AIR".to_string();
                                                    state.autocomplete_surface = None;
                                                    modified = true;
                                                }
                                            }

                                            for g in matches {
                                                let row_resp = ui.allocate_response(Vec2::new(ui.available_width(), 22.0), egui::Sense::click());
                                                let is_hov = row_resp.hovered();
                                                if is_hov {
                                                    ui.painter().rect_filled(row_resp.rect, 0.0, tokens.bg_control_active);
                                                }
                                                let text_pos = row_resp.rect.left_center() + Vec2::new(6.0, 0.0);
                                                let text_color = if is_hov { tokens.accent } else { tokens.text_primary };
                                                let label = format!("{:<9} nd={:.4} Vd={:.1}", g.name, g.nd, g.vd);
                                                ui.painter().text(
                                                    text_pos,
                                                    egui::Align2::LEFT_CENTER,
                                                    label,
                                                    egui::FontId::monospace(10.5),
                                                    text_color,
                                                );
                                                if row_resp.clicked() {
                                                    surf.material_name = g.name.clone();
                                                    state.autocomplete_surface = None;
                                                    modified = true;
                                                }
                                            }
                                        });
                                    });
                            });
                    });

                    // Close if escape pressed or clicked outside
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        state.autocomplete_surface = None;
                    }
                    if ui.input(|i| i.pointer.any_pressed()) {
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            if !resp.rect.contains(pos) && !area_resp.response.rect.contains(pos) {
                                state.autocomplete_surface = None;
                            }
                        }
                    }
                }
            }
        }

        // 2. Discrete academic [...] browse button
        let browse_btn = Button::new(
            RichText::new("...")
                .monospace()
                .size(10.5)
                .color(tokens.text_muted),
        )
        .fill(tokens.bg_control)
        .corner_radius(CornerRadius::ZERO);

        let btn_resp = ui.add(browse_btn).on_hover_text("Open Materials Catalog tab for this surface");
        if btn_resp.clicked() {
            state.materials_target_surface = idx;
            state.active_tab = LeftPanelTab::Materials;
        }
    });

    modified
}

/// Renders a strict numerical text input box (zero dragging, purely typed input).
fn float_input(ui: &mut Ui, id: Id, value: &mut f64, width: f32) -> bool {
    let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| {
        if value.is_infinite() {
            "Infinity".to_string()
        } else {
            format!("{:.3}", *value)
        }
    });

    let resp = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(width)
            .font(egui::TextStyle::Monospace),
    );

    let mut changed = false;
    if resp.changed() {
        if text.trim().eq_ignore_ascii_case("infinity") {
            *value = f64::INFINITY;
            changed = true;
        } else if let Ok(parsed) = text.trim().parse::<f64>() {
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

/// Renders an airy, modern scientific pill button for surface modifiers (Asph, Bic, Tor, Grin, CB).
fn feature_pill(
    ui: &mut Ui,
    label: &str,
    is_active: bool,
    active_color: egui::Color32,
    width: f32,
    tokens: &OpticsTokens,
) -> egui::Response {
    let (bg, stroke, text_color, text) = if is_active {
        (
            active_color.gamma_multiply(0.18),
            egui::Stroke::new(1.2, active_color),
            active_color,
            format!("● {}", label),
        )
    } else {
        (
            tokens.bg_control,
            tokens.border,
            tokens.text_muted,
            label.to_string(),
        )
    };

    let btn = Button::new(RichText::new(text).size(9.5).strong().color(text_color))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(width, 21.0));

    ui.add(btn)
}

/// Renders an academic variable toggle badge ('V' for Variable, 'F' for Fixed).
fn render_variable_badge(
    ui: &mut Ui,
    is_variable: bool,
    param_name: &str,
    tokens: &OpticsTokens,
) -> bool {
    let (bg, stroke, text_color, text, tooltip) = if is_variable {
        (
            tokens.bg_control_active,
            egui::Stroke::new(1.0, tokens.accent),
            tokens.accent,
            "V",
            format!("{} is Variable for optimization (DLS will adjust this value). Click to set Fixed.", param_name),
        )
    } else {
        (
            tokens.bg_control,
            tokens.border,
            tokens.text_muted.gamma_multiply(0.65),
            "F",
            format!("{} is Fixed. Click to mark as Variable (V) for optimization.", param_name),
        )
    };

    let btn = Button::new(RichText::new(text).size(9.0).monospace().strong().color(text_color))
        .fill(bg)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(2))
        .min_size(Vec2::new(15.0, 16.0));

    ui.add(btn).on_hover_text(tooltip).clicked()
}

/// Renders the compact technical specification card at the top of the Lens Data Editor.
fn render_specification_card(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    state: &mut LensEditorState,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    let frame = Frame::NONE
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(10, 8));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            // Header: Prescription Title, Settings Toggle & Aperture Stop
            let stop_label = prescription
                .surfaces
                .iter()
                .position(|s| s.is_stop)
                .map(|i| format!("Stop: S{}", i))
                .unwrap_or_else(|| "Stop: None".to_string());

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&prescription.name)
                        .strong()
                        .size(12.0)
                        .color(tokens.text_primary),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let tooltip = if state.system_explorer_open {
                        "Close System Explorer: Hide Wavelengths and Field Angles drawer"
                    } else {
                        "Open System Explorer: Configure Wavelengths and Field Angles"
                    };

                    let btn_resp = if state.system_explorer_open {
                        let icon_size = 14.0;
                        let img = Image::from_bytes(
                            LucideIcon::SlidersHorizontal.uri(),
                            LucideIcon::SlidersHorizontal.svg_content().as_bytes(),
                        )
                        .tint(tokens.accent)
                        .fit_to_exact_size(Vec2::splat(icon_size));

                        let btn = egui::Button::image_and_text(
                            img,
                            egui::RichText::new("Settings").size(11.0).strong().color(tokens.accent),
                        )
                        .fill(tokens.bg_control_active)
                        .stroke(tokens.border_hovered)
                        .corner_radius(CornerRadius::ZERO);

                        ui.add(btn).on_hover_text(tooltip)
                    } else {
                        lucide_text_button(ui, LucideIcon::SlidersHorizontal, "Settings", tokens)
                            .on_hover_text(tooltip)
                    };

                    if btn_resp.clicked() {
                        state.system_explorer_open = !state.system_explorer_open;
                    }

                    ui.label(
                        RichText::new(stop_label)
                            .monospace()
                            .strong()
                            .size(10.5)
                            .color(tokens.accent),
                    );
                });
            });

            ui.add_space(4.0);

            // Ensure prescription wavelengths are valid
            if prescription.wavelengths_nm.is_empty() {
                prescription.wavelengths_nm.push(587.56);
                prescription.primary_wavelength_index = 0;
            }
            if prescription.primary_wavelength_index >= prescription.wavelengths_nm.len() {
                prescription.primary_wavelength_index = 0;
            }

            // Technical Specs Row (Always visible & interactive)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);

                // EPD directly editable via DragValue
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.label(RichText::new("EPD:").size(10.5).color(tokens.text_muted));
                    let old_epd = prescription.entrance_pupil_diameter_mm;
                    let resp = ui.add(
                        DragValue::new(&mut prescription.entrance_pupil_diameter_mm)
                            .speed(0.1)
                            .range(0.2..=500.0)
                            .suffix(" mm"),
                    );
                    if resp.changed() || (prescription.entrance_pupil_diameter_mm - old_epd).abs() > 1e-6 {
                        modified = true;
                    }
                });

                // Primary Wavelength directly editable via DragValue
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.label(RichText::new("Wav:").size(10.5).color(tokens.text_muted));
                    let p_idx = prescription.primary_wavelength_index;
                    let old_wav = prescription.wavelengths_nm[p_idx];
                    let resp = ui.add(
                        DragValue::new(&mut prescription.wavelengths_nm[p_idx])
                            .speed(0.5)
                            .range(350.0..=14000.0)
                            .suffix(" nm"),
                    );
                    if resp.changed() || (prescription.wavelengths_nm[p_idx] - old_wav).abs() > 1e-3 {
                        modified = true;
                    }
                });
            });

            // -------------------------------------------------------------------
            // System Explorer Drawer: Wavelengths & Field Angles
            // -------------------------------------------------------------------
            if state.system_explorer_open {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // --- Section A: Wavelengths ---
                ui.horizontal(|ui| {
                    ui.label(RichText::new("WAVELENGTHS (nm):").strong().size(10.5).color(tokens.text_primary));
                });

                let mut wav_to_remove: Option<usize> = None;
                for (w_idx, &wav) in prescription.wavelengths_nm.iter().enumerate() {
                    let is_primary = w_idx == prescription.primary_wavelength_index;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                        if ui.radio(is_primary, "").on_hover_text("Set as primary design wavelength").clicked() {
                            prescription.primary_wavelength_index = w_idx;
                            modified = true;
                        }

                        ui.label(
                            RichText::new(format!("{:.2} nm", wav))
                                .monospace()
                                .size(10.5)
                                .color(if is_primary { tokens.accent } else { tokens.text_primary }),
                        );

                        if is_primary {
                            ui.label(RichText::new("[Primary]").size(9.5).color(tokens.accent));
                        }

                        if prescription.wavelengths_nm.len() > 1 {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Delete Wavelength", tokens).clicked() {
                                wav_to_remove = Some(w_idx);
                            }
                        }
                    });
                }

                if let Some(del_idx) = wav_to_remove {
                    prescription.wavelengths_nm.remove(del_idx);
                    if prescription.primary_wavelength_index >= prescription.wavelengths_nm.len() {
                        prescription.primary_wavelength_index = prescription.wavelengths_nm.len().saturating_sub(1);
                    }
                    modified = true;
                }

                // Add Wavelength row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.add(TextEdit::singleline(&mut state.new_wav_buf).desired_width(50.0).font(egui::TextStyle::Monospace));
                    if lucide_text_button(ui, LucideIcon::Plus, "Add", tokens).clicked() {
                        if let Ok(val) = state.new_wav_buf.trim().parse::<f64>() {
                            if val >= 300.0 && val <= 15000.0 && !prescription.wavelengths_nm.iter().any(|&w| (w - val).abs() < 0.1) {
                                prescription.wavelengths_nm.push(val);
                                modified = true;
                            }
                        }
                    }
                    // Presets
                    if ui.small_button("d (587.6)").clicked() {
                        if !prescription.wavelengths_nm.iter().any(|&w| (w - 587.56).abs() < 0.1) {
                            prescription.wavelengths_nm.push(587.56);
                            modified = true;
                        }
                    }
                    if ui.small_button("F (486.1)").clicked() {
                        if !prescription.wavelengths_nm.iter().any(|&w| (w - 486.13).abs() < 0.1) {
                            prescription.wavelengths_nm.push(486.13);
                            modified = true;
                        }
                    }
                    if ui.small_button("C (656.3)").clicked() {
                        if !prescription.wavelengths_nm.iter().any(|&w| (w - 656.27).abs() < 0.1) {
                            prescription.wavelengths_nm.push(656.27);
                            modified = true;
                        }
                    }
                });

                ui.add_space(4.0);

                // --- Section B: Field Angles (Grade) ---
                ui.horizontal(|ui| {
                    ui.label(RichText::new("FIELD ANGLES (°):").strong().size(10.5).color(tokens.text_primary));
                });

                if prescription.field_angles_deg.is_empty() {
                    prescription.field_angles_deg.push(0.0);
                }

                let mut field_to_remove: Option<usize> = None;
                for (f_idx, &field_deg) in prescription.field_angles_deg.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        let label = if field_deg == 0.0 {
                            format!("Field {}: 0.00° (On-Axis)", f_idx)
                        } else {
                            format!("Field {}: {:.2}°", f_idx, field_deg)
                        };

                        ui.label(
                            RichText::new(label)
                                .monospace()
                                .size(10.5)
                                .color(tokens.text_primary),
                        );

                        if prescription.field_angles_deg.len() > 1 && field_deg != 0.0 {
                            if lucide_small_icon_button(ui, LucideIcon::X, "Delete Field Angle", tokens).clicked() {
                                field_to_remove = Some(f_idx);
                            }
                        }
                    });
                }

                if let Some(del_idx) = field_to_remove {
                    prescription.field_angles_deg.remove(del_idx);
                    modified = true;
                }

                // Add Field row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.add(TextEdit::singleline(&mut state.new_field_buf).desired_width(50.0).font(egui::TextStyle::Monospace));
                    if lucide_text_button(ui, LucideIcon::Plus, "Add", tokens).clicked() {
                        if let Ok(val) = state.new_field_buf.trim().parse::<f64>() {
                            if (0.0..=89.0).contains(&val) && !prescription.field_angles_deg.iter().any(|&f| (f - val).abs() < 0.01) {
                                prescription.field_angles_deg.push(val);
                                prescription.field_angles_deg.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                                modified = true;
                            }
                        }
                    }
                    if ui.small_button("+ 10°").clicked() {
                        if !prescription.field_angles_deg.iter().any(|&f| (f - 10.0).abs() < 0.01) {
                            prescription.field_angles_deg.push(10.0);
                            prescription.field_angles_deg.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            modified = true;
                        }
                    }
                    if ui.small_button("+ 14°").clicked() {
                        if !prescription.field_angles_deg.iter().any(|&f| (f - 14.0).abs() < 0.01) {
                            prescription.field_angles_deg.push(14.0);
                            prescription.field_angles_deg.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            modified = true;
                        }
                    }
                    if ui.small_button("+ 20°").clicked() {
                        if !prescription.field_angles_deg.iter().any(|&f| (f - 20.0).abs() < 0.01) {
                            prescription.field_angles_deg.push(20.0);
                            prescription.field_angles_deg.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                            modified = true;
                        }
                    }
                });
            }
        });
    });

    modified
}

/// Re-indexes surface indices sequentially after addition or removal.
fn reindex_surfaces(prescription: &mut OpticalPrescription) {
    for (i, surf) in prescription.surfaces.iter_mut().enumerate() {
        surf.surface_index = i;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reindex_surfaces() {
        let mut rx = OpticalPrescription {
            name: "Test System".to_string(),
            surfaces: vec![
                OpticalSurface { surface_index: 99, ..Default::default() },
                OpticalSurface { surface_index: 42, ..Default::default() },
            ],
            ..Default::default()
        };

        reindex_surfaces(&mut rx);
        assert_eq!(rx.surfaces[0].surface_index, 0);
        assert_eq!(rx.surfaces[1].surface_index, 1);
    }

    #[test]
    fn test_variable_flags_tracking() {
        let mut state = LensEditorState::default();
        assert_eq!(state.total_variable_count(), 0);

        state.variable_radii.insert(1);
        state.variable_radii.insert(2);
        state.variable_thicknesses.insert(1);

        assert_eq!(state.total_variable_count(), 3);
        assert!(state.variable_radii.contains(&1));
        assert!(state.variable_thicknesses.contains(&1));
        assert!(!state.variable_radii.contains(&3));

        state.clear_all_variables();
        assert_eq!(state.total_variable_count(), 0);
    }
}
