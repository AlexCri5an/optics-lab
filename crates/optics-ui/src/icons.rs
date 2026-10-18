//! Typed Lucide vector icon integration.
//!
//! Provides resolution-independent Lucide icons compiled into the binary,
//! rendered via `egui_extras` SVG loader, and dynamically tinted to match
//! academic theme states (normal, hover, active, accent).

use egui::{Color32, CornerRadius, Image, Response, Ui, Vec2};

use crate::theme::OpticsTokens;

/// Typed identifier for all Lucide icons used across OpticsLab.
///
/// Missing or misspelled icons produce compile-time errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LucideIcon {
    /// New file / new optical prescription.
    FilePlus,
    /// Open file dialog.
    FolderOpen,
    /// Save current prescription.
    Save,
    /// Lens presets (Cooke Triplet, Double Gauss, etc.).
    Sparkles,
    /// Run raytrace simulation.
    Play,
    /// Reset canvas view / center optical axis.
    RotateCcw,
    /// Zoom fit optical system to viewport.
    Maximize2,
    /// Toggle left panel (Lens Data Editor).
    PanelLeft,
    /// Toggle right panel (Aberration & Analysis).
    PanelRight,
    /// System and simulation properties.
    SlidersHorizontal,
    /// Switch to light paper mode.
    Sun,
    /// Switch to dark slate mode.
    Moon,
    /// Help and documentation.
    HelpCircle,
    /// Add optical surface.
    Plus,
    /// Remove optical surface.
    Trash2,
    /// Diagnostic / aberration monitor.
    Activity,
    /// Target reticle / aperture stop.
    Target,
    /// Checkmark indicator.
    Check,
    /// Close cross.
    X,
    /// Layers / Stack of surfaces.
    Layers,
    /// Infinity symbol (planar surface curvature).
    Infinity,
    /// Minus / zoom out.
    Minus,
}

impl LucideIcon {
    /// Returns the static SVG string content with white strokes for tinting.
    #[must_use]
    pub const fn svg_content(self) -> &'static str {
        match self {
            Self::FilePlus => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M12 18v-6"/><path d="M9 15h6"/></svg>"#
            }
            Self::FolderOpen => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6a2 2 0 0 1-1.94 1.5H4a2 2 0 0 1-2-2V5c0-1.1.9-2 2-2h3.93a2 2 0 0 1 1.66.9l.82 1.2a2 2 0 0 0 1.66.9H18a2 2 0 0 1 2 2v2"/></svg>"#
            }
            Self::Save => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M7 3v4a1 1 0 0 0 1 1h7"/></svg>"#
            }
            Self::Sparkles => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"/><path d="M5 3v4"/><path d="M19 17v4"/><path d="M3 5h4"/><path d="M17 19h4"/></svg>"#
            }
            Self::Play => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="6 3 20 12 6 21 6 3"/></svg>"#
            }
            Self::RotateCcw => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"#
            }
            Self::Maximize2 => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" x2="14" y1="3" y2="10"/><line x1="3" x2="10" y1="21" y2="14"/></svg>"#
            }
            Self::PanelLeft => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/></svg>"#
            }
            Self::PanelRight => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/></svg>"#
            }
            Self::SlidersHorizontal => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="21" x2="14" y1="4" y2="4"/><line x1="10" x2="3" y1="4" y2="4"/><line x1="21" x2="12" y1="12" y2="12"/><line x1="8" x2="3" y1="12" y2="12"/><line x1="21" x2="16" y1="20" y2="20"/><line x1="12" x2="3" y1="20" y2="20"/><line x1="14" x2="14" y1="2" y2="6"/><line x1="8" x2="8" y1="10" y2="14"/><line x1="16" x2="16" y1="18" y2="22"/></svg>"#
            }
            Self::Sun => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/></svg>"#
            }
            Self::Moon => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/></svg>"#
            }
            Self::HelpCircle => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>"#
            }
            Self::Plus => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="M12 5v14"/></svg>"#
            }
            Self::Trash2 => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/><line x1="10" x2="10" y1="11" y2="17"/><line x1="14" x2="14" y1="11" y2="17"/></svg>"#
            }
            Self::Activity => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>"#
            }
            Self::Target => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="2"/></svg>"#
            }
            Self::Check => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>"#
            }
            Self::X => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#
            }
            Self::Layers => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/></svg>"#
            }
            Self::Infinity => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 12c-2-2.67-4-4-6-4a4 4 0 1 0 0 8c2 0 4-1.33 6-4Zm0 0c2 2.67 4 4 6 4a4 4 0 0 0 0-8c-2 0-4 1.33-6 4Z"/></svg>"#
            }
            Self::Minus => {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/></svg>"#
            }
        }
    }

    /// URI identifier for egui image caching.
    #[must_use]
    pub const fn uri(self) -> &'static str {
        match self {
            Self::FilePlus => "bytes://lucide/file-plus.svg",
            Self::FolderOpen => "bytes://lucide/folder-open.svg",
            Self::Save => "bytes://lucide/save.svg",
            Self::Sparkles => "bytes://lucide/sparkles.svg",
            Self::Play => "bytes://lucide/play.svg",
            Self::RotateCcw => "bytes://lucide/rotate-ccw.svg",
            Self::Maximize2 => "bytes://lucide/maximize-2.svg",
            Self::PanelLeft => "bytes://lucide/panel-left.svg",
            Self::PanelRight => "bytes://lucide/panel-right.svg",
            Self::SlidersHorizontal => "bytes://lucide/sliders-horizontal.svg",
            Self::Sun => "bytes://lucide/sun.svg",
            Self::Moon => "bytes://lucide/moon.svg",
            Self::HelpCircle => "bytes://lucide/help-circle.svg",
            Self::Plus => "bytes://lucide/plus.svg",
            Self::Trash2 => "bytes://lucide/trash-2.svg",
            Self::Activity => "bytes://lucide/activity.svg",
            Self::Target => "bytes://lucide/target.svg",
            Self::Check => "bytes://lucide/check.svg",
            Self::X => "bytes://lucide/x.svg",
            Self::Layers => "bytes://lucide/layers.svg",
            Self::Infinity => "bytes://lucide/infinity.svg",
            Self::Minus => "bytes://lucide/minus.svg",
        }
    }
}

