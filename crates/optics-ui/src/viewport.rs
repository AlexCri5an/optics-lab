//! Academic 2D optical viewport component.
//!
//! Provides a high-precision interactive optical canvas:
//! - Sub-pixel CAD rendering of optical axis, spherical arcs, planar surfaces, and glass bodies.
//! - Smooth exponential zoom centered at mouse cursor and frictionless panning.
//! - Automatic Zoom-Fit calculation for arbitrary optical prescriptions.
//! - Engineering scale indicator (metric scale bar).

use std::sync::LazyLock;

use egui::{
    Color32, CornerRadius, Frame, Margin, Painter, Pos2, Rect, Response, RichText, Sense, Stroke,
    Ui, Vec2,
};
use optics_core::prescription::trace_sequential;
use optics_core::{OpticalPrescription, OpticalSurface, Ray, Vec3};
use optics_glass::GlassCatalogDb;

static BUNDLED_CATALOG: LazyLock<GlassCatalogDb> = LazyLock::new(GlassCatalogDb::default);

use crate::theme::OpticsTokens;

/// Interactive 3D camera state for the hardware perspective viewport.
#[derive(Debug, Clone)]
pub struct Viewport3DCamera {
    /// Azimuth angle in radians (yaw around optical Y axis).
    pub azimuth_rad: f32,
    /// Elevation angle in radians (pitch above X-Z plane).
    pub elevation_rad: f32,
    /// Camera distance from target in world millimeters.
    pub distance_mm: f32,
    /// Target center point in world space (X, Y, Z) mm.
    pub target_mm: [f32; 3],
    /// Pan offset in camera screen space (X, Y) mm.
    pub pan_screen_mm: Vec2,
    /// Vertical field of view in degrees.
    pub fov_deg: f32,
    /// Whether initial zoom-fit was performed.
    pub initialized: bool,
}

impl Default for Viewport3DCamera {
    fn default() -> Self {
        Self {
            azimuth_rad: 35.0f32.to_radians(),
            elevation_rad: 22.0f32.to_radians(),
            distance_mm: 150.0,
            target_mm: [0.0, 0.0, 50.0],
            pan_screen_mm: Vec2::ZERO,
            fov_deg: 35.0,
            initialized: false,
        }
    }
}

impl Viewport3DCamera {
    /// Resets 3D camera to default isometric perspective.
    pub fn reset_view(&mut self) {
        self.azimuth_rad = 35.0f32.to_radians();
        self.elevation_rad = 22.0f32.to_radians();
        self.pan_screen_mm = Vec2::ZERO;
    }

    /// Fits the optical prescription into the 3D perspective viewport.
    pub fn zoom_fit(&mut self, prescription: &OpticalPrescription, _viewport_size: Vec2) {
        if prescription.surfaces.is_empty() {
            return;
        }
        let mut min_z = 0.0f64;
        let mut max_z = 0.0f64;
        let mut max_semi_d = 5.0f64;
        let mut cum_z = 0.0f64;

        for surf in &prescription.surfaces {
            min_z = min_z.min(cum_z);
            max_z = max_z.max(cum_z);
            let sd = if surf.semi_diameter_mm.is_finite() && surf.semi_diameter_mm > 0.0 {
                surf.semi_diameter_mm
            } else {
                5.0
            };
            max_semi_d = max_semi_d.max(sd);
            let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
                surf.thickness_mm
            } else {
                0.0
            };
            cum_z += th;
        }
        max_z = max_z.max(cum_z);

        let system_length = (max_z - min_z).clamp(1.0, 10000.0) as f32;
        let center_z = ((min_z + max_z) * 0.5) as f32;

        self.target_mm = [0.0, 0.0, center_z];
        self.pan_screen_mm = Vec2::ZERO;
        let extent = system_length.max(max_semi_d as f32 * 2.5);
        self.distance_mm = (extent * 2.1).clamp(20.0, 5000.0);
        self.initialized = true;
    }
}

/// Persistent state for the 2D/3D optical canvas viewport.
#[derive(Debug, Clone)]
pub struct ViewportState {
    /// Pan offset in world millimeters (relative to optical axis origin).
    pub pan_mm: Vec2,
    /// Pixels per world millimeter scale factor.
    pub zoom: f32,
    /// Whether the camera has been auto-centered at startup.
    pub initialized: bool,
    /// Whether multi-wavelength raytracing is active and drawn.
    pub show_rays: bool,
    /// Number of rays per beam/field in 2D tracing (3, 5, 7, 9). Default: 5.
    pub ray_count: usize,
    /// Ray aiming mode (Paraxial vs Real Iterative pupil aiming).
    pub ray_aiming_mode: optics_core::RayAimingMode,
    /// Optional non-sequential ghost ray path segments to overlay in the viewport.
    pub ghost_rays: Vec<optics_nonseq::trace::RayPathSegment>,
    /// Currently selected ghost surface pair `(S_front, S_rear)` from the Ghost Matrix table.
    pub selected_ghost_pair: Option<(usize, usize)>,
    /// Whether non-sequential ghost rays and flare paths are drawn in the viewport.
    pub show_ghost_rays: bool,
    /// Whether the viewport is in 3D Hardware Perspective mode (false = 2D CAD).
    pub is_3d: bool,
    /// 3D Camera navigation state.
    pub camera_3d: Viewport3DCamera,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            pan_mm: Vec2::ZERO,
            zoom: 5.0, // 5 pixels per mm default
            initialized: false,
            show_rays: true, // Visible by default
            ray_count: 5,    // Standard 5-ray beam
            ray_aiming_mode: optics_core::RayAimingMode::Paraxial,
            ghost_rays: Vec::new(),
            selected_ghost_pair: None,
            show_ghost_rays: true,
            is_3d: false,
            camera_3d: Viewport3DCamera::default(),
        }
    }
}

impl ViewportState {
    /// Centers and fits the entire optical prescription into the given screen rect.
    pub fn zoom_fit(&mut self, prescription: &OpticalPrescription, viewport_size: Vec2) {
        if prescription.surfaces.is_empty() || viewport_size.x <= 10.0 || viewport_size.y <= 10.0 {
            return;
        }

        let mut cumulative_z = 0.0;
        let mut min_z = 0.0f64;
        let mut max_z = 0.0f64;
        let mut max_semi_d = 5.0f64;

        for surf in &prescription.surfaces {
            min_z = min_z.min(cumulative_z);
            max_z = max_z.max(cumulative_z);
            let semi_d = if surf.semi_diameter_mm.is_finite() && surf.semi_diameter_mm > 0.0 {
                surf.semi_diameter_mm
            } else {
                5.0
            };
            max_semi_d = max_semi_d.max(semi_d);
            let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
                surf.thickness_mm
            } else {
                0.0
            };
            cumulative_z += th;
        }
        max_z = max_z.max(cumulative_z);

        if !min_z.is_finite() || !max_z.is_finite() || !max_semi_d.is_finite() || min_z > max_z {
            self.reset_view();
            return;
        }

        let system_length = (max_z - min_z).clamp(1.0, 50000.0) as f32;
        let system_height = (max_semi_d * 2.4).clamp(1.0, 10000.0) as f32;

        let margin_factor = 0.82; // 82% of viewport space
        let zoom_x = (viewport_size.x * margin_factor) / system_length;
        let zoom_y = (viewport_size.y * margin_factor) / system_height;

        self.zoom = zoom_x.min(zoom_y).clamp(0.05, 500.0);

        let center_z = ((min_z + max_z) * 0.5) as f32;
        let safe_center_z = if center_z.is_finite() { center_z } else { 0.0 };
        self.pan_mm = Vec2::new(-safe_center_z, 0.0);
        self.initialized = true;
    }

    /// Resets the camera to origin with a standard scale.
    pub fn reset_view(&mut self) {
        self.pan_mm = Vec2::ZERO;
        self.zoom = 5.0;
        self.initialized = true;
    }
}

