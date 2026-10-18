//! Academic Lens Data Editor (LDE) component.
//!
//! Provides a dense, high-precision technical surface editor:
//! - Strictly zero ASCII pseudo-icons (uses real Lucide vector icons).
//! - Direct, non-dragging numerical input boxes (pure text edit).
//! - Compact 2-row surface card architecture designed for ~330px width.
//! - Glass catalog dropdown supporting both classic and ISO designations.
//! - Clean, neutral selection with zero permanent hover highlight.

use egui::{
    Align, Button, ComboBox, CornerRadius, DragValue, Frame, Id, Layout, Margin, RichText,
    ScrollArea, TextEdit, Ui, Vec2,
};
use optics_core::{OpticalPrescription, OpticalSurface};

use crate::icons::{LucideIcon, lucide_small_icon_button, lucide_text_button, render_lucide_icon};
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

/// Persistent UI state for the Lens Data Editor and System Explorer.
#[derive(Debug, Clone)]
pub struct LensEditorState {
    /// Currently selected surface index.
    pub selected_surface: Option<usize>,
    /// Whether the System Explorer (Aperture, Fields, Wavelengths) section is expanded.
    pub system_explorer_open: bool,
    /// New field angle input buffer (string for typing, e.g. "20.0").
    pub new_field_buf: String,
    /// New wavelength input buffer (string for typing, e.g. "532.0").
    pub new_wav_buf: String,
}

impl Default for LensEditorState {
    fn default() -> Self {
        Self {
            selected_surface: None,
            system_explorer_open: false,
            new_field_buf: "20.0".to_string(),
            new_wav_buf: "532.0".to_string(),
        }
    }
}

/// Renders the complete academic Lens Data Editor panel.
///
/// Returns `true` if any surface parameter was edited this frame.
pub fn render_lens_editor(
    ui: &mut Ui,
    prescription: &mut OpticalPrescription,
    state: &mut LensEditorState,
    tokens: &OpticsTokens,
) -> bool {
    let mut modified = false;

    ui.vertical(|ui| {
        // -------------------------------------------------------------------
        // 1. Technical System Specification Card & System Explorer
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
                };

                if insert_at < prescription.surfaces.len() {
                    prescription.surfaces.insert(insert_at, new_surf);
                } else {
                    prescription.surfaces.push(new_surf);
                }
                reindex_surfaces(prescription);
                state.selected_surface = None;
                modified = true;
            }

            // Surface counter
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                        &mut surf_to_remove,
                        &mut surf_to_toggle_stop,
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
    surf_to_remove: &mut Option<usize>,
    surf_to_toggle_stop: &mut Option<usize>,
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
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                        render_lucide_icon(ui, LucideIcon::Target, 11.0, tokens.warning);
                        ui.label(
                            RichText::new("Aperture Stop")
                                .size(10.0)
                                .color(tokens.warning),
                        );
                    });
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
        // Row 2A: Radius & Thickness (Strict Numerical TextEdit Inputs)
        // -------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);

            // Radius input
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                ui.label(RichText::new("Radius:").size(10.5).color(tokens.text_muted));

                let is_flat = surf.radius_mm.is_infinite() || surf.radius_mm.abs() > 1e12;
                if is_flat {
                    render_lucide_icon(ui, LucideIcon::Infinity, 14.0, tokens.text_primary);
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
                        modified = true;
                    }
                }
            });

            // Thickness input (strictly 4 digits + point width)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
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

            // Glass Dropdown (Strictly locked fixed width: never resizes or stretches)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                ui.label(RichText::new("Glass:").size(10.5).color(tokens.text_muted));

                let display_glass = if surf.material_name.len() > 6 {
                    format!("{}…", &surf.material_name[..5])
                } else {
                    surf.material_name.clone()
                };

                ComboBox::from_id_salt(format!("glass_cb_{}", idx))
                    .selected_text(RichText::new(display_glass).monospace().size(10.0))
                    .width(58.0)
                    .show_ui(ui, |ui| {
                        for &glass in COMMON_OPTICAL_GLASSES {
                            if ui
                                .selectable_value(
                                    &mut surf.material_name,
                                    glass.to_string(),
                                    glass,
                                )
                                .clicked()
                            {
                                modified = true;
                            }
                        }
                    });
            });

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

