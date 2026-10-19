//! Academic 646-Glass Materials Catalog panel component.
//!
//! Docked directly inside the Left Panel (adjacent to Lens Data & Optimizer):
//! - Zero floating modals in the center of the screen — the central viewport remains 100% visible.
//! - Clean, fixed-width engineering grid with rigid pixel dimensions that never jump or wiggle.
//! - Larger, high-legibility typography (11.5–12.5px) making full use of panel space.
//! - Top section begins immediately with Search & Manufacturer filter pills.
//! - Bottom section unifies Target Surface selection ComboBox, "Set to AIR", active glass details, and dispersion formula in one cohesive panel without overlap.
//! - Exact vendor totals from database: CDGM (259), OHARA (222), SCHOTT (158), INFRARED (7), Total: 646.

use egui::{
    pos2, Align2, Button, Color32, ComboBox, CornerRadius, FontId, Frame, Margin, RichText,
    ScrollArea, Sense, TextEdit, Ui, Vec2,
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
        // 1. Search Filter Box (Top of panel - clean & immediate)
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
        // 2. Exact Vendor Filter Buttons (CDGM: 259, OHARA: 222, SCHOTT: 158, INFRARED: 7)
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
        // 3. Filter Catalog Glasses
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
        // 4. Pixel-Perfect Table with Strictly FIXED Column Widths Spanning Edge-to-Edge
        // -------------------------------------------------------------------
        let table_frame = Frame::NONE
            .fill(tokens.bg_canvas)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::same(6));

        let mut glass_to_apply: Option<String> = None;

        table_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Compute rigid column widths that span 100% to the right edge
            let avail_w = ui.available_width();
            let scroll_margin = 14.0;
            let usable_w = (avail_w - scroll_margin).max(340.0);
            let spacing = 6.0;

            let col_action_w = 46.0;
            let col_vd_w = 52.0;
            let col_nd_w = 60.0;
            let col_vendor_w = 70.0;
            let col_name_w = (usable_w - (col_vendor_w + col_nd_w + col_vd_w + col_action_w + spacing * 4.0)).max(100.0);

            // Pinned Table Header (Always visible, strictly fixed columns)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(spacing, 0.0);

                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_name_w, 18.0), Sense::hover());
                ui.painter().text(
                    pos2(rect.min.x + 2.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "NAME",
                    FontId::monospace(11.5),
                    tokens.text_muted,
                );

                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vendor_w, 18.0), Sense::hover());
                ui.painter().text(
                    pos2(rect.min.x + 2.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "VENDOR",
                    FontId::monospace(11.5),
                    tokens.text_muted,
                );

                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_nd_w, 18.0), Sense::hover());
                ui.painter().text(
                    pos2(rect.min.x + 2.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "nd",
                    FontId::monospace(11.5),
                    tokens.text_muted,
                );

                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vd_w, 18.0), Sense::hover());
                ui.painter().text(
                    pos2(rect.min.x + 2.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "Vd",
                    FontId::monospace(11.5),
                    tokens.text_muted,
                );

                let (rect, _) = ui.allocate_exact_size(Vec2::new(col_action_w, 18.0), Sense::hover());
                ui.painter().text(
                    pos2(rect.center().x, rect.center().y),
                    Align2::CENTER_CENTER,
                    "ACT",
                    FontId::monospace(11.5),
                    tokens.text_muted,
                );
            });

            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                        // Quick AIR row if query matches or is empty
                        if query.is_empty() || "AIR".contains(&query) {
                            let is_current = current_material == "AIR";
                            let row_bg = if is_current {
                                tokens.bg_control_active
                            } else {
                                tokens.bg_canvas
                            };

                            Frame::NONE
                                .fill(row_bg)
                                .stroke(if is_current { tokens.border_hovered } else { egui::Stroke::NONE })
                                .inner_margin(Margin::symmetric(2, 2))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(spacing, 0.0);

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_name_w, 20.0), Sense::hover());
                                        let name_color = if is_current { tokens.accent } else { tokens.text_primary };
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            "AIR",
                                            FontId::monospace(12.0),
                                            name_color,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vendor_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            "Vacuum",
                                            FontId::proportional(11.5),
                                            tokens.text_muted,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_nd_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            "1.0000",
                                            FontId::monospace(11.5),
                                            tokens.text_primary,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vd_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            "—",
                                            FontId::monospace(11.5),
                                            tokens.text_muted,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_action_w, 20.0), Sense::hover());
                                        let btn = Button::new(
                                            RichText::new(if is_current { "Active" } else { "Set" })
                                                .size(11.0)
                                                .strong()
                                                .color(if is_current { tokens.accent } else { tokens.text_primary }),
                                        )
                                        .fill(if is_current { tokens.bg_control_active } else { tokens.bg_control })
                                        .stroke(tokens.border)
                                        .corner_radius(CornerRadius::ZERO);

                                        if ui.put(rect, btn).clicked() {
                                            glass_to_apply = Some("AIR".to_string());
                                        }
                                    });
                                });
                        }

                        // Render all matched glasses with zebra striping
                        for (idx, g) in matches.iter().enumerate() {
                            let is_current = current_material == g.name;
                            let row_bg = if is_current {
                                tokens.bg_control_active
                            } else if idx % 2 == 1 {
                                tokens.bg_control
                            } else {
                                tokens.bg_canvas
                            };

                            let vendor_name = match g.catalog {
                                GlassCatalog::Cdgm => "CDGM",
                                GlassCatalog::Ohara => "OHARA",
                                GlassCatalog::Schott => "Schott",
                                _ => "Infrared",
                            };

                            Frame::NONE
                                .fill(row_bg)
                                .stroke(if is_current { tokens.border_hovered } else { egui::Stroke::NONE })
                                .inner_margin(Margin::symmetric(2, 2))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(spacing, 0.0);

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_name_w, 20.0), Sense::hover());
                                        let name_color = if is_current { tokens.accent } else { tokens.text_primary };
                                        let display_name = if g.name.len() > 14 {
                                            format!("{}…", &g.name[..13])
                                        } else {
                                            g.name.clone()
                                        };
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            display_name,
                                            FontId::monospace(12.0),
                                            name_color,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vendor_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            vendor_name,
                                            FontId::proportional(11.5),
                                            tokens.text_muted,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_nd_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            format!("{:.4}", g.nd),
                                            FontId::monospace(11.5),
                                            tokens.text_primary,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_vd_w, 20.0), Sense::hover());
                                        ui.painter().text(
                                            pos2(rect.min.x + 2.0, rect.center().y),
                                            Align2::LEFT_CENTER,
                                            format!("{:.1}", g.vd),
                                            FontId::monospace(11.5),
                                            tokens.text_primary,
                                        );

                                        let (rect, _) = ui.allocate_exact_size(Vec2::new(col_action_w, 20.0), Sense::hover());
                                        let btn_text = if is_current { "Active" } else { "Set" };
                                        let btn = Button::new(
                                            RichText::new(btn_text)
                                                .size(11.0)
                                                .strong()
                                                .color(if is_current { tokens.accent } else { tokens.text_primary }),
                                        )
                                        .fill(if is_current { tokens.bg_control_active } else { tokens.bg_control })
                                        .stroke(tokens.border)
                                        .corner_radius(CornerRadius::ZERO);

                                        if ui.put(rect, btn).clicked() {
                                            glass_to_apply = Some(g.name.clone());
                                        }
                                    });
                                });
                        }

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
        });

        // -------------------------------------------------------------------
        // 5. Unified Target Surface & Material Properties Inspector (All in One Card)
        // -------------------------------------------------------------------
        let info_frame = Frame::NONE
            .fill(tokens.bg_control)
            .stroke(tokens.border)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(14, 12));

        info_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 9.0);

                // Section 1: Target surface picker ComboBox + Set AIR button
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                    ui.label(RichText::new("Target:").strong().size(12.0).color(tokens.text_muted));

                    let selected_text = format!("Surface {} ({})", target_idx, surf_role);
                    let cb_resp = ComboBox::from_id_salt("unified_materials_target_cb")
                        .selected_text(RichText::new(selected_text).strong().size(11.5).color(tokens.text_primary))
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for (idx, surf) in prescription.surfaces.iter().enumerate() {
                                let role = if idx == 0 { "Object" } else if idx == total_surfs - 1 { "Image" } else if surf.is_stop { "Stop" } else { "Standard" };
                                let label = format!("Surface {} ({} : {})", idx, role, surf.material_name);
                                ui.selectable_value(&mut editor_state.materials_target_surface, idx, label);
                            }
                        });

                    let cb_height = cb_resp.response.rect.height();

                    let air_btn = Button::new(
                        RichText::new("Set to AIR")
                            .size(11.0)
                            .strong()
                            .color(if current_material == "AIR" { tokens.accent } else { tokens.text_primary }),
                    )
                    .fill(tokens.bg_canvas)
                    .stroke(tokens.border)
                    .corner_radius(CornerRadius::ZERO);

                    if ui.add_sized([72.0, cb_height], air_btn).on_hover_text("Assign AIR (Vacuum) to selected surface").clicked() {
                        if current_material != "AIR" {
                            prescription.surfaces[target_idx].material_name = "AIR".to_string();
                            modified = true;
                        }
                    }
                });

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Section 2: Active Glass Details with generous breathing room
                if current_material == "AIR" {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                        ui.label(RichText::new("Active:").strong().size(11.5).color(tokens.text_muted));
                        ui.label(RichText::new("AIR").monospace().strong().size(13.0).color(tokens.accent));
                        ui.label(RichText::new("(Vacuum / Free Space)").size(11.0).color(tokens.text_muted));
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        ui.label(RichText::new("Refractive Index:").size(11.0).color(tokens.text_muted));
                        ui.label(RichText::new("nd = 1.00000").monospace().strong().size(11.5).color(tokens.text_primary));
                    });
                } else if let Some(g) = catalog.lookup(&current_material) {
                    // Line 1: Glass Name & Catalog
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                        ui.label(RichText::new("Active:").strong().size(11.5).color(tokens.text_muted));
                        ui.label(RichText::new(&g.name).monospace().strong().size(13.5).color(tokens.accent));
                        ui.label(RichText::new(format!("[{}]", g.catalog.name())).size(11.0).color(tokens.text_muted));
                    });

                    // Line 2: Optical Constants (Index nd & Abbe number Vd spaced apart)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(20.0, 0.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(RichText::new("nd:").strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new(format!("{:.5}", g.nd)).monospace().strong().size(12.0).color(tokens.text_primary));
                        });

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(RichText::new("Vd:").strong().size(11.5).color(tokens.text_muted));
                            ui.label(RichText::new(format!("{:.2}", g.vd)).monospace().strong().size(12.0).color(tokens.text_primary));
                        });
                    });

                    let formula_desc = match g.dispersion {
                        optics_glass::DispersionModel::Sellmeier1 { .. } => "Sellmeier 1 (6-coef)",
                        optics_glass::DispersionModel::Sellmeier2 { .. } => "Sellmeier 2",
                        optics_glass::DispersionModel::Cauchy { .. } => "Cauchy",
                        optics_glass::DispersionModel::ExtendedCauchy { .. } => "Extended Cauchy",
                        _ => "Standard",
                    };
                    let thermal_desc = if g.thermal.is_some() { "Available (dn/dT)" } else { "20°C Reference" };

                    // Line 3: Mathematical Model & Thermal Status
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(RichText::new("Formula:").size(11.0).color(tokens.text_muted));
                            ui.label(RichText::new(formula_desc).size(11.0).color(tokens.text_primary));
                        });

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(RichText::new("Thermal:").size(11.0).color(tokens.text_muted));
                            ui.label(RichText::new(thermal_desc).size(11.0).color(tokens.text_primary));
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                        ui.label(RichText::new("Active:").strong().size(11.5).color(tokens.text_muted));
                        ui.label(RichText::new(&current_material).monospace().strong().size(13.0).color(tokens.warning));
                        ui.label(RichText::new("(Custom Material)").size(11.0).color(tokens.text_muted));
                    });
                    ui.label(RichText::new("Non-catalog material — custom dispersion model.").size(10.5).color(tokens.text_muted));
                }
            });
        });

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
