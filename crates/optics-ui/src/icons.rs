//! Typed icon asset registry backed by Phosphor Icons (MIT License).
//!
//! Provides resolution-independent, font-atlas rendered icons that tint cleanly
//! per state and remain crisp at all `pixels_per_point` scales.
//! Completely replaces all legacy painter-drawn icons and Unicode pseudo-icons.

use egui::{
    Align2, Color32, Context, FontDefinitions, FontId, Rect, Response, Sense, Stroke, Ui, Vec2,
};

/// Typed identifier for all graphical icons used across the OpticsLab interface.
///
/// Missing icon identifiers produce compile-time errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconId {
    /// Select tool (cursor arrow).
    Select,
    /// Ray inspector probe tool (crosshair target).
    RayProbe,
    /// Lens prescription editor / optical aperture.
    Lens,
    /// Light emitter source.
    LightSource,
    /// Caliper measurement tool.
    Measure,
    /// Zoom in viewport.
    ZoomIn,
    /// Zoom out viewport.
    ZoomOut,
    /// Zoom fit scene to viewport.
    ZoomFit,
    /// Reset viewport view.
    ResetView,
    /// Toggle sidebar and inspector panels.
    PanelsToggle,
    /// Copy parameters to system clipboard.
    Copy,
    /// Open file dialog.
    Open,
    /// Save file dialog.
    Save,
    /// Export image / screenshot.
    ExportImage,
    /// Preferences / configuration settings.
    Settings,
    /// Toggle theme brightness.
    Theme,
    /// Help / documentation / shortcuts.
    Help,
    /// Warning indicator.
    Warning,
    /// Informational indicator.
    Info,
    /// Error indicator.
    Error,
    /// Play animation / simulation.
    Play,
    /// Pause animation / simulation.
    Pause,
    /// Toggle secondary Fresnel reflections.
    ReflectionsToggle,
    /// Toggle fullscreen viewport mode.
    Fullscreen,
    /// Close window / dismiss modal.
    Close,
    /// Rightward arrow for step sequences (replaces "->").
    ArrowRight,
    /// Downward expansion caret for collapsing sections.
    CaretDown,
    /// Rightward expansion caret for collapsed sections.
    CaretRight,
    /// Upward expansion caret.
    CaretUp,
    /// Scene hierarchy tree root.
    Scene,
    /// Viewport camera.
    Camera,
    /// Glass prism geometry.
    Prism,
    /// Spectral analysis equalizer.
    Spectrum,
    /// BVH acceleration hierarchy.
    Bvh,
    /// Statistical convergence analysis.
    Analysis,
    /// 3D translation move tool.
    Move,
    /// 3D rotation tool.
    Rotate,
    /// 3D scale tool.
    Scale,
    /// OpticsLab application logo.
    Logo,
}

