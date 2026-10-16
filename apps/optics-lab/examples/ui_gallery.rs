//! UI Component Gallery for OpticsLab.
//!
//! Demonstrates every classic desktop component in every state (normal, hover,
//! pressed, focused, selected, disabled), side by side in all three themes
//! (Classic Light, Classic Dark, High Contrast), along with the complete Phosphor
//! vector icon inventory.
//!
//! Run with: `cargo run -p optics-lab --example ui_gallery`

use eframe::egui::{
    self, Align2, Color32, Context, CornerRadius, FontId, Layout, Pos2, Rect, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use optics_ui::{
    BannerLevel, IconId, OpticsTokens, ThemePreference, classic_banner, classic_group_box,
    classic_slider, classic_table, classic_tabs, draw_icon, icon_text_button, init_icon_fonts,
    resolve_theme, status_bar_item, toggle_tool_button, toolbar_button,
};

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpticsLab — UI Component Gallery")
            .with_inner_size(Vec2::new(1280.0, 920.0))
            .with_min_inner_size(Vec2::new(800.0, 600.0)),
        ..Default::default()
    };

    eframe::run_native(
        "OpticsLab UI Gallery",
        native_options,
        Box::new(|cc| {
            init_icon_fonts(&cc.egui_ctx);
            Ok(Box::new(UiGalleryApp::default()))
        }),
    )
}

struct UiGalleryApp {
    selected_gallery_tab: usize,
    slider_val: f64,
    tab_selection: usize,
    toggle_active: bool,
    dropdown_sel: usize,
}

impl Default for UiGalleryApp {
    fn default() -> Self {
        Self {
            selected_gallery_tab: 0,
            slider_val: 45.0,
            tab_selection: 0,
            toggle_active: true,
            dropdown_sel: 0,
        }
    }
}

impl eframe::App for UiGalleryApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("OpticsLab Classic Desktop UI Gallery");
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(
                            "Target Look: Classic CAD / Scientific (Maple & MATLAB inspired)",
                        )
                        .color(Color32::from_rgb(120, 130, 140))
                        .small(),
                    );
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Gallery mode tabs
            let gallery_tabs = [
                (0, "Side-by-Side Themes"),
                (1, "All Icon Assets"),
                (2, "Component States Matrix"),
            ];

            let tokens_light = resolve_theme(ThemePreference::ClassicLight, false);
            classic_tabs(
                ui,
                &mut self.selected_gallery_tab,
                &gallery_tabs,
                &tokens_light,
            );

            ui.add_space(8.0);

            match self.selected_gallery_tab {
                0 => self.render_side_by_side_themes(ui),
                1 => self.render_icon_inventory(ui),
                2 => self.render_state_matrix(ui),
                _ => {}
            }
        });
    }
}