/// Renders the 2D central optical canvas with CAD lenses, optical axis, and scale bar.
pub fn render_optical_viewport(
    ui: &mut Ui,
    prescription: &OpticalPrescription,
    state: &mut ViewportState,
    tokens: &OpticsTokens,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

    if !state.initialized && rect.width() > 50.0 && rect.height() > 50.0 {
        state.zoom_fit(prescription, rect.size());
    }

    let overlay_padding = 10.0;
    let overlay_width = if rect.width() < 720.0 { 325.0 } else { 385.0 };
    let overlay_height = 28.0;
    let overlay_rect = Rect::from_min_size(
        Pos2::new(rect.max.x - overlay_width - overlay_padding, rect.min.y + overlay_padding),
        Vec2::new(overlay_width, overlay_height),
    );

    // -----------------------------------------------------------------------
    // 1. Mouse & Trackpad Interaction (Pan & Zoom)
    // -----------------------------------------------------------------------
    if response.dragged() {
        let is_over_controls = response
            .interact_pointer_pos()
            .map(|p| overlay_rect.contains(p))
            .unwrap_or(false);

        if !is_over_controls {
            let delta_pixels = response.drag_delta();
            state.pan_mm.x += delta_pixels.x / state.zoom;
            state.pan_mm.y -= delta_pixels.y / state.zoom;
        }
    }

    if response.hovered() {
        let pinch = ui.input(|i| i.zoom_delta());
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        let smooth_scroll = ui.input(|i| i.smooth_scroll_delta);

        // Native Mac pinch gesture or mouse scroll wheel
        let mut zoom_factor = 1.0f32;
        if (pinch - 1.0).abs() > 0.001 {
            zoom_factor *= pinch;
        } else if scroll_y.abs() > 0.1 {
            zoom_factor *= (scroll_y * 0.003).exp();
        }

        if (zoom_factor - 1.0).abs() > 0.001 {
            let new_zoom = (state.zoom * zoom_factor).clamp(0.2, 500.0);

            if let Some(pointer_pos) = response.hover_pos() {
                let center = rect.center();
                let mouse_world_x = (pointer_pos.x - center.x) / state.zoom - state.pan_mm.x;
                let mouse_world_y = (center.y - pointer_pos.y) / state.zoom - state.pan_mm.y;

                state.zoom = new_zoom;
                state.pan_mm.x = (pointer_pos.x - center.x) / state.zoom - mouse_world_x;
                state.pan_mm.y = (center.y - pointer_pos.y) / state.zoom - mouse_world_y;
            } else {
                state.zoom = new_zoom;
            }
        } else if smooth_scroll.length_sq() > 0.1 && (pinch - 1.0).abs() <= 0.001 && scroll_y.abs() <= 0.1 {
            // Trackpad 2-finger pan on Mac
            state.pan_mm.x += smooth_scroll.x / state.zoom;
            state.pan_mm.y -= smooth_scroll.y / state.zoom;
        }
    }

    // -----------------------------------------------------------------------
    // 2. Painting Layers
    // -----------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let to_screen = |z_mm: f64, y_mm: f64| -> Pos2 {
        let safe_z = if z_mm.is_finite() { z_mm } else { 0.0 };
        let safe_y = if y_mm.is_finite() { y_mm } else { 0.0 };
        let safe_pan_x = if state.pan_mm.x.is_finite() { state.pan_mm.x } else { 0.0 };
        let safe_pan_y = if state.pan_mm.y.is_finite() { state.pan_mm.y } else { 0.0 };
        let safe_zoom = if state.zoom.is_finite() && state.zoom > 0.01 { state.zoom } else { 1.0 };
        Pos2::new(
            center.x + (safe_z as f32 + safe_pan_x) * safe_zoom,
            center.y - (safe_y as f32 + safe_pan_y) * safe_zoom,
        )
    };

    // Layer A: Background Canvas
    painter.rect_filled(rect, CornerRadius::ZERO, tokens.bg_canvas);

    // Layer A1: Viewport Boundary Separators (Ensures crisp vertical CAD border lines against Left/Right panels and Top/Bottom bars)
    painter.line_segment(
        [Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.min.x, rect.max.y)],
        tokens.border,
    );
    painter.line_segment(
        [Pos2::new(rect.max.x, rect.min.y), Pos2::new(rect.max.x, rect.max.y)],
        tokens.border,
    );
    painter.line_segment(
        [Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.max.x, rect.min.y)],
        tokens.border,
    );
    painter.line_segment(
        [Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)],
        tokens.border,
    );

    // Layer B: Subtle Engineering Grid (10 mm & 50 mm)
    draw_grid(&painter, rect, center, state, tokens);

    // Layer C: Optical Axis (z-axis, y = 0)
    let axis_y = center.y - state.pan_mm.y * state.zoom;
    if axis_y >= rect.min.y && axis_y <= rect.max.y {
        painter.line_segment(
            [Pos2::new(rect.min.x, axis_y), Pos2::new(rect.max.x, axis_y)],
            Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.55)),
        );
    }

    // Layer D: Physical Lenses & Surfaces
    draw_prescription_elements(&painter, prescription, to_screen, tokens);

    // Layer E: Multi-Wavelength Ray Tracing (Real-Time Refraction)
    if state.show_rays {
        draw_sequential_rays(&painter, prescription, state.ray_count, state.ray_aiming_mode, to_screen, tokens);
    }

    // Layer E2: Non-Sequential Ghost Rays & Internal Reflections
    if state.show_ghost_rays && !state.ghost_rays.is_empty() {
        draw_ghost_rays(&painter, &state.ghost_rays, to_screen, tokens);
    }

    // Ghost Focus Highlight Badge (When a pair is isolated in Ghost Matrix)
    if let Some((s_front, s_rear)) = state.selected_ghost_pair {
        let badge_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 10.0, rect.min.y + 10.0),
            Vec2::new(220.0, 22.0),
        );
        painter.rect_filled(badge_rect, CornerRadius::ZERO, tokens.bg_panel);
        painter.rect_stroke(
            badge_rect,
            CornerRadius::ZERO,
            tokens.border,
            egui::StrokeKind::Inside,
        );
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("GHOST FOCUS PAIR: S{} ↔ S{}", s_front, s_rear),
            egui::FontId::monospace(10.0),
            tokens.warning,
        );
    }

    // Layer F: High-Contrast Plane Markers (STO, IMAGE)
    // Rendered on top of rays with opaque background badges to ensure 100% legibility
    draw_plane_markers(&painter, prescription, to_screen, tokens);

    // Layer G: Engineering Scale Indicator (Bottom-Left)
    draw_scale_indicator(&painter, rect, state.zoom, tokens);

    // Layer H: Academic Viewport Ray Controls (Top-Right Overlay)
    draw_viewport_controls(ui, overlay_rect, state, tokens);

    response
}

/// Draws an academic background dot/line grid at 10 mm and 50 mm intervals.
fn draw_grid(
    painter: &Painter,
    rect: Rect,
    center: Pos2,
    state: &ViewportState,
    tokens: &OpticsTokens,
) {
    if !state.zoom.is_finite() || state.zoom < 0.05 {
        return;
    }
    if !state.pan_mm.x.is_finite() || !state.pan_mm.y.is_finite() {
        return;
    }

    let grid_step_mm = if state.zoom > 15.0 {
        10.0
    } else if state.zoom > 3.0 {
        25.0
    } else {
        100.0
    };

    let start_z = ((rect.min.x - center.x) / state.zoom - state.pan_mm.x) as f64;
    let end_z = ((rect.max.x - center.x) / state.zoom - state.pan_mm.x) as f64;
    let start_y = ((center.y - rect.max.y) / state.zoom - state.pan_mm.y) as f64;
    let end_y = ((center.y - rect.min.y) / state.zoom - state.pan_mm.y) as f64;

    if !start_z.is_finite() || !end_z.is_finite() || !start_y.is_finite() || !end_y.is_finite() {
        return;
    }

    let steps_z = ((end_z - start_z) / grid_step_mm).abs();
    let steps_y = ((end_y - start_y) / grid_step_mm).abs();
    if steps_z > 200.0 || steps_y > 200.0 || steps_z * steps_y > 40000.0 {
        return;
    }

    let first_grid_z = (start_z / grid_step_mm).floor() * grid_step_mm;
    let first_grid_y = (start_y / grid_step_mm).floor() * grid_step_mm;

    let grid_dot_color = tokens.canvas_fg_muted.gamma_multiply(0.35);

    let mut z = first_grid_z;
    let mut total_dots = 0;
    while z <= end_z {
        let screen_x = center.x + (z as f32 + state.pan_mm.x) * state.zoom;
        let mut y = first_grid_y;
        while y <= end_y {
            let screen_y = center.y - (y as f32 + state.pan_mm.y) * state.zoom;
            painter.circle_filled(Pos2::new(screen_x, screen_y), 1.0, grid_dot_color);
            y += grid_step_mm;
            total_dots += 1;
            if total_dots > 10000 {
                return;
            }
        }
        z += grid_step_mm;
    }
}

/// Computes the mechanical outer height for the aperture stop diaphragm
/// based on the stop aperture and adjacent lens elements.
fn get_stop_outer_y(prescription: &OpticalPrescription, stop_idx: usize) -> f64 {
    let surf = &prescription.surfaces[stop_idx];
    let mut local_max = surf.semi_diameter_mm;
    if stop_idx > 0 {
        let prev = &prescription.surfaces[stop_idx - 1];
        if !prev.material_name.is_empty() && !prev.material_name.eq_ignore_ascii_case("AIR") {
            local_max = local_max.max(prev.semi_diameter_mm);
        }
    }
    if stop_idx + 1 < prescription.surfaces.len() {
        let next = &prescription.surfaces[stop_idx + 1];
        if !next.material_name.is_empty() && !next.material_name.eq_ignore_ascii_case("AIR") {
            local_max = local_max.max(next.semi_diameter_mm);
        }
    }
    (local_max + 1.5).max(surf.semi_diameter_mm + 1.2)
}

/// Computes the detector active semi-diameter for the image plane.
fn get_detector_semi_d(prescription: &OpticalPrescription, img_idx: usize) -> f64 {
    let surf = &prescription.surfaces[img_idx];
    if surf.semi_diameter_mm > 0.5 {
        surf.semi_diameter_mm
    } else {
        prescription
            .surfaces
            .iter()
            .take(img_idx)
            .filter(|s| !s.material_name.is_empty() && !s.material_name.eq_ignore_ascii_case("AIR"))
            .map(|s| s.semi_diameter_mm)
            .fold(6.0_f64, f64::max)
    }
}

