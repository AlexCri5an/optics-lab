//! Minimalist vector icons for optical laboratory tools, rendered via `egui::Painter`.
//!
//! Inspired by classic technical CAD, RenderDoc, and scientific desktop tools.
//! Avoids third-party raster assets or icon fonts, ensuring zero dependency bloat,
//! resolution independence, and strict contrast control.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Tool and action icon identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolIcon {
    /// Select tool (cursor arrow).
    Select,
    /// Move tool (4-directional cross arrows).
    Move,
    /// Rotate tool (circular curved arrow).
    Rotate,
    /// Scale tool (bounding box corner resize).
    Scale,
    /// Ray inspector probe tool (crosshair target with ray).
    RayInspector,
    /// Spectral equalizer / bands tool.
    Spectrum,
    /// BVH acceleration hierarchy tool (wireframe bounding cubes).
    Bvh,
    /// Analytics / line chart tool.
    Analysis,
    /// Camera tool.
    Camera,
    /// Settings / gear tool.
    Settings,

    // Hierarchy icons
    /// Scene hierarchy root icon.
    Scene,
    /// Glass prism geometry icon.
    Prism,
    /// Glass node (concentric circular node).
    GlassNode,
    /// Material swatch / node.
    MaterialNode,
    /// Light emitter source.
    Light,
    /// Ground perspective plane / grid.
    Ground,

    // Viewport controls
    /// Camera screenshot / capture icon.
    Screenshot,
    /// Frame / focus brackets.
    Frame,
    /// Fullscreen expand.
    Fullscreen,

    // Window controls
    /// Window minimize bar.
    Minimize,
    /// Window maximize square.
    Maximize,
    /// Window close cross.
    Close,

    // General icons
    /// Logo icon (prism triangle with internal refraction).
    Logo,
    /// Ray probe alias.
    RayProbe,
    /// Lens surface tool.
    LensSurface,
    /// Light source tool alias.
    LightSource,
    /// Measurement caliper tool.
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
    let stroke = Stroke::new(1.2, stroke_color);

    match icon {
        ToolIcon::Select => {
            // Minimalist CAD cursor arrow: sharp tip at top-left, angled body
            let tip = center + Vec2::new(-5.5, -6.5);
            let p1 = tip + Vec2::new(0.0, 12.0);
            let p2 = tip + Vec2::new(3.5, 9.0);
            let p3 = tip + Vec2::new(7.0, 12.5);
            let p4 = tip + Vec2::new(9.0, 10.5);
            let p5 = tip + Vec2::new(5.5, 7.0);
            let p6 = tip + Vec2::new(9.5, 6.5);

            let points = vec![tip, p1, p2, p3, p4, p5, p6];
            painter.add(egui::Shape::convex_polygon(
                points,
                fill_color,
                Stroke::new(1.2, stroke_color),
            ));
        }
        ToolIcon::Move => {
            // 4-way cross arrows
            let len = 6.5;
            painter.line_segment(
                [center - Vec2::new(len, 0.0), center + Vec2::new(len, 0.0)],
                stroke,
            );
            painter.line_segment(
                [center - Vec2::new(0.0, len), center + Vec2::new(0.0, len)],
                stroke,
            );
            // Arrowheads
            let d = 2.5;
            // Left
            painter.line_segment(
                [
                    center + Vec2::new(-len, 0.0),
                    center + Vec2::new(-len + d, -d),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-len, 0.0),
                    center + Vec2::new(-len + d, d),
                ],
                stroke,
            );
            // Right
            painter.line_segment(
                [
                    center + Vec2::new(len, 0.0),
                    center + Vec2::new(len - d, -d),
                ],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(len, 0.0), center + Vec2::new(len - d, d)],
                stroke,
            );
            // Top
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -len),
                    center + Vec2::new(-d, -len + d),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -len),
                    center + Vec2::new(d, -len + d),
                ],
                stroke,
            );
            // Bottom
            painter.line_segment(
                [
                    center + Vec2::new(0.0, len),
                    center + Vec2::new(-d, len - d),
                ],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(0.0, len), center + Vec2::new(d, len - d)],
                stroke,
            );
        }
        ToolIcon::Rotate => {
            // Circular arc with arrow
            let radius = 6.0;
            let mut points = Vec::with_capacity(16);
            for i in 0..=12 {
                let angle =
                    (i as f32 / 12.0) * std::f32::consts::PI * 1.5 - std::f32::consts::PI * 0.75;
                points.push(center + Vec2::new(angle.cos() * radius, angle.sin() * radius));
            }
            painter.add(egui::epaint::PathShape::line(points, stroke));
            // Arrowhead at end
            let end = center + Vec2::new(radius, 0.0);
            painter.line_segment([end, end + Vec2::new(-3.0, -2.5)], stroke);
            painter.line_segment([end, end + Vec2::new(-3.0, 2.5)], stroke);
        }
        ToolIcon::Scale => {
            // Diagonal expand arrows / bounding box resize
            let s = 5.5;
            let p_tl = center + Vec2::new(-s, -s);
            let p_br = center + Vec2::new(s, s);
            painter.line_segment([p_tl, p_br], stroke);
            let d = 2.5;
            // Top-left arrow
            painter.line_segment([p_tl, p_tl + Vec2::new(0.0, d)], stroke);
            painter.line_segment([p_tl, p_tl + Vec2::new(d, 0.0)], stroke);
            // Bottom-right arrow
            painter.line_segment([p_br, p_br - Vec2::new(0.0, d)], stroke);
            painter.line_segment([p_br, p_br - Vec2::new(d, 0.0)], stroke);
            // Small guide box corners
            painter.rect_stroke(
                Rect::from_center_size(center, Vec2::new(s * 1.4, s * 1.4)),
                egui::CornerRadius::ZERO,
                Stroke::new(
                    0.8,
                    Color32::from_rgba_premultiplied(
                        stroke_color.r(),
                        stroke_color.g(),
                        stroke_color.b(),
                        100,
                    ),
                ),
                egui::StrokeKind::Inside,
            );
        }
        ToolIcon::RayInspector | ToolIcon::RayProbe => {
            // Target reticle with ray line
            let r = 5.5;
            painter.circle_stroke(center, r, stroke);
            painter.circle_stroke(center, 2.0, Stroke::new(1.0, stroke_color));
            // Crosshairs
            painter.line_segment(
                [
                    center - Vec2::new(r + 2.0, 0.0),
                    center - Vec2::new(2.5, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(2.5, 0.0),
                    center + Vec2::new(r + 2.0, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center - Vec2::new(0.0, r + 2.0),
                    center - Vec2::new(0.0, 2.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.0, 2.5),
                    center + Vec2::new(0.0, r + 2.0),
                ],
                stroke,
            );
        }
        ToolIcon::Spectrum => {
            // Multi-bar spectral chart icon
            let bar_w = 2.0;
            let heights = [4.0, 7.0, 11.0, 8.0, 5.0];
            let start_x = center.x - 6.0;
            for (i, &h) in heights.iter().enumerate() {
                let x = start_x + (i as f32) * 3.0;
                let bottom = center.y + 6.0;
                let bar_rect =
                    Rect::from_min_max(Pos2::new(x, bottom - h), Pos2::new(x + bar_w, bottom));
                painter.rect_filled(bar_rect, egui::CornerRadius::ZERO, stroke_color);
            }
        }
        ToolIcon::Bvh => {
            // Nested 3D bounding cubes
            let s = 6.0;
            let front = Rect::from_center_size(center + Vec2::new(-1.5, 1.5), Vec2::new(s, s));
            let back = Rect::from_center_size(center + Vec2::new(1.5, -1.5), Vec2::new(s, s));
            painter.rect_stroke(
                back,
                egui::CornerRadius::ZERO,
                Stroke::new(0.8, stroke_color),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                front,
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
            // Connecting edge lines
            painter.line_segment([front.left_top(), back.left_top()], stroke);
            painter.line_segment([front.right_top(), back.right_top()], stroke);
            painter.line_segment([front.right_bottom(), back.right_bottom()], stroke);
            painter.line_segment([front.left_bottom(), back.left_bottom()], stroke);
        }
        ToolIcon::Analysis => {
            // Analytics line chart with axes
            let p_origin = center + Vec2::new(-6.0, 6.0);
            painter.line_segment([p_origin, p_origin + Vec2::new(12.0, 0.0)], stroke);
            painter.line_segment([p_origin, p_origin + Vec2::new(0.0, -12.0)], stroke);
            // Trend curve
            let pts = [
                p_origin + Vec2::new(1.0, -2.0),
                p_origin + Vec2::new(4.0, -8.0),
                p_origin + Vec2::new(7.0, -5.0),
                p_origin + Vec2::new(11.0, -11.0),
            ];
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], Stroke::new(1.4, stroke_color));
            }
        }
        ToolIcon::Camera | ToolIcon::Screenshot => {
            // Camera body and lens circle
            let body = Rect::from_center_size(center + Vec2::new(0.0, 1.0), Vec2::new(13.0, 9.0));
            painter.rect_stroke(
                body,
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            // Top flash bump
            let flash = Rect::from_min_max(
                center + Vec2::new(-2.5, -5.5),
                center + Vec2::new(2.5, -3.5),
            );
            painter.rect_stroke(
                flash,
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
            // Lens circle
            painter.circle_stroke(center + Vec2::new(0.0, 1.0), 2.8, stroke);
        }
        ToolIcon::Settings => {
            // Gear / settings cog
            painter.circle_stroke(center, 4.5, stroke);
            painter.circle_stroke(center, 2.0, Stroke::new(1.0, stroke_color));
            for i in 0..6 {
                let angle = (i as f32) * std::f32::consts::PI / 3.0;
                let dir = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + dir * 4.5, center + dir * 6.5],
                    Stroke::new(1.6, stroke_color),
                );
            }
        }
        ToolIcon::Scene => {
            // Hierarchy node cube
            let rect_s = Rect::from_center_size(center, Vec2::new(9.0, 9.0));
            painter.rect_stroke(
                rect_s,
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [rect_s.left_center(), rect_s.right_center()],
                Stroke::new(0.8, stroke_color),
            );
            painter.line_segment(
                [rect_s.center_top(), rect_s.center_bottom()],
                Stroke::new(0.8, stroke_color),
            );
        }
        ToolIcon::Prism => {
            // Triangular prism glyph
            let top = center + Vec2::new(0.0, -6.0);
            let bl = center + Vec2::new(-6.0, 5.5);
            let br = center + Vec2::new(6.0, 5.5);
            painter.add(egui::Shape::convex_polygon(
                vec![top, br, bl],
                fill_color,
                stroke,
            ));
        }
        ToolIcon::GlassNode => {
            // Glass circle node
            painter.circle(center, 4.0, fill_color, stroke);
            painter.circle_filled(center, 1.5, stroke_color);
        }
        ToolIcon::MaterialNode => {
            // Diamond / material swatch node
            let top = center + Vec2::new(0.0, -5.0);
            let right = center + Vec2::new(5.0, 0.0);
            let bottom = center + Vec2::new(0.0, 5.0);
            let left = center + Vec2::new(-5.0, 0.0);
            painter.add(egui::Shape::convex_polygon(
                vec![top, right, bottom, left],
                fill_color,
                stroke,
            ));
        }
        ToolIcon::Light | ToolIcon::LightSource => {
            // Sun emitter core circle with 4 rays
            painter.circle(center, 3.0, fill_color, stroke);
            let len = 3.5;
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -4.0),
                    center + Vec2::new(0.0, -4.0 - len),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.0, 4.0),
                    center + Vec2::new(0.0, 4.0 + len),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-4.0, 0.0),
                    center + Vec2::new(-4.0 - len, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(4.0, 0.0),
                    center + Vec2::new(4.0 + len, 0.0),
                ],
                stroke,
            );
        }
        ToolIcon::Ground => {
            // Perspective floor grid
            let p_bl = center + Vec2::new(-6.5, 4.0);
            let p_br = center + Vec2::new(6.5, 4.0);
            let p_tl = center + Vec2::new(-3.5, -4.0);
            let p_tr = center + Vec2::new(3.5, -4.0);
            painter.line_segment([p_bl, p_br], stroke);
            painter.line_segment([p_tl, p_tr], stroke);
            painter.line_segment([p_bl, p_tl], stroke);
            painter.line_segment([p_br, p_tr], stroke);
            painter.line_segment(
                [center + Vec2::new(0.0, 4.0), center + Vec2::new(0.0, -4.0)],
                Stroke::new(0.8, stroke_color),
            );
        }
        ToolIcon::Frame => {
            // Focus frame brackets [ ]
            let s = 5.0;
            let d = 2.5;
            // TL
            painter.line_segment(
                [center + Vec2::new(-s, -s), center + Vec2::new(-s + d, -s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-s, -s), center + Vec2::new(-s, -s + d)],
                stroke,
            );
            // TR
            painter.line_segment(
                [center + Vec2::new(s, -s), center + Vec2::new(s - d, -s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(s, -s), center + Vec2::new(s, -s + d)],
                stroke,
            );
            // BL
            painter.line_segment(
                [center + Vec2::new(-s, s), center + Vec2::new(-s + d, s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-s, s), center + Vec2::new(-s, s - d)],
                stroke,
            );
            // BR
            painter.line_segment(
                [center + Vec2::new(s, s), center + Vec2::new(s - d, s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(s, s), center + Vec2::new(s, s - d)],
                stroke,
            );
        }
        ToolIcon::Fullscreen => {
            // 4 corner expand arrows
            let s = 5.5;
            let d = 2.0;
            // TL arrow
            painter.line_segment(
                [center + Vec2::new(-s, -s), center + Vec2::new(-s + d, -s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-s, -s), center + Vec2::new(-s, -s + d)],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-s, -s),
                    center + Vec2::new(-s + 3.0, -s + 3.0),
                ],
                stroke,
            );
            // BR arrow
            painter.line_segment(
                [center + Vec2::new(s, s), center + Vec2::new(s - d, s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(s, s), center + Vec2::new(s, s - d)],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(s, s),
                    center + Vec2::new(s - 3.0, s - 3.0),
                ],
                stroke,
            );
        }
        ToolIcon::Minimize => {
            painter.line_segment(
                [center + Vec2::new(-4.5, 0.0), center + Vec2::new(4.5, 0.0)],
                stroke,
            );
        }
        ToolIcon::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(center, Vec2::new(9.0, 9.0)),
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        ToolIcon::Close => {
            let d = 4.0;
            painter.line_segment(
                [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(d, -d), center + Vec2::new(-d, d)],
                stroke,
            );
        }
        ToolIcon::Logo => {
            // Delta triangle with refracted internal ray
            let top = center + Vec2::new(0.0, -6.5);
            let bl = center + Vec2::new(-6.5, 5.5);
            let br = center + Vec2::new(6.5, 5.5);
            painter.add(egui::Shape::convex_polygon(
                vec![top, br, bl],
                fill_color,
                stroke,
            ));
            // Internal ray segment
            painter.line_segment(
                [center + Vec2::new(-3.5, 0.5), center + Vec2::new(2.5, 1.5)],
                Stroke::new(1.0, Color32::from_rgb(0x00, 0xd4, 0xf5)),
            );
        }
        ToolIcon::LensSurface => {
            let top = center + Vec2::new(0.0, -7.0);
            let bottom = center + Vec2::new(0.0, 7.0);
            painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
                [top, center + Vec2::new(-4.5, 0.0), bottom],
                false,
                fill_color,
                stroke,
            ));
            painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
                [top, center + Vec2::new(4.5, 0.0), bottom],
                false,
                fill_color,
                stroke,
            ));
        }
        ToolIcon::Measure => {
            let p_left = center + Vec2::new(-7.0, 0.0);
            let p_right = center + Vec2::new(7.0, 0.0);
            painter.line_segment([p_left, p_right], stroke);
            painter.line_segment(
                [p_left + Vec2::new(0.0, -4.0), p_left + Vec2::new(0.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [
                    p_right + Vec2::new(0.0, -4.0),
                    p_right + Vec2::new(0.0, 4.0),
                ],
                stroke,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::theme::{OpticsTokens, wcag_contrast_ratio};

    #[test]
    fn test_ui_allowed_character_set() {
        let allowed_ui_strings = [
            "OpticsLab",
            "File",
            "Edit",
            "View",
            "Render",
            "Diagnostics",
            "Help",
            "Select",
            "Move",
            "Rotate",
            "Scale",
            "Ray Inspector",
            "Spectrum",
            "BVH",
            "Analysis",
            "Camera",
            "Settings",
            "Scene",
            "Prism",
            "Glass (BK7)",
            "Material",
            "Light",
            "Ground",
            "3D View",
            "2D View",
            "Spectral",
            "Convergence",
            "Convergence",
            "RMSE",
            "Variance",
            "Samples",
            "error",
            "Spectral Power",
            "λ (nm)",
            "BVH Diagnostics",
            "TLAS",
            "BLAS",
            "Traversal Cost",
            "High",
            "Low",
            "Median",
            "P95",
            "Max",
            "Material / Glass",
            "BK7 (Schott)",
            "n (587.6 nm)",
            "Abbe Vd",
            "Dispersion",
            "Cauchy",
            "Sellmeier",
            "Yes",
            "Edit Material",
            "Pixel (842, 412) -> Ray #1842",
            "λ = 587.56 nm (green)",
            "Bounce 0",
            "Bounce 1",
            "Bounce 2",
            "Path",
            "Surface",
            "1 (prism)",
            "n1",
            "1.0000",
            "n2",
            "1.5168 (BK7)",
            "theta_i (°)",
            "theta_t (°)",
            "Rs (%)",
            "Rp (%)",
            "R (total) (%)",
            "T (total) (%)",
            "Event",
            "Refraction",
            "Ready",
            "128 spp | 1920x1080 | GPU: Vulkan | 60 FPS",
        ];

        for s in &allowed_ui_strings {
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
    fn test_tool_icon_contrast_ratios() {
        let tokens = OpticsTokens::dark_scientific();

        // 1. Secondary text (#8995a3) on panel (#0d141c)
        let cr_normal = wcag_contrast_ratio(tokens.text_secondary, tokens.panel_bg);
        assert!(
            cr_normal >= 4.5,
            "Normal icon contrast too low: {cr_normal:.2} < 4.5"
        );

        // 2. Primary text (#d8dee6) on panel (#0d141c)
        let cr_primary = wcag_contrast_ratio(tokens.text_primary, tokens.panel_bg);
        assert!(
            cr_primary >= 4.5,
            "Primary text contrast too low: {cr_primary:.2} < 4.5"
        );
    }
}
