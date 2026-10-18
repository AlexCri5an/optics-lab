//! Academic minimalist design tokens inspired by classic scientific software (Zemax 95, SGI IRIX,
//! VT220/VT100 terminals, NeXTSTEP, Blueprint drafting, and Solarized).

use egui::{Color32, Stroke};

/// Design tokens defining the visual language of OpticsLab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpticsTokens {
    /// Window and background canvas clear color.
    pub bg_canvas: Color32,
    /// Background color of top bar, status bar, and side panels.
    pub bg_panel: Color32,
    /// Background color of buttons, scrub fields, and input controls.
    pub bg_control: Color32,
    /// Control background when hovered.
    pub bg_control_hovered: Color32,
    /// Control background when pressed or active.
    pub bg_control_active: Color32,
    /// 1px crisp separation border stroke.
    pub border: Stroke,
    /// Border stroke when focused or hovered.
    pub border_hovered: Stroke,
    /// Primary high-contrast technical text.
    pub text_primary: Color32,
    /// Secondary muted text for units, labels, and shortcuts.
    pub text_muted: Color32,
    /// Scientific accent color.
    pub accent: Color32,
    /// Accent color when hovered.
    pub accent_hovered: Color32,
    /// Warning / Aberration notice color.
    pub warning: Color32,
    /// Success / Validated metric color.
    pub success: Color32,
    /// High-contrast foreground color on canvas & plots.
    pub canvas_fg: Color32,
    /// Muted secondary text / grid / axis color on canvas & plots.
    pub canvas_fg_muted: Color32,
    /// Translucent fill color for optical glass lens bodies in viewport.
    pub glass_fill: Color32,
    /// Crisp outline stroke for optical lens surfaces in viewport.
    pub glass_stroke: Stroke,
    /// Tangential curve color in MTF and diagnostic plots.
    pub plot_tangential: Color32,
    /// Sagittal curve color in MTF and diagnostic plots.
    pub plot_sagittal: Color32,
    /// Primary on-axis ray / spot color.
    pub plot_axial: Color32,
}

impl OpticsTokens {
    /// Returns true if the background canvas has dark luminance.
    #[must_use]
    pub fn is_canvas_dark(&self) -> bool {
        let (r, g, b, _) = self.bg_canvas.to_tuple();
        (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) < 128.0
    }
}

/// Available retro and scientific engineering theme modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Classic Zemax OpticStudio 95 workstation (Windows 95/NT retro CAD).
    #[default]
    ZemaxClassic,
    /// Amber Phosphor CRT Terminal (DEC VT220 vintage amber monochrome).
    AmberPhosphor,
    /// Green Phosphor CRT Terminal (IBM 3270 / VT100 monochrome green).
    GreenPhosphor,
    /// Silicon Graphics IRIX Unix workstation (Indigo / Octane slate CAD).
    SgiIrix,
    /// Blueprint CAD cyanotype drafting table.
    Blueprint,
    /// Steve Jobs' NeXTSTEP 1990 workstation (dark magnesium / silver).
    NextStep,
    /// Olivetti / Braun Industrial Cyberdeck (carbon charcoal with hazard orange).
    Cyberdeck,
    /// Ethan Schoonover's Solarized Dark scientific palette.
    SolarizedDark,
    /// Ethan Schoonover's Solarized Light scientific palette.
    SolarizedLight,
    /// High-contrast pure black & white laboratory bench.
    MonochromeLab,
    /// Modern academic dark slate.
    DarkSlate,
    /// Clean academic paper publication light mode.
    PaperLight,
}

impl ThemeMode {
    /// List of all available themes.
    pub const ALL: [ThemeMode; 12] = [
        ThemeMode::ZemaxClassic,
        ThemeMode::AmberPhosphor,
        ThemeMode::GreenPhosphor,
        ThemeMode::SgiIrix,
        ThemeMode::Blueprint,
        ThemeMode::NextStep,
        ThemeMode::Cyberdeck,
        ThemeMode::SolarizedDark,
        ThemeMode::SolarizedLight,
        ThemeMode::MonochromeLab,
        ThemeMode::DarkSlate,
        ThemeMode::PaperLight,
    ];