/// Draws closed mechanical lens profiles, canonical open iris stops, and image planes.
fn draw_prescription_elements(
    painter: &Painter,
    prescription: &OpticalPrescription,
    to_screen: impl Fn(f64, f64) -> Pos2,
    tokens: &OpticsTokens,
) {
    if prescription.surfaces.is_empty() {
        return;
    }

    // 1. Cumulative z vertex positions for every surface
    let mut z_vertices = Vec::with_capacity(prescription.surfaces.len());
    let mut cum_z = 0.0f64;
    for surf in &prescription.surfaces {
        z_vertices.push(cum_z);
        let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
            surf.thickness_mm
        } else {
            0.0
        };
        cum_z += th;
    }

    // 2. Draw solid closed lens bodies (Front arc + Top barrel edge + Back arc + Bottom barrel edge)
    let lens_stroke = tokens.glass_stroke;
    let glass_tint = tokens.glass_fill;

    for i in 0..prescription.surfaces.len().saturating_sub(1) {
        let front = &prescription.surfaces[i];
        let back = &prescription.surfaces[i + 1];

        let is_glass = !front.material_name.is_empty()
            && !front.material_name.eq_ignore_ascii_case("AIR")
            && front.thickness_mm > 0.001;

        if is_glass {
            let z_front = z_vertices[i];
            let z_back = z_vertices[i + 1];

            // Sample front arc from +y down to -y
            let mut front_pts = sample_surface_arc(front, z_front, 28);
            front_pts.reverse();

            // Sample back arc from -y up to +y
            let back_pts = sample_surface_arc(back, z_back, 28);

            let mut polygon_screen = Vec::with_capacity(front_pts.len() + back_pts.len());
            for (z, y) in &front_pts {
                polygon_screen.push(to_screen(*z, *y));
            }
            for (z, y) in &back_pts {
                polygon_screen.push(to_screen(*z, *y));
            }

            // Fill solid lens element with guaranteed-contrast themed glass tint
            painter.add(egui::Shape::convex_polygon(
                polygon_screen.clone(),
                glass_tint,
                Stroke::NONE,
            ));

            // Complete closed mechanical contour stroke (barrel top, bottom, and curves)
            for window in polygon_screen.windows(2) {
                painter.line_segment([window[0], window[1]], lens_stroke);
            }
            if let (Some(&first), Some(&last)) = (polygon_screen.first(), polygon_screen.last()) {
                painter.line_segment([last, first], lens_stroke);
            }
        }
    }

    // 2.5 Draw standalone optical surfaces (e.g. mirrors)
    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let is_image_plane = i == prescription.surfaces.len().saturating_sub(1);
        if !is_image_plane && !surf.is_stop {
            let is_glass_front = !surf.material_name.is_empty()
                && !surf.material_name.eq_ignore_ascii_case("AIR")
                && surf.thickness_mm > 0.001;
            let is_glass_back = i > 0
                && !prescription.surfaces[i - 1].material_name.is_empty()
                && !prescription.surfaces[i - 1].material_name.eq_ignore_ascii_case("AIR")
                && prescription.surfaces[i - 1].thickness_mm > 0.001;
            if !is_glass_front && !is_glass_back {
                let is_mirror = surf.material_name.eq_ignore_ascii_case("MIRROR");
                if is_mirror {
                    let z_vert = z_vertices[i];
                    let pts = sample_surface_arc(surf, z_vert, 28);
                    for window in pts.windows(2) {
                        painter.line_segment(
                            [to_screen(window[0].0, window[0].1), to_screen(window[1].0, window[1].1)],
                            tokens.glass_stroke,
                        );
                    }
                }
            }
        }
    }

    // 2.6 Draw optical thin-film coating accent overlay on coated optical surfaces
    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let is_image_plane = i == prescription.surfaces.len().saturating_sub(1);
        if !is_image_plane && !surf.is_stop {
            if let Some(ref coat) = surf.coating {
                let z_vert = z_vertices[i];
                let pts = sample_surface_arc(surf, z_vert, 28);
                let coat_color = match coat {
                    optics_core::SurfaceCoating::HighReflectanceMirror => tokens.warning,
                    _ => tokens.plot_tangential,
                };
                let coat_stroke = egui::Stroke::new(1.8, coat_color.gamma_multiply(0.85));
                for window in pts.windows(2) {
                    painter.line_segment(
                        [to_screen(window[0].0, window[0].1), to_screen(window[1].0, window[1].1)],
                        coat_stroke,
                    );
                }
            }
        }
    }

    // 3. Draw Mechanical Aperture Stop (Diaphragm) & Image Plane (Detector)
    let total_surfaces = prescription.surfaces.len();

    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let z_vert = z_vertices[i];
        let is_image_plane = i == total_surfaces.saturating_sub(1);

        if is_image_plane {
            // Standard Academic Image Plane (Focal Detector)
            // Even if marked as stop, it must ALWAYS render as the sensor detector plane!
            let det_semi_d = get_detector_semi_d(prescription, i);

            let img_top = to_screen(z_vert, det_semi_d);
            let img_bot = to_screen(z_vert, -det_semi_d);
            let img_color = if surf.is_stop { tokens.warning } else { tokens.accent };
            let img_stroke = Stroke::new(1.8, img_color);

            // Vertical sensor active area
            painter.line_segment([img_top, img_bot], img_stroke);

            // Sensor bracket caps [ ... ]
            let cap_w = 4.0;
            painter.line_segment(
                [Pos2::new(img_top.x - cap_w, img_top.y), Pos2::new(img_top.x + cap_w, img_top.y)],
                img_stroke,
            );
            painter.line_segment(
                [Pos2::new(img_bot.x - cap_w, img_bot.y), Pos2::new(img_bot.x + cap_w, img_bot.y)],
                img_stroke,
            );
        } else if surf.is_stop {
            // Standard Optical CAD Aperture Stop: Physical Mechanical Diaphragm
            // Outer blades mount to local lens barrel; center aperture is open according to physical aperture
            let outer_y = get_stop_outer_y(prescription, i);

            let (top_clear_y, bot_clear_y) = if let Some(ap) = &surf.aperture {
                match &ap.shape {
                    optics_core::surface::ApertureShape::Rectangular { half_height_mm, .. } => {
                        (ap.decenter_y_mm + half_height_mm, ap.decenter_y_mm - half_height_mm)
                    }
                    optics_core::surface::ApertureShape::Elliptical { semi_axis_y_mm, .. } => {
                        (ap.decenter_y_mm + semi_axis_y_mm, ap.decenter_y_mm - semi_axis_y_mm)
                    }
                    optics_core::surface::ApertureShape::Annular { outer_radius_mm, .. } => {
                        (ap.decenter_y_mm + outer_radius_mm, ap.decenter_y_mm - outer_radius_mm)
                    }
                    optics_core::surface::ApertureShape::Circular { radius_mm } => {
                        (ap.decenter_y_mm + radius_mm, ap.decenter_y_mm - radius_mm)
                    }
                    optics_core::surface::ApertureShape::Polygonal { .. } => {
                        let r = surf.semi_diameter_mm.max(0.2);
                        (r, -r)
                    }
                }
            } else {
                let r = surf.semi_diameter_mm.max(0.2);
                (r, -r)
            };

            let top_outer = to_screen(z_vert, outer_y);
            let top_inner = to_screen(z_vert, top_clear_y);
            let bot_inner = to_screen(z_vert, bot_clear_y);
            let bot_outer = to_screen(z_vert, -outer_y);

            let stop_stroke = Stroke::new(2.2, tokens.warning);

            // Upper mechanical iris blade
            painter.line_segment([top_outer, top_inner], stop_stroke);
            // Lower mechanical iris blade
            painter.line_segment([bot_outer, bot_inner], stop_stroke);

            // Standard CAD inward-pointing diaphragm wedges
            let tri_top = vec![
                Pos2::new(top_inner.x - 3.5, top_inner.y - 6.0),
                Pos2::new(top_inner.x + 3.5, top_inner.y - 6.0),
                top_inner,
            ];
            painter.add(egui::Shape::convex_polygon(tri_top, tokens.warning, Stroke::NONE));

            let tri_bot = vec![
                Pos2::new(bot_inner.x - 3.5, bot_inner.y + 6.0),
                Pos2::new(bot_inner.x + 3.5, bot_inner.y + 6.0),
                bot_inner,
            ];
            painter.add(egui::Shape::convex_polygon(tri_bot, tokens.warning, Stroke::NONE));

            // Outer barrel mount flange ticks
            painter.line_segment(
                [Pos2::new(top_outer.x - 4.0, top_outer.y), Pos2::new(top_outer.x + 4.0, top_outer.y)],
                stop_stroke,
            );
            painter.line_segment(
                [Pos2::new(bot_outer.x - 4.0, bot_outer.y), Pos2::new(bot_outer.x + 4.0, bot_outer.y)],
                stop_stroke,
            );
        }

        // Central Obscuration / Spider disc visualization (e.g. Cassegrain secondary mirror or central baffle)
        if let Some(ap) = &surf.aperture {
            let (has_central_block, block_r) = match &ap.shape {
                optics_core::surface::ApertureShape::Annular { inner_radius_mm, .. } => (true, *inner_radius_mm),
                optics_core::surface::ApertureShape::Circular { radius_mm } if ap.is_obscuration => (true, *radius_mm),
                optics_core::surface::ApertureShape::Rectangular { half_height_mm, .. } if ap.is_obscuration => (true, *half_height_mm),
                optics_core::surface::ApertureShape::Elliptical { semi_axis_y_mm, .. } if ap.is_obscuration => (true, *semi_axis_y_mm),
                _ => (false, 0.0),
            };

            if has_central_block && block_r > 0.05 {
                let obs_top = to_screen(z_vert, ap.decenter_y_mm + block_r);
                let obs_bot = to_screen(z_vert, ap.decenter_y_mm - block_r);
                let obs_stroke = Stroke::new(3.0, tokens.text_muted.gamma_multiply(0.85));
                painter.line_segment([obs_top, obs_bot], obs_stroke);

                let tick_w = 4.0;
                painter.line_segment([Pos2::new(obs_top.x - tick_w, obs_top.y), Pos2::new(obs_top.x + tick_w, obs_top.y)], obs_stroke);
                painter.line_segment([Pos2::new(obs_bot.x - tick_w, obs_bot.y), Pos2::new(obs_bot.x + tick_w, obs_bot.y)], obs_stroke);
            }
        }
    }
}

