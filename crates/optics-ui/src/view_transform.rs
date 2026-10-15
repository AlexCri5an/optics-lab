//! 2D View transform mapping between optical scene world coordinates (mm)
//! and egui screen coordinates (pixels).

use egui::{Pos2, Rect, Vec2};

/// 2D pan and zoom transform for interactive canvas rendering.
///
/// In world space, Y points upwards and coordinates are in millimeters.
/// In screen space, Y points downwards and coordinates are in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// World coordinate (mm) that appears at the center of the screen rectangle.
    pub center_world: Pos2,
    /// Pixels per millimeter scale factor.
    pub zoom: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            center_world: Pos2::new(0.0, 20.0),
            zoom: 6.0,
        }
    }
}

impl ViewTransform {
    /// Creates a new view transform with specified center and zoom.
    #[must_use]
    pub const fn new(center_world: Pos2, zoom: f32) -> Self {
        Self { center_world, zoom }
    }

    /// Converts a world-space point (mm, Y-up) to screen-space coordinates (pixels, Y-down).
    #[must_use]
    pub fn world_to_screen(&self, world_pt: Pos2, screen_rect: Rect) -> Pos2 {
        let center_screen = screen_rect.center();
        let dx = (world_pt.x - self.center_world.x) * self.zoom;
        let dy = (world_pt.y - self.center_world.y) * self.zoom;
        // Invert Y because screen space has +Y pointing downwards
        Pos2::new(center_screen.x + dx, center_screen.y - dy)
    }

    /// Converts a screen-space point (pixels, Y-down) to world-space coordinates (mm, Y-up).
    #[must_use]
    pub fn screen_to_world(&self, screen_pt: Pos2, screen_rect: Rect) -> Pos2 {
        let center_screen = screen_rect.center();
        let dx = (screen_pt.x - center_screen.x) / self.zoom;
        let dy = (screen_pt.y - center_screen.y) / self.zoom;
        Pos2::new(self.center_world.x + dx, self.center_world.y - dy)
    }

    /// Pans the view by a delta given in screen pixels.
    pub fn pan_by_screen_delta(&mut self, screen_delta: Vec2) {
        self.center_world.x -= screen_delta.x / self.zoom;
        // Screen +Y is downwards, which is world -Y
        self.center_world.y += screen_delta.y / self.zoom;
    }

    /// Zooms the view centered on a specific screen position (e.g. cursor position).
    ///
    /// Preserves the world position underneath `cursor_screen` before and after zooming.
    pub fn zoom_at(&mut self, factor: f32, cursor_screen: Pos2, screen_rect: Rect) {
        if factor <= 0.0 || !factor.is_finite() {
            return;
        }
        let world_before = self.screen_to_world(cursor_screen, screen_rect);
        self.zoom = (self.zoom * factor).clamp(0.5, 500.0);
        let world_after = self.screen_to_world(cursor_screen, screen_rect);

        // Shift center so world_before and world_after coincide
        self.center_world.x += world_before.x - world_after.x;
        self.center_world.y += world_before.y - world_after.y;
    }

    /// Fits the view to encompass a given world-space bounding box within the screen rectangle.
    pub fn fit_to_rect(&mut self, world_bounds: Rect, screen_rect: Rect, margin_px: f32) {
        if world_bounds.width() <= 0.0 || world_bounds.height() <= 0.0 {
            return;
        }
        let avail_w = (screen_rect.width() - margin_px * 2.0).max(10.0);
        let avail_h = (screen_rect.height() - margin_px * 2.0).max(10.0);

        let zoom_x = avail_w / world_bounds.width();
        let zoom_y = avail_h / world_bounds.height();
        self.zoom = zoom_x.min(zoom_y).clamp(0.5, 200.0);
        self.center_world = world_bounds.center();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_screen_round_trip() {
        let view = ViewTransform::new(Pos2::new(10.0, -5.0), 8.5);
        let screen = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(800.0, 600.0));

        let test_points = [
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, -5.0),
            Pos2::new(-35.2, 42.1),
            Pos2::new(120.0, -80.0),
        ];

        for &pt in &test_points {
            let s = view.world_to_screen(pt, screen);
            let round_trip = view.screen_to_world(s, screen);
            assert!(
                (round_trip.x - pt.x).abs() < 1e-4,
                "X mismatch: got {}, expected {}",
                round_trip.x,
                pt.x
            );
            assert!(
                (round_trip.y - pt.y).abs() < 1e-4,
                "Y mismatch: got {}, expected {}",
                round_trip.y,
                pt.y
            );
        }
    }

    #[test]
    fn test_zoom_keeps_cursor_world_point_fixed() {
        let mut view = ViewTransform::new(Pos2::new(0.0, 20.0), 6.0);
        let screen = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1000.0, 700.0));
        let cursor = Pos2::new(350.0, 280.0);

        let world_before = view.screen_to_world(cursor, screen);
        view.zoom_at(1.25, cursor, screen);
        let world_after = view.screen_to_world(cursor, screen);

        assert!(
            (world_after.x - world_before.x).abs() < 1e-4,
            "Zoom shifted cursor world X: before {}, after {}",
            world_before.x,
            world_after.x
        );
        assert!(
            (world_after.y - world_before.y).abs() < 1e-4,
            "Zoom shifted cursor world Y: before {}, after {}",
            world_before.y,
            world_after.y
        );
    }

    #[test]
    fn test_fit_to_rect_contains_bounds_with_margin() {
        let mut view = ViewTransform::default();
        let screen = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        let bounds = Rect::from_min_max(Pos2::new(-50.0, -10.0), Pos2::new(50.0, 45.0));
        let margin = 30.0;

        view.fit_to_rect(bounds, screen, margin);

        // Check that all 4 corners of bounds map inside screen_rect shrunk by margin
        let min_corner = view.world_to_screen(bounds.min, screen);
        let max_corner = view.world_to_screen(bounds.max, screen);

        let inner_rect = screen.shrink(margin - 0.5);
        assert!(inner_rect.contains(min_corner) || (min_corner.x >= inner_rect.min.x - 1e-3));
        assert!(inner_rect.contains(max_corner) || (max_corner.y >= inner_rect.min.y - 1e-3));
    }
}