    /// User-friendly label for menus and dropdowns.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ZemaxClassic => "Zemax Classic 95",
            Self::AmberPhosphor => "Amber CRT (VT220)",
            Self::GreenPhosphor => "Green CRT (VT100)",
            Self::SgiIrix => "SGI IRIX Unix",
            Self::Blueprint => "Blueprint CAD",
            Self::NextStep => "NeXTSTEP 1990",
            Self::Cyberdeck => "Cyberdeck Olivetti",
            Self::SolarizedDark => "Solarized Dark",
            Self::SolarizedLight => "Solarized Light",
            Self::MonochromeLab => "Monochrome Lab",
            Self::DarkSlate => "Dark Slate",
            Self::PaperLight => "Paper Light",
        }
    }

    /// Whether this theme has a dark background.
    #[must_use]
    pub const fn is_dark(self) -> bool {
        match self {
            Self::ZemaxClassic | Self::SolarizedLight | Self::PaperLight => false,
            _ => true,
        }
    }

    /// Cycles to the next theme mode.
    #[must_use]
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// Resolves design tokens for the current theme mode.
    #[must_use]
    pub fn tokens(self) -> OpticsTokens {
        match self {
            // 1. Zemax Classic 95 (Retro Windows 95 / NT CAD)
            Self::ZemaxClassic => OpticsTokens {
                bg_canvas: Color32::from_rgb(212, 208, 200),
                bg_panel: Color32::from_rgb(212, 208, 200),
                bg_control: Color32::from_rgb(240, 238, 232),
                bg_control_hovered: Color32::from_rgb(222, 218, 210),
                bg_control_active: Color32::from_rgb(196, 192, 184),
                border: Stroke::new(1.0, Color32::from_rgb(128, 128, 128)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(64, 64, 64)),
                text_primary: Color32::from_rgb(10, 10, 10),
                text_muted: Color32::from_rgb(85, 85, 85),
                accent: Color32::from_rgb(10, 36, 106), // Win95 title bar navy
                accent_hovered: Color32::from_rgb(16, 60, 160),
                warning: Color32::from_rgb(180, 70, 0),
                success: Color32::from_rgb(0, 120, 0),
                canvas_fg: Color32::from_rgb(10, 10, 10),
                canvas_fg_muted: Color32::from_rgb(90, 90, 90),
                glass_fill: Color32::from_rgba_unmultiplied(10, 60, 160, 52),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(10, 10, 10)),
                plot_tangential: Color32::from_rgb(10, 36, 160),
                plot_sagittal: Color32::from_rgb(0, 130, 30),
                plot_axial: Color32::from_rgb(10, 36, 160),
            },

            // 2. Amber Phosphor CRT (VT220 Terminal)
            Self::AmberPhosphor => OpticsTokens {
                bg_canvas: Color32::from_rgb(12, 10, 6),
                bg_panel: Color32::from_rgb(22, 18, 12),
                bg_control: Color32::from_rgb(36, 28, 16),
                bg_control_hovered: Color32::from_rgb(52, 40, 22),
                bg_control_active: Color32::from_rgb(70, 52, 26),
                border: Stroke::new(1.0, Color32::from_rgb(85, 62, 20)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(160, 115, 30)),
                text_primary: Color32::from_rgb(255, 176, 0), // P3 amber
                text_muted: Color32::from_rgb(180, 120, 10),
                accent: Color32::from_rgb(255, 195, 0),
                accent_hovered: Color32::from_rgb(255, 225, 60),
                warning: Color32::from_rgb(255, 110, 0),
                success: Color32::from_rgb(255, 215, 0),
                canvas_fg: Color32::from_rgb(255, 176, 0),
                canvas_fg_muted: Color32::from_rgb(180, 120, 10),
                glass_fill: Color32::from_rgba_unmultiplied(255, 176, 0, 48),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(255, 200, 50)),
                plot_tangential: Color32::from_rgb(255, 215, 0),
                plot_sagittal: Color32::from_rgb(255, 125, 0),
                plot_axial: Color32::from_rgb(255, 176, 0),
            },

            // 3. Green Phosphor CRT (VT100 / IBM 3270)
            Self::GreenPhosphor => OpticsTokens {
                bg_canvas: Color32::from_rgb(8, 14, 8),
                bg_panel: Color32::from_rgb(14, 24, 14),
                bg_control: Color32::from_rgb(20, 38, 20),
                bg_control_hovered: Color32::from_rgb(28, 54, 28),
                bg_control_active: Color32::from_rgb(38, 72, 38),
                border: Stroke::new(1.0, Color32::from_rgb(30, 80, 30)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(50, 150, 50)),
                text_primary: Color32::from_rgb(51, 255, 51), // P1 phosphor
                text_muted: Color32::from_rgb(30, 160, 30),
                accent: Color32::from_rgb(0, 255, 100),
                accent_hovered: Color32::from_rgb(120, 255, 120),
                warning: Color32::from_rgb(200, 255, 0),
                success: Color32::from_rgb(51, 255, 51),
                canvas_fg: Color32::from_rgb(51, 255, 51),
                canvas_fg_muted: Color32::from_rgb(30, 160, 30),
                glass_fill: Color32::from_rgba_unmultiplied(51, 255, 51, 48),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(100, 255, 100)),
                plot_tangential: Color32::from_rgb(120, 255, 120),
                plot_sagittal: Color32::from_rgb(0, 200, 80),
                plot_axial: Color32::from_rgb(51, 255, 51),
            },

            // 4. SGI IRIX Unix Workstation
            Self::SgiIrix => OpticsTokens {
                bg_canvas: Color32::from_rgb(18, 22, 36),
                bg_panel: Color32::from_rgb(36, 42, 58),
                bg_control: Color32::from_rgb(48, 56, 78),
                bg_control_hovered: Color32::from_rgb(62, 72, 98),
                bg_control_active: Color32::from_rgb(78, 90, 120),
                border: Stroke::new(1.0, Color32::from_rgb(68, 80, 110)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(110, 130, 180)),
                text_primary: Color32::from_rgb(240, 242, 250),
                text_muted: Color32::from_rgb(155, 165, 190),
                accent: Color32::from_rgb(0, 215, 255), // SGI Electric Cyan
                accent_hovered: Color32::from_rgb(90, 235, 255),
                warning: Color32::from_rgb(255, 175, 40),
                success: Color32::from_rgb(50, 220, 140),
                canvas_fg: Color32::from_rgb(240, 242, 250),
                canvas_fg_muted: Color32::from_rgb(140, 155, 185),
                glass_fill: Color32::from_rgba_unmultiplied(0, 215, 255, 50),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(180, 235, 255)),
                plot_tangential: Color32::from_rgb(0, 215, 255),
                plot_sagittal: Color32::from_rgb(50, 220, 140),
                plot_axial: Color32::from_rgb(0, 215, 255),
            },

            // 5. Blueprint CAD Cyanotype
            Self::Blueprint => OpticsTokens {
                bg_canvas: Color32::from_rgb(12, 38, 76),
                bg_panel: Color32::from_rgb(16, 48, 96),
                bg_control: Color32::from_rgb(24, 66, 128),
                bg_control_hovered: Color32::from_rgb(34, 84, 158),
                bg_control_active: Color32::from_rgb(44, 102, 188),
                border: Stroke::new(1.0, Color32::from_rgb(42, 96, 172)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(90, 160, 240)),
                text_primary: Color32::from_rgb(242, 248, 255),
                text_muted: Color32::from_rgb(135, 185, 235),
                accent: Color32::from_rgb(0, 230, 255),
                accent_hovered: Color32::from_rgb(100, 245, 255),
                warning: Color32::from_rgb(255, 205, 50),
                success: Color32::from_rgb(60, 240, 180),
                canvas_fg: Color32::from_rgb(242, 248, 255),
                canvas_fg_muted: Color32::from_rgb(135, 185, 235),
                glass_fill: Color32::from_rgba_unmultiplied(180, 230, 255, 65),
                glass_stroke: Stroke::new(1.5, Color32::from_rgb(255, 255, 255)),
                plot_tangential: Color32::from_rgb(0, 230, 255),
                plot_sagittal: Color32::from_rgb(60, 240, 180),
                plot_axial: Color32::from_rgb(180, 235, 255),
            },

            // 6. NeXTSTEP 1990
            Self::NextStep => OpticsTokens {
                bg_canvas: Color32::from_rgb(10, 10, 10),
                bg_panel: Color32::from_rgb(44, 44, 44),
                bg_control: Color32::from_rgb(58, 58, 58),
                bg_control_hovered: Color32::from_rgb(74, 74, 74),
                bg_control_active: Color32::from_rgb(92, 92, 92),
                border: Stroke::new(1.0, Color32::from_rgb(80, 80, 80)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(140, 140, 140)),
                text_primary: Color32::from_rgb(245, 245, 245),
                text_muted: Color32::from_rgb(165, 165, 165),
                accent: Color32::from_rgb(150, 210, 60),
                accent_hovered: Color32::from_rgb(180, 235, 90),
                warning: Color32::from_rgb(240, 160, 30),
                success: Color32::from_rgb(100, 220, 80),
                canvas_fg: Color32::from_rgb(245, 245, 245),
                canvas_fg_muted: Color32::from_rgb(160, 160, 160),
                glass_fill: Color32::from_rgba_unmultiplied(160, 200, 240, 52),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(230, 235, 245)),
                plot_tangential: Color32::from_rgb(150, 210, 60),
                plot_sagittal: Color32::from_rgb(100, 180, 255),
                plot_axial: Color32::from_rgb(160, 220, 70),
            },

            // 7. Cyberdeck Olivetti
            Self::Cyberdeck => OpticsTokens {
                bg_canvas: Color32::from_rgb(16, 17, 20),
                bg_panel: Color32::from_rgb(26, 28, 32),
                bg_control: Color32::from_rgb(38, 41, 48),
                bg_control_hovered: Color32::from_rgb(52, 56, 65),
                bg_control_active: Color32::from_rgb(66, 71, 82),
                border: Stroke::new(1.0, Color32::from_rgb(60, 65, 75)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(100, 108, 122)),
                text_primary: Color32::from_rgb(242, 240, 232),
                text_muted: Color32::from_rgb(148, 150, 155),
                accent: Color32::from_rgb(255, 102, 0), // Hazard safety orange
                accent_hovered: Color32::from_rgb(255, 140, 50),
                warning: Color32::from_rgb(255, 180, 0),
                success: Color32::from_rgb(40, 210, 110),
                canvas_fg: Color32::from_rgb(242, 240, 232),
                canvas_fg_muted: Color32::from_rgb(148, 150, 155),
                glass_fill: Color32::from_rgba_unmultiplied(255, 140, 50, 48),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(255, 200, 140)),
                plot_tangential: Color32::from_rgb(255, 102, 0),
                plot_sagittal: Color32::from_rgb(40, 210, 220),
                plot_axial: Color32::from_rgb(255, 140, 50),
            },

            // 8. Solarized Dark
            Self::SolarizedDark => OpticsTokens {
                bg_canvas: Color32::from_rgb(0, 43, 54),
                bg_panel: Color32::from_rgb(7, 54, 66),
                bg_control: Color32::from_rgb(14, 70, 84),
                bg_control_hovered: Color32::from_rgb(24, 88, 104),
                bg_control_active: Color32::from_rgb(34, 106, 124),
                border: Stroke::new(1.0, Color32::from_rgb(28, 92, 108)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(42, 161, 152)),
                text_primary: Color32::from_rgb(147, 161, 161),
                text_muted: Color32::from_rgb(101, 123, 131),
                accent: Color32::from_rgb(42, 161, 152),
                accent_hovered: Color32::from_rgb(60, 195, 185),
                warning: Color32::from_rgb(181, 137, 0),
                success: Color32::from_rgb(133, 153, 0),
                canvas_fg: Color32::from_rgb(131, 148, 150),
                canvas_fg_muted: Color32::from_rgb(88, 110, 117),
                glass_fill: Color32::from_rgba_unmultiplied(42, 161, 152, 58),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(147, 161, 161)),
                plot_tangential: Color32::from_rgb(42, 161, 152),
                plot_sagittal: Color32::from_rgb(133, 153, 0),
                plot_axial: Color32::from_rgb(38, 139, 210),
            },

            // 9. Solarized Light
            Self::SolarizedLight => OpticsTokens {
                bg_canvas: Color32::from_rgb(253, 246, 227),
                bg_panel: Color32::from_rgb(238, 232, 213),
                bg_control: Color32::from_rgb(253, 246, 227),
                bg_control_hovered: Color32::from_rgb(230, 222, 200),
                bg_control_active: Color32::from_rgb(215, 205, 180),
                border: Stroke::new(1.0, Color32::from_rgb(195, 185, 160)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(150, 140, 115)),
                text_primary: Color32::from_rgb(101, 123, 131),
                text_muted: Color32::from_rgb(147, 161, 161),
                accent: Color32::from_rgb(38, 139, 210),
                accent_hovered: Color32::from_rgb(30, 110, 180),
                warning: Color32::from_rgb(181, 137, 0),
                success: Color32::from_rgb(133, 153, 0),
                canvas_fg: Color32::from_rgb(88, 110, 117),
                canvas_fg_muted: Color32::from_rgb(147, 161, 161),
                glass_fill: Color32::from_rgba_unmultiplied(38, 139, 210, 68),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(88, 110, 117)),
                plot_tangential: Color32::from_rgb(38, 139, 210),
                plot_sagittal: Color32::from_rgb(133, 153, 0),
                plot_axial: Color32::from_rgb(211, 54, 130),
            },

            // 10. Monochrome Lab (Stark B&W High Contrast)
            Self::MonochromeLab => OpticsTokens {
                bg_canvas: Color32::from_rgb(0, 0, 0),
                bg_panel: Color32::from_rgb(18, 18, 18),
                bg_control: Color32::from_rgb(32, 32, 32),
                bg_control_hovered: Color32::from_rgb(50, 50, 50),
                bg_control_active: Color32::from_rgb(70, 70, 70),
                border: Stroke::new(1.0, Color32::from_rgb(90, 90, 90)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(180, 180, 180)),
                text_primary: Color32::from_rgb(255, 255, 255),
                text_muted: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(255, 255, 255),
                accent_hovered: Color32::from_rgb(200, 200, 200),
                warning: Color32::from_rgb(230, 230, 230),
                success: Color32::from_rgb(255, 255, 255),
                canvas_fg: Color32::from_rgb(255, 255, 255),
                canvas_fg_muted: Color32::from_rgb(170, 170, 170),
                glass_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 50),
                glass_stroke: Stroke::new(1.5, Color32::from_rgb(255, 255, 255)),
                plot_tangential: Color32::from_rgb(255, 255, 255),
                plot_sagittal: Color32::from_rgb(190, 190, 190),
                plot_axial: Color32::from_rgb(255, 255, 255),
            },

            // 11. Dark Slate (Modern Dark)
            Self::DarkSlate => OpticsTokens {
                bg_canvas: Color32::from_rgb(18, 19, 23),
                bg_panel: Color32::from_rgb(25, 26, 32),
                bg_control: Color32::from_rgb(34, 36, 44),
                bg_control_hovered: Color32::from_rgb(46, 49, 60),
                bg_control_active: Color32::from_rgb(58, 62, 76),
                border: Stroke::new(1.0, Color32::from_rgb(48, 52, 64)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(82, 88, 108)),
                text_primary: Color32::from_rgb(235, 238, 245),
                text_muted: Color32::from_rgb(140, 145, 160),
                accent: Color32::from_rgb(59, 130, 246),
                accent_hovered: Color32::from_rgb(96, 165, 250),
                warning: Color32::from_rgb(245, 158, 11),
                success: Color32::from_rgb(34, 197, 94),
                canvas_fg: Color32::from_rgb(235, 238, 245),
                canvas_fg_muted: Color32::from_rgb(140, 145, 160),
                glass_fill: Color32::from_rgba_unmultiplied(59, 130, 246, 55),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(190, 210, 245)),
                plot_tangential: Color32::from_rgb(59, 130, 246),
                plot_sagittal: Color32::from_rgb(34, 197, 94),
                plot_axial: Color32::from_rgb(56, 189, 248),
            },

            // 12. Paper Light (Academic Paper)
            Self::PaperLight => OpticsTokens {
                bg_canvas: Color32::from_rgb(250, 250, 252),
                bg_panel: Color32::from_rgb(241, 243, 247),
                bg_control: Color32::from_rgb(255, 255, 255),
                bg_control_hovered: Color32::from_rgb(233, 236, 242),
                bg_control_active: Color32::from_rgb(218, 222, 232),
                border: Stroke::new(1.0, Color32::from_rgb(210, 214, 224)),
                border_hovered: Stroke::new(1.0, Color32::from_rgb(150, 158, 175)),
                text_primary: Color32::from_rgb(17, 24, 39),
                text_muted: Color32::from_rgb(100, 108, 125),
                accent: Color32::from_rgb(37, 99, 235),
                accent_hovered: Color32::from_rgb(29, 78, 216),
                warning: Color32::from_rgb(217, 119, 6),
                success: Color32::from_rgb(22, 163, 74),
                canvas_fg: Color32::from_rgb(17, 24, 39),
                canvas_fg_muted: Color32::from_rgb(100, 108, 125),
                glass_fill: Color32::from_rgba_unmultiplied(37, 99, 235, 62),
                glass_stroke: Stroke::new(1.4, Color32::from_rgb(17, 24, 39)),
                plot_tangential: Color32::from_rgb(29, 78, 216),
                plot_sagittal: Color32::from_rgb(21, 128, 61),
                plot_axial: Color32::from_rgb(29, 78, 216),
            },
        }
    }
}