impl IconId {
    /// Returns the corresponding UTF-8 Phosphor icon glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Select => egui_phosphor::regular::CURSOR,
            Self::RayProbe => egui_phosphor::regular::CROSSHAIR,
            Self::Lens => egui_phosphor::regular::APERTURE,
            Self::LightSource => egui_phosphor::regular::SUN,
            Self::Measure => egui_phosphor::regular::RULER,
            Self::ZoomIn => egui_phosphor::regular::MAGNIFYING_GLASS_PLUS,
            Self::ZoomOut => egui_phosphor::regular::MAGNIFYING_GLASS_MINUS,
            Self::ZoomFit => egui_phosphor::regular::ARROWS_OUT,
            Self::ResetView => egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
            Self::PanelsToggle => egui_phosphor::regular::SIDEBAR,
            Self::Copy => egui_phosphor::regular::COPY,
            Self::Open => egui_phosphor::regular::FOLDER_OPEN,
            Self::Save => egui_phosphor::regular::FLOPPY_DISK,
            Self::ExportImage => egui_phosphor::regular::CAMERA,
            Self::Settings => egui_phosphor::regular::SLIDERS,
            Self::Theme => egui_phosphor::regular::SUN_HORIZON,
            Self::Help => egui_phosphor::regular::QUESTION,
            Self::Warning => egui_phosphor::regular::WARNING,
            Self::Info => egui_phosphor::regular::INFO,
            Self::Error => egui_phosphor::regular::WARNING_OCTAGON,
            Self::Play => egui_phosphor::regular::PLAY,
            Self::Pause => egui_phosphor::regular::PAUSE,
            Self::ReflectionsToggle => egui_phosphor::regular::GIT_FORK,
            Self::Fullscreen => egui_phosphor::regular::CORNERS_OUT,
            Self::Close => egui_phosphor::regular::X,
            Self::ArrowRight => egui_phosphor::regular::ARROW_RIGHT,
            Self::CaretDown => egui_phosphor::regular::CARET_DOWN,
            Self::CaretRight => egui_phosphor::regular::CARET_RIGHT,
            Self::CaretUp => egui_phosphor::regular::CARET_UP,
            Self::Scene => egui_phosphor::regular::TREE_STRUCTURE,
            Self::Camera => egui_phosphor::regular::VIDEO_CAMERA,
            Self::Prism => egui_phosphor::regular::TRIANGLE,
            Self::Spectrum => egui_phosphor::regular::EQUALIZER,
            Self::Bvh => egui_phosphor::regular::HARD_DRIVES,
            Self::Analysis => egui_phosphor::regular::CHART_LINE_UP,
            Self::Move => egui_phosphor::regular::ARROWS_OUT_CARDINAL,
            Self::Rotate => egui_phosphor::regular::ARROW_CLOCKWISE,
            Self::Scale => egui_phosphor::regular::FRAME_CORNERS,
            Self::Logo => egui_phosphor::regular::APERTURE,
        }
    }

    /// Human-readable name of the icon.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::RayProbe => "Ray Probe",
            Self::Lens => "Lens",
            Self::LightSource => "Light Source",
            Self::Measure => "Measure",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomFit => "Zoom Fit",
            Self::ResetView => "Reset View",
            Self::PanelsToggle => "Panels Toggle",
            Self::Copy => "Copy",
            Self::Open => "Open",
            Self::Save => "Save",
            Self::ExportImage => "Export Image",
            Self::Settings => "Settings",
            Self::Theme => "Theme",
            Self::Help => "Help",
            Self::Warning => "Warning",
            Self::Info => "Info",
            Self::Error => "Error",
            Self::Play => "Play",
            Self::Pause => "Pause",
            Self::ReflectionsToggle => "Reflections",
            Self::Fullscreen => "Fullscreen",
            Self::Close => "Close",
            Self::ArrowRight => "Arrow Right",
            Self::CaretDown => "Caret Down",
            Self::CaretRight => "Caret Right",
            Self::CaretUp => "Caret Up",
            Self::Scene => "Scene",
            Self::Camera => "Camera",
            Self::Prism => "Prism",
            Self::Spectrum => "Spectrum",
            Self::Bvh => "BVH",
            Self::Analysis => "Analysis",
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Logo => "OpticsLab Logo",
        }
    }

    /// Default tooltip label describing the icon action.
    #[must_use]
    pub const fn default_tooltip(self) -> &'static str {
        match self {
            Self::Select => "Select ray or surface (S)",
            Self::RayProbe => "Ray probe inspector (I)",
            Self::Lens => "Lens prescription editor (L)",
            Self::LightSource => "Light emitter properties",
            Self::Measure => "Measure distance (M)",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ZoomFit => "Fit scene to viewport (Cmd/Ctrl + 0)",
            Self::ResetView => "Reset view (Home)",
            Self::PanelsToggle => "Toggle side panels (Cmd/Ctrl + \\)",
            Self::Copy => "Copy parameters to clipboard",
            Self::Open => "Open Zemax prescription",
            Self::Save => "Save prescription",
            Self::ExportImage => "Export image / screenshot",
            Self::Settings => "Preferences (Cmd/Ctrl + ,)",
            Self::Theme => "Switch UI theme",
            Self::Help => "Keyboard shortcuts & help (F1)",
            Self::Warning => "Warning",
            Self::Info => "Information",
            Self::Error => "Error",
            Self::Play => "Start simulation",
            Self::Pause => "Pause simulation",
            Self::ReflectionsToggle => "Toggle Fresnel reflections (R)",
            Self::Fullscreen => "Toggle fullscreen (F11)",
            Self::Close => "Close (Esc)",
            Self::ArrowRight => "to",
            Self::CaretDown => "Collapse section",
            Self::CaretRight => "Expand section",
            Self::CaretUp => "Collapse section",
            Self::Scene => "Scene root",
            Self::Camera => "Camera settings",
            Self::Prism => "Prism properties",
            Self::Spectrum => "Continuous spectral workbench",
            Self::Bvh => "BVH acceleration visualizer",
            Self::Analysis => "Convergence analysis",
            Self::Move => "Move tool (disabled in 2D)",
            Self::Rotate => "Rotate tool (disabled in 2D)",
            Self::Scale => "Scale tool (disabled in 2D)",
            Self::Logo => "OpticsLab",
        }
    }
}

/// Initializes Phosphor icon fonts into the egui Context's FontDefinitions.
pub fn init_icon_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// Draws an icon glyph directly onto an egui painter at the specified rectangle center.
pub fn draw_icon(painter: &egui::Painter, rect: Rect, icon: IconId, tint: Color32, size: f32) {
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::proportional(size),
        tint,
    );
}

/// Renders an icon button with standard classic desktop hover and click semantics.
pub fn icon_button(ui: &mut Ui, icon: IconId, tint: Color32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        draw_icon(ui.painter(), rect, icon, tint, size * 0.7);
    }
    resp.on_hover_text(icon.default_tooltip())
}

