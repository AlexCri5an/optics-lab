//! Responsive and adaptive layout engine for OpticsLab.
//!
//! Provides the pure function `compute_layout` which plans panel visibility,
//! geometry, breakpoints, and minimum canvas reservation with zero side effects.

use egui::Vec2;

use crate::theme::AppPreferences;

/// Minimum logical width reserved for the optical canvas.
pub const MIN_CANVAS_WIDTH: f32 = 320.0;

/// Minimum logical height reserved for the optical canvas.
pub const MIN_CANVAS_HEIGHT: f32 = 240.0;

/// Fixed height of the top menu bar in logical points.
pub const TOP_MENU_BAR_HEIGHT: f32 = 28.0;

/// Fixed height of the icon toolbar in logical points.
pub const TOOLBAR_HEIGHT: f32 = 30.0;

/// Fixed height of the bottom status bar in logical points.
pub const STATUS_BAR_HEIGHT: f32 = 22.0;

/// Total combined height of all fixed vertical chrome elements (menu, toolbar, status bar).
pub const TOTAL_VERTICAL_CHROME_HEIGHT: f32 =
    TOP_MENU_BAR_HEIGHT + TOOLBAR_HEIGHT + STATUS_BAR_HEIGHT;

/// Responsive layout breakpoints based on window logical width in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint {
    /// Window width < 900 px: side panels become single active drawers, toolbar overflows.
    Compact,
    /// Window width 900–1500 px: standard desktop dual side panel capable.
    Regular,
    /// Window width 1500–2400 px: wide multi-panel view with permanent bottom dock.
    Wide,
    /// Window width > 2400 px: ultrawide display; side panel widths are capped and canvas expands.
    Ultrawide,
}

impl Breakpoint {
    /// Classifies an available width into a responsive breakpoint.
    #[must_use]
    pub const fn from_width(width: f32) -> Self {
        if width < 900.0 {
            Self::Compact
        } else if width <= 1500.0 {
            Self::Regular
        } else if width <= 2400.0 {
            Self::Wide
        } else {
            Self::Ultrawide
        }
    }
}

/// Calculated plan for interface layout, panel dimensions, and canvas allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutPlan {
    /// Active responsive breakpoint.
    pub breakpoint: Breakpoint,
    /// Whether touch mode ergonomics are active.
    pub is_touch: bool,
    /// Whether the left sidebar (modes & hierarchy) is visible.
    pub show_left_sidebar: bool,
    /// Width of the left sidebar in logical points.
    pub left_sidebar_width: f32,
    /// Whether the right inspector (ray data & prescription) is visible.
    pub show_right_inspector: bool,
    /// Width of the right inspector in logical points.
    pub right_inspector_width: f32,
    /// Whether the bottom dock (analytics & diagnostics) is visible.
    pub show_bottom_dock: bool,
    /// Height of the bottom dock in logical points.
    pub bottom_dock_height: f32,
    /// Allocated logical size for the central canvas.
    pub canvas_rect_size: Vec2,
    /// Minimum touch target size in logical points (40 px in touch mode, 20 px desktop).
    pub touch_target_min_size: f32,
}