/// Draws opaque high-contrast badges for key optical planes (STO, IMAGE)
/// on top of optical rays, ensuring intersecting rays do not impair label legibility.
fn draw_plane_markers(
    painter: &Painter,
    prescription: &OpticalPrescription,
    to_screen: impl Fn(f64, f64) -> Pos2,
    tokens: &OpticsTokens,
) {
    let total_surfaces = prescription.surfaces.len();
    if total_surfaces == 0 {
        return;
    }

    let mut z_vertices = Vec::with_capacity(total_surfaces);
    let mut current_z = 0.0;
    for surf in &prescription.surfaces {
        z_vertices.push(current_z);
        let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
            surf.thickness_mm
        } else {
            0.0
        };
        current_z += th;
    }

    let draw_badge = |anchor_top: Pos2, text: &str, text_color: Color32, border_color: Color32| {
        let font_id = egui::FontId::monospace(9.0);
        let galley = painter.layout_no_wrap(text.to_string(), font_id, text_color);
        let pad_x = 4.0;
        let pad_y = 2.0;
        let size = galley.size() + Vec2::new(pad_x * 2.0, pad_y * 2.0);
        // Center the badge horizontally directly above the anchor point
        let badge_rect = Rect::from_min_size(
            Pos2::new(anchor_top.x - size.x * 0.5, anchor_top.y - size.y - 2.0),
            size,
        );

        // Solid opaque background badge masking any rays underneath
        painter.rect_filled(badge_rect, CornerRadius::ZERO, tokens.bg_panel);
        // Crisp high-contrast border
        painter.rect_stroke(
            badge_rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, border_color),
            egui::StrokeKind::Inside,
        );
        // Monospace text precisely centered in the badge
        let text_pos = Pos2::new(badge_rect.min.x + pad_x, badge_rect.min.y + pad_y);
        painter.galley(text_pos, galley, text_color);
    };

    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let z_vert = z_vertices[i];
        let is_image_plane = i == total_surfaces.saturating_sub(1);

        if is_image_plane {
            let det_semi_d = get_detector_semi_d(prescription, i);
            let img_top = to_screen(z_vert, det_semi_d);
            let label = if surf.is_stop { "IMAGE (STO)" } else { "IMAGE" };
            let img_color = if surf.is_stop { tokens.warning } else { tokens.accent };
            let border_color = if surf.is_stop {
                tokens.warning.gamma_multiply(0.7)
            } else {
                tokens.accent.gamma_multiply(0.7)
            };

            draw_badge(img_top, label, img_color, border_color);
        } else if surf.is_stop {
            let outer_y = get_stop_outer_y(prescription, i);
            let top_outer = to_screen(z_vert, outer_y);
            draw_badge(
                top_outer,
                "STO",
                tokens.warning,
                tokens.warning.gamma_multiply(0.7),
            );
        }
    }
}

/// Computes symmetric entrance pupil radial fractions from +max_frac down to -max_frac.
fn pupil_fractions(count: usize, max_frac: f64) -> Vec<f64> {
    if count <= 1 {
        return vec![0.0];
    }
    let mut fracs = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f64 / (count - 1) as f64;
        let frac = max_frac * (1.0 - 2.0 * t);
        fracs.push(frac);
    }
    fracs
}

/// Traces multi-field and chromatic paraxial rays according to standard optical CAD conventions (Zemax OpticStudio).
fn draw_sequential_rays(
    painter: &Painter,
    prescription: &OpticalPrescription,
    ray_count: usize,
    aiming_mode: optics_core::RayAimingMode,
    to_screen: impl Fn(f64, f64) -> Pos2,
    tokens: &OpticsTokens,
) {
    if prescription.surfaces.is_empty() {
        return;
    }

    let epd = prescription.entrance_pupil_diameter_mm;
    let semi_epd = (epd * 0.5).max(1.0);

    let mut z_vertices = Vec::with_capacity(prescription.surfaces.len());
    let mut cum_z = 0.0f64;
    for surf in &prescription.surfaces {
        z_vertices.push(cum_z);
        let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
            surf.thickness_mm
        } else {
            0.0
        };
        cum_z += th;
    }

    let z0 = z_vertices.first().copied().unwrap_or(0.0);
    let margin = (semi_epd * 2.5).clamp(0.5, 12.0);
    let start_z = z0 - margin;

    let resolved_opt = prescription.resolve(&*BUNDLED_CATALOG).ok();

    // Helper closure to trace a single ray across the optical system
    let trace_ray = |y_pupil: f64, u_in: f64, wav_nm: f64, color: Color32, stroke_width: f32| {
        let y_start = y_pupil - (z0 - start_z) * u_in;

        // 1. Try real sequential 3D ray tracing (accurately tracks GRIN RK4 arcs, Coordinate Breaks, and Aspheres)
        if let Some(resolved) = &resolved_opt {
            let u_cos = (1.0 - u_in * u_in).max(0.0).sqrt();
            let dir = Vec3::new(0.0, u_in, u_cos).normalize();
            if let Ok(ray) = Ray::new(
                Vec3::new(0.0, y_start, start_z),
                dir,
                wav_nm,
                1.0,
            ) {
                if let Ok(path_rec) = trace_sequential(resolved, &ray) {
                    if path_rec.points.len() >= 2 {
                        for window in path_rec.points.windows(2) {
                            let p1 = to_screen(window[0].z, window[0].y);
                            let p2 = to_screen(window[1].z, window[1].y);
                            painter.line_segment([p1, p2], Stroke::new(stroke_width, color));
                        }
                        return;
                    }
                }
            }
        }

        // 2. Paraxial fallback
        let mut y = y_pupil;
        let mut u = u_in;
        let mut prev_pt = to_screen(start_z, y_start);

        for (i, (&z_surf, surf)) in z_vertices.iter().zip(&prescription.surfaces).enumerate() {
            let hit_pt = to_screen(z_surf, y);
            painter.line_segment([prev_pt, hit_pt], Stroke::new(stroke_width, color));
            prev_pt = hit_pt;

            // Physical aperture clipping: if ray misses clear aperture, terminate ray
            if surf.semi_diameter_mm > 0.0 && y.abs() > surf.semi_diameter_mm * 1.35 {
                break;
            }

            if i + 1 < prescription.surfaces.len() {
                let n_in = get_refractive_index(&surf.material_name, wav_nm);
                let n_out = get_refractive_index(&prescription.surfaces[i + 1].material_name, wav_nm);

                let c = if surf.radius_mm.is_infinite() || surf.radius_mm.abs() > 1e10 || !surf.radius_mm.is_finite() {
                    0.0
                } else {
                    1.0 / surf.radius_mm
                };

                // Paraxial Snell refraction across surface
                if n_out.abs() > 1e-4 {
                    u = (n_in / n_out) * u + ((n_in - n_out) / n_out) * y * c;
                }
                // Ray transfer along thickness to next surface
                let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 {
                    surf.thickness_mm
                } else {
                    0.0
                };
                y += th * u;
                if !y.is_finite() || !u.is_finite() {
                    break;
                }
            }
        }
    };

    // -----------------------------------------------------------------------
    // 1. Zemax Axial (0.0°) Beam
    //    Signature high-contrast theme ray color (Electric Cyan on dark, Royal Blue on light)
    // -----------------------------------------------------------------------
    let axial_color = tokens.plot_axial;

    // Subtle chromatic dispersion on marginal pupil rays (C-line red and F-line blue)
    let (c_line_red, f_line_blue) = if tokens.is_canvas_dark() {
        (
            Color32::from_rgba_unmultiplied(248, 113, 113, 180),
            Color32::from_rgba_unmultiplied(129, 140, 248, 180),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(220, 38, 38, 210),
            Color32::from_rgba_unmultiplied(37, 99, 235, 210),
        )
    };

    if ray_count >= 3 {
        for &frac in &[0.85, -0.85] {
            trace_ray(frac * semi_epd, 0.0, 656.27, c_line_red, 0.95);  // C-line (red)
            trace_ray(frac * semi_epd, 0.0, 486.13, f_line_blue, 0.95); // F-line (blue)
        }
    }

    // Primary design wavelength (from prescription) in dominant crisp on-axis beam
    let primary_wl = prescription
        .wavelengths_nm
        .get(prescription.primary_wavelength_index)
        .copied()
        .unwrap_or(587.56);

    let axial_fracs = pupil_fractions(ray_count, 0.85);
    for &frac in &axial_fracs {
        trace_ray(frac * semi_epd, 0.0, primary_wl, axial_color, 1.25);
    }

    // -----------------------------------------------------------------------
    // 2. Off-Axis Fields dynamically from prescription.field_angles_deg
    // -----------------------------------------------------------------------
    let off_axis_colors = if tokens.is_canvas_dark() {
        [
            Color32::from_rgb(34, 197, 94),   // Emerald Green
            Color32::from_rgb(244, 63, 94),   // Coral Red
            Color32::from_rgb(168, 85, 247),  // Purple
            Color32::from_rgb(234, 179, 8),   // Amber
        ]
    } else {
        [
            Color32::from_rgb(21, 128, 61),   // Deep Forest Green
            Color32::from_rgb(225, 29, 72),   // Deep Rose Red
            Color32::from_rgb(126, 34, 206),  // Deep Violet
            Color32::from_rgb(180, 83, 9),    // Deep Amber
        ]
    };

    let off_axis_fields: Vec<f64> = prescription
        .field_angles_deg
        .iter()
        .copied()
        .filter(|&a| a.abs() > 0.05)
        .collect();

    let off_axis_fracs = pupil_fractions(ray_count, 0.65);

    for (idx, &angle_deg) in off_axis_fields.iter().enumerate() {
        let col = off_axis_colors[idx % off_axis_colors.len()];

        // Tracing with either Real Iterative Aiming or Paraxial Ray Aiming
        if aiming_mode == optics_core::RayAimingMode::RealIterative {
            if let Some(resolved) = &resolved_opt {
                for &frac in &off_axis_fracs {
                    let pupil_coord = optics_core::types::Pupil::new(0.0, frac)
                        .unwrap_or_else(|_| optics_core::types::Pupil::new(0.0, 0.0).unwrap());
                    let field = optics_core::types::Field::AngleDegrees { x: 0.0, y: angle_deg };
                    let wl = optics_core::types::Wavelength::from_nanometers(primary_wl)
                        .unwrap_or(optics_core::types::Wavelength::D_LINE);

                    if let Ok(aimed_ray) = optics_core::aiming::trace_aimed_ray(
                        resolved,
                        field,
                        pupil_coord,
                        wl,
                        optics_core::RayAimingMode::RealIterative,
                    ) {
                        if let Ok(path_rec) = trace_sequential(resolved, &aimed_ray) {
                            if path_rec.points.len() >= 2 {
                                for window in path_rec.points.windows(2) {
                                    let p1 = to_screen(window[0].z, window[0].y);
                                    let p2 = to_screen(window[1].z, window[1].y);
                                    painter.line_segment([p1, p2], Stroke::new(1.15, col.gamma_multiply(0.88)));
                                }
                            }
                        }
                    }
                }
                continue;
            }
        }

        let u_pos = (angle_deg * std::f64::consts::PI / 180.0).tan();
        let u_neg = (-angle_deg * std::f64::consts::PI / 180.0).tan();

        // Upper beam (+angle)
        for &frac in &off_axis_fracs {
            trace_ray(frac * semi_epd, u_pos, primary_wl, col.gamma_multiply(0.88), 1.15);
        }

        // Lower symmetrical beam (-angle)
        if idx == 0 || idx == off_axis_fields.len() - 1 {
            for &frac in &off_axis_fracs {
                trace_ray(frac * semi_epd, u_neg, primary_wl, col.gamma_multiply(0.7), 1.0);
            }
        }
    }
}

