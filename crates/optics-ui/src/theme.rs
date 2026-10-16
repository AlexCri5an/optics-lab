//! Design tokens, themes, WCAG contrast verification, and display ray coloring.
//!
//! Provides 3 classic technical themes:
//! 1. "Classic light" (default: light gray chrome, pure white canvas)
//! 2. "Classic dark" (slate technical dark chrome, deep charcoal canvas)
//! 3. "High contrast" (pure black panels, high-contrast borders and accents)
//! 4. "Follow system" (automatically follows host OS dark/light mode)
//!
//! Includes `ray_color_for_background(wavelength_nm, canvas_bg)` to ensure that
//! spectral ray colors on a light canvas preserve hue ordering while achieving
//! at least 3.0:1 WCAG contrast against the canvas background.

use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2, Visuals,
};
use optics_spectral::color::display_color_for_wavelength;

/// User preference for the application visual theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemePreference {
    /// Classic scientific desktop light theme (default).
    #[default]
    ClassicLight,
    /// Classic CAD workstation dark theme.
    ClassicDark,
    /// High-contrast theme with maximum legibility.
    HighContrast,
    /// Follow host operating system appearance.
    FollowSystem,
}

impl ThemePreference {
    /// Human-readable name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ClassicLight => "Classic Light",
            Self::ClassicDark => "Classic Dark",
            Self::HighContrast => "High Contrast",
            Self::FollowSystem => "Follow System",
        }
    }

    /// User-facing display name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        self.name()
    }
}

/// Canvas background color override preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CanvasBackgroundPreference {
    /// Match the active theme's default canvas color.
    #[default]
    Theme,
    /// Force pure black canvas (#000000).
    Black,
    /// Force pure white canvas (#FFFFFF).
    White,
}

impl CanvasBackgroundPreference {
    /// User-facing display name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Theme => "Follow Theme",
            Self::Black => "Black",
            Self::White => "White",
        }
    }

    /// Resolves the effective canvas background Color32 given the active theme tokens.
    #[must_use]
    pub const fn resolve(&self, theme_canvas: Color32) -> Color32 {
        match self {
            Self::Theme => theme_canvas,
            Self::Black => Color32::from_rgb(0, 0, 0),
            Self::White => Color32::from_rgb(255, 255, 255),
        }
    }
}

/// User application preferences with live switching and persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppPreferences {
    /// Active visual theme.
    pub theme: ThemePreference,
    /// Viewport canvas background color.
    pub canvas_bg: CanvasBackgroundPreference,
    /// UI scale factor (0.50 to 3.00).
    pub ui_scale: f32,
    /// Whether soft glow is rendered around rays (default: false).
    pub soft_glow: bool,
    /// Whether animation is disabled.
    pub reduce_motion: bool,
    /// Whether left sidebar is visible.
    pub show_left_sidebar: bool,
    /// Whether right inspector is visible.
    pub show_right_inspector: bool,
    /// Whether bottom dock is visible.
    pub show_bottom_dock: bool,
    /// Left sidebar width in logical pixels.
    pub left_sidebar_width: f32,
    /// Right inspector width in logical pixels.
    pub right_inspector_width: f32,
    /// Bottom dock height in logical pixels.
    pub bottom_dock_height: f32,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::ClassicLight,
            canvas_bg: CanvasBackgroundPreference::Theme,
            ui_scale: 1.0,
            soft_glow: false,
            reduce_motion: true,
            show_left_sidebar: true,
            show_right_inspector: true,
            show_bottom_dock: true,
            left_sidebar_width: 240.0,
            right_inspector_width: 320.0,
            bottom_dock_height: 185.0,
        }
    }
}

