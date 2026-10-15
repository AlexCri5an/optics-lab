//! Design tokens and theme system for OpticsLab.
//!
//! Provides the canonical `MediumDark` design token palette and applies WCAG AA compliant
//! styling across all egui widgets and panels.

use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};

/// OpticsLab UI theme tokens defining the color palette and interaction states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticsTokens {
    /// Background color for the 2D ray tracing canvas.
    pub canvas: Color32,
    /// Background color for dock panels, sidebars, and dialog windows.
    pub panel_bg: Color32,
    /// High-contrast primary text color.
    pub text_primary: Color32,
    /// Secondary muted text color for labels, hints, and readouts.
    pub text_dim: Color32,
    /// Vivid cyan primary accent color for active tools, selection highlights, and links.
    pub accent: Color32,
    /// Amber highlight color for warnings, critical angles, and special indicators.
    pub amber: Color32,
    /// Border and separator stroke color.
    pub border: Color32,
    /// Background color for hovered interactive elements.
    pub hover_bg: Color32,
    /// Background color for actively pressed interactive elements.
    pub active_bg: Color32,
}

impl OpticsTokens {
    /// Canonical MediumDark theme palette specified in Slice A.
    ///
    /// - Canvas: `#11141a`
    /// - Panels: `#1a1f2e`
    /// - Accent: `#00d4f5`
    /// - Amber: `#f5a623`
    /// - Dim text: `#8b93a7`
    /// - Text primary: `#e6e9f0`
    #[must_use]
    pub fn medium_dark() -> Self {
        Self {
            canvas: Color32::from_rgb(0x11, 0x14, 0x1a),
            panel_bg: Color32::from_rgb(0x1a, 0x1f, 0x2e),
            text_primary: Color32::from_rgb(0xe6, 0xe9, 0xf0),
            text_dim: Color32::from_rgb(0x8b, 0x93, 0xa7),
            accent: Color32::from_rgb(0x00, 0xd4, 0xf5),
            amber: Color32::from_rgb(0xf5, 0xa6, 0x23),
            border: Color32::from_rgb(0x2d, 0x35, 0x48),
            hover_bg: Color32::from_rgb(0x26, 0x2e, 0x42),
            active_bg: Color32::from_rgb(0x2f, 0x39, 0x52),
        }
    }
}

impl Default for OpticsTokens {
    fn default() -> Self {
        Self::medium_dark()
    }
}

/// Applies the given `OpticsTokens` palette and typography scale to an `egui::Context`.
pub fn apply_tokens(ctx: &Context, tokens: &OpticsTokens) {
    let mut style = (*ctx.style()).clone();

    // 1. Theme colors
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = tokens.panel_bg;
    style.visuals.window_fill = tokens.panel_bg;
    style.visuals.extreme_bg_color = tokens.canvas;
    style.visuals.faint_bg_color = tokens.canvas;
    style.visuals.code_bg_color = tokens.canvas;

    // Window strokes and corner radius
    style.visuals.window_stroke = Stroke::new(1.0, tokens.border);
    style.visuals.window_corner_radius = CornerRadius::same(8);

    // Non-interactive widgets
    style.visuals.widgets.noninteractive.bg_fill = tokens.panel_bg;
    style.visuals.widgets.noninteractive.weak_bg_fill = tokens.panel_bg;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, tokens.border);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

    // Inactive widgets
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(38, 46, 66, 200);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, tokens.border);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

    // Hovered widgets
    style.visuals.widgets.hovered.bg_fill = tokens.hover_bg;
    style.visuals.widgets.hovered.weak_bg_fill = tokens.hover_bg;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, tokens.accent);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

    // Active / pressed widgets
    style.visuals.widgets.active.bg_fill = tokens.active_bg;
    style.visuals.widgets.active.weak_bg_fill = tokens.active_bg;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, tokens.accent);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(4);

    // Selection visuals
    style.visuals.selection.bg_fill = Color32::from_rgba_premultiplied(0, 212, 245, 60);
    style.visuals.selection.stroke = Stroke::new(1.5, tokens.accent);

    // 2. Spacing and padding
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.slider_width = 160.0;

    // 3. Typography scale
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Name("Subheading".into()),
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
    ]
    .into();

    ctx.set_style(style);
}

/// Computes the WCAG 2.1 relative luminance of an sRGB color.
///
/// Formula: <https://www.w3.org/TR/WCAG21/#dfn-relative-luminance>
#[must_use]
pub fn wcag_relative_luminance(c: Color32) -> f64 {
    let to_linear = |v_u8: u8| -> f64 {
        let v = f64::from(v_u8) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = to_linear(c.r());
    let g = to_linear(c.g());
    let b = to_linear(c.b());
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Computes the WCAG 2.1 contrast ratio between two colors in $[1.0, 21.0]$.
///
/// Formula: $(L_1 + 0.05) / (L_2 + 0.05)$ where $L_1 \ge L_2$.
#[must_use]
pub fn wcag_contrast_ratio(c1: Color32, c2: Color32) -> f64 {
    let l1 = wcag_relative_luminance(c1);
    let l2 = wcag_relative_luminance(c2);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wcag_contrast_ratios() {
        let tokens = OpticsTokens::medium_dark();

        // 1. Primary text (#e6e9f0) on Panel background (#1a1f2e)
        let cr_text_panel = wcag_contrast_ratio(tokens.text_primary, tokens.panel_bg);
        assert!(
            cr_text_panel >= 4.5,
            "Primary text on panel background must satisfy WCAG AA (>=4.5:1), got {cr_text_panel:.2}:1"
        );

        // 2. Primary text (#e6e9f0) on Canvas background (#11141a)
        let cr_text_canvas = wcag_contrast_ratio(tokens.text_primary, tokens.canvas);
        assert!(
            cr_text_canvas >= 4.5,
            "Primary text on canvas background must satisfy WCAG AA (>=4.5:1), got {cr_text_canvas:.2}:1"
        );

        // 3. Dim text (#8b93a7) on Panel background (#1a1f2e)
        let cr_dim_panel = wcag_contrast_ratio(tokens.text_dim, tokens.panel_bg);
        assert!(
            cr_dim_panel >= 4.5,
            "Dim text on panel background must satisfy WCAG AA (>=4.5:1), got {cr_dim_panel:.2}:1"
        );

        // 4. Dim text (#8b93a7) on Canvas background (#11141a)
        let cr_dim_canvas = wcag_contrast_ratio(tokens.text_dim, tokens.canvas);
        assert!(
            cr_dim_canvas >= 4.5,
            "Dim text on canvas background must satisfy WCAG AA (>=4.5:1), got {cr_dim_canvas:.2}:1"
        );
    }
}