/// Computes the complete deterministic interface layout for the given dimensions and preferences.
///
/// # Invariants
/// 1. Pure function: produces identical `LayoutPlan` for identical arguments.
/// 2. Panels never exceed available window size.
/// 3. Canvas keeps at least 320x240 logical px if window permits; panels collapse in priority order:
///    Bottom dock collapses first -> Right inspector collapses next -> Left sidebar collapses last.
/// 4. No panel overlap occurs.
#[must_use]
pub fn compute_layout(
    available_logical_size: Vec2,
    prefs: &AppPreferences,
    touch_mode: bool,
) -> LayoutPlan {
    let width = available_logical_size.x.max(0.0);
    let height = available_logical_size.y.max(0.0);
    let breakpoint = Breakpoint::from_width(width);

    let touch_target_min_size = if touch_mode { 40.0 } else { 20.0 };

    // Initial panel dimensions requested by preferences, clamped to valid bounds
    let mut left_w = prefs.left_sidebar_width.clamp(180.0, 380.0);
    let mut right_w = prefs.right_inspector_width.clamp(240.0, 480.0);
    let mut bottom_h = prefs.bottom_dock_height.clamp(100.0, 360.0);

    // In Ultrawide (>2400 px), cap panel widths so they do not dominate; canvas takes extra space
    if breakpoint == Breakpoint::Ultrawide {
        left_w = left_w.min(280.0);
        right_w = right_w.min(360.0);
    }

    let mut show_left = prefs.show_left_sidebar;
    let mut show_right = prefs.show_right_inspector;
    let mut show_bottom = prefs.show_bottom_dock;

    // Compact breakpoint (<900 px): only allow one side panel open at a time
    if breakpoint == Breakpoint::Compact && show_left && show_right {
        // Prioritize left sidebar; collapse right inspector into drawer
        show_right = false;
    }

    // Vertical budget: fixed chrome takes 80 px
    let vertical_usable = (height - TOTAL_VERTICAL_CHROME_HEIGHT).max(0.0);

    // If bottom dock would compress canvas height below MIN_CANVAS_HEIGHT, collapse bottom dock
    if show_bottom {
        if vertical_usable < MIN_CANVAS_HEIGHT + bottom_h {
            show_bottom = false;
        } else {
            // Ensure bottom dock doesn't take more than 50% of vertical usable
            bottom_h = bottom_h.min(vertical_usable * 0.5);
        }
    }

    let effective_bottom_h = if show_bottom { bottom_h } else { 0.0 };
    let canvas_h = (vertical_usable - effective_bottom_h).max(0.0);

    // Horizontal budget: enforce MIN_CANVAS_WIDTH
    let mut active_side_w =
        (if show_left { left_w } else { 0.0 }) + (if show_right { right_w } else { 0.0 });

    if width - active_side_w < MIN_CANVAS_WIDTH {
        // Priority 1: collapse right inspector first
        show_right = false;
        active_side_w = if show_left { left_w } else { 0.0 };

        // Priority 2: if still constrained, collapse left sidebar
        if width - active_side_w < MIN_CANVAS_WIDTH {
            show_left = false;
            active_side_w = 0.0;
        }
    }

    let canvas_w = (width - active_side_w).max(0.0);

    LayoutPlan {
        breakpoint,
        is_touch: touch_mode,
        show_left_sidebar: show_left,
        left_sidebar_width: if show_left { left_w } else { 0.0 },
        show_right_inspector: show_right,
        right_inspector_width: if show_right { right_w } else { 0.0 },
        show_bottom_dock: show_bottom,
        bottom_dock_height: if show_bottom { bottom_h } else { 0.0 },
        canvas_rect_size: Vec2::new(canvas_w, canvas_h),
        touch_target_min_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_classification() {
        assert_eq!(Breakpoint::from_width(640.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(899.9), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(900.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1440.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1500.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1500.1), Breakpoint::Wide);
        assert_eq!(Breakpoint::from_width(1920.0), Breakpoint::Wide);
        assert_eq!(Breakpoint::from_width(2400.0), Breakpoint::Wide);
        assert_eq!(Breakpoint::from_width(2400.1), Breakpoint::Ultrawide);
        assert_eq!(Breakpoint::from_width(3840.0), Breakpoint::Ultrawide);
    }

    #[test]
    fn test_minimum_canvas_preservation() {
        let prefs = AppPreferences {
            show_left_sidebar: true,
            show_right_inspector: true,
            show_bottom_dock: true,
            left_sidebar_width: 240.0,
            right_inspector_width: 320.0,
            bottom_dock_height: 185.0,
            ..Default::default()
        };

        // Window size 640x480 (native min size):
        // 640 width with both panels (240+320 = 560) leaves only 80 px canvas (< 320 px).
        // Therefore right panel MUST collapse, leaving 640 - 240 = 400 px (>= 320 px).
        // 480 height - 80 fixed chrome = 400 px. 400 - 185 = 215 px (< 240 px).
        // Therefore bottom dock MUST collapse to protect 240 px canvas height!
        let plan = compute_layout(Vec2::new(640.0, 480.0), &prefs, false);
        assert_eq!(plan.breakpoint, Breakpoint::Compact);
        assert!(plan.show_left_sidebar);
        assert!(!plan.show_right_inspector);
        assert!(!plan.show_bottom_dock);
        assert!(plan.canvas_rect_size.x >= MIN_CANVAS_WIDTH);
        assert!(plan.canvas_rect_size.y >= MIN_CANVAS_HEIGHT);
    }

    #[test]
    fn test_wide_and_ultrawide_layout() {
        let prefs = AppPreferences {
            show_left_sidebar: true,
            show_right_inspector: true,
            show_bottom_dock: true,
            left_sidebar_width: 300.0,
            right_inspector_width: 400.0,
            bottom_dock_height: 200.0,
            ..Default::default()
        };

        // Wide desktop 1920x1080
        let plan_wide = compute_layout(Vec2::new(1920.0, 1080.0), &prefs, false);
        assert_eq!(plan_wide.breakpoint, Breakpoint::Wide);
        assert!(plan_wide.show_left_sidebar);
        assert!(plan_wide.show_right_inspector);
        assert!(plan_wide.show_bottom_dock);
        assert_eq!(plan_wide.left_sidebar_width, 300.0);
        assert_eq!(plan_wide.right_inspector_width, 400.0);
        assert!(plan_wide.canvas_rect_size.x > 1000.0);

        // Ultrawide 3440x1440: panel widths capped
        let plan_ultra = compute_layout(Vec2::new(3440.0, 1440.0), &prefs, false);
        assert_eq!(plan_ultra.breakpoint, Breakpoint::Ultrawide);
        assert!(plan_ultra.left_sidebar_width <= 280.0);
        assert!(plan_ultra.right_inspector_width <= 360.0);
    }

    #[test]
    fn test_touch_mode_target_size() {
        let prefs = AppPreferences::default();
        let plan_touch = compute_layout(Vec2::new(390.0, 844.0), &prefs, true);
        assert!(plan_touch.is_touch);
        assert_eq!(plan_touch.touch_target_min_size, 40.0);

        let plan_desktop = compute_layout(Vec2::new(1920.0, 1080.0), &prefs, false);
        assert!(!plan_desktop.is_touch);
        assert_eq!(plan_desktop.touch_target_min_size, 20.0);
    }

    #[test]
    fn test_matrix_dimensions_never_overlap_or_negative() {
        // Complete test matrix from U5 specification:
        let sizes = [
            // Desktop matrix
            Vec2::new(640.0, 480.0),
            Vec2::new(800.0, 600.0),
            Vec2::new(900.0, 600.0), // Official minimum window size
            Vec2::new(1024.0, 768.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(1366.0, 768.0),
            Vec2::new(1440.0, 900.0),
            Vec2::new(1800.0, 1200.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(2560.0, 1440.0),
            Vec2::new(3440.0, 1440.0),
            Vec2::new(3840.0, 2160.0),
            // Mobile / Web matrix
            Vec2::new(360.0, 640.0),
            Vec2::new(390.0, 844.0),
            Vec2::new(450.0, 300.0), // 900x600 at 2x DPI scaling
            Vec2::new(768.0, 1024.0),
            Vec2::new(1024.0, 768.0),
        ];

        let prefs = AppPreferences::default();

        for size in sizes {
            for touch in [false, true] {
                let plan = compute_layout(size, &prefs, touch);

                // Invariant: panels never exceed window dimensions
                assert!(
                    plan.left_sidebar_width <= size.x,
                    "left width > screen for {size:?}"
                );
                assert!(
                    plan.right_inspector_width <= size.x,
                    "right width > screen for {size:?}"
                );
                assert!(
                    plan.bottom_dock_height <= size.y,
                    "bottom height > screen for {size:?}"
                );

                // Invariant: sum of side panels + canvas == screen width
                let total_w =
                    plan.left_sidebar_width + plan.right_inspector_width + plan.canvas_rect_size.x;
                assert!(
                    (total_w - size.x).abs() < 1e-4,
                    "Total width mismatch for {size:?}: expected {}, got {total_w}",
                    size.x
                );

                // Invariant: canvas non-negative
                assert!(plan.canvas_rect_size.x >= 0.0);
                assert!(plan.canvas_rect_size.y >= 0.0);
            }
        }
    }

    #[test]
    fn test_minimum_window_size_900x600_and_hidpi() {
        let prefs = AppPreferences::default();

        // 1x DPI: 900x600
        let plan_1x = compute_layout(Vec2::new(900.0, 600.0), &prefs, false);
        assert_eq!(plan_1x.breakpoint, Breakpoint::Regular);
        assert!(plan_1x.canvas_rect_size.x >= MIN_CANVAS_WIDTH);
        assert!(plan_1x.canvas_rect_size.y >= MIN_CANVAS_HEIGHT);
        assert!(plan_1x.show_left_sidebar);
        assert!(plan_1x.show_right_inspector);
        assert!(plan_1x.show_bottom_dock);

        // 2x DPI: 900x600 physical gives 450x300 logical points
        let plan_2x = compute_layout(Vec2::new(450.0, 300.0), &prefs, false);
        assert_eq!(plan_2x.breakpoint, Breakpoint::Compact);
        assert!(plan_2x.canvas_rect_size.x > 0.0);
        assert!(plan_2x.canvas_rect_size.y > 0.0);
        // Panels gracefully collapse on extreme constrained view
        assert!(!plan_2x.show_right_inspector);
        assert!(!plan_2x.show_bottom_dock);
        let total_w =
            plan_2x.left_sidebar_width + plan_2x.right_inspector_width + plan_2x.canvas_rect_size.x;
        assert!((total_w - 450.0).abs() < 1e-4);
    }

    #[test]
    fn test_property_randomized_fuzzing() {
        // Sweeps 1,000 randomized combinations of width, height, user preferences, and touch mode
        for i in 1..=1000 {
            let w = 200.0 + (i as f32 * 7.91) % 3800.0;
            let h = 150.0 + (i as f32 * 5.17) % 2100.0;
            let touch = (i % 2) == 0;

            let prefs = AppPreferences {
                show_left_sidebar: (i % 3) != 0,
                show_right_inspector: (i % 5) != 0,
                show_bottom_dock: (i % 7) != 0,
                left_sidebar_width: 180.0 + (i as f32 * 3.3) % 200.0,
                right_inspector_width: 240.0 + (i as f32 * 4.7) % 240.0,
                bottom_dock_height: 100.0 + (i as f32 * 2.9) % 260.0,
                ..Default::default()
            };

            let plan = compute_layout(Vec2::new(w, h), &prefs, touch);

            assert!(plan.canvas_rect_size.x >= 0.0);
            assert!(plan.canvas_rect_size.y >= 0.0);
            assert!(plan.left_sidebar_width + plan.right_inspector_width <= w);

            // Total width invariant (f32 machine epsilon around 2000 is ~0.0002)
            let total_w =
                plan.left_sidebar_width + plan.right_inspector_width + plan.canvas_rect_size.x;
            assert!(
                (total_w - w).abs() < 1e-3,
                "Mismatch at w={w}, got {total_w}"
            );

            // If window width >= MIN_CANVAS_WIDTH, canvas width must be >= MIN_CANVAS_WIDTH
            if w >= MIN_CANVAS_WIDTH {
                assert!(
                    plan.canvas_rect_size.x >= MIN_CANVAS_WIDTH,
                    "Canvas width collapsed prematurely at w={w}, plan={plan:?}"
                );
            }
        }
    }
}