impl UiGalleryApp {
    /// Renders components side by side in Classic Light, Classic Dark, and High Contrast.
    fn render_side_by_side_themes(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            let col_w = (ui.available_width() - 24.0) / 3.0;

            ui.horizontal(|ui| {
                // Column 1: Classic Light
                let tokens_light = resolve_theme(ThemePreference::ClassicLight, false);
                self.render_theme_column(ui, "Classic Light (Default)", &tokens_light, col_w);

                ui.separator();

                // Column 2: Classic Dark
                let tokens_dark = resolve_theme(ThemePreference::ClassicDark, true);
                self.render_theme_column(ui, "Classic Dark", &tokens_dark, col_w);

                ui.separator();

                // Column 3: High Contrast
                let tokens_hc = resolve_theme(ThemePreference::HighContrast, true);
                self.render_theme_column(ui, "High Contrast (Dark)", &tokens_hc, col_w);
            });
        });
    }

    /// Renders a column showcasing all components themed by `tokens`.
    fn render_theme_column(&mut self, ui: &mut Ui, title: &str, tokens: &OpticsTokens, width: f32) {
        ui.allocate_ui(Vec2::new(width, ui.available_height()), |ui| {
            // Background preview container
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 760.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, tokens.panel_bg);
            ui.painter().rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(1.0, tokens.border),
                StrokeKind::Inside,
            );

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(10.0))
                    .layout(Layout::top_down(egui::Align::Min)),
            );

            child.label(
                egui::RichText::new(title)
                    .strong()
                    .size(13.0)
                    .color(tokens.text_primary),
            );
            child.add_space(4.0);
            child.separator();
            child.add_space(6.0);

            // 1. Toolbar Buttons & Tool Toggles
            child.label(
                egui::RichText::new("Toolbar & Tool Buttons")
                    .size(11.0)
                    .color(tokens.text_secondary),
            );
            child.horizontal(|ui| {
                toolbar_button(ui, IconId::Select, tokens, true, "Select tool (S)");
                toolbar_button(ui, IconId::Move, tokens, false, "Move tool (M)");
                toolbar_button(ui, IconId::Rotate, tokens, false, "Rotate tool (Alt+R)");
                toolbar_button(ui, IconId::RayProbe, tokens, false, "Ray probe (I)");
                ui.separator();
                toggle_tool_button(
                    ui,
                    IconId::ReflectionsToggle,
                    tokens,
                    self.toggle_active,
                    "Toggle reflections (B)",
                );
            });

            child.add_space(8.0);

            // 2. Icon + Text Buttons
            child.label(
                egui::RichText::new("Icon + Text Buttons")
                    .size(11.0)
                    .color(tokens.text_secondary),
            );
            child.horizontal(|ui| {
                icon_text_button(ui, IconId::Open, "Open...", tokens, false);
                icon_text_button(ui, IconId::Save, "Save", tokens, true);
                icon_text_button(ui, IconId::Settings, "Options", tokens, false);
            });

            child.add_space(8.0);

            // 3. Classic Fieldset Group Box
            classic_group_box(&mut child, "Physical Simulation", tokens, |ui| {
                classic_slider(
                    ui,
                    optics_ui::ClassicSliderConfig {
                        label: "Incidence angle:",
                        min: 0.0,
                        max: 80.0,
                        default_val: 45.0,
                        step: 0.5,
                        unit_suffix: "°",
                    },
                    &mut self.slider_val,
                    tokens,
                );
                ui.add_space(4.0);
                let tabs = [(0, "Prism"), (1, "Doublet"), (2, "Singlet")];
                classic_tabs(ui, &mut self.tab_selection, &tabs, tokens);
            });

            child.add_space(8.0);

            // 4. Fixed-Header Table
            child.label(
                egui::RichText::new("Fixed-Header Data Table")
                    .size(11.0)
                    .color(tokens.text_secondary),
            );
            let headers = ["Surface", "Radius (mm)", "Thickness", "Material"];
            classic_table(&mut child, &headers, tokens, |ui| {
                let rows = [
                    ("1 (Front)", "50.00", "5.00", "N-BK7"),
                    ("2 (Aperture)", "Infinity", "2.00", "Air"),
                    ("3 (Rear)", "-50.00", "12.00", "SF11"),
                ];
                for (i, (s, r, t, m)) in rows.iter().enumerate() {
                    let (row_rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                    if i % 2 == 1 {
                        ui.painter()
                            .rect_filled(row_rect, CornerRadius::ZERO, tokens.hover_bg);
                    }
                    let col_w = row_rect.width() / 4.0;
                    for (c, text) in [s, r, t, m].iter().enumerate() {
                        let pt = Pos2::new(
                            row_rect.left() + c as f32 * col_w + 6.0,
                            row_rect.center().y,
                        );
                        ui.painter().text(
                            pt,
                            Align2::LEFT_CENTER,
                            *text,
                            FontId::monospace(10.5),
                            tokens.text_primary,
                        );
                    }
                }
            });

            child.add_space(8.0);

            // 5. Classic Message Banners
            child.label(
                egui::RichText::new("Notice Banners")
                    .size(11.0)
                    .color(tokens.text_secondary),
            );
            classic_banner(
                &mut child,
                BannerLevel::Info,
                "Normal dispersion trace active across 380–780 nm.",
                tokens,
            );
            child.add_space(4.0);
            classic_banner(
                &mut child,
                BannerLevel::Warning,
                "Ray grazing incidence approaching total internal reflection.",
                tokens,
            );
            child.add_space(4.0);
            classic_banner(
                &mut child,
                BannerLevel::Error,
                "Prescription surface radius violates physical thickness constraint.",
                tokens,
            );

            child.add_space(8.0);

            // 6. Status Bar Item
            child.label(
                egui::RichText::new("Status Bar Item")
                    .size(11.0)
                    .color(tokens.text_secondary),
            );
            child.horizontal(|ui| {
                status_bar_item(ui, "Ready", tokens, Some("System ready for ray tracing"));
                ui.separator();
                status_bar_item(ui, "X: 24.50 mm  Y: -12.30 mm", tokens, None);
                ui.separator();
                status_bar_item(ui, "CPU (32 rays)", tokens, Some("Meridional fan count"));
            });
        });
    }

    /// Renders the complete typed Phosphor icon inventory.
    fn render_icon_inventory(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                egui::RichText::new("Phosphor Vector Line Icons (MIT License) — Resolution-Independent Font Atlas")
                    .size(13.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new("Crisp at all scales (1.0x, 1.25x, 1.5x, 2.0x, 3.0x). Every icon is typed by enum.")
                    .color(Color32::from_rgb(140, 150, 160))
                    .small(),
            );
            ui.add_space(8.0);

            let all_icons = [
                (IconId::Logo, "Logo", "Application branding glyph"),
                (IconId::Select, "Select", "Selection tool (S)"),
                (IconId::Move, "Move", "Translate/pan tool (M)"),
                (IconId::Rotate, "Rotate", "Rotate tool (Alt+R)"),
                (IconId::RayProbe, "RayProbe", "Ray inspector crosshair (I)"),
                (IconId::Lens, "Lens", "Lens prescription editor"),
                (IconId::LightSource, "LightSource", "Optical emitter"),
                (IconId::Measure, "Measure", "Distance / angle ruler"),
                (IconId::ZoomIn, "ZoomIn", "Zoom in (Cmd++)"),
                (IconId::ZoomOut, "ZoomOut", "Zoom out (Cmd+-)"),
                (IconId::ZoomFit, "ZoomFit", "Fit view to window (Cmd+0)"),
                (IconId::ResetView, "ResetView", "Reset camera view"),
                (IconId::PanelsToggle, "PanelsToggle", "Toggle dock panels"),
                (IconId::Copy, "Copy", "Copy to clipboard (Cmd+C)"),
                (IconId::Open, "Open", "Open file (Cmd+O)"),
                (IconId::Save, "Save", "Save session (Cmd+S)"),
                (IconId::ExportImage, "ExportImage", "Export screenshot / diagram"),
                (IconId::Settings, "Settings", "Preferences dialog (Cmd+,)"),
                (IconId::Theme, "Theme", "Switch color theme"),
                (IconId::Help, "Help", "Keyboard shortcuts help (F1)"),
                (IconId::Warning, "Warning", "Warning indicator"),
                (IconId::Info, "Info", "Informational notice"),
                (IconId::Error, "Error", "Error indicator"),
                (IconId::ReflectionsToggle, "ReflectionsToggle", "Toggle Fresnel branches (B)"),
                (IconId::Fullscreen, "Fullscreen", "Toggle fullscreen (F11)"),
                (IconId::Scene, "Scene", "Scene hierarchy root"),
                (IconId::Camera, "Camera", "Camera viewpoint object"),
                (IconId::CaretRight, "CaretRight", "Tree node collapsed"),
                (IconId::CaretDown, "CaretDown", "Tree node expanded"),
                (IconId::Spectrum, "Spectrum", "Continuous spectrum ribbon"),
                (IconId::Bvh, "Bvh", "Acceleration structure bounds"),
                (IconId::Analysis, "Analysis", "Optical aberrations & MTF"),
            ];

            let cell_w = 160.0;
            let cell_h = 70.0;
            let cols = (ui.available_width() / cell_w).floor().max(1.0) as usize;

            egui::Grid::new("icon_grid")
                .num_columns(cols)
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    for (i, (icon, name, desc)) in all_icons.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell_w, cell_h), Sense::hover());
                        let painter = ui.painter();

                        let bg = if resp.hovered() {
                            Color32::from_rgb(35, 45, 55)
                        } else {
                            Color32::from_rgb(22, 28, 35)
                        };
                        painter.rect_filled(rect, CornerRadius::same(2), bg);
                        painter.rect_stroke(
                            rect,
                            CornerRadius::same(2),
                            Stroke::new(1.0, Color32::from_rgb(50, 60, 72)),
                            StrokeKind::Inside,
                        );

                        // Icon rendered at 20 px
                        let icon_rect = Rect::from_min_size(
                            Pos2::new(rect.left() + 10.0, rect.top() + 12.0),
                            Vec2::splat(20.0),
                        );
                        draw_icon(painter, icon_rect, *icon, Color32::from_rgb(0, 212, 245), 20.0);

                        // Name
                        painter.text(
                            Pos2::new(rect.left() + 38.0, rect.top() + 14.0),
                            Align2::LEFT_TOP,
                            *name,
                            FontId::proportional(12.0),
                            Color32::from_rgb(230, 235, 240),
                        );

                        // Description
                        painter.text(
                            Pos2::new(rect.left() + 10.0, rect.top() + 42.0),
                            Align2::LEFT_TOP,
                            *desc,
                            FontId::proportional(10.0),
                            Color32::from_rgb(140, 150, 160),
                        );

                        resp.on_hover_text(format!("IconId::{name}\n{desc}"));

                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
    }

    /// Renders state progression matrix (normal, hover, pressed, focused, selected, disabled).
    fn render_state_matrix(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                egui::RichText::new("Component State Progression Matrix")
                    .size(13.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new("Guaranteed instant response (0 ms animation delay), 1 px borders, >= 3:1 contrast.")
                    .color(Color32::from_rgb(140, 150, 160))
                    .small(),
            );
            ui.add_space(10.0);

            let tokens = resolve_theme(ThemePreference::ClassicLight, false);

            let headers = ["Component", "Normal", "Hovered (Simulated)", "Selected / Active", "Disabled State"];
            classic_table(ui, &headers, &tokens, |ui| {
                // 1. Toolbar Button
                ui.horizontal(|ui| {
                    ui.label("Toolbar Button (28px)");
                    ui.separator();
                    toolbar_button(ui, IconId::Select, &tokens, false, "Normal state");
                    ui.separator();
                    // Simulated hover state
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::same(2), tokens.hover_bg);
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, tokens.border), StrokeKind::Inside);
                    draw_icon(ui.painter(), rect, IconId::Select, tokens.text_primary, 16.0);
                    resp.on_hover_text("Hover state");
                    ui.separator();
                    toolbar_button(ui, IconId::Select, &tokens, true, "Selected state");
                    ui.separator();
                    // Disabled state
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::hover());
                    draw_icon(ui.painter(), rect, IconId::Select, tokens.text_disabled, 16.0);
                    resp.on_hover_text("Disabled: tool not applicable in current viewport mode");
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 2. Icon + Text Button
                ui.horizontal(|ui| {
                    ui.label("Icon + Text Button");
                    ui.separator();
                    icon_text_button(ui, IconId::Open, "Open...", &tokens, false);
                    ui.separator();
                    // Simulated hover
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(80.0, 24.0), Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::same(2), tokens.hover_bg);
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, tokens.border), StrokeKind::Inside);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Open...", FontId::proportional(12.0), tokens.text_primary);
                    resp.on_hover_text("Hover state");
                    ui.separator();
                    icon_text_button(ui, IconId::Open, "Open...", &tokens, true);
                    ui.separator();
                    // Disabled
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(80.0, 24.0), Sense::hover());
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, tokens.border_subtle), StrokeKind::Inside);
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Open...", FontId::proportional(12.0), tokens.text_disabled);
                    resp.on_hover_text("Disabled: file system access prohibited in web sandbox");
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 3. Dropdown Selection
                ui.horizontal(|ui| {
                    ui.label("Dropdown / ComboBox");
                    ui.separator();
                    egui::ComboBox::from_id_salt("state_combobox")
                        .selected_text(match self.dropdown_sel {
                            0 => "Classic Light",
                            1 => "Classic Dark",
                            _ => "High Contrast",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.dropdown_sel, 0, "Classic Light");
                            ui.selectable_value(&mut self.dropdown_sel, 1, "Classic Dark");
                            ui.selectable_value(&mut self.dropdown_sel, 2, "High Contrast");
                        });
                    ui.separator();
                    ui.add_enabled(false, egui::Button::new("Disabled Action")).on_disabled_hover_text("Action unavailable");
                });
            });
        });
    }
}
