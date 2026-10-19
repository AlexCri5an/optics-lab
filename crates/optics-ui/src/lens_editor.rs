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
    Align, Area, Button, CornerRadius, DragValue, Frame, Id, Layout, Margin, RichText,
    ScrollArea, TextEdit, Ui, Vec2,
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
        if render_specification_card(ui, prescription, tokens) {
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
                let suggestions: Vec<&optics_glass::GlassMaterial> = matches.into_iter().take(6).collect();
                let air_matches = "AIR".contains(&query) && query != "AIR";

                if !suggestions.is_empty() || air_matches {
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
                                ui.set_max_width(220.0);
                                ui.vertical(|ui| {
                                    if air_matches {
                                        let air_btn = Button::new(RichText::new("AIR (nd=1.0000)").monospace().size(11.0).color(tokens.accent))
                                            .fill(tokens.bg_control)
                                            .corner_radius(CornerRadius::ZERO);
                                        if ui.add(air_btn).clicked() {
                                            surf.material_name = "AIR".to_string();
                                            state.autocomplete_surface = None;
                                            modified = true;
                                        }
                                    }
                                    for g in suggestions {
                                        let label = format!("{} (nd={:.4}, Vd={:.1})", g.name, g.nd, g.vd);
                                        let btn = Button::new(RichText::new(label).monospace().size(11.0).color(tokens.text_primary))
                                            .fill(tokens.bg_control)
                                            .corner_radius(CornerRadius::ZERO);
                                        if ui.add(btn).clicked() {
                                            surf.material_name = g.name.clone();
                                            state.autocomplete_surface = None;
                                            modified = true;
                                        }
                                    }
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
            // Header: Prescription Title & Aperture Stop
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
