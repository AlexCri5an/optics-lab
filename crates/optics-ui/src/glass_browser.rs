//! Academic 646-Glass Materials Catalog panel component.
//!
//! Docked directly inside the Left Panel (adjacent to Lens Data & Optimizer):
//! - Zero floating modals in the center of the screen — the central viewport remains 100% visible.
//! - Clean surface switcher ComboBox with integrated "Set AIR" quick button.
//! - Larger, high-legibility typography (11.5–12px) making full use of panel space.
//! - Strict pixel-aligned engineering grid (`egui::Grid`) with generous spacing.
//! - Exact vendor totals from database: CDGM (259), OHARA (222), SCHOTT (158), INFRARED (7), Total: 646.
//! - Instant 1-click [Set] assignment with live raytrace feedback.

use egui::{
    Align, Button, Color32, ComboBox, CornerRadius, Frame, Grid, Layout, Margin, RichText,
    ScrollArea, TextEdit, Ui, Vec2,
};
use optics_core::OpticalPrescription;
use optics_glass::{GlassCatalog, GlassCatalogDb, GlassMaterial};

use crate::lens_editor::{COMMON_OPTICAL_GLASSES, LensEditorState};
use crate::theme::OpticsTokens;

/// Selected manufacturer filter for the Materials panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaterialVendorFilter {
    /// Show all 646 glasses.
    #[default]
    All,
    /// 17 common standard glasses + AIR.
    Common,
    /// CDGM Optical Glass (China, 259 glasses).
    Cdgm,
    /// Ohara Corporation (Japan, 222 glasses).
    Ohara,
    /// Schott Glass (Germany, 158 glasses).
    Schott,
    /// Infrared crystals & other vendors (Vitron 5, AMI 2 = 7 glasses).
    InfraredOther,
}