/// Draws non-sequential ghost rays and internal reflections with intensity-based alpha and color coding.
fn draw_ghost_rays(
    painter: &Painter,
    ghost_rays: &[optics_nonseq::trace::RayPathSegment],
    to_screen: impl Fn(f64, f64) -> Pos2,
    _tokens: &OpticsTokens,
) {
    for seg in ghost_rays {
        let p1 = to_screen(seg.start.z, seg.start.y);
        let p2 = to_screen(seg.end.z, seg.end.y);

        // Color and stroke based on interaction type and power
        let (color, width) = if seg.interaction == "FresnelReflect" {
            // Internal ghost reflection: amber-orange with intensity-modulated alpha
            let alpha = ((seg.power_watts * 8.0).clamp(0.20, 0.95) * 255.0) as u8;
            (Color32::from_rgba_unmultiplied(255, 130, 30, alpha), 1.25)
        } else if seg.interaction == "TIR" {
            // Total internal reflection: bright gold
            let alpha = ((seg.power_watts * 3.0).clamp(0.35, 0.95) * 255.0) as u8;
            (Color32::from_rgba_unmultiplied(255, 215, 0, alpha), 1.3)
        } else {
            // Refracted non-sequential ray path: subtle cyan
            let alpha = ((seg.power_watts * 2.0).clamp(0.12, 0.65) * 255.0) as u8;
            (Color32::from_rgba_unmultiplied(70, 180, 220, alpha), 0.9)
        };

        painter.line_segment([p1, p2], Stroke::new(width, color));
    }
}

/// Renders an academic floating control bar for ray display and ray density.
fn draw_viewport_controls(
    ui: &mut Ui,
    overlay_rect: Rect,
    state: &mut ViewportState,
    tokens: &OpticsTokens,
) {
    let frame = Frame::NONE
        .fill(tokens.bg_panel)
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(4, 3));

    ui.scope_builder(egui::UiBuilder::new().max_rect(overlay_rect), |ui| {
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                let rays_active = state.show_rays;
                let (rays_bg, rays_stroke, rays_text_color) = if rays_active {
                    (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
                } else {
                    (tokens.bg_control, tokens.border, tokens.text_muted)
                };

                let rays_btn = egui::Button::new(
                    RichText::new("RAYS")
                        .size(10.5)
                        .monospace()
                        .strong()
                        .color(rays_text_color),
                )
                .fill(rays_bg)
                .stroke(rays_stroke)
                .corner_radius(CornerRadius::ZERO);

                if ui
                    .add(rays_btn)
                    .on_hover_text("Toggle ray tracing in 2D viewport")
                    .clicked()
                {
                    state.show_rays = !state.show_rays;
                }

                ui.separator();

                for &count in &[3, 5, 7, 9] {
                    let is_selected = state.show_rays && state.ray_count == count;
                    let (bg, stroke, text_color) = if is_selected {
                        (tokens.bg_control_active, tokens.border_hovered, tokens.text_primary)
                    } else {
                        (tokens.bg_control, tokens.border, tokens.text_muted)
                    };

                    let count_btn = egui::Button::new(
                        RichText::new(format!("{}", count))
                            .size(10.5)
                            .monospace()
                            .strong()
                            .color(text_color),
                    )
                    .fill(bg)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::ZERO)
                    .min_size(Vec2::new(20.0, 18.0));

                    if ui
                        .add(count_btn)
                        .on_hover_text(format!("Trace {} rays per beam", count))
                        .clicked()
                    {
                        state.ray_count = count;
                        state.show_rays = true;
                    }
                }

                ui.separator();

                let is_aim_real = state.ray_aiming_mode == optics_core::RayAimingMode::RealIterative;
                let aim_label = if is_aim_real { "AIM: REAL" } else { "AIM: PARAXIAL" };
                let (aim_bg, aim_stroke, aim_text_color) = if is_aim_real {
                    (tokens.bg_control_active, tokens.border_hovered, tokens.accent)
                } else {
                    (tokens.bg_control, tokens.border, tokens.text_muted)
                };

                let aim_btn = egui::Button::new(
                    RichText::new(aim_label)
                        .size(10.0)
                        .monospace()
                        .strong()
                        .color(aim_text_color),
                )
                .fill(aim_bg)
                .stroke(aim_stroke)
                .corner_radius(CornerRadius::ZERO);

                if ui
                    .add(aim_btn)
                    .on_hover_text("Toggle between Paraxial Pupil Aiming and Real Iterative Stop Aiming (Gauss-Newton)")
                    .clicked()
                {
                    state.ray_aiming_mode = if is_aim_real {
                        optics_core::RayAimingMode::Paraxial
                    } else {
                        optics_core::RayAimingMode::RealIterative
                    };
                }

                ui.separator();

                let ghost_active = state.show_ghost_rays && !state.ghost_rays.is_empty();
                let ghost_count = state.ghost_rays.len();
                let ghost_label = if ghost_count > 0 {
                    format!("GHOSTS ({})", ghost_count)
                } else {
                    "GHOSTS".to_string()
                };
                let (ghost_bg, ghost_stroke, ghost_text_color) = if ghost_active {
                    (tokens.bg_control_active, tokens.border_hovered, tokens.warning)
                } else {
                    (tokens.bg_control, tokens.border, tokens.text_muted)
                };

                let ghost_btn = egui::Button::new(
                    RichText::new(ghost_label)
                        .size(10.0)
                        .monospace()
                        .strong()
                        .color(ghost_text_color),
                )
                .fill(ghost_bg)
                .stroke(ghost_stroke)
                .corner_radius(CornerRadius::ZERO);

                if ui
                    .add(ghost_btn)
                    .on_hover_text("Toggle non-sequential ghost rays & internal reflections overlay in 2D viewport")
                    .clicked()
                {
                    state.show_ghost_rays = !state.show_ghost_rays;
                }
            });
        });
    });
}

/// Resolves refractive index for standard catalog glasses across wavelengths.
fn get_refractive_index(glass: &str, wav_nm: f64) -> f64 {
    let g = glass.trim();
    if g.is_empty() || g.eq_ignore_ascii_case("AIR") {
        return 1.0;
    }

    let is_f = wav_nm < 520.0;
    let is_c = wav_nm > 620.0;

    // Model glass format: N_1.6375_V56.1 or similar
    if g.starts_with("N_") && g.contains("_V") {
        let parts: Vec<&str> = g.strip_prefix("N_").unwrap_or(g).split("_V").collect();
        if parts.len() == 2 {
            if let (Ok(nd), Ok(vd)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                let delta_n = (nd - 1.0) / vd.max(1.0);
                return if is_f {
                    nd + delta_n * 0.70
                } else if is_c {
                    nd - delta_n * 0.30
                } else {
                    nd
                };
            }
        }
    }

    match g.to_uppercase().as_str() {
        "WATER" | "H2O" => if is_f { 1.3371 } else if is_c { 1.3311 } else { 1.3330 },
        "FK54" | "N-FK54" => if is_f { 1.4394 } else if is_c { 1.4346 } else { 1.4370 },
        "FK51" | "N-FK51" | "N-FK51A" => if is_f { 1.4893 } else if is_c { 1.4845 } else { 1.4866 },
        "SK16" | "N-SK16" => if is_f { 1.62725 } else if is_c { 1.61750 } else { 1.62041 },
        "F2" | "N-F2" => if is_f { 1.63186 } else if is_c { 1.61510 } else { 1.62004 },
        "BK7" | "N-BK7" => if is_f { 1.52238 } else if is_c { 1.51432 } else { 1.51680 },
        "SF1" | "N-SF1" => if is_f { 1.73462 } else if is_c { 1.71031 } else { 1.71736 },
        "SF2" | "N-SF2" => if is_f { 1.6534 } else if is_c { 1.6421 } else { 1.6477 },
        "SF5" | "N-SF5" => if is_f { 1.6830 } else if is_c { 1.6666 } else { 1.6727 },
        "SF11" | "N-SF11" => if is_f { 1.80645 } else if is_c { 1.77598 } else { 1.78472 },
        "LAK9" | "N-LAK9" => if is_f { 1.69974 } else if is_c { 1.68750 } else { 1.69100 },
        "BAF10" | "N-BAF10" => if is_f { 1.67812 } else if is_c { 1.66620 } else { 1.67003 },
        "LLF1" | "N-LLF1" => if is_f { 1.5542 } else if is_c { 1.5443 } else { 1.5481 },
        "F5" | "N-F5" => if is_f { 1.6121 } else if is_c { 1.5976 } else { 1.6034 },
        "SSK2" | "N-SSK2" => if is_f { 1.6285 } else if is_c { 1.6183 } else { 1.6223 },
        "SK2" | "N-SK2" => if is_f { 1.6128 } else if is_c { 1.6041 } else { 1.6074 },
        "SK11" | "N-SK11" => if is_f { 1.5694 } else if is_c { 1.5607 } else { 1.5638 },
        "KZFS4" | "N-KZFS4" => if is_f { 1.6212 } else if is_c { 1.6074 } else { 1.6133 },
        "FUSED_SILICA" | "FUSED SILICA" | "SILICA" | "LITHOSIL-Q" => {
            if is_f { 1.46313 } else if is_c { 1.45637 } else { 1.45846 }
        }
        _ => if is_f { 1.53 } else if is_c { 1.515 } else { 1.52 },
    }
}