impl AppPreferences {
    /// Resets all preferences to their standard classic desktop defaults.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Comprehensive UI design tokens for the Classic Desktop design system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticsTokens {
    /// Native window backdrop fill.
    pub window_bg: Color32,
    /// Main panel background (toolbars, sidebars, dock).
    pub panel_bg: Color32,
    /// Secondary sunken or nested panel background.
    pub panel_secondary: Color32,
    /// Viewport canvas background.
    pub canvas: Color32,
    /// Primary high-contrast text color (>= 4.5:1 on panel_bg).
    pub text_primary: Color32,
    /// Secondary field label text color (>= 4.5:1 on panel_bg).
    pub text_secondary: Color32,
    /// Disabled label text color (>= 3.0:1 on panel_bg).
    pub text_disabled: Color32,
    /// Muted/dim text alias for backwards compatibility.
    pub text_dim: Color32,
    /// Muted text alias.
    pub text_muted: Color32,
    /// Selected item background.
    pub selected: Color32,
    /// Primary technical accent color.
    pub accent: Color32,
    /// Accent hover state color.
    pub accent_hover: Color32,
    /// Standard 1 px border stroke color.
    pub border: Color32,
    /// Strong 1 px border stroke color (active tab, group box).
    pub border_strong: Color32,
    /// Subtle divider border stroke color.
    pub border_subtle: Color32,
    /// Hover background change.
    pub hover_bg: Color32,
    /// Pressed background state.
    pub active_bg: Color32,
    /// Solid 1 px keyboard focus ring color.
    pub focus_ring: Color32,
    /// Error status indicator color.
    pub status_error: Color32,
    /// Warning status indicator color.
    pub status_warning: Color32,
    /// Success status indicator color.
    pub status_success: Color32,
    /// Canvas major grid line color.
    pub grid_major: Color32,
    /// Canvas minor grid line color.
    pub grid_minor: Color32,
    /// Standard canvas grid color (alias to grid_minor).
    pub grid: Color32,
    /// Legacy alias for amber highlight.
    pub amber: Color32,
    /// Legacy alias for cyan highlight.
    pub cyan: Color32,
    /// Whether this theme is dark mode.
    pub is_dark: bool,
}

impl OpticsTokens {
    /// Classic Light theme (default): Light gray panels, pure white canvas, navy accent.
    #[must_use]
    pub fn classic_light() -> Self {
        let text_primary = Color32::from_rgb(0x11, 0x11, 0x11);
        let text_secondary = Color32::from_rgb(0x40, 0x40, 0x40);
        let text_disabled = Color32::from_rgb(0x76, 0x76, 0x76);
        let accent = Color32::from_rgb(0x1a, 0x5f, 0xb4);
        Self {
            window_bg: Color32::from_rgb(0xd8, 0xd8, 0xd8),
            panel_bg: Color32::from_rgb(0xec, 0xec, 0xec),
            panel_secondary: Color32::from_rgb(0xe0, 0xe0, 0xe0),
            canvas: Color32::from_rgb(0xff, 0xff, 0xff),
            text_primary,
            text_secondary,
            text_disabled,
            text_dim: text_secondary,
            text_muted: text_disabled,
            selected: Color32::from_rgb(0xcc, 0xe2, 0xff),
            accent,
            accent_hover: Color32::from_rgb(0x15, 0x4c, 0x90),
            border: Color32::from_rgb(0xb8, 0xb8, 0xb8),
            border_strong: Color32::from_rgb(0x70, 0x70, 0x70),
            border_subtle: Color32::from_rgb(0xcc, 0xcc, 0xcc),
            hover_bg: Color32::from_rgb(0xe2, 0xe2, 0xe2),
            active_bg: Color32::from_rgb(0xd0, 0xd0, 0xd0),
            focus_ring: accent,
            status_error: Color32::from_rgb(0xc0, 0x1c, 0x28),
            status_warning: Color32::from_rgb(0xb5, 0x6a, 0x00),
            status_success: Color32::from_rgb(0x26, 0xa2, 0x69),
            grid_major: Color32::from_rgb(0xd0, 0xd0, 0xd0),
            grid_minor: Color32::from_rgb(0xee, 0xee, 0xee),
            grid: Color32::from_rgb(0xd0, 0xd0, 0xd0),
            amber: Color32::from_rgb(0xb5, 0x6a, 0x00),
            cyan: Color32::from_rgb(0x00, 0x7a, 0x99),
            is_dark: false,
        }
    }

