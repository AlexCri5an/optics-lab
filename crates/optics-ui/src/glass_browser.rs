//! Glass Catalog Browser side panel and material inspector.

use egui::{ScrollArea, Ui};
use optics_glass::{DispersionModel, GlassCatalogDb, GlassMaterial};

use crate::theme::OpticsTokens;
use crate::widgets::{abbe_diagram, dispersion_curve_plot};

/// Interactive state for the Glass Catalog Browser panel.
#[derive(Debug, Clone)]
pub struct GlassBrowserState {
    /// Text filter query for glass names.
    pub search_query: String,
    /// Currently inspected glass name.
    pub selected_glass: String,
    /// Whether the Abbe diagram scatter plot is expanded.
    pub show_abbe_diagram: bool,
    /// Whether the dispersion curve plot is expanded.
    pub show_dispersion_curve: bool,
}

impl Default for GlassBrowserState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_glass: "N-BK7".to_string(),
            show_abbe_diagram: true,
            show_dispersion_curve: true,
        }
    }
}

/// Renders the Glass Catalog Browser panel.
pub fn render_glass_catalog_panel(
    ui: &mut Ui,
    catalog: &GlassCatalogDb,
    state: &mut GlassBrowserState,
    tokens: &OpticsTokens,
) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);

        // Header: Search box
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.search_query);
        });

        ui.add_space(6.0);

        // Selected glass quick summary
        if let Some(glass) = catalog.lookup(&state.selected_glass) {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(&glass.name);
                    ui.label(format!("({})", glass.catalog.name()));
                });

                ui.horizontal(|ui| {
                    ui.label(format!("nd = {:.4}", glass.nd));
                    ui.label(format!("Vd = {:.2}", glass.vd));
                });

                if let DispersionModel::Sellmeier1 { b, c } = &glass.dispersion {
                    ui.collapsing("Sellmeier Coefficients", |ui| {
                        ui.monospace(format!("B1: {:.6e}  C1: {:.6e}", b[0], c[0]));
                        ui.monospace(format!("B2: {:.6e}  C2: {:.6e}", b[1], c[1]));
                        ui.monospace(format!("B3: {:.6e}  C3: {:.6e}", b[2], c[2]));
                    });
                }
            });

            ui.add_space(8.0);

            // Live n(lambda) dispersion curve plot
            ui.collapsing("Dispersion Curve n(λ)", |ui| {
                dispersion_curve_plot(ui, glass, tokens, 120.0);
            });
        }

        ui.add_space(8.0);

        // Abbe Diagram
        ui.collapsing("Abbe Diagram (Vd vs nd)", |ui| {
            abbe_diagram(ui, catalog, Some(&state.selected_glass), tokens, 160.0);
        });

        ui.add_space(8.0);

        // Glass Table List
        ui.label("Catalog Glasses:");
        let query_lower = state.search_query.trim().to_lowercase();

        let filtered_glasses: Vec<&GlassMaterial> = catalog
            .all_glasses()
            .filter(|g| query_lower.is_empty() || g.name.to_lowercase().contains(&query_lower))
            .take(100) // Display top 100 matches
            .collect();

        egui::Grid::new("glass_catalog_grid")
            .striped(true)
            .min_col_width(50.0)
            .show(ui, |ui| {
                ui.label("Name");
                ui.label("Catalog");
                ui.label("nd");
                ui.label("Vd");
                ui.end_row();

                for g in filtered_glasses {
                    let is_selected = g.name == state.selected_glass;
                    if ui.selectable_label(is_selected, &g.name).clicked() {
                        state.selected_glass = g.name.clone();
                    }
                    ui.label(g.catalog.name());
                    ui.label(format!("{:.4}", g.nd));
                    ui.label(format!("{:.2}", g.vd));
                    ui.end_row();
                }
            });
    });
}