/// Samples 2D points (z, y) along a spherical, aspheric, freeform, or planar surface arc.
fn sample_surface_arc(surf: &OpticalSurface, z_vertex: f64, num_samples: usize) -> Vec<(f64, f64)> {
    let mut points = Vec::with_capacity(num_samples + 1);
    let semi_d = surf.semi_diameter_mm.max(0.1);
    let is_flat = surf.radius_mm.is_infinite() || surf.radius_mm.abs() > 1e12;

    for k in 0..=num_samples {
        let t = k as f64 / num_samples as f64;
        let y = -semi_d + 2.0 * semi_d * t; // from -semi_d to +semi_d

        let sag = if let Some(profile) = &surf.profile {
            profile.sag(0.0, y).unwrap_or(0.0)
        } else if !surf.asphere_coefficients.is_empty() {
            let r_abs = y.abs();
            let c = if is_flat { 0.0 } else { 1.0 / surf.radius_mm };
            let conic_sag = if c.abs() < 1e-15 {
                0.0
            } else {
                let s = (1.0 - (1.0 + surf.conic_constant) * c * c * y * y).max(0.0).sqrt();
                (c * y * y) / (1.0 + s)
            };
            let mut asph_sag = 0.0;
            for (i, &a) in surf.asphere_coefficients.iter().enumerate() {
                let power = 2 * (i + 2);
                asph_sag += a * r_abs.powi(power as i32);
            }
            conic_sag + asph_sag
        } else if is_flat {
            0.0
        } else {
            let r = surf.radius_mm;
            let disc = r * r - y * y;
            if disc > 0.0 {
                r - r.signum() * disc.sqrt()
            } else {
                0.0
            }
        };

        points.push((z_vertex + sag, y));
    }

    points
}

/// Draws an academic metric scale bar in the bottom-left corner of the canvas.
fn draw_scale_indicator(
    painter: &Painter,
    canvas_rect: Rect,
    zoom_px_per_mm: f32,
    tokens: &OpticsTokens,
) {
    let candidate_steps_mm = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];
    let mut target_mm = 20.0;
    for &step in &candidate_steps_mm {
        let px = step * zoom_px_per_mm;
        if (50.0..=140.0).contains(&px) {
            target_mm = step;
            break;
        }
    }

    let bar_width_px = target_mm * zoom_px_per_mm;
    let anchor_x = canvas_rect.min.x + 16.0;
    let anchor_y = canvas_rect.max.y - 18.0;

    let bar_rect = Rect::from_min_size(
        Pos2::new(anchor_x - 6.0, anchor_y - 18.0),
        Vec2::new(bar_width_px + 12.0, 28.0),
    );

    // Subtle dark translucent badge
    let badge_frame = Frame::NONE
        .fill(tokens.bg_panel.gamma_multiply(0.85))
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(4, 2));

    painter.add(badge_frame.paint(bar_rect));

    // Scale bar line with end ticks
    let line_y = anchor_y + 2.0;
    painter.line_segment(
        [Pos2::new(anchor_x, line_y), Pos2::new(anchor_x + bar_width_px, line_y)],
        Stroke::new(1.5, tokens.text_primary),
    );
    // Left tick
    painter.line_segment(
        [Pos2::new(anchor_x, line_y - 3.0), Pos2::new(anchor_x, line_y + 3.0)],
        Stroke::new(1.5, tokens.text_primary),
    );
    // Right tick
    painter.line_segment(
        [
            Pos2::new(anchor_x + bar_width_px, line_y - 3.0),
            Pos2::new(anchor_x + bar_width_px, line_y + 3.0),
        ],
        Stroke::new(1.5, tokens.text_primary),
    );

    // Scale label
    let text = format!("{:.0} mm", target_mm);
    painter.text(
        Pos2::new(anchor_x + bar_width_px * 0.5, anchor_y - 8.0),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::monospace(9.5),
        tokens.text_muted,
    );
}

/// Computes surface sag z(r) for an optical surface at radial distance r from optical axis.
fn sample_surface_sag(surf: &OpticalSurface, r: f64) -> f64 {
    let is_flat = surf.radius_mm.is_infinite() || surf.radius_mm.abs() > 1e12;
    if let Some(profile) = &surf.profile {
        profile.sag(0.0, r).unwrap_or(0.0)
    } else if !surf.asphere_coefficients.is_empty() {
        let r_abs = r.abs();
        let c = if is_flat { 0.0 } else { 1.0 / surf.radius_mm };
        let conic_sag = if c.abs() < 1e-15 {
            0.0
        } else {
            let s = (1.0 - (1.0 + surf.conic_constant) * c * c * r * r).max(0.0).sqrt();
            (c * r * r) / (1.0 + s)
        };
        let mut asph_sag = 0.0;
        for (i, &a) in surf.asphere_coefficients.iter().enumerate() {
            let power = 2 * (i + 2);
            asph_sag += a * r_abs.powi(power as i32);
        }
        conic_sag + asph_sag
    } else if is_flat {
        0.0
    } else {
        let radius = surf.radius_mm;
        let disc = radius * radius - r * r;
        if disc > 0.0 {
            radius - radius.signum() * disc.sqrt()
        } else {
            0.0
        }
    }
}

