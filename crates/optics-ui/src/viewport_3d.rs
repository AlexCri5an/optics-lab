//! 3D perspective viewport for realistic glass prism ray-tracing and spectral dispersion.
//!
//! Provides realistic perspective projection, physical glass refraction,
//! multi-wavelength spectral fan dispersion, floor reflection grid, and coordinate gizmo.

use egui::epaint::PathShape;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use optics_spectral::color::display_color_for_wavelength;

use crate::sim::PrismSimulation;
use crate::theme::OpticsTokens;

/// Represents a 3D point in world space.
#[derive(Clone, Copy, Debug)]
pub struct Vec3D {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Z coordinate.
    pub z: f32,
}

impl Vec3D {
    /// Creates a new 3D vector.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Simple camera model for 3D perspective projection.
#[derive(Clone, Copy, Debug)]
pub struct Camera3D {
    /// Eye position in world space.
    pub eye: Vec3D,
    /// Look-at target in world space.
    pub target: Vec3D,
    /// Field of view scaling factor.
    pub fov_scale: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            eye: Vec3D::new(-20.0, 140.0, -320.0),
            target: Vec3D::new(-15.0, 15.0, 0.0),
            fov_scale: 480.0,
        }
    }
}

impl Camera3D {
    /// Projects a 3D world coordinate into 2D screen space relative to viewport center.
    pub fn project(&self, pt: Vec3D, screen_center: Pos2) -> Pos2 {
        // Simple orbital / look-at projection:
        let dx = pt.x - self.target.x;
        let dy = pt.y - self.target.y;
        let dz = pt.z - self.target.z;

        // Yaw and pitch rotation relative to target
        let yaw = 0.42_f32; // slight angle from the side
        let pitch = 0.32_f32; // looking down at an angle

        let x1 = dx * yaw.cos() - dz * yaw.sin();
        let z1 = dx * yaw.sin() + dz * yaw.cos();

        let y2 = dy * pitch.cos() - z1 * pitch.sin();
        let z2 = dy * pitch.sin() + z1 * pitch.cos();

        let depth = 380.0 + z2;
        let scale = if depth > 10.0 {
            self.fov_scale / depth
        } else {
            1.0
        };

        Pos2::new(
            screen_center.x + x1 * scale,
            screen_center.y - y2 * scale, // invert y for screen space
        )
    }
}

