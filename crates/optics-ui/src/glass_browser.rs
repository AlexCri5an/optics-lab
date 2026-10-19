//! Academic Glass Catalog Browser modal dialog component.
//!
//! Conforms to Zemax OpticStudio / CODE V desktop CAD standards:
//! - Full-screen / floating modal dialog dedicated to exploring and selecting optical materials.
//! - Fast search filter across all 646 optical glasses in the database.
//! - Categorization by manufacturer (Schott, Ohara, CDGM, Infrared & Other, Common).
//! - Clean engineering table displaying commercial designation, manufacturer, $n_d$, $V_d$, and dispersion formula.
//! - Physical details inspector showing dispersion formula and thermal status.
//! - Direct assignment to any target surface with double-click or "Apply" button.

use egui::{
    Align, Button, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea,
    TextEdit, Vec2, Window,
};
use optics_glass::{GlassCatalog, GlassCatalogDb, GlassMaterial};

use crate::lens_editor::COMMON_OPTICAL_GLASSES;
use crate::theme::OpticsTokens;

/// Selected manufacturer filter in the catalog browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogFilter {
    /// Show all 646 glasses.
    #[default]
    All,
    /// 17 common standard glasses + AIR.
    Common,
    /// Schott Glass (Germany, 155 glasses).
    Schott,
    /// Ohara Corporation (Japan, 143 glasses).
    Ohara,
    /// CDGM Optical Glass (China, 241 glasses).
    Cdgm,
    /// Infrared crystals and other non-standard materials.
    InfraredOther,
}

/// Persistent UI state for the Glass Catalog Browser dialog.
#[derive(Debug, Clone)]
pub struct GlassBrowserState {
    /// Whether the dialog window is open.
    pub open: bool,
    /// Target surface index in the optical prescription.
    pub target_surface: Option<usize>,
    /// Live search query filter.
    pub search_query: String,
    /// Selected manufacturer filter.
    pub active_filter: CatalogFilter,
    /// Currently highlighted glass designation in the browser table.
    pub highlighted_glass: Option<String>,
}

impl Default for GlassBrowserState {
    fn default() -> Self {
        Self {
            open: false,
            target_surface: None,
            search_query: String::new(),
            active_filter: CatalogFilter::All,
            highlighted_glass: Some("N-BK7".to_string()),
        }
    }
}

impl GlassBrowserState {
    /// Opens the browser for a specific surface index, pre-selecting its current material.
    pub fn open_for_surface(&mut self, surface_idx: usize, current_material: &str) {
        self.open = true;
        self.target_surface = Some(surface_idx);
        let mat = current_material.trim().to_uppercase();
        if !mat.is_empty() && mat != "AIR" {
            self.highlighted_glass = Some(mat);
        }
    }
}

/// Action outcome from the Glass Catalog Browser modal.
#[derive(Debug, Clone, PartialEq)]
pub enum GlassBrowserAction {
    /// None (user is browsing).
    None,
    /// Apply specified material to the target surface.
    Apply {
        /// Target surface index.
        surface_idx: usize,
        /// Commercial material designation (e.g. "N-BK7", "SF11", "AIR").
        material_name: String,
    },
    /// Close the modal without making changes.
    Close,
}