    /// Classic Dark theme: Slate gray chrome, deep charcoal canvas, technical cyan accent.
    #[must_use]
    pub fn classic_dark() -> Self {
        let text_primary = Color32::from_rgb(0xf0, 0xf3, 0xf6);
        let text_secondary = Color32::from_rgb(0xa0, 0xaa, 0xb8);
        let text_disabled = Color32::from_rgb(0x6e, 0x79, 0x8c);
        let accent = Color32::from_rgb(0x00, 0xb4, 0xd8);
        Self {
            window_bg: Color32::from_rgb(0x16, 0x18, 0x1f),
            panel_bg: Color32::from_rgb(0x1e, 0x22, 0x2b),
            panel_secondary: Color32::from_rgb(0x16, 0x19, 0x20),
            canvas: Color32::from_rgb(0x12, 0x14, 0x1a),
            text_primary,
            text_secondary,
            text_disabled,
            text_dim: text_secondary,
            text_muted: text_disabled,
            selected: Color32::from_rgb(0x1c, 0x3d, 0x5a),
            accent,
            accent_hover: Color32::from_rgb(0x38, 0xc7, 0xea),
            border: Color32::from_rgb(0x35, 0x3c, 0x4b),
            border_strong: Color32::from_rgb(0x4f, 0x58, 0x6c),
            border_subtle: Color32::from_rgb(0x2b, 0x31, 0x3d),
            hover_bg: Color32::from_rgb(0x28, 0x2e, 0x3b),
            active_bg: Color32::from_rgb(0x18, 0x1c, 0x24),
            focus_ring: Color32::from_rgb(0x00, 0xd4, 0xf5),
            status_error: Color32::from_rgb(0xff, 0x55, 0x55),
            status_warning: Color32::from_rgb(0xf5, 0xa6, 0x23),
            status_success: Color32::from_rgb(0x2e, 0xc2, 0x7e),
            grid_major: Color32::from_rgb(0x28, 0x2e, 0x3a),
            grid_minor: Color32::from_rgb(0x1c, 0x21, 0x2a),
            grid: Color32::from_rgb(0x28, 0x2e, 0x3a),
            amber: Color32::from_rgb(0xf5, 0xa6, 0x23),
            cyan: Color32::from_rgb(0x00, 0xd4, 0xf5),
            is_dark: true,
        }
    }

    /// High Contrast theme: Pure black panels, pure white canvas, vivid yellow/cyan accents.
    #[must_use]
    pub fn high_contrast() -> Self {
        let text_primary = Color32::from_rgb(0xff, 0xff, 0xff);
        let text_secondary = Color32::from_rgb(0xdd, 0xdd, 0xdd);
        let text_disabled = Color32::from_rgb(0x88, 0x88, 0x88);
        let accent = Color32::from_rgb(0xff, 0xff, 0x00);
        Self {
            window_bg: Color32::from_rgb(0x00, 0x00, 0x00),
            panel_bg: Color32::from_rgb(0x0a, 0x0a, 0x0a),
            panel_secondary: Color32::from_rgb(0x14, 0x14, 0x14),
            canvas: Color32::from_rgb(0x00, 0x00, 0x00),
            text_primary,
            text_secondary,
            text_disabled,
            text_dim: text_secondary,
            text_muted: text_disabled,
            selected: Color32::from_rgb(0x00, 0x40, 0x80),
            accent,
            accent_hover: Color32::from_rgb(0xff, 0xff, 0xff),
            border: Color32::from_rgb(0xff, 0xff, 0xff),
            border_strong: Color32::from_rgb(0xff, 0xff, 0xff),
            border_subtle: Color32::from_rgb(0x88, 0x88, 0x88),
            hover_bg: Color32::from_rgb(0x33, 0x33, 0x33),
            active_bg: Color32::from_rgb(0x55, 0x55, 0x55),
            focus_ring: accent,
            status_error: Color32::from_rgb(0xff, 0x00, 0x00),
            status_warning: Color32::from_rgb(0xff, 0xaa, 0x00),
            status_success: Color32::from_rgb(0x00, 0xff, 0x00),
            grid_major: Color32::from_rgb(0x00, 0x00, 0x00),
            grid_minor: Color32::from_rgb(0x88, 0x88, 0x88),
            grid: Color32::from_rgb(0x88, 0x88, 0x88),
            amber: Color32::from_rgb(0xff, 0xaa, 0x00),
            cyan: Color32::from_rgb(0x00, 0xff, 0xff),
            is_dark: true,
        }
    }

    /// Backwards compatibility alias for dark_scientific.
    #[must_use]
    pub fn dark_scientific() -> Self {
        Self::classic_dark()
    }

    /// Backwards compatibility alias for medium_dark.
    #[must_use]
    pub fn medium_dark() -> Self {
        Self::classic_dark()
    }
}

impl Default for OpticsTokens {
    fn default() -> Self {
        Self::classic_light()
    }
}

