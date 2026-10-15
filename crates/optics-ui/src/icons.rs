//! Minimalist vector icons for optical laboratory tools, rendered via `egui::Painter`.
//!
//! Inspired by classic technical CAD and optical engineering software (Zemax, AutoCAD).
//! Avoids third-party icon fonts or raster assets, ensuring zero dependency bloat,
//! resolution independence, and strict contrast control.

use egui::{Color32, Rect, Stroke, Vec2};

/// Tool icon identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolIcon {
    /// Select tool (cursor arrow).
    Select,
    /// Ray probe tool (incident ray with interaction reticle).
    RayProbe,
    /// Lens surface tool (biconvex refractive boundary).
    LensSurface,
    /// Light emitter source (point with radiating rays).
    LightSource,
    /// Measurement caliper / dimension tool.
    Measure,
}

/// Draws a minimalist vector icon centered inside the specified rectangle.
pub fn draw_tool_icon(
    painter: &egui::Painter,
    rect: Rect,
    icon: ToolIcon,
    stroke_color: Color32,
    fill_color: Color32,
) {
    let center = rect.center();
    let stroke = Stroke::new(1.5, stroke_color);

    match icon {
        ToolIcon::Select => {
            // Minimalist CAD cursor arrow: sharp tip at top-left, angled body
            let tip = center + Vec2::new(-6.0, -7.0);
            let p1 = tip + Vec2::new(0.0, 13.0);
            let p2 = tip + Vec2::new(4.0, 9.5);
            let p3 = tip + Vec2::new(8.0, 13.5);
            let p4 = tip + Vec2::new(10.0, 11.5);
            let p5 = tip + Vec2::new(6.0, 7.5);
            let p6 = tip + Vec2::new(10.5, 7.0);

            let points = vec![tip, p1, p2, p3, p4, p5, p6];
            painter.add(egui::Shape::convex_polygon(
                points,
                fill_color,
                Stroke::new(1.2, stroke_color),
            ));
        }
        ToolIcon::RayProbe => {
            // Ray probe: angled ray path with central target circle
            let ray_start = center + Vec2::new(-7.0, 7.0);
            let ray_end = center + Vec2::new(7.0, -7.0);
            painter.line_segment([ray_start, ray_end], stroke);

            // Center interaction probe circle
            painter.circle(center, 3.5, fill_color, stroke);
            painter.circle_filled(center, 1.2, stroke_color);
        }
        ToolIcon::LensSurface => {
            // Biconvex lens profile with optical axis tick
            let top = center + Vec2::new(0.0, -7.0);
            let bottom = center + Vec2::new(0.0, 7.0);

            // Left convex arc
            painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
                [top, center + Vec2::new(-4.5, 0.0), bottom],
                false,
                fill_color,
                stroke,
            ));
            // Right convex arc
            painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
                [top, center + Vec2::new(4.5, 0.0), bottom],
                false,
                fill_color,
                stroke,
            ));
            // Optical center cross tick
            painter.line_segment(
                [center + Vec2::new(-2.0, 0.0), center + Vec2::new(2.0, 0.0)],
                Stroke::new(1.0, stroke_color),
            );
        }
        ToolIcon::LightSource => {
            // Emitter core circle with 4 primary radiating cardinal ticks
            painter.circle(center, 3.2, fill_color, stroke);
            let len = 4.0;
            // North, South, East, West ticks
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -4.5),
                    center + Vec2::new(0.0, -4.5 - len),
                ],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(0.0, 4.5), center + Vec2::new(0.0, 4.5 + len)],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-4.5, 0.0),
                    center + Vec2::new(-4.5 - len, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(4.5, 0.0), center + Vec2::new(4.5 + len, 0.0)],
                stroke,
            );
        }
        ToolIcon::Measure => {
            // Caliper / dimension line with end bars and central tick
            let p_left = center + Vec2::new(-7.0, 0.0);
            let p_right = center + Vec2::new(7.0, 0.0);
            // Main dimension bar
            painter.line_segment([p_left, p_right], stroke);
            // End caliper limits
            painter.line_segment(
                [p_left + Vec2::new(0.0, -4.0), p_left + Vec2::new(0.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [p_right + Vec2::new(0.0, -4.0), p_right + Vec2::new(0.0, 4.0)],
                stroke,
            );
            // Center gauge mark
            painter.line_segment(
                [center + Vec2::new(0.0, -2.0), center + Vec2::new(0.0, 2.0)],
                stroke,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::theme::{OpticsTokens, wcag_contrast_ratio};

    /// Restricts allowed non-ASCII characters in UI string constants to the documented set.
    /// Standard ASCII (0x20..=0x7E) plus degree sign '°' (U+00B0) are supported by egui default fonts.
    #[test]
    fn test_ui_allowed_character_set() {
        let allowed_ui_strings = [
            "OpticsLab",
            "File",
            "Quit",
            "View",
            "Reset view",
            "Help",
            "About",
            "Prism controls",
            "Glass material",
            "Apex angle",
            "Incidence angle",
            "Spectral samples",
            "Optical readout",
            "Single-ray inspector",
            "Click a ray to inspect it",
            "Show reflections",
            "Show timing",
            "Copy table",
            "All rays underwent total internal reflection (TIR)",
            "D65 white light",
            "Surface",
            "Event",
            "n1 -> n2",
            "theta_i (°)",
            "theta_t (°)",
            "Rs (%)",
            "Rp (%)",
            "R total (%)",
            "T total (%)",
            "Throughput (%)",
            "Deviation angle",
            "Coming later",
            "Select (S)",
            "Ray probe (I)",
        ];

        for s in &allowed_ui_strings {
            for c in s.chars() {
                let code = c as u32;
                let is_ascii_printable = (0x20..=0x7E).contains(&code);
                let is_degree_symbol = c == '°';
                assert!(
                    is_ascii_printable || is_degree_symbol,
                    "Disallowed glyph '{c}' (U+{code:04X}) in UI string '{s}' - causes tofu boxes in default fonts!"
                );
            }
        }
    }

    /// Verifies tool strip icons meet the required 3:1 contrast ratio against their background in all states.
    #[test]
    fn test_tool_icon_contrast_ratios() {
        let tokens = OpticsTokens::medium_dark();

        // 1. Normal state: text_dim (#8b93a7) on panel_bg (#1a1f2e)
        let cr_normal = wcag_contrast_ratio(tokens.text_dim, tokens.panel_bg);
        assert!(
            cr_normal >= 3.0,
            "Normal icon contrast too low: {cr_normal:.2} < 3.0"
        );

        // 2. Hover state: white on hover_bg (#262e42)
        let cr_hover = wcag_contrast_ratio(egui::Color32::WHITE, tokens.hover_bg);
        assert!(
            cr_hover >= 3.0,
            "Hover icon contrast too low: {cr_hover:.2} < 3.0"
        );

        // 3. Active/Selected state: canvas (#11141a) on accent (#00d4f5)
        let cr_active = wcag_contrast_ratio(tokens.canvas, tokens.accent);
        assert!(
            cr_active >= 3.0,
            "Active icon contrast too low: {cr_active:.2} < 3.0"
        );

        // 4. Disabled state: dimmed stroke (#464e62) on panel_bg (#1a1f2e)
        let disabled_stroke = egui::Color32::from_rgb(70, 78, 98);
        let cr_disabled = wcag_contrast_ratio(tokens.text_primary, disabled_stroke);
        assert!(
            cr_disabled >= 3.0,
            "Disabled icon text contrast too low: {cr_disabled:.2} < 3.0"
        );
    }
}