/// Renders a Lucide icon image tinted with the given color and size.
pub fn render_lucide_icon(ui: &mut Ui, icon: LucideIcon, size: f32, color: Color32) {
    let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
        .tint(color)
        .fit_to_exact_size(Vec2::splat(size));
    ui.add(img);
}

/// Renders a crisp academic toolbar button with a Lucide icon.
///
/// Features:
/// - 1px fine academic border.
/// - Distinct hover, active, and toggle states.
/// - Native click registration across the entire button surface.
/// - Automatic tooltip.
pub fn lucide_toolbar_button(
    ui: &mut Ui,
    icon: LucideIcon,
    tooltip: &str,
    is_active: bool,
    tokens: &OpticsTokens,
) -> Response {
    let icon_color = if is_active {
        tokens.accent
    } else {
        tokens.text_muted
    };

    let bg_color = if is_active {
        tokens.bg_control_active
    } else {
        tokens.bg_control
    };

    let border_stroke = if is_active {
        egui::Stroke::new(1.0, tokens.accent)
    } else {
        tokens.border
    };

    let icon_size = 15.0;
    let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
        .tint(icon_color)
        .fit_to_exact_size(Vec2::splat(icon_size));

    let btn = egui::Button::image(img)
        .min_size(Vec2::new(26.0, 24.0))
        .fill(bg_color)
        .stroke(border_stroke)
        .corner_radius(egui::CornerRadius::ZERO);

    ui.add(btn).on_hover_text(tooltip)
}

/// Renders a compact academic button with a Lucide icon and text label.
pub fn lucide_text_button(
    ui: &mut Ui,
    icon: LucideIcon,
    label: &str,
    tokens: &OpticsTokens,
) -> Response {
    let icon_size = 14.0;
    let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
        .tint(tokens.accent)
        .fit_to_exact_size(Vec2::splat(icon_size));

    let btn = egui::Button::image_and_text(img, egui::RichText::new(label).size(11.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

    ui.add(btn)
}

/// Renders a tiny icon-only square button (e.g. for close X or delete).
pub fn lucide_small_icon_button(
    ui: &mut Ui,
    icon: LucideIcon,
    tooltip: &str,
    tokens: &OpticsTokens,
) -> Response {
    let icon_size = 12.0;
    let img = Image::from_bytes(icon.uri(), icon.svg_content().as_bytes())
        .tint(tokens.text_muted)
        .fit_to_exact_size(Vec2::splat(icon_size));

    let btn = egui::Button::image(img)
        .min_size(Vec2::new(18.0, 18.0))
        .fill(tokens.bg_control)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO);

    ui.add(btn).on_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_lucide_icons_have_valid_svg() {
        let icons = [
            LucideIcon::FilePlus,
            LucideIcon::FolderOpen,
            LucideIcon::Save,
            LucideIcon::Sparkles,
            LucideIcon::Play,
            LucideIcon::RotateCcw,
            LucideIcon::Maximize2,
            LucideIcon::PanelLeft,
            LucideIcon::PanelRight,
            LucideIcon::SlidersHorizontal,
            LucideIcon::Sun,
            LucideIcon::Moon,
            LucideIcon::HelpCircle,
            LucideIcon::Plus,
            LucideIcon::Trash2,
            LucideIcon::Activity,
            LucideIcon::Target,
            LucideIcon::Check,
            LucideIcon::X,
            LucideIcon::Layers,
            LucideIcon::Infinity,
            LucideIcon::Minus,
        ];

        for icon in icons {
            let svg = icon.svg_content();
            assert!(svg.starts_with("<svg"), "Icon {:?} must start with <svg", icon);
            assert!(svg.ends_with("</svg>"), "Icon {:?} must end with </svg>", icon);
            assert!(icon.uri().starts_with("bytes://lucide/"), "Icon {:?} URI must be namespaced", icon);
        }
    }
}