/// Renders the academic Glass Catalog Browser window dialog.
pub fn render_glass_browser_modal(
    ctx: &egui::Context,
    state: &mut GlassBrowserState,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> GlassBrowserAction {
    if !state.open {
        return GlassBrowserAction::None;
    }

    let mut action = GlassBrowserAction::None;
    let target_idx_text = state
        .target_surface
        .map(|i| format!("Surface {}", i))
        .unwrap_or_else(|| "Active Surface".to_string());

    let title = format!("Glass Catalog Browser — Selection for {}", target_idx_text);

    let mut is_open = state.open;
    Window::new(RichText::new(&title).strong().size(12.5).color(tokens.text_primary))
        .open(&mut is_open)
        .resizable(true)
        .collapsible(false)
        .default_size(Vec2::new(720.0, 520.0))
        .min_size(Vec2::new(560.0, 380.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(
            Frame::window(&ctx.style())
                .fill(tokens.bg_panel)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::same(12)),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);

            // -------------------------------------------------------------------
            // 1. Search Bar & Target Surface Info
            // -------------------------------------------------------------------
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                ui.label(RichText::new("Search:").strong().size(11.0).color(tokens.text_muted));

                let search_box = TextEdit::singleline(&mut state.search_query)
                    .hint_text("Type glass name (e.g. N-BK7, SF11, F2, LAK9)...")
                    .desired_width(260.0);

                ui.add(search_box);

                if !state.search_query.is_empty() && ui.small_button("Clear").clicked() {
                    state.search_query.clear();
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("Total in DB: {} glasses", catalog.len()))
                            .monospace()
                            .size(10.5)
                            .color(tokens.text_muted),
                    );
                });
            });

            // -------------------------------------------------------------------
            // 2. Manufacturer Filter Tabs (Pills)
            // -------------------------------------------------------------------
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                let filters = [
                    (CatalogFilter::All, format!("ALL ({})", catalog.len())),
                    (CatalogFilter::Common, format!("COMMON ({})", COMMON_OPTICAL_GLASSES.len())),
                    (CatalogFilter::Schott, "SCHOTT (155)".to_string()),
                    (CatalogFilter::Ohara, "OHARA (143)".to_string()),
                    (CatalogFilter::Cdgm, "CDGM (241)".to_string()),
                    (CatalogFilter::InfraredOther, "INFRARED & OTHER (15)".to_string()),
                ];

                for (flt, label) in filters {
                    let is_active = state.active_filter == flt;
                    let btn_color = if is_active { tokens.accent } else { tokens.text_muted };
                    let btn = Button::new(
                        RichText::new(label)
                            .size(10.0)
                            .strong()
                            .color(if is_active { Color32::WHITE } else { btn_color }),
                    )
                    .fill(if is_active { tokens.accent } else { tokens.bg_control })
                    .corner_radius(CornerRadius::ZERO);

                    if ui.add(btn).clicked() {
                        state.active_filter = flt;
                    }
                }
            });

            ui.separator();

            // -------------------------------------------------------------------
            // 3. Filter and Prepare Glass List
            // -------------------------------------------------------------------
            let query = state.search_query.trim().to_uppercase();
            let mut list: Vec<&GlassMaterial> = catalog
                .all_glasses()
                .filter(|g| {
                    // Filter by manufacturer
                    let matches_catalog = match state.active_filter {
                        CatalogFilter::All => true,
                        CatalogFilter::Common => COMMON_OPTICAL_GLASSES.contains(&g.name.as_str()),
                        CatalogFilter::Schott => g.catalog == GlassCatalog::Schott,
                        CatalogFilter::Ohara => g.catalog == GlassCatalog::Ohara,
                        CatalogFilter::Cdgm => g.catalog == GlassCatalog::Cdgm,
                        CatalogFilter::InfraredOther => {
                            g.catalog != GlassCatalog::Schott
                                && g.catalog != GlassCatalog::Ohara
                                && g.catalog != GlassCatalog::Cdgm
                        }
                    };

                    // Filter by search query
                    let matches_query = query.is_empty() || g.name.to_uppercase().contains(&query);

                    matches_catalog && matches_query
                })
                .collect();

            list.sort_by_key(|g| &g.name);

            // -------------------------------------------------------------------
            // 4. Academic Data Table (Headers + ScrollArea)
            // -------------------------------------------------------------------
            let table_frame = Frame::NONE
                .fill(tokens.bg_canvas)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::same(4));

            let mut chosen_material: Option<String> = None;

            table_frame.show(ui, |ui| {
                // Table Header
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                    ui.label(RichText::new("  NAME").monospace().strong().size(10.5).color(tokens.text_muted));
                    ui.add_space(90.0);
                    ui.label(RichText::new("CATALOG").monospace().strong().size(10.5).color(tokens.text_muted));
                    ui.add_space(55.0);
                    ui.label(RichText::new("nd (587.6 nm)").monospace().strong().size(10.5).color(tokens.text_muted));
                    ui.add_space(35.0);
                    ui.label(RichText::new("Vd (Abbe)").monospace().strong().size(10.5).color(tokens.text_muted));
                    ui.add_space(35.0);
                    ui.label(RichText::new("DISPERSION MODEL").monospace().strong().size(10.5).color(tokens.text_muted));
                });
                ui.separator();

                ScrollArea::vertical()
                    .max_height(250.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Quick AIR option if query is empty or matches AIR
                        if query.is_empty() || "AIR".contains(&query) {
                            let is_selected = state.highlighted_glass.as_deref() == Some("AIR");
                            let bg_color = if is_selected {
                                tokens.bg_control_active
                            } else {
                                Color32::TRANSPARENT
                            };

                            let row_resp = ui.horizontal(|ui| {
                                ui.set_width(ui.available_width());
                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

                                let row_frame = Frame::NONE
                                    .fill(bg_color)
                                    .inner_margin(Margin::symmetric(6, 3));

                                row_frame.show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let name_txt = RichText::new("AIR")
                                            .monospace()
                                            .strong()
                                            .size(11.0)
                                            .color(tokens.accent);
                                        ui.add_sized([120.0, 16.0], egui::Label::new(name_txt));

                                        let cat_txt = RichText::new("Vacuum / Free Space")
                                            .size(10.5)
                                            .color(tokens.text_muted);
                                        ui.add_sized([100.0, 16.0], egui::Label::new(cat_txt));

                                        let nd_txt = RichText::new("1.00000")
                                            .monospace()
                                            .size(10.5)
                                            .color(tokens.text_primary);
                                        ui.add_sized([90.0, 16.0], egui::Label::new(nd_txt));

                                        let vd_txt = RichText::new("—")
                                            .monospace()
                                            .size(10.5)
                                            .color(tokens.text_muted);
                                        ui.add_sized([80.0, 16.0], egui::Label::new(vd_txt));

                                        let model_txt = RichText::new("Ideal Constant")
                                            .size(10.5)
                                            .color(tokens.text_muted);
                                        ui.add_sized([140.0, 16.0], egui::Label::new(model_txt));
                                    });
                                });
                            });

                            if row_resp.response.interact(egui::Sense::click()).clicked() {
                                state.highlighted_glass = Some("AIR".to_string());
                            }
                            if row_resp.response.interact(egui::Sense::click()).double_clicked() {
                                chosen_material = Some("AIR".to_string());
                            }
                        }

                        // Render filtered list
                        for g in &list {
                            let is_selected = state.highlighted_glass.as_deref() == Some(&g.name);
                            let bg_color = if is_selected {
                                tokens.bg_control_active
                            } else {
                                Color32::TRANSPARENT
                            };

                            let model_desc = match &g.dispersion {
                                optics_glass::DispersionModel::Sellmeier1 { .. } => "Sellmeier 1 (6-coef)",
                                optics_glass::DispersionModel::Sellmeier2 { .. } => "Sellmeier 2",
                                optics_glass::DispersionModel::Sellmeier3 { .. } => "Sellmeier 3",
                                optics_glass::DispersionModel::Cauchy { .. } => "Cauchy",
                                optics_glass::DispersionModel::ExtendedCauchy { .. } => "Extended Cauchy",
                                optics_glass::DispersionModel::Constant { .. } => "Constant Index",
                                _ => "Empirical",
                            };

                            let row_frame = Frame::NONE
                                .fill(bg_color)
                                .inner_margin(Margin::symmetric(6, 3));

                            let resp = row_frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let name_color = if is_selected { tokens.accent } else { tokens.text_primary };
                                    let name_txt = RichText::new(&g.name)
                                        .monospace()
                                        .strong()
                                        .size(11.0)
                                        .color(name_color);
                                    ui.add_sized([120.0, 16.0], egui::Label::new(name_txt));

                                    let cat_txt = RichText::new(g.catalog.name())
                                        .size(10.5)
                                        .color(tokens.text_muted);
                                    ui.add_sized([100.0, 16.0], egui::Label::new(cat_txt));

                                    let nd_txt = RichText::new(format!("{:.5}", g.nd))
                                        .monospace()
                                        .size(10.5)
                                        .color(tokens.text_primary);
                                    ui.add_sized([90.0, 16.0], egui::Label::new(nd_txt));

                                    let vd_txt = RichText::new(format!("{:.2}", g.vd))
                                        .monospace()
                                        .size(10.5)
                                        .color(tokens.text_primary);
                                    ui.add_sized([80.0, 16.0], egui::Label::new(vd_txt));

                                    let model_txt = RichText::new(model_desc)
                                        .size(10.5)
                                        .color(tokens.text_muted);
                                    ui.add_sized([140.0, 16.0], egui::Label::new(model_txt));
                                });
                            });

                            let interact = resp.response.interact(egui::Sense::click());
                            if interact.clicked() {
                                state.highlighted_glass = Some(g.name.clone());
                            }
                            if interact.double_clicked() {
                                chosen_material = Some(g.name.clone());
                            }
                        }

                        if list.is_empty() && query != "AIR" {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new("No matching glasses found in catalog.")
                                        .size(11.0)
                                        .color(tokens.text_muted),
                                );
                                if !query.is_empty() {
                                    if ui.button(format!("Use custom designation '{}'", query)).clicked() {
                                        chosen_material = Some(query.clone());
                                    }
                                }
                                ui.add_space(20.0);
                            });
                        }
                    });
            });

            // -------------------------------------------------------------------
            // 5. Selected Glass Physical Details Footer
            // -------------------------------------------------------------------
            let highlighted = state.highlighted_glass.clone().unwrap_or_else(|| "AIR".to_string());
            let (details_title, details_sub) = if highlighted == "AIR" {
                (
                    "AIR (Free Space / Vacuum)".to_string(),
                    "Refractive index nd = 1.000000 | Non-dispersive | Free transmission".to_string(),
                )
            } else if let Some(g) = catalog.lookup(&highlighted) {
                let therm = if g.thermal.is_some() { "Available" } else { "Standard 20°C" };
                (
                    format!("{}: {} (Catalog: {})", g.catalog.name(), g.name, g.catalog.name()),
                    format!(
                        "nd = {:.5} | Vd = {:.2} | Formula: {} | Thermal dn/dT: {}",
                        g.nd,
                        g.vd,
                        match g.dispersion {
                            optics_glass::DispersionModel::Sellmeier1 { .. } => "Sellmeier-1",
                            optics_glass::DispersionModel::Sellmeier2 { .. } => "Sellmeier-2",
                            optics_glass::DispersionModel::Cauchy { .. } => "Cauchy",
                            optics_glass::DispersionModel::ExtendedCauchy { .. } => "Extended Cauchy",
                            _ => "Empirical Formula",
                        },
                        therm
                    ),
                )
            } else {
                (
                    format!("Custom Material: '{}'", highlighted),
                    "User-defined material (not tabulated in standard vendor catalogs)".to_string(),
                )
            };

            Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(RichText::new(details_title).strong().size(11.5).color(tokens.text_primary));
                        ui.label(RichText::new(details_sub).size(10.5).color(tokens.text_muted));
                    });
                });

            // -------------------------------------------------------------------
            // 6. Action Buttons Bar
            // -------------------------------------------------------------------
            ui.horizontal(|ui| {
                // Set to Air button
                if ui.button(RichText::new("Set to AIR").size(11.0)).on_hover_text("Quickly assign AIR medium").clicked() {
                    chosen_material = Some("AIR".to_string());
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Apply Button
                    let apply_btn = Button::new(
                        RichText::new(format!("Apply Material ('{}')", highlighted))
                            .strong()
                            .size(11.0)
                            .color(Color32::WHITE),
                    )
                    .fill(tokens.accent)
                    .corner_radius(CornerRadius::ZERO);

                    if ui.add(apply_btn).clicked() {
                        chosen_material = Some(highlighted);
                    }

                    // Cancel Button
                    if ui.button(RichText::new("Cancel").size(11.0)).clicked() {
                        action = GlassBrowserAction::Close;
                    }
                });
            });

            // Handle chosen material
            if let Some(mat) = chosen_material {
                if let Some(target_idx) = state.target_surface {
                    action = GlassBrowserAction::Apply {
                        surface_idx: target_idx,
                        material_name: mat,
                    };
                }
                state.open = false;
            }
        });

    if !is_open {
        state.open = false;
        action = GlassBrowserAction::Close;
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glass_browser_state_default() {
        let state = GlassBrowserState::default();
        assert!(!state.open);
        assert_eq!(state.target_surface, None);
        assert_eq!(state.active_filter, CatalogFilter::All);
        assert_eq!(state.highlighted_glass.as_deref(), Some("N-BK7"));
    }

    #[test]
    fn test_glass_browser_open_for_surface() {
        let mut state = GlassBrowserState::default();
        state.open_for_surface(3, "SF11");
        assert!(state.open);
        assert_eq!(state.target_surface, Some(3));
        assert_eq!(state.highlighted_glass.as_deref(), Some("SF11"));
    }
}