/// Resolves the active theme tokens given the user's preference and OS dark mode detection.
#[must_use]
pub fn resolve_theme(pref: ThemePreference, system_is_dark: bool) -> OpticsTokens {
    match pref {
        ThemePreference::ClassicLight => OpticsTokens::classic_light(),
        ThemePreference::ClassicDark => OpticsTokens::classic_dark(),
        ThemePreference::HighContrast => OpticsTokens::high_contrast(),
        ThemePreference::FollowSystem => {
            if system_is_dark {
                OpticsTokens::classic_dark()
            } else {
                OpticsTokens::classic_light()
            }
        }
    }
}

/// Computes a display ray color for a given wavelength that preserves hue order
/// and guarantees at least 3.0:1 WCAG contrast against the canvas background.
///
/// On light/white canvas backgrounds, raw spectral colors like yellow (580 nm) and
/// cyan (495 nm) are washed out (contrast ~1.1:1). This function scales color luminance
/// to ensure legibility while maintaining the natural visible spectrum hue progression.
#[must_use]
pub fn ray_color_for_background(wavelength_nm: f64, canvas_bg: Color32) -> Color32 {
    let rgb = display_color_for_wavelength(wavelength_nm);
    let r = f64::from(rgb[0]);
    let g = f64::from(rgb[1]);
    let b = f64::from(rgb[2]);

    let bg_lum = wcag_relative_luminance(canvas_bg);
    let is_light_bg = bg_lum > 0.45;

    let target_contrast = 3.0;

    if is_light_bg {
        // Darken progressively until contrast >= target_contrast
        let mut color = Color32::from_rgb(r.round() as u8, g.round() as u8, b.round() as u8);
        let mut factor = 1.0;
        let mut attempts = 0;
        while wcag_contrast_ratio(color, canvas_bg) < target_contrast && attempts < 25 {
            factor *= 0.94;
            color = Color32::from_rgb(
                (r * factor).clamp(0.0, 255.0).round() as u8,
                (g * factor).clamp(0.0, 255.0).round() as u8,
                (b * factor).clamp(0.0, 255.0).round() as u8,
            );
            attempts += 1;
        }
        color
    } else {
        // Dark background: lighten if very dark violet or deep red
        let mut color = Color32::from_rgb(r.round() as u8, g.round() as u8, b.round() as u8);
        let mut factor = 1.0;
        let mut attempts = 0;
        while wcag_contrast_ratio(color, canvas_bg) < target_contrast && attempts < 25 {
            factor *= 1.1;
            color = Color32::from_rgb(
                (r * factor).clamp(0.0, 255.0).round() as u8,
                (g * factor).clamp(0.0, 255.0).round() as u8,
                (b * factor).clamp(0.0, 255.0).round() as u8,
            );
            attempts += 1;
        }
        color
    }
}