/// Renders the technical specification card and Zemax System Explorer.
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
        ui.vertical(|ui| {
            // Header: Prescription Title & System Setup Button (Typed Lucide icon, NO ASCII)
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&prescription.name)
                        .strong()
                        .size(12.5)
                        .color(tokens.text_primary),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if lucide_text_button(ui, LucideIcon::SlidersHorizontal, "System Setup", tokens).clicked() {
                        state.system_explorer_open = !state.system_explorer_open;
                    }
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

            let stop_label = prescription
                .surfaces
                .iter()
                .position(|s| s.is_stop)
                .map(|i| format!("Surface {}", i))
                .unwrap_or_else(|| "Undefined".to_string());

            // Technical Specs Row (Always visible & interactive)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);

                // EPD directly editable via DragValue
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                    ui.label(RichText::new("EPD:").size(11.0).color(tokens.text_muted));
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
                    ui.label(RichText::new("Wav:").size(11.0).color(tokens.text_muted));
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

            ui.add_space(3.0);

            // Aperture Stop Indicator
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                ui.label(RichText::new("Aperture Stop:").size(10.5).color(tokens.text_muted));
                ui.label(
                    RichText::new(stop_label)
                        .monospace()
                        .size(10.5)
                        .color(tokens.accent),
                );
            });

            // ---------------------------------------------------------------
            // Expanded System Explorer (Aperture, Fields, Wavelengths)
            // ---------------------------------------------------------------
            if state.system_explorer_open {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Section 1: APERTURE PRESETS
                ui.label(RichText::new("SYSTEM APERTURE PRESETS").size(11.0).strong().monospace().color(tokens.text_primary));
                ui.add_space(3.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for &epd_val in &[5.0, 10.0, 20.0, 35.0, 50.0] {
                        let is_curr = (prescription.entrance_pupil_diameter_mm - epd_val).abs() < 0.1;
                        let (bg, stroke, text_color) = if is_curr {
                            (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
                        } else {
                            (tokens.bg_control, tokens.border, tokens.text_muted)
                        };
                        let btn = Button::new(
                            RichText::new(format!("{:.0} mm", epd_val))
                                .size(10.5)
                                .monospace()
                                .strong()
                                .color(text_color),
                        )
                        .fill(bg)
                        .stroke(stroke)
                        .corner_radius(CornerRadius::ZERO);

                        if ui.add(btn).clicked() {
                            prescription.entrance_pupil_diameter_mm = epd_val;
                            modified = true;
                        }
                    }
                });

                ui.add_space(8.0);

                // Section 2: FIELD ANGLES MANAGER
                ui.label(RichText::new("FIELD ANGLES (DEG)").size(11.0).strong().monospace().color(tokens.text_primary));
                ui.add_space(3.0);

                // Current fields list
                let mut field_to_remove: Option<usize> = None;
                let mut field_to_update: Option<(usize, f64)> = None;
                for (f_idx, &field_deg) in prescription.field_angles_deg.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        if field_deg.abs() < 1e-3 {
                            ui.label(RichText::new(format!("Field {}: 0.0° (On-Axis)", f_idx)).size(11.0).monospace().color(tokens.accent));
                        } else {
                            ui.label(RichText::new(format!("Field {}:", f_idx)).size(11.0).monospace().color(tokens.text_muted));
                            let mut val = field_deg;
                            let resp = ui.add(DragValue::new(&mut val).speed(0.1).range(0.1..=85.0).suffix("°"));
                            if resp.changed() {
                                field_to_update = Some((f_idx, val));
                            }
                            if lucide_small_icon_button(ui, LucideIcon::Trash2, "Delete field angle", tokens).clicked() {
                                field_to_remove = Some(f_idx);
                            }
                        }
                    });
                }

                if let Some((idx, val)) = field_to_update {
                    if idx < prescription.field_angles_deg.len() {
                        prescription.field_angles_deg[idx] = val;
                        modified = true;
                    }
                }

                if let Some(del_idx) = field_to_remove {
                    if del_idx < prescription.field_angles_deg.len() {
                        prescription.field_angles_deg.remove(del_idx);
                        modified = true;
                    }
                }

                ui.add_space(4.0);

                // Add field input
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    ui.label(RichText::new("New Angle:").size(11.0).color(tokens.text_muted));
                    ui.add(TextEdit::singleline(&mut state.new_field_buf).desired_width(50.0));
                    if lucide_text_button(ui, LucideIcon::Plus, "Add Field", tokens).clicked() {
                        if let Ok(angle) = state.new_field_buf.trim().parse::<f64>() {
                            if angle >= 0.0 && angle <= 85.0 && !prescription.field_angles_deg.iter().any(|&f| (f - angle).abs() < 1e-2) {
                                prescription.field_angles_deg.push(angle);
                                prescription.field_angles_deg.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                modified = true;
                            }
                        }
                    }
                });

                ui.add_space(4.0);

                // Zemax Field Presets
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    let make_preset_btn = |ui: &mut Ui, label: &str| -> egui::Response {
                        ui.add(
                            Button::new(RichText::new(label).size(10.5).monospace().color(tokens.text_primary))
                                .fill(tokens.bg_control)
                                .stroke(tokens.border)
                                .corner_radius(CornerRadius::ZERO),
                        )
                    };
                    if make_preset_btn(ui, "Cooke (0, 10, 14°)").clicked() {
                        prescription.field_angles_deg = vec![0.0, 10.0, 14.0];
                        modified = true;
                    }
                    if make_preset_btn(ui, "Photo (0..21°)").clicked() {
                        prescription.field_angles_deg = vec![0.0, 7.0, 14.0, 18.0, 21.5];
                        modified = true;
                    }
                    if make_preset_btn(ui, "Wide (0..35°)").clicked() {
                        prescription.field_angles_deg = vec![0.0, 15.0, 25.0, 35.0];
                        modified = true;
                    }
                });

                ui.add_space(8.0);

                // Section 3: WAVELENGTHS & DISPERSION
                ui.label(RichText::new("WAVELENGTHS (NM)").size(11.0).strong().monospace().color(tokens.text_primary));
                ui.add_space(3.0);

                let mut wav_to_remove: Option<usize> = None;
                let mut wav_to_update: Option<(usize, f64)> = None;

                for (w_idx, &wl) in prescription.wavelengths_nm.iter().enumerate() {
                    let is_primary = prescription.primary_wavelength_index == w_idx;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        if ui.radio(is_primary, "").on_hover_text("Set as primary design wavelength").clicked() {
                            prescription.primary_wavelength_index = w_idx;
                            modified = true;
                        }
                        let mut val = wl;
                        let resp = ui.add(DragValue::new(&mut val).speed(0.5).range(350.0..=14000.0).suffix(" nm"));
                        if resp.changed() {
                            wav_to_update = Some((w_idx, val));
                        }
                        if is_primary {
                            ui.label(RichText::new("(Primary)").size(10.5).monospace().color(tokens.accent));
                        } else if prescription.wavelengths_nm.len() > 1 {
                            if lucide_small_icon_button(ui, LucideIcon::Trash2, "Delete wavelength", tokens).clicked() {
                                wav_to_remove = Some(w_idx);
                            }
                        }
                    });
                }

                if let Some((idx, val)) = wav_to_update {
                    if idx < prescription.wavelengths_nm.len() {
                        prescription.wavelengths_nm[idx] = val;
                        modified = true;
                    }
                }

                if let Some(del_idx) = wav_to_remove {
                    if prescription.wavelengths_nm.len() > 1 && del_idx < prescription.wavelengths_nm.len() {
                        prescription.wavelengths_nm.remove(del_idx);
                        if prescription.primary_wavelength_index >= prescription.wavelengths_nm.len() {
                            prescription.primary_wavelength_index = 0;
                        }
                        modified = true;
                    }
                }

                ui.add_space(4.0);

                // Add Wavelength input
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    ui.label(RichText::new("New Wav:").size(11.0).color(tokens.text_muted));
                    ui.add(TextEdit::singleline(&mut state.new_wav_buf).desired_width(50.0));
                    if lucide_text_button(ui, LucideIcon::Plus, "Add Wav", tokens).clicked() {
                        if let Ok(wav) = state.new_wav_buf.trim().parse::<f64>() {
                            if wav >= 200.0 && wav <= 20000.0 && !prescription.wavelengths_nm.iter().any(|&w| (w - wav).abs() < 1e-1) {
                                prescription.wavelengths_nm.push(wav);
                                modified = true;
                            }
                        }
                    }
                });

                ui.add_space(4.0);

                // Spectral Presets
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    if ui.button(RichText::new("Visible F,d,C").size(10.5).monospace()).on_hover_text("486.1 nm (Blue), 587.6 nm (Yellow), 656.3 nm (Red)").clicked() {
                        prescription.wavelengths_nm = vec![486.13, 587.56, 656.27];
                        prescription.primary_wavelength_index = 1; // 587.56 nm
                        modified = true;
                    }
                    if ui.button(RichText::new("HeNe 632.8").size(10.5).monospace()).clicked() {
                        prescription.wavelengths_nm = vec![632.8];
                        prescription.primary_wavelength_index = 0;
                        modified = true;
                    }
                    if ui.button(RichText::new("NIR 850").size(10.5).monospace()).clicked() {
                        prescription.wavelengths_nm = vec![850.0, 940.0, 1064.0];
                        prescription.primary_wavelength_index = 0;
                        modified = true;
                    }
                    if ui.button(RichText::new("SWIR 1550").size(10.5).monospace()).clicked() {
                        prescription.wavelengths_nm = vec![1310.0, 1550.0];
                        prescription.primary_wavelength_index = 1;
                        modified = true;
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
}