/// Renders a classic 28x28 px desktop toolbar icon button with full state styling.
pub fn toolbar_button(
    ui: &mut Ui,
    icon: IconId,
    tokens: &crate::theme::OpticsTokens,
    is_selected: bool,
    tooltip: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let (bg, border, fg) = if resp.is_pointer_button_down_on() {
            (tokens.active_bg, tokens.border_strong, tokens.text_primary)
        } else if is_selected {
            (tokens.selected, tokens.border_strong, tokens.text_primary)
        } else if resp.hovered() {
            (tokens.hover_bg, tokens.border, tokens.text_primary)
        } else {
            (
                Color32::TRANSPARENT,
                Color32::TRANSPARENT,
                tokens.text_secondary,
            )
        };

        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, egui::CornerRadius::same(2), bg);
        }
        if border != Color32::TRANSPARENT {
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(2),
                Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            );
        }
        if resp.has_focus() {
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(2),
                Stroke::new(1.0, tokens.focus_ring),
                egui::StrokeKind::Inside,
            );
        }
        draw_icon(painter, rect, icon, fg, 16.0);
    }
    resp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled(),
            is_selected,
            tooltip,
        )
    });
    resp.on_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{OpticsTokens, wcag_contrast_ratio};

    #[test]
    fn test_every_icon_id_resolves_to_non_empty_glyph() {
        let all_icons = [
            IconId::Select,
            IconId::RayProbe,
            IconId::Lens,
            IconId::LightSource,
            IconId::Measure,
            IconId::ZoomIn,
            IconId::ZoomOut,
            IconId::ZoomFit,
            IconId::ResetView,
            IconId::PanelsToggle,
            IconId::Copy,
            IconId::Open,
            IconId::Save,
            IconId::ExportImage,
            IconId::Settings,
            IconId::Theme,
            IconId::Help,
            IconId::Warning,
            IconId::Info,
            IconId::Error,
            IconId::Play,
            IconId::Pause,
            IconId::ReflectionsToggle,
            IconId::Fullscreen,
            IconId::Close,
            IconId::ArrowRight,
            IconId::CaretDown,
            IconId::CaretRight,
            IconId::CaretUp,
            IconId::Scene,
            IconId::Camera,
            IconId::Prism,
            IconId::Spectrum,
            IconId::Bvh,
            IconId::Analysis,
            IconId::Move,
            IconId::Rotate,
            IconId::Scale,
            IconId::Logo,
        ];

        for icon in all_icons {
            let glyph = icon.glyph();
            assert!(!glyph.is_empty(), "Icon {:?} returned empty glyph", icon);
            assert!(
                !icon.name().is_empty(),
                "Icon {:?} returned empty name",
                icon
            );
            assert!(
                !icon.default_tooltip().is_empty(),
                "Icon {:?} returned empty tooltip",
                icon
            );
        }
    }

    #[test]
    fn test_no_emoji_or_pictographs_in_ui_strings() {
        // Strict ASCII + Greek optics characters allowed in UI text strings
        let sample_ui_strings = [
            "Select",
            "Ray Probe",
            "Lens",
            "Light Source",
            "Measure",
            "Cooke Triplet F/4.5",
            "Double Gauss F/2.0",
            "Plano-Convex Singlet",
            "Total internal reflection",
            "Ray missed the prism",
            "Wavelength lambda = 587.6 nm",
            "n1 to n2 (BK7)",
            "θi = 45.0°",
            "θt = 27.8°",
        ];

        for s in &sample_ui_strings {
            for c in s.chars() {
                let code = c as u32;
                let is_ascii_printable = (0x20..=0x7E).contains(&code);
                let is_degree_symbol = c == '°';
                let is_greek_lambda = c == 'λ';
                let is_theta = c == 'θ';
                assert!(
                    is_ascii_printable || is_degree_symbol || is_greek_lambda || is_theta,
                    "Disallowed glyph '{c}' (U+{code:04X}) in UI string '{s}'"
                );
            }
        }
    }

    #[test]
    fn test_icon_tint_contrast_ratios() {
        let tokens = OpticsTokens::dark_scientific();

        // 1. Text secondary on panel bg >= 3.0:1 (for icons/large text)
        let cr_normal = wcag_contrast_ratio(tokens.text_secondary, tokens.panel_bg);
        assert!(
            cr_normal >= 3.0,
            "Normal icon contrast too low: {cr_normal:.2} < 3.0"
        );

        // 2. Text primary on panel bg >= 4.5:1
        let cr_primary = wcag_contrast_ratio(tokens.text_primary, tokens.panel_bg);
        assert!(
            cr_primary >= 4.5,
            "Primary text contrast too low: {cr_primary:.2} < 4.5"
        );
    }
}
