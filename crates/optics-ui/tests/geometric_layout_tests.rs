//! Geometric layout assertions, non-overlap tests, bounding box checks, and galley text fit (P9).

use egui::{Color32, Context, FontId, Pos2, RawInput, Rect, Vec2};
use optics_ui::app::OpticsLabApp;
use optics_ui::commands::Command;
use optics_ui::layout::{
    Breakpoint, MIN_CANVAS_HEIGHT, MIN_CANVAS_WIDTH, STATUS_BAR_HEIGHT, TOOLBAR_HEIGHT,
    TOP_MENU_BAR_HEIGHT, compute_layout,
};
use optics_ui::theme::AppPreferences;

#[test]
fn test_geometric_widget_bounds_at_minimum_window_size() {
    let ctx = Context::default();
    let mut app = OpticsLabApp::default();

    // 900x600 logical points (official minimum window size)
    let screen_size = Vec2::new(900.0, 600.0);
    let raw_input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen_size)),
        ..Default::default()
    };

    let _ = ctx.run(raw_input, |ctx| {
        app.update_ui(ctx);
    });

    let widgets = &app.ui_registry.widgets;
    assert!(
        widgets.len() >= 10,
        "A rendered frame at 900x600 must register interactive widgets (found {})",
        widgets.len()
    );

    for widget in widgets {
        let r = widget.rect;
        assert!(
            r.is_finite(),
            "Widget {:?} at '{}' has non-finite rect: {:?}",
            widget.command,
            widget.location,
            r
        );
        assert!(
            r.width() > 0.0,
            "Widget {:?} at '{}' has zero or negative width: {}",
            widget.command,
            widget.location,
            r.width()
        );
        assert!(
            r.height() > 0.0,
            "Widget {:?} at '{}' has zero or negative height: {}",
            widget.command,
            widget.location,
            r.height()
        );

        // All widgets must lie within or reasonably close to screen boundaries (allowing subpixel border padding)
        assert!(
            r.min.x >= -1.0,
            "Widget {:?} at '{}' overflows left screen edge: min.x = {}",
            widget.command,
            widget.location,
            r.min.x
        );
        assert!(
            r.min.y >= -1.0,
            "Widget {:?} at '{}' overflows top screen edge: min.y = {}",
            widget.command,
            widget.location,
            r.min.y
        );
        assert!(
            r.max.x <= screen_size.x + 1.0,
            "Widget {:?} at '{}' overflows right screen edge: max.x = {}, screen.w = {}",
            widget.command,
            widget.location,
            r.max.x,
            screen_size.x
        );
        assert!(
            r.max.y <= screen_size.y + 1.0,
            "Widget {:?} at '{}' overflows bottom screen edge: max.y = {}, screen.h = {}",
            widget.command,
            widget.location,
            r.max.y,
            screen_size.y
        );
    }
}