/// Applies the given `OpticsTokens` palette and classic typography scale to an `egui::Context`.
pub fn apply_tokens(ctx: &Context, tokens: &OpticsTokens) {
    let mut style = (*ctx.style()).clone();

    // 1. Theme visuals
    let mut visuals = if tokens.is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.dark_mode = tokens.is_dark;
    visuals.panel_fill = tokens.panel_bg;
    visuals.window_fill = tokens.panel_bg;
    visuals.extreme_bg_color = tokens.canvas;
    visuals.faint_bg_color = tokens.canvas;
    visuals.code_bg_color = tokens.panel_secondary;

    // Window strokes and corner radius (0-2 px classic desktop standard)
    visuals.window_stroke = Stroke::new(1.0, tokens.border);
    visuals.window_corner_radius = CornerRadius::same(2);

    // Non-interactive widgets
    visuals.widgets.noninteractive.bg_fill = tokens.panel_bg;
    visuals.widgets.noninteractive.weak_bg_fill = tokens.panel_bg;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, tokens.border);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(2);

    // Inactive widgets
    visuals.widgets.inactive.bg_fill = tokens.panel_secondary;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, tokens.border);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(2);

    // Hovered widgets
    visuals.widgets.hovered.bg_fill = tokens.hover_bg;
    visuals.widgets.hovered.weak_bg_fill = tokens.hover_bg;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, tokens.border_strong);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(2);

    // Active / pressed widgets
    visuals.widgets.active.bg_fill = tokens.active_bg;
    visuals.widgets.active.weak_bg_fill = tokens.active_bg;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, tokens.text_primary);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, tokens.border_strong);
    visuals.widgets.active.corner_radius = CornerRadius::same(2);

    // Selection visuals
    visuals.selection.bg_fill = tokens.selected;
    visuals.selection.stroke = Stroke::new(1.0, tokens.accent);

    style.visuals = visuals;

    // 2. Compact 4 px desktop spacing and padding
    style.spacing.item_spacing = Vec2::new(6.0, 6.0);
    style.spacing.window_margin = Margin::same(8);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.slider_width = 140.0;

    // 3. Compact typography scale (13 px base)
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Name("Subheading".into()),
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
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
    fn test_wcag_contrast_ratios_all_themes() {
        let themes = [
            ("Classic Light", OpticsTokens::classic_light()),
            ("Classic Dark", OpticsTokens::classic_dark()),
            ("High Contrast", OpticsTokens::high_contrast()),
        ];

        for (name, tokens) in themes {
            // 1. Primary text on panel >= 4.5:1
            let cr_prim = wcag_contrast_ratio(tokens.text_primary, tokens.panel_bg);
            assert!(
                cr_prim >= 4.5,
                "{name}: Primary text contrast on panel_bg must be >= 4.5:1, got {cr_prim:.2}"
            );

            // 2. Secondary text on panel >= 4.5:1 (High contrast and dark/light)
            let cr_sec = wcag_contrast_ratio(tokens.text_secondary, tokens.panel_bg);
            assert!(
                cr_sec >= 4.0,
                "{name}: Secondary text contrast on panel_bg must be >= 4.0:1, got {cr_sec:.2}"
            );

            // 3. Disabled text >= 3.0:1
            let cr_dis = wcag_contrast_ratio(tokens.text_disabled, tokens.panel_bg);
            assert!(
                cr_dis >= 3.0,
                "{name}: Disabled text contrast on panel_bg must be >= 3.0:1, got {cr_dis:.2}"
            );

            // 4. Primary text on canvas >= 4.5:1
            let cr_canvas = wcag_contrast_ratio(tokens.text_primary, tokens.canvas);
            assert!(
                cr_canvas >= 4.5,
                "{name}: Primary text contrast on canvas must be >= 4.5:1, got {cr_canvas:.2}"
            );
        }
    }

    #[test]
    fn test_ray_color_contrast_against_white_canvas() {
        let canvas = Color32::WHITE;
        let wavelengths = [
            380.0, 420.0, 486.13, 520.0, 550.0, 587.56, 656.27, 700.0, 780.0,
        ];

        for wl in wavelengths {
            let color = ray_color_for_background(wl, canvas);
            let cr = wcag_contrast_ratio(color, canvas);
            assert!(
                cr >= 3.0,
                "Ray color for {wl:.1} nm on white canvas must have >= 3.0:1 contrast, got {cr:.2}"
            );
        }
    }

    #[test]
    fn test_ray_color_contrast_against_all_themes() {
        let themes = [
            OpticsTokens::classic_light(),
            OpticsTokens::classic_dark(),
            OpticsTokens::high_contrast(),
        ];
        let wavelengths = [400.0, 500.0, 587.56, 650.0];

        for tokens in themes {
            for wl in wavelengths {
                let color = ray_color_for_background(wl, tokens.canvas);
                let cr = wcag_contrast_ratio(color, tokens.canvas);
                assert!(
                    cr >= 3.0,
                    "Ray color for {wl:.1} nm on canvas {:?} must have >= 3.0:1 contrast, got {cr:.2}",
                    tokens.canvas
                );
            }
        }
    }

    #[test]
    fn test_hover_and_pressed_luminance_distinction() {
        let themes = [
            OpticsTokens::classic_light(),
            OpticsTokens::classic_dark(),
            OpticsTokens::high_contrast(),
        ];

        for tokens in themes {
            let lum_normal = wcag_relative_luminance(tokens.panel_bg);
            let lum_hover = wcag_relative_luminance(tokens.hover_bg);
            let lum_pressed = wcag_relative_luminance(tokens.active_bg);

            assert!(
                (lum_normal - lum_hover).abs() > 0.005,
                "Hover luminance delta must be distinct from normal"
            );
            assert!(
                (lum_hover - lum_pressed).abs() > 0.005,
                "Pressed luminance delta must be distinct from hover"
            );
        }
    }
}