/// Renders the docked Materials Catalog panel inside the left sidebar.
///
/// Returns `true` if the optical prescription was modified this frame.
pub fn render_materials_panel(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    editor_state: &mut LensEditorState,
    catalog: &GlassCatalogDb,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    // Ensure target surface index is within valid range
    let total_surfs = prescription.surfaces.len();
    if total_surfs == 0 {
        ui.label(RichText::new("No optical surfaces in prescription.").size(12.0).color(tokens.text_muted));
        return false;
    }

    if editor_state.materials_target_surface >= total_surfs {
        editor_state.materials_target_surface = total_surfs.saturating_sub(1);
    }

    let target_idx = editor_state.materials_target_surface;
    let current_material = prescription.surfaces[target_idx].material_name.clone();

    let surf_role = if target_idx == 0 {
        "Object"
    } else if target_idx == total_surfs - 1 {
        "Image"
    } else if prescription.surfaces[target_idx].is_stop {
        "Stop"
    } else {
        "Standard"
    };

    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);

        // -------------------------------------------------------------------
        // 1. Target Surface Switcher Card & Integrated Set AIR Button
        // -------------------------------------------------------------------
        let card_frame = Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(12, 10));

        card_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                ui.label(
                    RichText::new("Target:")
                        .strong()
                        .size(12.0)
                        .color(tokens.text_muted),
                );

                // Clean Surface Dropdown ComboBox (replaces clumsy < and > buttons)
                let selected_text = format!("Surface {} ({})", target_idx, surf_role);
                ComboBox::from_id_salt("materials_target_surf_cb")
                    .selected_text(RichText::new(selected_text).strong().size(12.0).color(tokens.text_primary))
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for (idx, surf) in prescription.surfaces.iter().enumerate() {
                            let role = if idx == 0 {
                                "Object"
                            } else if idx == total_surfs - 1 {
                                "Image"
                            } else if surf.is_stop {
                                "Stop"
                            } else {
                                "Standard"
                            };
                            let label = format!("Surface {} ({}) : {}", idx, role, surf.material_name);
                            ui.selectable_value(&mut editor_state.materials_target_surface, idx, label);
                        }
                    });

                // Right aligned current material and Set AIR button
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let air_btn = Button::new(
                        RichText::new("Set AIR")
                            .size(11.5)
                            .strong()
                            .color(if current_material == "AIR" { tokens.accent } else { tokens.text_primary }),
                    )
                    .fill(tokens.bg_canvas)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO);

                    if ui.add(air_btn).on_hover_text("Assign AIR (Vacuum) medium to selected surface").clicked() {
                        if current_material != "AIR" {
                            prescription.surfaces[target_idx].material_name = "AIR".to_string();
                            modified = true;
                        }
                    }

                    ui.label(
                        RichText::new(format!("Current: {}", current_material))
                            .monospace()
                            .strong()
                            .size(12.0)
                            .color(tokens.accent),
                    );
                });
            });
        });

        // -------------------------------------------------------------------
        // 2. Search Filter Box (Larger font, comfortable width)
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            ui.label(RichText::new("Search:").strong().size(12.0).color(tokens.text_muted));

            let search_edit = TextEdit::singleline(&mut editor_state.glass_search_query)
                .font(egui::TextStyle::Monospace)
                .hint_text("Filter by name (e.g. BK7, SF11, F2)...")
                .desired_width(ui.available_width() - 55.0);

            ui.add(search_edit);

            if !editor_state.glass_search_query.is_empty() && ui.button("x").on_hover_text("Clear search").clicked() {
                editor_state.glass_search_query.clear();
            }
        });

        // -------------------------------------------------------------------
        // 3. Exact Vendor Filter Buttons (CDGM: 259, OHARA: 222, SCHOTT: 158, INFRARED: 7)
        // -------------------------------------------------------------------
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

            let filters = [
                (MaterialVendorFilter::All, format!("ALL ({})", catalog.len())),
                (MaterialVendorFilter::Common, format!("COMMON ({})", COMMON_OPTICAL_GLASSES.len())),
                (MaterialVendorFilter::Cdgm, "CDGM (259)".to_string()),
                (MaterialVendorFilter::Ohara, "OHARA (222)".to_string()),
                (MaterialVendorFilter::Schott, "SCHOTT (158)".to_string()),
                (MaterialVendorFilter::InfraredOther, "INFRARED (7)".to_string()),
            ];

            for (flt, label) in filters {
                let is_active = editor_state.materials_filter == flt;
                let btn = Button::new(
                    RichText::new(label)
                        .size(10.5)
                        .strong()
                        .color(if is_active { Color32::WHITE } else { tokens.text_muted }),
                )
                .fill(if is_active { tokens.accent } else { tokens.bg_control })
                .stroke(if is_active { tokens.border_hovered } else { tokens.border })
                .corner_radius(CornerRadius::ZERO);

                if ui.add(btn).clicked() {
                    editor_state.materials_filter = flt;
                }
            }
        });

        // -------------------------------------------------------------------
        // 4. Filter Catalog Glasses
        // -------------------------------------------------------------------
        let query = editor_state.glass_search_query.trim().to_uppercase();

        let mut matches: Vec<&GlassMaterial> = catalog
            .all_glasses()
            .filter(|g| {
                let matches_vendor = match editor_state.materials_filter {
                    MaterialVendorFilter::All => true,
                    MaterialVendorFilter::Common => COMMON_OPTICAL_GLASSES.contains(&g.name.as_str()),
                    MaterialVendorFilter::Cdgm => g.catalog == GlassCatalog::Cdgm,
                    MaterialVendorFilter::Ohara => g.catalog == GlassCatalog::Ohara,
                    MaterialVendorFilter::Schott => g.catalog == GlassCatalog::Schott,
                    MaterialVendorFilter::InfraredOther => {
                        g.catalog != GlassCatalog::Cdgm
                            && g.catalog != GlassCatalog::Ohara
                            && g.catalog != GlassCatalog::Schott
                    }
                };

                let matches_query = query.is_empty() || g.name.to_uppercase().contains(&query);

                matches_vendor && matches_query
            })
            .collect();

        matches.sort_by_key(|g| &g.name);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Showing {} glasses", matches.len()))
                    .size(11.0)
                    .color(tokens.text_muted),
            );
        });

        // -------------------------------------------------------------------
        // 5. Pixel-Perfect Engineering Grid Table (Guaranteed Header & Row Alignment, Larger Font)
        // -------------------------------------------------------------------
        let table_frame = Frame::NONE
            .fill(tokens.bg_canvas)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::same(6));

        let mut glass_to_apply: Option<String> = None;

        table_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(380.0)
                .show(ui, |ui| {
                    Grid::new("materials_catalog_grid")
                        .num_columns(5)
                        .striped(true)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            // Header Row (Larger font, high contrast)
                            ui.label(RichText::new("NAME").monospace().strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new("VENDOR").monospace().strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new("nd").monospace().strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new("Vd").monospace().strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new("ACTION").monospace().strong().size(11.5).color(tokens.text_muted));
                            ui.end_row();

                            // Quick AIR row if search matches
                            if query.is_empty() || "AIR".contains(&query) {
                                let is_current = current_material == "AIR";
                                let name_color = if is_current { tokens.accent } else { tokens.text_primary };

                                ui.label(RichText::new("AIR").monospace().strong().size(12.0).color(name_color));
                                ui.label(RichText::new("Vacuum").size(11.5).color(tokens.text_muted));
                                ui.label(RichText::new("1.0000").monospace().size(11.5).color(tokens.text_primary));
                                ui.label(RichText::new("—").monospace().size(11.5).color(tokens.text_muted));

                                let btn = Button::new(
                                    RichText::new(if is_current { "Active" } else { "Set" })
                                        .size(11.0)
                                        .strong()
                                        .color(if is_current { tokens.accent } else { tokens.text_primary }),
                                )
                                .min_size(Vec2::new(42.0, 20.0))
                                .fill(if is_current { tokens.bg_control_active } else { tokens.bg_control })
                                .stroke(tokens.border)
                                .corner_radius(CornerRadius::ZERO);

                                if ui.add(btn).clicked() {
                                    glass_to_apply = Some("AIR".to_string());
                                }
                                ui.end_row();
                            }

                            // Render all matched glasses
                            for g in &matches {
                                let is_current = current_material == g.name;
                                let name_color = if is_current { tokens.accent } else { tokens.text_primary };

                                let vendor_name = match g.catalog {
                                    GlassCatalog::Cdgm => "CDGM",
                                    GlassCatalog::Ohara => "OHARA",
                                    GlassCatalog::Schott => "Schott",
                                    _ => "Infrared",
                                };

                                ui.label(RichText::new(&g.name).monospace().strong().size(12.0).color(name_color));
                                ui.label(RichText::new(vendor_name).size(11.5).color(tokens.text_muted));
                                ui.label(RichText::new(format!("{:.4}", g.nd)).monospace().size(11.5).color(tokens.text_primary));
                                ui.label(RichText::new(format!("{:.1}", g.vd)).monospace().size(11.5).color(tokens.text_primary));

                                let btn_text = if is_current { "Active" } else { "Set" };
                                let btn = Button::new(
                                    RichText::new(btn_text)
                                        .size(11.0)
                                        .strong()
                                        .color(if is_current { tokens.accent } else { tokens.text_primary }),
                                )
                                .min_size(Vec2::new(42.0, 20.0))
                                .fill(if is_current { tokens.bg_control_active } else { tokens.bg_control })
                                .stroke(tokens.border)
                                .corner_radius(CornerRadius::ZERO);

                                if ui.add(btn).clicked() {
                                    glass_to_apply = Some(g.name.clone());
                                }
                                ui.end_row();
                            }
                        });

                    if matches.is_empty() && query != "AIR" {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("No catalog glass matches query.").size(11.5).color(tokens.text_muted));
                            if !query.is_empty() {
                                if ui.button(format!("Use custom name '{}'", query)).clicked() {
                                    glass_to_apply = Some(query.clone());
                                }
                            }
                            ui.add_space(20.0);
                        });
                    }
                });
        });

        // -------------------------------------------------------------------
        // 6. Selected Glass Inspector Footer (Larger font)
        // -------------------------------------------------------------------
        if let Some(g) = catalog.lookup(&current_material) {
            let info_frame = Frame::NONE
                .fill(tokens.bg_control)
                .stroke(tokens.border)
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(Margin::symmetric(12, 8));

            info_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("Active Glass: {} ({})", g.name, g.catalog.name()))
                            .strong()
                            .size(12.0)
                            .color(tokens.text_primary),
                    );
                    ui.label(
                        RichText::new(format!(
                            "nd = {:.5} | Vd = {:.2} | Model: {}",
                            g.nd,
                            g.vd,
                            match g.dispersion {
                                optics_glass::DispersionModel::Sellmeier1 { .. } => "Sellmeier 1 (6-coef)",
                                optics_glass::DispersionModel::Sellmeier2 { .. } => "Sellmeier 2",
                                optics_glass::DispersionModel::Cauchy { .. } => "Cauchy",
                                optics_glass::DispersionModel::ExtendedCauchy { .. } => "Extended Cauchy",
                                _ => "Standard",
                            }
                        ))
                        .size(11.0)
                        .color(tokens.text_muted),
                    );
                });
            });
        }

        // Apply chosen glass
        if let Some(glass_name) = glass_to_apply {
            if prescription.surfaces[target_idx].material_name != glass_name {
                prescription.surfaces[target_idx].material_name = glass_name;
                modified = true;
            }
        }
    });

    modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_vendor_filter_default() {
        let flt = MaterialVendorFilter::default();
        assert_eq!(flt, MaterialVendorFilter::All);
    }

    #[test]
    fn test_catalog_exact_vendor_counts() {
        let db = GlassCatalogDb::default();
        assert_eq!(db.len(), 646);

        let mut cdgm = 0;
        let mut ohara = 0;
        let mut schott = 0;
        let mut other = 0;

        for g in db.all_glasses() {
            match g.catalog {
                GlassCatalog::Cdgm => cdgm += 1,
                GlassCatalog::Ohara => ohara += 1,
                GlassCatalog::Schott => schott += 1,
                _ => other += 1,
            }
        }

        assert_eq!(cdgm, 259);
        assert_eq!(ohara, 222);
        assert_eq!(schott, 158);
        assert_eq!(other, 7);
        assert_eq!(cdgm + ohara + schott + other, 646);
    }
}