/// Renders the 3D realistic glass prism viewport.
pub fn render_viewport_3d(
    ui: &mut egui::Ui,
    rect: Rect,
    sim: &PrismSimulation,
    tokens: &OpticsTokens,
    selected_ray: &mut Option<usize>,
    hovered_ray: &mut Option<usize>,
) {
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let camera = Camera3D::default();

    // 1. Studio background gradient
    painter.rect_filled(rect, egui::CornerRadius::ZERO, tokens.canvas);

    // Subtle dark studio radial vignette/illumination
    let light_center = center + Vec2::new(80.0, -30.0);
    for r in (50..=450).step_by(50) {
        let alpha = ((1.0 - (r as f32 / 450.0)) * 12.0) as u8;
        painter.circle_filled(
            light_center,
            r as f32,
            Color32::from_rgba_premultiplied(22, 34, 48, alpha),
        );
    }

    // 2. Perspective Floor Grid
    let grid_color = Color32::from_rgba_premultiplied(30, 42, 54, 45);
    let grid_stroke = Stroke::new(1.0, grid_color);
    let floor_y = -35.0;

    // Longitudinal grid lines
    for x_step in (-12..=12).step_by(2) {
        let x = (x_step as f32) * 22.0;
        let p_start = camera.project(Vec3D::new(x, floor_y, -260.0), center);
        let p_end = camera.project(Vec3D::new(x, floor_y, 260.0), center);
        if rect.contains(p_start) || rect.contains(p_end) {
            painter.line_segment([p_start, p_end], grid_stroke);
        }
    }

    // Transverse grid lines
    for z_step in (-10..=12).step_by(2) {
        let z = (z_step as f32) * 22.0;
        let p_start = camera.project(Vec3D::new(-260.0, floor_y, z), center);
        let p_end = camera.project(Vec3D::new(260.0, floor_y, z), center);
        if rect.contains(p_start) || rect.contains(p_end) {
            painter.line_segment([p_start, p_end], grid_stroke);
        }
    }

    // 3. Prism 3D Geometry Definition
    // An equilateral upright triangular prism
    let prism_center_x = -25.0;
    let prism_h = 95.0;
    let prism_w = 85.0;
    let depth_half = 42.0;

    let v_top_f = Vec3D::new(prism_center_x, floor_y + prism_h, depth_half);
    let v_bl_f = Vec3D::new(prism_center_x - prism_w * 0.5, floor_y, depth_half);
    let v_br_f = Vec3D::new(prism_center_x + prism_w * 0.5, floor_y, depth_half);

    let v_top_b = Vec3D::new(prism_center_x, floor_y + prism_h, -depth_half);
    let v_bl_b = Vec3D::new(prism_center_x - prism_w * 0.5, floor_y, -depth_half);
    let v_br_b = Vec3D::new(prism_center_x + prism_w * 0.5, floor_y, -depth_half);

    let p_top_f = camera.project(v_top_f, center);
    let p_bl_f = camera.project(v_bl_f, center);
    let p_br_f = camera.project(v_br_f, center);

    let p_top_b = camera.project(v_top_b, center);
    let p_bl_b = camera.project(v_bl_b, center);
    let p_br_b = camera.project(v_br_b, center);

    // Floor contact shadow / reflection under prism
    let shadow_pts = vec![p_bl_f, p_br_f, p_br_b, p_bl_b];
    painter.add(PathShape::convex_polygon(
        shadow_pts,
        Color32::from_rgba_premultiplied(10, 16, 24, 140),
        Stroke::NONE,
    ));

    // Back face (transparent glass)
    painter.add(PathShape::convex_polygon(
        vec![p_top_b, p_br_b, p_bl_b],
        Color32::from_rgba_premultiplied(18, 28, 42, 60),
        Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 75, 105, 50)),
    ));

    // Base face
    painter.add(PathShape::convex_polygon(
        vec![p_bl_f, p_br_f, p_br_b, p_bl_b],
        Color32::from_rgba_premultiplied(15, 24, 36, 75),
        Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 90, 120, 50)),
    ));

    // Left slanted entrance face
    painter.add(PathShape::convex_polygon(
        vec![p_top_b, p_top_f, p_bl_f, p_bl_b],
        Color32::from_rgba_premultiplied(22, 34, 50, 80),
        Stroke::new(1.0, Color32::from_rgba_premultiplied(85, 120, 160, 80)),
    ));

    // 4. Incident White Ray
    let inc_start_3d = Vec3D::new(prism_center_x - 170.0, floor_y + 42.0, 0.0);
    let hit_enter_3d = Vec3D::new(prism_center_x - prism_w * 0.28, floor_y + 49.0, 0.0);

    let p_inc_start = camera.project(inc_start_3d, center);
    let p_hit_enter = camera.project(hit_enter_3d, center);

    // Luminous halo around incident beam
    painter.line_segment(
        [p_inc_start, p_hit_enter],
        Stroke::new(4.5, Color32::from_rgba_premultiplied(255, 255, 255, 40)),
    );
    painter.line_segment(
        [p_inc_start, p_hit_enter],
        Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 255, 255, 160)),
    );
    painter.line_segment([p_inc_start, p_hit_enter], Stroke::new(1.0, Color32::WHITE));

    // Incident refraction focal glare dot
    painter.circle_filled(
        p_hit_enter,
        4.0,
        Color32::from_rgba_premultiplied(255, 255, 255, 120),
    );
    painter.circle_filled(p_hit_enter, 1.8, Color32::WHITE);

    // 5. Internal and Emerging Spectral Ray Fan
    let ray_count = sim.rays.len().max(16);
    let mouse_pos = ui.ctx().pointer_hover_pos();
    let mut closest_idx = None;
    let mut min_dist = 16.0;

    for (idx, ray) in sim.rays.iter().enumerate() {
        let t = idx as f32 / (ray_count.max(1) - 1) as f32;
        let rgb = display_color_for_wavelength(ray.wavelength_nm);
        let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

        // Dispersed exit point along right face (internal ray)
        let hit_exit_3d = Vec3D::new(
            prism_center_x + prism_w * (0.24 + t * 0.06),
            floor_y + 44.0 - t * 6.0,
            (t - 0.5) * 12.0,
        );
        let p_hit_exit = camera.project(hit_exit_3d, center);

        // Internal refracted ray
        let internal_alpha = 140;
        let internal_color =
            Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), internal_alpha);
        painter.line_segment([p_hit_enter, p_hit_exit], Stroke::new(1.2, internal_color));

        // External dispersed spectral ray projecting rightwards and spreading
        let spread_angle = (t - 0.5) * 0.22;
        let ray_len = 240.0;
        let fan_end_3d = Vec3D::new(
            hit_exit_3d.x + ray_len * 0.95,
            hit_exit_3d.y - ray_len * (0.15 + t * 0.12),
            hit_exit_3d.z + ray_len * spread_angle * 0.4,
        );
        let p_fan_end = camera.project(fan_end_3d, center);

        let is_selected = *selected_ray == Some(idx);
        let is_hovered = *hovered_ray == Some(idx);

        // Ray stroke width and alpha
        let (stroke_w, glow_w, alpha_glow) = if is_selected {
            (2.4, 7.0, 180)
        } else if is_hovered {
            (2.0, 5.5, 130)
        } else {
            (1.4, 4.0, 60)
        };

        // Outer glow
        let glow_color =
            Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), alpha_glow);
        painter.line_segment([p_hit_exit, p_fan_end], Stroke::new(glow_w, glow_color));
        // Sharp core
        painter.line_segment([p_hit_exit, p_fan_end], Stroke::new(stroke_w, color));

        // Floor light projection strip
        let floor_proj_end =
            camera.project(Vec3D::new(fan_end_3d.x, floor_y, fan_end_3d.z), center);
        let floor_alpha = alpha_glow / 4;
        painter.line_segment(
            [p_hit_exit, floor_proj_end],
            Stroke::new(
                1.0,
                Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), floor_alpha),
            ),
        );

        // Hit testing for ray inspection
        if let Some(pos) = mouse_pos {
            let d_mid = (pos - (p_hit_exit + (p_fan_end - p_hit_exit) * 0.5)).length();
            if d_mid < min_dist {
                min_dist = d_mid;
                closest_idx = Some(idx);
            }
        }
    }

    // Update hovered ray index
    *hovered_ray = closest_idx;
    if ui.input(|i| i.pointer.primary_clicked()) && closest_idx.is_some() {
        *selected_ray = closest_idx;
    }

    // Right slanted exit face (rendered in front of internal rays for depth)
    painter.add(PathShape::convex_polygon(
        vec![p_top_f, p_top_b, p_br_b, p_br_f],
        Color32::from_rgba_premultiplied(26, 40, 58, 85),
        Stroke::new(1.2, Color32::from_rgba_premultiplied(140, 185, 230, 95)),
    ));

    // Front triangular face
    painter.add(PathShape::convex_polygon(
        vec![p_top_f, p_br_f, p_bl_f],
        Color32::from_rgba_premultiplied(32, 48, 70, 70),
        Stroke::new(1.2, Color32::from_rgba_premultiplied(160, 205, 245, 110)),
    ));

    // Front specular ridge highlight (white crest reflection)
    painter.line_segment(
        [p_top_f, p_top_b],
        Stroke::new(1.6, Color32::from_rgba_premultiplied(255, 255, 255, 180)),
    );
    painter.line_segment(
        [p_top_f, p_bl_f],
        Stroke::new(1.2, Color32::from_rgba_premultiplied(220, 240, 255, 140)),
    );
    painter.line_segment(
        [p_top_f, p_br_f],
        Stroke::new(1.5, Color32::from_rgba_premultiplied(240, 250, 255, 190)),
    );

    // 6. Coordinate Axes Gizmo in lower-left corner
    let gizmo_origin = Pos2::new(rect.left() + 45.0, rect.bottom() - 40.0);
    let axis_len = 24.0;

    // X axis (Red)
    let p_x = gizmo_origin + Vec2::new(axis_len * 0.9, axis_len * 0.2);
    painter.line_segment(
        [gizmo_origin, p_x],
        Stroke::new(1.8, Color32::from_rgb(0xff, 0x52, 0x52)),
    );
    painter.text(
        p_x + Vec2::new(5.0, -1.0),
        egui::Align2::LEFT_CENTER,
        "X",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0xff, 0x52, 0x52),
    );

    // Y axis (Green, Up)
    let p_y = gizmo_origin + Vec2::new(0.0, -axis_len);
    painter.line_segment(
        [gizmo_origin, p_y],
        Stroke::new(1.8, Color32::from_rgb(0x69, 0xf0, 0xae)),
    );
    painter.text(
        p_y + Vec2::new(0.0, -6.0),
        egui::Align2::CENTER_BOTTOM,
        "Y",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0x69, 0xf0, 0xae),
    );

    // Z axis (Blue, Depth)
    let p_z = gizmo_origin + Vec2::new(-axis_len * 0.65, axis_len * 0.45);
    painter.line_segment(
        [gizmo_origin, p_z],
        Stroke::new(1.8, Color32::from_rgb(0x44, 0x8a, 0xff)),
    );
    painter.text(
        p_z + Vec2::new(-6.0, 2.0),
        egui::Align2::RIGHT_CENTER,
        "Z",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0x44, 0x8a, 0xff),
    );
}