#[test]
fn test_no_panel_overlap_across_all_desktop_resolutions() {
    let resolutions = [
        Vec2::new(900.0, 600.0),   // Minimum supported size
        Vec2::new(1024.0, 768.0),  // Standard XGA
        Vec2::new(1280.0, 720.0),  // 720p HD
        Vec2::new(1366.0, 768.0),  // Common laptop
        Vec2::new(1440.0, 900.0),  // WXGA+
        Vec2::new(1920.0, 1080.0), // 1080p Full HD
        Vec2::new(2560.0, 1440.0), // 1440p QHD
        Vec2::new(3440.0, 1440.0), // Ultrawide
        Vec2::new(3840.0, 2160.0), // 4K UHD
    ];

    let prefs = AppPreferences::default();

    for res in resolutions {
        let plan = compute_layout(res, &prefs, false);

        let w = res.x;
        let h = res.y;

        // Chrome regions
        let top_menu_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, TOP_MENU_BAR_HEIGHT));
        let toolbar_rect = Rect::from_min_size(
            Pos2::new(0.0, TOP_MENU_BAR_HEIGHT),
            Vec2::new(w, TOOLBAR_HEIGHT),
        );
        let status_bar_rect = Rect::from_min_size(
            Pos2::new(0.0, h - STATUS_BAR_HEIGHT),
            Vec2::new(w, STATUS_BAR_HEIGHT),
        );

        // Fixed chrome no-overlap
        assert!(
            !top_menu_rect.intersect(toolbar_rect).is_positive(),
            "Top menu and toolbar overlap at res {res:?}"
        );
        assert!(
            !toolbar_rect.intersect(status_bar_rect).is_positive(),
            "Toolbar and status bar overlap at res {res:?}"
        );

        let chrome_top = TOP_MENU_BAR_HEIGHT + TOOLBAR_HEIGHT;
        let chrome_bottom = h - STATUS_BAR_HEIGHT;
        let usable_h = chrome_bottom - chrome_top;

        // Side panels
        let left_rect = if plan.show_left_sidebar {
            Rect::from_min_size(
                Pos2::new(0.0, chrome_top),
                Vec2::new(plan.left_sidebar_width, usable_h),
            )
        } else {
            Rect::NOTHING
        };

        let right_rect = if plan.show_right_inspector {
            Rect::from_min_size(
                Pos2::new(w - plan.right_inspector_width, chrome_top),
                Vec2::new(plan.right_inspector_width, usable_h),
            )
        } else {
            Rect::NOTHING
        };

        // Left and Right panels never intersect
        if plan.show_left_sidebar && plan.show_right_inspector {
            assert!(
                !left_rect.intersect(right_rect).is_positive(),
                "Left and right side panels overlap at res {res:?}"
            );
        }

        // Bottom dock
        let dock_rect = if plan.show_bottom_dock {
            Rect::from_min_size(
                Pos2::new(
                    plan.left_sidebar_width,
                    chrome_bottom - plan.bottom_dock_height,
                ),
                Vec2::new(plan.canvas_rect_size.x, plan.bottom_dock_height),
            )
        } else {
            Rect::NOTHING
        };

        // Canvas
        let canvas_rect = Rect::from_min_size(
            Pos2::new(plan.left_sidebar_width, chrome_top),
            plan.canvas_rect_size,
        );

        // Canvas and dock never intersect
        if plan.show_bottom_dock {
            assert!(
                !canvas_rect.intersect(dock_rect).is_positive(),
                "Canvas and bottom dock overlap at res {res:?}"
            );
        }

        // Canvas and left panel never intersect
        if plan.show_left_sidebar {
            assert!(
                !canvas_rect.intersect(left_rect).is_positive(),
                "Canvas and left sidebar overlap at res {res:?}"
            );
        }

        // Canvas and right panel never intersect
        if plan.show_right_inspector {
            assert!(
                !canvas_rect.intersect(right_rect).is_positive(),
                "Canvas and right inspector overlap at res {res:?}"
            );
        }

        // Canvas size preservation invariant
        if res.x >= 900.0 && res.y >= 600.0 {
            assert!(
                plan.canvas_rect_size.x >= MIN_CANVAS_WIDTH,
                "Canvas width below minimum at {res:?}: {}",
                plan.canvas_rect_size.x
            );
            assert!(
                plan.canvas_rect_size.y >= MIN_CANVAS_HEIGHT,
                "Canvas height below minimum at {res:?}: {}",
                plan.canvas_rect_size.y
            );
        }
    }
}

#[test]
fn test_galley_text_fit_assertions() {
    let ctx = Context::default();
    let _ = ctx.run(RawInput::default(), |_| {});

    // Verify all primary command labels produce valid, bounded text galleys
    let sample_commands = [
        Command::ResetDefaults,
        Command::ResetView,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ToggleReflections,
        Command::ToggleTiming,
        Command::ToggleSoftGlow,
        Command::ToggleLeftSidebar,
        Command::ToggleRightInspector,
        Command::ToggleBottomDock,
        Command::ResetLayout,
        Command::ToggleFullscreen,
        Command::CycleTheme,
        Command::OpenPreferences,
        Command::OpenAbout,
        Command::OpenShortcutsHelp,
        Command::CopyRayBouncesTsv,
        Command::ExportRaysCsv,
        Command::ExportImage,
        Command::SaveProject,
        Command::ToggleRenderPause,
        Command::ResetAccumulation,
        Command::ToggleBeamRibbons,
        Command::ToggleFloorCaustic,
        Command::ToggleTofPulses,
    ];

    for cmd in sample_commands {
        let label = cmd.label();
        assert!(!label.is_empty());

        let font_id = FontId::proportional(12.0);
        let galley = ctx.fonts(|f| f.layout_no_wrap(label.to_string(), font_id, Color32::WHITE));

        let size = galley.size();
        assert!(size.x > 0.0, "Label '{label}' has zero width");
        assert!(size.y > 0.0, "Label '{label}' has zero height");
        assert!(
            size.x < 240.0,
            "Label '{label}' is unusually wide ({} px) and may clip in CAD chrome",
            size.x
        );
        assert!(
            size.y < 30.0,
            "Label '{label}' height is unexpectedly large ({} px)",
            size.y
        );
    }
}

#[test]
fn test_touch_target_bounds_in_touch_mode() {
    let prefs = AppPreferences::default();
    let mobile_size = Vec2::new(390.0, 844.0); // Modern smartphone viewport
    let plan = compute_layout(mobile_size, &prefs, true);

    assert!(plan.is_touch);
    assert_eq!(plan.touch_target_min_size, 40.0);
    assert_eq!(plan.breakpoint, Breakpoint::Compact);

    // On mobile touch view, panels must collapse or fit strictly inside 390 px width
    assert!(plan.left_sidebar_width <= mobile_size.x);
    assert!(plan.right_inspector_width <= mobile_size.x);
    assert!(plan.canvas_rect_size.x >= 0.0);
    assert!(plan.canvas_rect_size.y >= 0.0);
}