/// Renders the interactive 3D Hardware Perspective viewport with CAD lenses, rays, coordinate triad, and HUD.
pub fn render_optical_viewport_3d(
    ui: &mut Ui,
    prescription: &OpticalPrescription,
    state: &mut ViewportState,
    tokens: &OpticsTokens,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

    if !state.camera_3d.initialized && rect.width() > 50.0 && rect.height() > 50.0 {
        state.camera_3d.zoom_fit(prescription, rect.size());
    }

    // Top-right HUD bounds
    let is_compact_hud = rect.width() < 640.0;
    let overlay_w = if is_compact_hud { 220.0 } else { 340.0 };
    let overlay_h = 28.0;
    let overlay_rect = Rect::from_min_size(
        Pos2::new(rect.max.x - overlay_w - 10.0, rect.min.y + 10.0),
        Vec2::new(overlay_w, overlay_h),
    );

    // -----------------------------------------------------------------------
    // 1. Mouse & Gesture 3D Camera Controls (Orbit, Pan, Zoom)
    // -----------------------------------------------------------------------
    let is_over_controls = response
        .interact_pointer_pos()
        .map_or(false, |p| overlay_rect.contains(p));

    if response.dragged() && !is_over_controls {
        let delta = response.drag_delta();
        let shift_down = ui.input(|i| i.modifiers.shift);
        let middle_drag = response.dragged_by(egui::PointerButton::Middle);

        if shift_down || middle_drag {
            // Camera Pan in screen space
            let pan_scale = (state.camera_3d.distance_mm / rect.height().max(50.0)) * 0.8;
            state.camera_3d.pan_screen_mm.x += delta.x * pan_scale;
            state.camera_3d.pan_screen_mm.y -= delta.y * pan_scale;
        } else {
            // Orbit camera around optical system target
            state.camera_3d.azimuth_rad -= delta.x * 0.007;
            state.camera_3d.elevation_rad = (state.camera_3d.elevation_rad - delta.y * 0.007).clamp(-1.45, 1.45);
        }
    }

    if response.hovered() && !is_over_controls {
        let pinch = ui.input(|i| i.zoom_delta());
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        let mut zoom_factor = 1.0f32;
        if (pinch - 1.0).abs() > 0.001 {
            zoom_factor /= pinch;
        } else if scroll_y.abs() > 0.1 {
            zoom_factor *= (-scroll_y * 0.003).exp();
        }

        if (zoom_factor - 1.0).abs() > 0.001 {
            state.camera_3d.distance_mm = (state.camera_3d.distance_mm * zoom_factor).clamp(5.0, 20000.0);
        }
    }

    // -----------------------------------------------------------------------
    // 2. Camera Mathematics & 3D Perspective Projection
    // -----------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, tokens.bg_canvas);

    // Viewport CAD boundaries
    painter.line_segment([Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.min.x, rect.max.y)], tokens.border);
    painter.line_segment([Pos2::new(rect.max.x, rect.min.y), Pos2::new(rect.max.x, rect.max.y)], tokens.border);
    painter.line_segment([Pos2::new(rect.min.x, rect.min.y), Pos2::new(rect.max.x, rect.min.y)], tokens.border);
    painter.line_segment([Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)], tokens.border);

    let cam = &state.camera_3d;
    let target = Vec3::new(cam.target_mm[0] as f64, cam.target_mm[1] as f64, cam.target_mm[2] as f64);
    let cos_el = (cam.elevation_rad as f64).cos();
    let sin_el = (cam.elevation_rad as f64).sin();
    let cos_az = (cam.azimuth_rad as f64).cos();
    let sin_az = (cam.azimuth_rad as f64).sin();

    let eye = target + Vec3::new(
        (cam.distance_mm as f64) * cos_el * sin_az,
        (cam.distance_mm as f64) * sin_el,
        (cam.distance_mm as f64) * cos_el * cos_az,
    );

    let fwd = (target - eye).normalize();
    let up_ref = if cos_el.abs() > 0.001 { Vec3::new(0.0, 1.0, 0.0) } else { Vec3::new(0.0, 0.0, -1.0) };
    let right = fwd.cross(&up_ref).normalize();
    let up = right.cross(&fwd).normalize();

    let fov_rad = (cam.fov_deg as f64).to_radians();
    let focal_len_px = (rect.height() as f64 * 0.5) / (fov_rad * 0.5).tan();
    let center_screen = rect.center();

    let project_3d = |p: Vec3| -> Option<(Pos2, f64)> {
        let v = p - eye;
        let z_cam = v.dot(&fwd);
        if z_cam <= 0.1 {
            return None;
        }
        let x_cam = v.dot(&right) + cam.pan_screen_mm.x as f64;
        let y_cam = v.dot(&up) + cam.pan_screen_mm.y as f64;

        let sx = center_screen.x + (x_cam / z_cam * focal_len_px) as f32;
        let sy = center_screen.y - (y_cam / z_cam * focal_len_px) as f32;
        Some((Pos2::new(sx, sy), z_cam))
    };

    // -----------------------------------------------------------------------
    // 3. 3D Optical Axis and Reference Geometry
    // -----------------------------------------------------------------------
    let mut z_vertices = Vec::with_capacity(prescription.surfaces.len());
    let mut cum_z = 0.0f64;
    for surf in &prescription.surfaces {
        z_vertices.push(cum_z);
        let th = if surf.thickness_mm.is_finite() && surf.thickness_mm >= 0.0 { surf.thickness_mm } else { 0.0 };
        cum_z += th;
    }

    let min_axis_z = z_vertices.first().copied().unwrap_or(0.0) - 15.0;
    let max_axis_z = cum_z + 15.0;

    if let (Some((p_start, _)), Some((p_end, _))) = (
        project_3d(Vec3::new(0.0, 0.0, min_axis_z)),
        project_3d(Vec3::new(0.0, 0.0, max_axis_z)),
    ) {
        painter.line_segment([p_start, p_end], Stroke::new(1.0, tokens.canvas_fg_muted.gamma_multiply(0.40)));
    }

    // -----------------------------------------------------------------------
    // 4. 3D Physical Lenses and Surfaces
    // -----------------------------------------------------------------------
    let num_ang_segments = 24;
    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let z_vert = z_vertices[i];
        let semi_d = surf.semi_diameter_mm.max(0.5);

        // A. Draw 3D surface rim perimeter ring
        let mut rim_pts_screen = Vec::with_capacity(num_ang_segments);
        for k in 0..num_ang_segments {
            let theta = (k as f64 * 2.0 * std::f64::consts::PI) / num_ang_segments as f64;
            let sag = sample_surface_sag(surf, semi_d);
            let pt_world = Vec3::new(semi_d * theta.cos(), semi_d * theta.sin(), z_vert + sag);
            if let Some((pt_scr, _)) = project_3d(pt_world) {
                rim_pts_screen.push(pt_scr);
            }
        }
        if rim_pts_screen.len() >= 3 {
            for k in 0..rim_pts_screen.len() {
                let p0 = rim_pts_screen[k];
                let p1 = rim_pts_screen[(k + 1) % rim_pts_screen.len()];
                let stroke_color = if surf.is_stop {
                    tokens.warning
                } else if i == prescription.surfaces.len() - 1 {
                    tokens.accent
                } else {
                    tokens.accent.gamma_multiply(0.75)
                };
                painter.line_segment([p0, p1], Stroke::new(1.2, stroke_color));
            }
        }

        // B. If physical lens body (connects surface i with surface i+1)
        let is_glass = !surf.material_name.is_empty() && !surf.material_name.eq_ignore_ascii_case("AIR");
        if is_glass && i + 1 < prescription.surfaces.len() {
            let next_surf = &prescription.surfaces[i + 1];
            let next_z = z_vertices[i + 1];
            let next_semi_d = next_surf.semi_diameter_mm.max(0.5);

            // Longitudinal outer barrel lines connecting front and rear rims
            for k in (0..num_ang_segments).step_by(2) {
                let theta = (k as f64 * 2.0 * std::f64::consts::PI) / num_ang_segments as f64;
                let sag_f = sample_surface_sag(surf, semi_d);
                let sag_r = sample_surface_sag(next_surf, next_semi_d);

                let pf = Vec3::new(semi_d * theta.cos(), semi_d * theta.sin(), z_vert + sag_f);
                let pr = Vec3::new(next_semi_d * theta.cos(), next_semi_d * theta.sin(), next_z + sag_r);

                if let (Some((scr_f, _)), Some((scr_r, _))) = (project_3d(pf), project_3d(pr)) {
                    painter.line_segment([scr_f, scr_r], Stroke::new(1.0, tokens.glass_stroke.color.gamma_multiply(0.45)));
                }
            }

            // Translucent glass body quads for solid CAD visualization
            for k in 0..num_ang_segments {
                let theta0 = (k as f64 * 2.0 * std::f64::consts::PI) / num_ang_segments as f64;
                let theta1 = (((k + 1) % num_ang_segments) as f64 * 2.0 * std::f64::consts::PI) / num_ang_segments as f64;

                let pf0 = Vec3::new(semi_d * theta0.cos(), semi_d * theta0.sin(), z_vert + sample_surface_sag(surf, semi_d));
                let pf1 = Vec3::new(semi_d * theta1.cos(), semi_d * theta1.sin(), z_vert + sample_surface_sag(surf, semi_d));
                let pr1 = Vec3::new(next_semi_d * theta1.cos(), next_semi_d * theta1.sin(), next_z + sample_surface_sag(next_surf, next_semi_d));
                let pr0 = Vec3::new(next_semi_d * theta0.cos(), next_semi_d * theta0.sin(), next_z + sample_surface_sag(next_surf, next_semi_d));

                if let (Some((s_f0, _)), Some((s_f1, _)), Some((s_r1, _)), Some((s_r0, _))) = (
                    project_3d(pf0),
                    project_3d(pf1),
                    project_3d(pr1),
                    project_3d(pr0),
                ) {
                    let quad = vec![s_f0, s_f1, s_r1, s_r0];
                    painter.add(egui::Shape::convex_polygon(
                        quad,
                        tokens.glass_fill,
                        Stroke::NONE,
                    ));
                }
            }
        }

        // C. Mechanical Stop diaphragm disk
        if surf.is_stop {
            let outer_r = semi_d * 1.5 + 2.0;
            for k in (0..num_ang_segments).step_by(2) {
                let theta = (k as f64 * 2.0 * std::f64::consts::PI) / num_ang_segments as f64;
                let p_inner = Vec3::new(semi_d * theta.cos(), semi_d * theta.sin(), z_vert);
                let p_outer = Vec3::new(outer_r * theta.cos(), outer_r * theta.sin(), z_vert);
                if let (Some((s_in, _)), Some((s_out, _))) = (project_3d(p_inner), project_3d(p_outer)) {
                    painter.line_segment([s_in, s_out], Stroke::new(1.4, tokens.warning));
                }
            }
        }

        // D. Image detector sensor rectangle
        if i == prescription.surfaces.len() - 1 {
            let w = semi_d * 1.2;
            let h = semi_d * 0.9;
            let p_corners = [
                Vec3::new(-w, -h, z_vert),
                Vec3::new(w, -h, z_vert),
                Vec3::new(w, h, z_vert),
                Vec3::new(-w, h, z_vert),
            ];
            let scr_corners: Vec<_> = p_corners.iter().filter_map(|&p| project_3d(p).map(|(s, _)| s)).collect();
            if scr_corners.len() == 4 {
                for c in 0..4 {
                    painter.line_segment([scr_corners[c], scr_corners[(c + 1) % 4]], Stroke::new(1.8, tokens.accent));
                }
                painter.add(egui::Shape::convex_polygon(
                    scr_corners,
                    tokens.accent.gamma_multiply(0.15),
                    Stroke::NONE,
                ));
            }
        }
    }

    // -----------------------------------------------------------------------
    // 5. 3D Sequential Real-Time Ray Tracing
    // -----------------------------------------------------------------------
    if state.show_rays {
        if let Ok(resolved) = prescription.resolve(&*BUNDLED_CATALOG) {
            let epd = prescription.entrance_pupil_diameter_mm;
            let semi_epd = (epd * 0.5).max(1.0);
            let z0 = z_vertices.first().copied().unwrap_or(0.0);
            let start_z = z0 - (semi_epd * 2.0).clamp(1.0, 15.0);

            let ray_rings = 8;
            for k in 0..ray_rings {
                let alpha = (k as f64 * 2.0 * std::f64::consts::PI) / ray_rings as f64;
                let x_pupil = semi_epd * alpha.cos();
                let y_pupil = semi_epd * alpha.sin();

                let ray = Ray {
                    origin: Vec3::new(x_pupil, y_pupil, start_z),
                    direction: Vec3::new(0.0, 0.0, 1.0),
                    wavelength_nm: 587.6, // d-line green
                    energy: 1.0,
                };

                if let Ok(record) = trace_sequential(&resolved, &ray) {
                    for seg in record.points.windows(2) {
                        if let (Some((s0, _)), Some((s1, _))) = (project_3d(seg[0]), project_3d(seg[1])) {
                            painter.line_segment([s0, s1], Stroke::new(1.2, Color32::from_rgb(46, 204, 113)));
                        }
                    }
                }
            }

            // Chief axial ray
            let chief_ray = Ray {
                origin: Vec3::new(0.0, 0.0, start_z),
                direction: Vec3::new(0.0, 0.0, 1.0),
                wavelength_nm: 587.6,
                energy: 1.0,
            };
            if let Ok(record) = trace_sequential(&resolved, &chief_ray) {
                for seg in record.points.windows(2) {
                    if let (Some((s0, _)), Some((s1, _))) = (project_3d(seg[0]), project_3d(seg[1])) {
                        painter.line_segment([s0, s1], Stroke::new(1.5, Color32::from_rgb(241, 196, 15)));
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 6. 3D Non-Sequential Ghost Rays
    // -----------------------------------------------------------------------
    if state.show_ghost_rays && !state.ghost_rays.is_empty() {
        for seg in &state.ghost_rays {
            if let (Some((s0, _)), Some((s1, _))) = (project_3d(seg.start), project_3d(seg.end)) {
                let stroke = Stroke::new(1.2, tokens.warning.gamma_multiply(0.85));
                painter.line_segment([s0, s1], stroke);
            }
        }
    }

    // Ghost Focus Highlight Badge (When a pair is isolated in Ghost Matrix)
    if let Some((s_front, s_rear)) = state.selected_ghost_pair {
        let badge_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 10.0, rect.min.y + 36.0),
            Vec2::new(220.0, 22.0),
        );
        painter.rect_filled(badge_rect, CornerRadius::ZERO, tokens.bg_panel);
        painter.rect_stroke(
            badge_rect,
            CornerRadius::ZERO,
            tokens.border,
            egui::StrokeKind::Inside,
        );
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("GHOST FOCUS PAIR: S{} ↔ S{}", s_front, s_rear),
            egui::FontId::monospace(10.0),
            tokens.warning,
        );
    }

    // -----------------------------------------------------------------------
    // 7. 3D Orientation Triad (Bottom-Left)
    // -----------------------------------------------------------------------
    let triad_center = Pos2::new(rect.min.x + 46.0, rect.max.y - 46.0);
    let triad_len = 24.0f32;

    let draw_axis_vector = |dir: Vec3, label: &str, color: Color32| {
        let dx = dir.dot(&right) as f32;
        let dy = dir.dot(&up) as f32;
        let p_end = Pos2::new(triad_center.x + dx * triad_len, triad_center.y - dy * triad_len);
        painter.line_segment([triad_center, p_end], Stroke::new(2.0, color));
        painter.text(
            Pos2::new(p_end.x + dx * 6.0, p_end.y - dy * 6.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(9.5),
            color,
        );
    };

    draw_axis_vector(Vec3::new(1.0, 0.0, 0.0), "+X", Color32::from_rgb(235, 75, 75));
    draw_axis_vector(Vec3::new(0.0, 1.0, 0.0), "+Y", Color32::from_rgb(60, 200, 90));
    draw_axis_vector(Vec3::new(0.0, 0.0, 1.0), "+Z", Color32::from_rgb(60, 160, 240));

    // -----------------------------------------------------------------------
    // 8. 3D Top-Left Mode Badge
    // -----------------------------------------------------------------------
    let badge_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 10.0, rect.min.y + 10.0),
        Vec2::new(230.0, 22.0),
    );
    painter.rect_filled(badge_rect, CornerRadius::ZERO, tokens.bg_panel.gamma_multiply(0.92));
    painter.rect_stroke(badge_rect, CornerRadius::ZERO, tokens.border, egui::StrokeKind::Inside);
    painter.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        "3D HARDWARE VIEWPORT (Ctrl+2: 2D CAD)",
        egui::FontId::monospace(9.5),
        tokens.text_primary,
    );

    // -----------------------------------------------------------------------
    // 9. 3D Top-Right Camera Navigation HUD
    // -----------------------------------------------------------------------
    let overlay_frame = Frame::NONE
        .fill(tokens.bg_panel.gamma_multiply(0.95))
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(4, 2));

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(overlay_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            overlay_frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                if ui.button(RichText::new("ISO").monospace().size(10.0)).clicked() {
                    state.camera_3d.azimuth_rad = 35.0f32.to_radians();
                    state.camera_3d.elevation_rad = 22.0f32.to_radians();
                    state.camera_3d.pan_screen_mm = Vec2::ZERO;
                }
                if ui.button(RichText::new("SIDE").monospace().size(10.0)).clicked() {
                    state.camera_3d.azimuth_rad = 90.0f32.to_radians();
                    state.camera_3d.elevation_rad = 0.0;
                    state.camera_3d.pan_screen_mm = Vec2::ZERO;
                }
                if !is_compact_hud {
                    if ui.button(RichText::new("TOP").monospace().size(10.0)).clicked() {
                        state.camera_3d.azimuth_rad = 0.0;
                        state.camera_3d.elevation_rad = 89.0f32.to_radians();
                        state.camera_3d.pan_screen_mm = Vec2::ZERO;
                    }
                    if ui.button(RichText::new("FRONT").monospace().size(10.0)).clicked() {
                        state.camera_3d.azimuth_rad = 0.0;
                        state.camera_3d.elevation_rad = 0.0;
                        state.camera_3d.pan_screen_mm = Vec2::ZERO;
                    }
                }
                if ui.button(RichText::new("FIT").monospace().size(10.0)).clicked() {
                    state.camera_3d.zoom_fit(prescription, rect.size());
                }

                ui.separator();

                let ghost_active_3d = state.show_ghost_rays && !state.ghost_rays.is_empty();
                let (ghost_3d_text, ghost_3d_color) = if ghost_active_3d {
                    ("GHOSTS", tokens.warning)
                } else {
                    ("GHOSTS", tokens.text_muted)
                };
                if ui.button(
                    RichText::new(ghost_3d_text)
                        .monospace()
                        .size(10.0)
                        .color(ghost_3d_color),
                ).on_hover_text("Toggle non-sequential ghost rays in 3D viewport").clicked() {
                    state.show_ghost_rays = !state.show_ghost_rays;
                }

                if ui.button(
                    RichText::new("2D CAD")
                        .monospace()
                        .size(10.0)
                        .color(tokens.accent),
                ).clicked() {
                    state.is_3d = false;
                }

                if !is_compact_hud {
                    ui.label(
                        RichText::new(format!(
                            "Az:{:.0}° El:{:.0}°",
                            state.camera_3d.azimuth_rad.to_degrees(),
                            state.camera_3d.elevation_rad.to_degrees()
                        ))
                        .monospace()
                        .size(9.0)
                        .color(tokens.text_muted),
                    );
                }
            });
        },
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_surface_arc_flat() {
        let surf = OpticalSurface {
            radius_mm: f64::INFINITY,
            semi_diameter_mm: 10.0,
            ..Default::default()
        };
        let pts = sample_surface_arc(&surf, 5.0, 10);
        assert_eq!(pts.len(), 11);
        for (z, _) in pts {
            assert!((z - 5.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_viewport_zoom_fit() {
        let mut state = ViewportState::default();
        let rx = OpticalPrescription {
            surfaces: vec![
                OpticalSurface {
                    thickness_mm: 10.0,
                    semi_diameter_mm: 12.0,
                    ..Default::default()
                },
                OpticalSurface {
                    thickness_mm: 20.0,
                    semi_diameter_mm: 12.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        state.zoom_fit(&rx, Vec2::new(800.0, 600.0));
        assert!(state.zoom > 1.0);
        assert!(state.initialized);
    }

    #[test]
    fn test_viewport_3d_camera_defaults_and_zoom_fit() {
        let mut cam = Viewport3DCamera::default();
        assert!(!cam.initialized);
        assert!((cam.azimuth_rad - 35.0f32.to_radians()).abs() < 1e-4);
        assert!((cam.elevation_rad - 22.0f32.to_radians()).abs() < 1e-4);

        let rx = OpticalPrescription {
            surfaces: vec![
                OpticalSurface {
                    thickness_mm: 15.0,
                    semi_diameter_mm: 10.0,
                    ..Default::default()
                },
                OpticalSurface {
                    thickness_mm: 25.0,
                    semi_diameter_mm: 12.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        cam.zoom_fit(&rx, Vec2::new(800.0, 600.0));
        assert!(cam.initialized);
        assert!(cam.distance_mm > 50.0);
        assert!((cam.target_mm[2] - 20.0).abs() < 1e-3);

        cam.reset_view();
        assert!((cam.azimuth_rad - 35.0f32.to_radians()).abs() < 1e-4);
        assert_eq!(cam.pan_screen_mm, Vec2::ZERO);
    }

    #[test]
    fn test_sample_surface_sag() {
        let flat_surf = OpticalSurface {
            radius_mm: f64::INFINITY,
            semi_diameter_mm: 10.0,
            ..Default::default()
        };
        assert_eq!(sample_surface_sag(&flat_surf, 5.0), 0.0);

        let curved_surf = OpticalSurface {
            radius_mm: 50.0,
            semi_diameter_mm: 15.0,
            ..Default::default()
        };
        let sag = sample_surface_sag(&curved_surf, 10.0);
        // Exact sag for sphere r=50, y=10: 50 - sqrt(2500 - 100) = 50 - sqrt(2400) ≈ 1.0102
        assert!((sag - (50.0 - (2400.0f64).sqrt())).abs() < 1e-6);
    }
}
